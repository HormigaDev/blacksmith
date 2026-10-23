//! `/api/v1/services/{name}/…` de extremo a extremo: router real, base de datos en memoria y un
//! `systemctl` falso que anota cada invocación.

// Los helpers no son `#[test]`, así que `allow-unwrap-in-tests` no los cubre.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Duration,
};

use agent::{
    config::Config,
    db::Database,
    deploy::Paths,
    http,
    services::{PermissionCheck, ServiceDescription, ServiceManager, ServiceName, ServiceSlug},
    state::AppState,
    systemd::{Journalctl, Scope, Systemctl},
};
use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use shared::{
    ActiveState, ApiErrorBody, ErrorCode, GlobalEnvironment, LoadState, ServiceActionReport,
    ServiceDeployment, ServiceEnvironment, ServiceOutcome, ServiceStatusReport, ServiceSummary,
};
use tower::ServiceExt;

/// `$3` es el verbo (tras `--no-ask-password --no-pager`) y el último argumento, la unidad.
/// `MainPID=$PPID`: el proceso del test, que está vivo y se puede medir de verdad.
const FAKE: &str = r#"#!/bin/sh
for unit; do :; done
echo "$3 $unit" >> "__LOG__"
# Como systemd con la plantilla `bbel@.service` instalada (salvo en la unidad inexistente).
if [ "$3" = show ] && [ "$5" = EnvironmentFiles ]; then
    if [ "$unit" != bbel@sin-unidad.service ]; then
        slug=${unit#bbel@}; slug=${slug%.service}
        echo "EnvironmentFiles=__ENVDIR__/global.env (ignore_errors=yes)"
        echo "EnvironmentFiles=__ENVDIR__/services/$slug.env (ignore_errors=yes)"
    fi
    exit 0
fi
case "$3:$unit" in
    show:bbel@api.service)
        echo "MainPID=$PPID"; echo "LoadState=loaded"; echo "ActiveState=active"; echo "SubState=running" ;;
    show:bbel@parado.service|show:bbel@sin-version.service)
        echo "MainPID=0"; echo "LoadState=loaded"; echo "ActiveState=inactive"; echo "SubState=dead" ;;
    show:bbel@sin-unidad.service)
        echo "MainPID=0"; echo "LoadState=not-found"; echo "ActiveState=inactive"; echo "SubState=dead" ;;
    show:bbel@raro.service) echo "MainPID=abc" ;;
    start:bbel@api.service|stop:bbel@api.service|restart:bbel@api.service) exit 0 ;;
    # nginx: rechaza mientras exista el archivo de control; certbot siempre emite.
    start:bbel-nginx.service)
        if [ -e "__DIR__/nginx-rechaza" ]; then
            echo "Job for bbel-nginx.service failed because the control process exited with error code." >&2
            exit 1
        fi
        exit 0 ;;
    start:bbel-certbot@*) exit 0 ;;
    stop:bbel@sin-version.service) exit 0 ;;
    start:bbel@falla.service)
        echo "Job for bbel@falla.service failed because the control process exited with error code." >&2
        echo "See \"systemctl status bbel@falla.service\" for details." >&2
        exit 1 ;;
    start:bbel@sin-permiso.service)
        echo "Failed to start bbel@sin-permiso.service: Access denied" >&2; exit 1 ;;
    start:bbel@sin-unidad.service)
        echo "Failed to start bbel@sin-unidad.service: Unit bbel@sin-unidad.service not found." >&2; exit 5 ;;
    start:bbel@lento.service) exec /bin/sleep 30 ;;
    stop:bbel@agent-permission-check.service)
        echo "Failed to stop bbel@agent-permission-check.service: Unit bbel@agent-permission-check.service not loaded." >&2
        exit 5 ;;
    *) echo "inesperado: $*" >&2; exit 99 ;;
esac
"#;

struct Fake {
    program: PathBuf,
    log: PathBuf,
    env_dir: PathBuf,
    paths: Paths,
}

/// Se escribe una sola vez antes de cualquier ejecución (escribir un ejecutable mientras otro
/// hilo hace `fork` puede dar `ETXTBSY`).
fn fake() -> &'static Fake {
    static FAKE_SYSTEMCTL: OnceLock<Fake> = OnceLock::new();
    FAKE_SYSTEMCTL.get_or_init(|| {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("services-http-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let log = dir.join("invocaciones.log");
        let program = dir.join("systemctl");
        let paths = Paths::rooted(&dir.join("estado"), &dir.join("config"), None);
        let env_dir = paths.env_dir.clone();
        let script = FAKE
            .replace("__LOG__", log.to_str().unwrap())
            .replace("__DIR__", dir.to_str().unwrap())
            .replace("__ENVDIR__", env_dir.to_str().unwrap());
        fs::write(&program, script).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        Fake {
            program,
            log,
            env_dir,
            paths,
        }
    })
}

const DEPLOYED: [&str; 7] = [
    "api",
    "parado",
    "sin-unidad",
    "raro",
    "falla",
    "sin-permiso",
    "lento",
];

fn invocations() -> String {
    fs::read_to_string(&fake().log).unwrap_or_default()
}

async fn app() -> Router {
    let config = Config::from_vars([("BLACKSMITH_ENV", "production")]).unwrap();
    let db = Database::open_in_memory().unwrap();
    for slug in DEPLOYED.into_iter().chain(["sin-version"]) {
        let service = db
            .insert_service(
                ServiceSlug::new(slug).unwrap(),
                ServiceName::new(slug).unwrap(),
                ServiceDescription::default(),
            )
            .await
            .unwrap();
        // Todos con versión activa salvo `sin-version`, que nunca se desplegó.
        if slug != "sin-version" {
            db.activate_release(service.id, "r0000000001-000000000".into())
                .await
                .unwrap();
        }
    }
    let systemctl = Systemctl::new(&fake().program, Scope::System, Duration::from_millis(500));
    http::router(AppState::with_commands(
        Arc::new(config),
        db,
        systemctl,
        Journalctl::default(),
        fake().paths.clone(),
    ))
}

async fn send(method: Method, uri: &str) -> (StatusCode, Vec<u8>) {
    call(&app().await, method, uri, None).await
}

/// Petición contra un router concreto (para encadenar varias sobre la misma base de datos).
async fn call(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, Vec<u8>) {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4040");
    let request = match body {
        Some(body) => builder
            .header("content-type", "application/json")
            .body(Body::from(body.to_string())),
        None => builder.body(Body::empty()),
    }
    .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let content_type = response.headers().get("content-type").cloned();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    if status != StatusCode::NO_CONTENT {
        assert_eq!(content_type.unwrap(), "application/json", "{uri}");
    }
    (status, body)
}

async fn ok<T: DeserializeOwned>(method: Method, uri: &str) -> T {
    let (status, body) = send(method, uri).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    serde_json::from_slice(&body).unwrap()
}

async fn fail(method: Method, uri: &str) -> (StatusCode, ApiErrorBody) {
    let (status, body) = send(method, uri).await;
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn estado_con_pid_y_metricas() {
    let report: ServiceStatusReport = ok(Method::GET, "/api/v1/services/api/status").await;
    assert_eq!(report.name, "api");
    assert_eq!(report.unit, "bbel@api.service");
    assert_eq!(report.load_state, LoadState::Loaded);
    assert_eq!(report.active_state, ActiveState::Active);
    assert_eq!(report.sub_state, "running");
    assert_eq!(report.pid, Some(std::process::id()));
    assert!(report.deployed);
    let metrics = report.metrics.expect("métricas del proceso");
    assert!(metrics.rss_bytes > 0);
    assert!(metrics.cpu_percent >= 0.0);
}

#[tokio::test]
async fn estado_de_un_servicio_parado_o_sin_unidad() {
    let report: ServiceStatusReport = ok(Method::GET, "/api/v1/services/parado/status").await;
    assert_eq!(report.active_state, ActiveState::Inactive);
    assert_eq!(report.pid, None);
    assert_eq!(report.metrics, None);

    let report: ServiceStatusReport = ok(Method::GET, "/api/v1/services/sin-unidad/status").await;
    assert_eq!(report.load_state, LoadState::NotFound);
}

#[tokio::test]
async fn acciones() {
    for (action, outcome) in [
        ("start", ServiceOutcome::Started),
        ("stop", ServiceOutcome::Stopped),
        ("restart", ServiceOutcome::Restarted),
    ] {
        let report: ServiceActionReport =
            ok(Method::POST, &format!("/api/v1/services/api/{action}")).await;
        assert_eq!(report.outcome, outcome);
        assert_eq!(report.unit, "bbel@api.service");
        assert!(invocations().contains(&format!("{action} bbel@api.service")));
    }
}

/// La cuenta de GitHub del servidor: se guarda, se consulta sin el token y se olvida.
#[tokio::test]
async fn acceso_a_github_sin_devolver_nunca_el_token() {
    let router = app().await;
    let token = "ghu_0123456789abcdefghijKLMNOP";
    let (status, body) = call(&router, Method::GET, "/api/v1/github", None).await;
    assert_eq!(
        (status, body.as_slice()),
        (StatusCode::OK, b"null".as_slice())
    );

    let (status, body) = call(
        &router,
        Method::PUT,
        "/api/v1/github",
        Some(serde_json::json!({ "login": "octocat", "token": token })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert!(!String::from_utf8_lossy(&body).contains(token));
    let (_, body) = call(&router, Method::GET, "/api/v1/github", None).await;
    let access: shared::GitHubAccess = serde_json::from_slice(&body).unwrap();
    assert_eq!(access.login, "octocat");
    assert!(!String::from_utf8_lossy(&body).contains(token));

    let (status, _) = call(
        &router,
        Method::PUT,
        "/api/v1/github",
        Some(serde_json::json!({ "login": "octocat", "token": "x$(id)\n" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = call(&router, Method::DELETE, "/api/v1/github", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = call(&router, Method::GET, "/api/v1/github", None).await;
    assert_eq!(body.as_slice(), b"null");
}

/// Sitio de nginx de un servicio: se aplica con `bbel-nginx.service`, se restaura si nginx lo
/// rechaza, los dominios no se comparten y HTTPS llega en segundo plano.
#[tokio::test]
async fn sitio_de_nginx() {
    let router = app().await;
    let site_file = fake().paths.nginx_sites_dir.join("api.conf");
    let rejects = fake().log.with_file_name("nginx-rechaza");
    let put = |body: serde_json::Value| {
        call(
            &router,
            Method::PUT,
            "/api/v1/services/api/site",
            Some(body),
        )
    };

    let (status, body) = call(&router, Method::GET, "/api/v1/services/api/site", None).await;
    assert_eq!(
        (status, body.as_slice()),
        (StatusCode::OK, b"null".as_slice())
    );

    let app_site = serde_json::json!({
        "template": "app", "domains": ["api.example.com"], "port": 3000, "https": false, "email": null
    });
    let (status, body) = put(app_site.clone()).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let site: shared::Site = serde_json::from_slice(&body).unwrap();
    assert_eq!(site.tls, shared::TlsStatus::Off);
    let written = fs::read_to_string(&site_file).unwrap();
    assert!(
        written.contains("server_name api.example.com;"),
        "{written}"
    );
    assert!(
        written.contains("proxy_pass http://127.0.0.1:3000;"),
        "{written}"
    );
    assert!(invocations().contains("start bbel-nginx.service"));

    // Validación: una plantilla de archivos sin carpeta pública, o un puerto que falta.
    for invalid in [
        serde_json::json!({"template": "spa", "domains": ["api.example.com"], "port": null, "https": false, "email": null}),
        serde_json::json!({"template": "api", "domains": ["api.example.com"], "port": null, "https": false, "email": null}),
        serde_json::json!({"template": "app", "domains": ["a.com;"], "port": 3000, "https": false, "email": null}),
    ] {
        let (status, body) = put(invalid).await;
        let error: ApiErrorBody = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            (status, error.code),
            (StatusCode::BAD_REQUEST, ErrorCode::InvalidSite)
        );
    }

    // Otro servicio no puede quedarse el dominio.
    let (status, body) = call(
        &router,
        Method::PUT,
        "/api/v1/services/parado/site",
        Some(app_site),
    )
    .await;
    let error: ApiErrorBody = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        (status, error.code),
        (StatusCode::CONFLICT, ErrorCode::DomainInUse)
    );

    // nginx rechaza: se mantiene la configuración anterior, también en disco.
    fs::write(&rejects, "").unwrap();
    let (status, body) = put(serde_json::json!({
        "template": "api", "domains": ["otro.example.com"], "port": 3001, "https": false, "email": null
    }))
    .await;
    fs::remove_file(&rejects).unwrap();
    let error: ApiErrorBody = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        (status, error.code),
        (StatusCode::UNPROCESSABLE_ENTITY, ErrorCode::NginxRejected)
    );
    assert_eq!(fs::read_to_string(&site_file).unwrap(), written);

    // HTTPS: primero por HTTP mientras se pide el certificado; después, con TLS.
    let (status, body) = put(serde_json::json!({
        "template": "app", "domains": ["api.example.com", "www.api.example.com"], "port": 3000,
        "https": true, "email": "ops@example.com"
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let mut tls = shared::TlsStatus::Issuing;
    for _ in 0..100 {
        let (_, body) = call(&router, Method::GET, "/api/v1/services/api/site", None).await;
        let site: shared::Site = serde_json::from_slice(&body).unwrap();
        tls = site.tls;
        if tls != shared::TlsStatus::Issuing {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(tls, shared::TlsStatus::Active);
    assert!(invocations().contains("start bbel-certbot@api.example.com.service"));
    let certbot_args =
        fs::read_to_string(fake().paths.nginx_certs_dir.join("api.example.com.env")).unwrap();
    assert_eq!(
        certbot_args,
        "BBEL_CERTBOT_ARGS=-d api.example.com -d www.api.example.com --email ops@example.com\n"
    );
    let written = fs::read_to_string(&site_file).unwrap();
    assert!(
        written.contains("ssl_certificate /etc/letsencrypt/live/api.example.com/fullchain.pem;"),
        "{written}"
    );
    assert!(
        written.contains("return 301 https://$host$request_uri;"),
        "{written}"
    );

    let (status, _) = call(&router, Method::DELETE, "/api/v1/services/api/site", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!site_file.exists());
}

/// Sin versión activa, systemd omitiría el arranque en silencio: el agente lo rechaza antes.
#[tokio::test]
async fn sin_version_activa_no_arranca_pero_se_puede_parar() {
    let report: ServiceStatusReport = ok(Method::GET, "/api/v1/services/sin-version/status").await;
    assert!(!report.deployed);
    for action in ["start", "restart"] {
        let (status, body) = fail(
            Method::POST,
            &format!("/api/v1/services/sin-version/{action}"),
        )
        .await;
        assert_eq!(
            (status, body.code),
            (StatusCode::CONFLICT, ErrorCode::NotDeployed),
            "{action}"
        );
    }
    assert!(
        !invocations().contains("start bbel@sin-version"),
        "{}",
        invocations()
    );
    let _: ServiceActionReport = ok(Method::POST, "/api/v1/services/sin-version/stop").await;
    // Tampoco hay versión anterior a la que volver.
    let (status, body) = fail(Method::POST, "/api/v1/services/sin-version/rollback").await;
    assert_eq!(
        (status, body.code),
        (StatusCode::CONFLICT, ErrorCode::NothingToRollBack)
    );
}

#[tokio::test]
async fn un_servicio_no_registrado_no_llega_a_systemd() {
    // `bbel@ajeno.service` podría existir en systemd: para el cliente no existe.
    let (status, body) = fail(Method::POST, "/api/v1/services/ajeno/start").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body.code, ErrorCode::ServiceNotFound);
    let (status, body) = fail(Method::GET, "/api/v1/services/ajeno/status").await;
    assert_eq!(
        (status, body.code),
        (StatusCode::NOT_FOUND, ErrorCode::ServiceNotFound)
    );
    assert!(!invocations().contains("ajeno"), "{}", invocations());
}

#[tokio::test]
async fn nombres_invalidos() {
    for name in ["API", "-x", "a.b", "a%20b", "bbel@api.service"] {
        let (status, body) = fail(Method::POST, &format!("/api/v1/services/{name}/start")).await;
        assert_eq!(
            (status, body.code),
            (StatusCode::BAD_REQUEST, ErrorCode::BadRequest),
            "{name}"
        );
    }
    let long = "a".repeat(200);
    let (_, body) = fail(Method::GET, &format!("/api/v1/services/{long}/status")).await;
    assert!(!body.message.contains(&long), "no repite la entrada");
}

/// Cada fallo de systemd tiene su código y un mensaje que sirve para depurar desde el cliente.
#[tokio::test]
async fn errores_con_codigo_y_diagnostico() {
    let cases = [
        (
            "falla",
            StatusCode::BAD_GATEWAY,
            ErrorCode::ServiceJobFailed,
            "failed because the control process exited with error code",
        ),
        (
            "sin-permiso",
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::SystemdPermissionDenied,
            "deploy/polkit/10-blacksmith.rules",
        ),
        (
            "sin-unidad",
            StatusCode::CONFLICT,
            ErrorCode::UnitNotFound,
            "systemd no tiene su unidad",
        ),
        (
            "lento",
            StatusCode::GATEWAY_TIMEOUT,
            ErrorCode::SystemdTimeout,
            "no terminó en",
        ),
    ];
    for (name, status, code, hint) in cases {
        let (got_status, body) =
            fail(Method::POST, &format!("/api/v1/services/{name}/start")).await;
        assert_eq!(
            (got_status, body.code),
            (status, code),
            "{name}: {}",
            body.message
        );
        assert!(body.message.contains(hint), "{name}: {}", body.message);
    }
    // El diagnóstico de un fallo del trabajo remite al journal de la unidad.
    let (_, body) = fail(Method::POST, "/api/v1/services/falla/start").await;
    assert!(
        body.message.contains("journalctl -u bbel@falla.service"),
        "{}",
        body.message
    );
}

#[tokio::test]
async fn salida_inesperada_de_systemd_es_un_500_util() {
    let (status, body) = fail(Method::GET, "/api/v1/services/raro/status").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body.code, ErrorCode::SystemdFailed);
    assert!(
        body.message.contains("salida inesperada"),
        "{}",
        body.message
    );
    assert!(body.message.contains("MainPID=abc"), "{}", body.message);
}

#[tokio::test]
async fn metodos_equivocados() {
    let (status, body) = fail(Method::GET, "/api/v1/services/api/start").await;
    assert_eq!(
        (status, body.code),
        (StatusCode::METHOD_NOT_ALLOWED, ErrorCode::MethodNotAllowed)
    );
    let (status, body) = fail(Method::POST, "/api/v1/services/api/status").await;
    assert_eq!(
        (status, body.code),
        (StatusCode::METHOD_NOT_ALLOWED, ErrorCode::MethodNotAllowed)
    );
}

// --- Comprobación de permisos al arrancar ------------------------------------------------

#[tokio::test]
async fn permisos_autorizados() {
    let manager = ServiceManager::new(
        Database::open_in_memory().unwrap(),
        Systemctl::new(&fake().program, Scope::System, Duration::from_secs(5)),
        fake().paths.clone(),
    );
    assert!(matches!(
        manager.check_permissions().await,
        PermissionCheck::Authorized
    ));
    assert!(invocations().contains("stop bbel@agent-permission-check.service"));
}

#[tokio::test]
async fn permisos_sin_systemctl_es_inconcluso() {
    let manager = ServiceManager::new(
        Database::open_in_memory().unwrap(),
        Systemctl::new(
            "/no/existe/systemctl",
            Scope::System,
            Duration::from_secs(5),
        ),
        fake().paths.clone(),
    );
    assert!(matches!(
        manager.check_permissions().await,
        PermissionCheck::Inconclusive(_)
    ));
}

/// Con el systemd real y un usuario sin la regla de polkit (p. ej. el de desarrollo).
#[tokio::test]
#[ignore = "usa el systemd real; ejecutar con un usuario que polkit no autorice"]
async fn permisos_reales_denegados() {
    let manager = ServiceManager::new(
        Database::open_in_memory().unwrap(),
        Systemctl::default(),
        fake().paths.clone(),
    );
    assert!(matches!(
        manager.check_permissions().await,
        PermissionCheck::Denied
    ));
}

// --- Registro y variables de entorno -----------------------------------------------------------

async fn json<T: DeserializeOwned>(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<serde_json::Value>,
    expected: StatusCode,
) -> T {
    let (status, bytes) = call(router, method, uri, body).await;
    assert_eq!(
        status,
        expected,
        "{uri}: {}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn registro_listado_y_baja() {
    let router = app().await;
    let created: ServiceSummary = json(
        &router,
        Method::POST,
        "/api/v1/services",
        Some(serde_json::json!({
            "name": " nuevo ",
            "displayName": "Servicio nuevo",
            "description": "API de pagos.\nSegunda línea."
        })),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(created.name, "nuevo");
    assert_eq!(created.unit, "bbel@nuevo.service");
    assert_eq!(created.description, "API de pagos.\nSegunda línea.");

    let listed: Vec<ServiceSummary> = json(
        &router,
        Method::GET,
        "/api/v1/services",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(listed.last(), Some(&created));
    assert_eq!(listed.first().map(|s| s.name.as_str()), Some("api"));

    let duplicate: ApiErrorBody = json(
        &router,
        Method::POST,
        "/api/v1/services",
        Some(serde_json::json!({ "name": "nuevo", "displayName": "Otro" })),
        StatusCode::CONFLICT,
    )
    .await;
    assert_eq!(duplicate.code, ErrorCode::ServiceAlreadyExists);

    for body in [
        serde_json::json!({ "name": "Mal Nombre", "displayName": "x" }),
        serde_json::json!({ "name": "agent-permission-check", "displayName": "x" }),
        serde_json::json!({ "name": "ok", "displayName": "" }),
        serde_json::json!({ "name": "ok", "displayName": "x", "description": "a\u{1b}b" }),
    ] {
        let error: ApiErrorBody = json(
            &router,
            Method::POST,
            "/api/v1/services",
            Some(body.clone()),
            StatusCode::BAD_REQUEST,
        )
        .await;
        assert_eq!(error.code, ErrorCode::BadRequest, "{body}");
    }

    let (status, _) = call(&router, Method::DELETE, "/api/v1/services/nuevo", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let listed: Vec<ServiceSummary> = json(
        &router,
        Method::GET,
        "/api/v1/services",
        None,
        StatusCode::OK,
    )
    .await;
    assert!(listed.iter().all(|service| service.name != "nuevo"));
    let missing: ApiErrorBody = json(
        &router,
        Method::DELETE,
        "/api/v1/services/nuevo",
        None,
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(missing.code, ErrorCode::ServiceNotFound);
    // Registrar no toca systemd.
    assert!(!invocations().contains("bbel@nuevo"), "{}", invocations());
}

#[tokio::test]
async fn variables_de_entorno() {
    let router = app().await;
    let file = fake().env_dir.join("services/api.env");
    let variables = serde_json::json!({ "variables": [
        { "key": "DATABASE_URL", "value": "postgres://u:p@localhost/db?x=1" },
        { "key": "SALUDO", "value": "  hola \"mundo\" $HOME  " },
    ]});
    let saved: ServiceEnvironment = json(
        &router,
        Method::PUT,
        "/api/v1/services/api/env",
        Some(variables),
        StatusCode::OK,
    )
    .await;
    assert_eq!(saved.unit, "bbel@api.service");
    assert_eq!(saved.file, file.display().to_string());
    assert!(saved.loaded_by_unit, "el drop-in inyecta el archivo");
    assert!(saved.global_loaded_by_unit, "y el global, antes");
    assert_eq!(saved.variables.len(), 2);
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let read: ServiceEnvironment = json(
        &router,
        Method::GET,
        "/api/v1/services/api/env",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(read, saved);

    // Inválido: 400 con el motivo y el archivo intacto.
    let before = fs::read_to_string(&file).unwrap();
    let error: ApiErrorBody = json(
        &router,
        Method::PUT,
        "/api/v1/services/api/env",
        Some(serde_json::json!({ "variables": [
            { "key": "BIEN", "value": "x" },
            { "key": "MAL", "value": "a\nINYECTADA=1" },
        ]})),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(error.code, ErrorCode::InvalidEnvironment);
    assert!(error.message.contains("MAL"), "{}", error.message);
    assert_eq!(fs::read_to_string(&file).unwrap(), before);

    // Sin la directiva en la unidad: se avisa con `loadedByUnit: false`.
    let missing_unit: ServiceEnvironment = json(
        &router,
        Method::GET,
        "/api/v1/services/sin-unidad/env",
        None,
        StatusCode::OK,
    )
    .await;
    assert!(!missing_unit.loaded_by_unit);
    assert!(!missing_unit.global_loaded_by_unit);
    assert!(missing_unit.variables.is_empty());

    let unregistered: ApiErrorBody = json(
        &router,
        Method::PUT,
        "/api/v1/services/ajeno/env",
        Some(serde_json::json!({ "variables": [] })),
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(unregistered.code, ErrorCode::ServiceNotFound);
    assert!(!fake().env_dir.join("services/ajeno.env").exists());
}

/// Al quitar un servicio, sus variables (secretos) no se quedan en el servidor.
#[tokio::test]
async fn la_baja_borra_las_variables() {
    let router = app().await;
    let _: ServiceSummary = json(
        &router,
        Method::POST,
        "/api/v1/services",
        Some(serde_json::json!({ "name": "temporal", "displayName": "Temporal" })),
        StatusCode::CREATED,
    )
    .await;
    let _: ServiceEnvironment = json(
        &router,
        Method::PUT,
        "/api/v1/services/temporal/env",
        Some(serde_json::json!({ "variables": [{ "key": "TOKEN", "value": "secreto" }] })),
        StatusCode::OK,
    )
    .await;
    let file = fake().env_dir.join("services/temporal.env");
    assert!(file.exists());
    let (status, _) = call(&router, Method::DELETE, "/api/v1/services/temporal", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!file.exists());
}

#[tokio::test]
async fn variables_globales() {
    let router = app().await;
    let saved: GlobalEnvironment = json(
        &router,
        Method::PUT,
        "/api/v1/env",
        Some(serde_json::json!({ "variables": [{ "key": "NODE_ENV", "value": "production" }] })),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        saved.file,
        fake().env_dir.join("global.env").display().to_string()
    );
    let read: GlobalEnvironment =
        json(&router, Method::GET, "/api/v1/env", None, StatusCode::OK).await;
    assert_eq!(read, saved);
    let error: ApiErrorBody = json(
        &router,
        Method::PUT,
        "/api/v1/env",
        Some(serde_json::json!({ "variables": [{ "key": "1MAL", "value": "x" }] })),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(error.code, ErrorCode::InvalidEnvironment);
}

#[tokio::test]
async fn definicion_del_despliegue() {
    let router = app().await;
    let empty: ServiceDeployment = json(
        &router,
        Method::GET,
        "/api/v1/services/parado/deployment",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(empty.repository, None);
    assert!(empty.setup_commands.is_empty());
    assert_eq!(empty.updated_at, None);
    assert_eq!(
        empty.working_directory,
        fake()
            .paths
            .apps_dir
            .join("parado/app")
            .display()
            .to_string()
    );

    let saved: ServiceDeployment = json(
        &router,
        Method::PUT,
        "/api/v1/services/parado/deployment",
        Some(serde_json::json!({
            "repository": { "owner": "HormigaDev", "name": "api", "branch": "main" },
            "setupCommands": [" npm ci ", "npm run build"],
            "startCommand": " node dist/server.js "
        })),
        StatusCode::OK,
    )
    .await;
    assert_eq!(saved.setup_commands, ["npm ci", "npm run build"]);
    assert_eq!(saved.start_command, "node dist/server.js");
    assert!(saved.updated_at.is_some());
    let read: ServiceDeployment = json(
        &router,
        Method::GET,
        "/api/v1/services/parado/deployment",
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(read, saved);

    for (body, hint) in [
        (
            serde_json::json!({ "repository": { "owner": "o", "name": "r", "branch": "a..b" }, "setupCommands": [], "startCommand": "" }),
            "rama",
        ),
        (
            serde_json::json!({ "repository": null, "setupCommands": ["npm ci\nrm -rf /"], "startCommand": "" }),
            "comando de preparación 0",
        ),
    ] {
        let error: ApiErrorBody = json(
            &router,
            Method::PUT,
            "/api/v1/services/parado/deployment",
            Some(body),
            StatusCode::BAD_REQUEST,
        )
        .await;
        assert_eq!(error.code, ErrorCode::InvalidDeployment);
        assert!(error.message.contains(hint), "{}", error.message);
    }
    // Guardar no ejecuta nada en systemd.
    assert!(
        !invocations().contains("start bbel@parado"),
        "{}",
        invocations()
    );
}

// --- systemd real: modo local completo ----------------------------------------------------

/// Plantillas de usuario, versiones y archivos del servicio de prueba: se borran al terminar
/// aunque el test falle.
struct LocalDeployment {
    slug: ServiceSlug,
    paths: Paths,
}

impl Drop for LocalDeployment {
    fn drop(&mut self) {
        let systemctl = |args: &[&str]| {
            let _ = std::process::Command::new("/usr/bin/systemctl")
                .arg("--user")
                .args(args)
                .output();
        };
        let unit = self.slug.unit();
        let deploy = self.slug.deploy_unit();
        systemctl(&["stop", unit.as_str(), deploy.as_str()]);
        systemctl(&["reset-failed", unit.as_str(), deploy.as_str()]);
        let _ = fs::remove_dir_all(self.paths.app_dir(&self.slug));
        let _ = fs::remove_dir_all(self.paths.deployment_dir(&self.slug));
        let _ = fs::remove_file(
            self.paths
                .env_dir
                .join(format!("services/{}.env", self.slug)),
        );
    }
}

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("/usr/bin/git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// Lo mismo que hace el agente en modo local, de punta a punta: plantillas en el systemd del
/// usuario, cuenta de GitHub del servidor, `bbel-deploy@` clona y prepara, `bbel@` arranca con
/// las variables; un reinicio aplica las nuevas. El repositorio es local (`file://`) en lugar
/// de GitHub.
#[tokio::test]
#[ignore = "usa el systemd de usuario real (instala plantillas y despliega un servicio de prueba)"]
async fn modo_local_de_punta_a_punta() {
    let paths = Paths::for_scope(Scope::User).unwrap();
    let units_dir = paths.units_dir.clone().unwrap();
    let binary = Path::new(env!("CARGO_BIN_EXE_blacksmith-agent"));
    if agent::deploy::units::install_user_units(&units_dir, binary).unwrap() {
        std::process::Command::new("/usr/bin/systemctl")
            .args(["--user", "daemon-reload"])
            .status()
            .unwrap();
    }
    let slug = ServiceSlug::new("blacksmith-prueba-local").unwrap();
    let _cleanup = LocalDeployment {
        slug: slug.clone(),
        paths: paths.clone(),
    };

    // Repositorio de origen: acme/demo con un servicio que deja constancia de lo que ve.
    let origin = tempfile::tempdir().unwrap();
    let work = origin.path().join("trabajo");
    fs::create_dir_all(&work).unwrap();
    fs::write(
        work.join("app.sh"),
        "printf '%s|%s' \"$SALUDO\" \"$(cat build.txt)\" > \"$HOME/visto\"\nexec sleep 300\n",
    )
    .unwrap();
    git(&work, &["init", "--quiet", "--initial-branch=main"]);
    git(&work, &["add", "."]);
    git(&work, &["commit", "--quiet", "-m", "inicial"]);
    let repos = origin.path().join("repos");
    fs::create_dir_all(repos.join("acme")).unwrap();
    git(
        origin.path(),
        &[
            "clone",
            "--quiet",
            "--bare",
            "trabajo",
            "repos/acme/demo.git",
        ],
    );

    let db = Database::open_in_memory().unwrap();
    db.insert_service(
        slug.clone(),
        ServiceName::new("Prueba local").unwrap(),
        ServiceDescription::default(),
    )
    .await
    .unwrap();
    let manager = ServiceManager::new(
        db,
        Systemctl::new("/usr/bin/systemctl", Scope::User, Duration::from_secs(30)),
        paths.clone(),
    )
    .with_clone_base(format!("file://{}/", repos.display()));

    manager
        .set_deployment(
            &slug,
            shared::UpdateDeployment {
                repository: Some(shared::RepositoryRef {
                    owner: "acme".into(),
                    name: "demo".into(),
                    branch: "main".into(),
                }),
                access: shared::RepositoryAccess::Account,
                setup_commands: vec!["echo construido > build.txt".into()],
                start_command: "sh app.sh".into(),
                publish_dir: None,
            },
        )
        .await
        .unwrap();
    let variable = |value: &str| shared::EnvVariable {
        key: "SALUDO".into(),
        value: value.into(),
    };
    manager
        .set_environment(&slug, vec![variable("  primero $HOME  ")])
        .await
        .unwrap();
    // Con cuenta de GitHub: el token llega a la unidad de despliegue y su copia se borra al
    // terminar (con `file://` git no lo pide, pero la credencial debe existir igual).
    let github_before = manager.github_access().await.unwrap();
    manager
        .set_github_access(shared::SetGitHubAccess {
            login: "octocat".into(),
            token: "ghu_prueba0123456789abcdef".into(),
        })
        .await
        .unwrap();
    let token_copy = paths.deployment_dir(&slug).join("github-token");

    let wait_run = || async {
        for _ in 0..150 {
            let runs = manager.deploy_runs(&slug).await.unwrap();
            if let Some(run) = runs.first()
                && run.status != shared::DeployStatus::Running
            {
                return run.clone();
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        panic!("el despliegue no terminó");
    };
    let seen = paths.app_dir(&slug).join("visto");
    let wait_seen = || async {
        for _ in 0..100 {
            if let Ok(value) = fs::read_to_string(&seen)
                && !value.is_empty()
            {
                return value;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("el servicio no escribió nada");
    };

    manager.deploy(&slug).await.unwrap();
    let run = wait_run().await;
    assert_eq!(
        run.status,
        shared::DeployStatus::Succeeded,
        "{:?}",
        run.detail
    );
    assert_eq!(wait_seen().await, "  primero $HOME  |construido");
    assert!(!token_copy.exists(), "la copia del token sigue en disco");
    if github_before.is_none() {
        manager.remove_github_access().await.unwrap();
    }
    let status = manager.status(&slug).await.unwrap();
    assert_eq!(status.active_state, shared::ActiveState::Active);
    assert!(status.pid.is_some());

    // Variables nuevas: se aplican al reiniciar, sin redesplegar.
    manager
        .set_environment(&slug, vec![variable("segundo")])
        .await
        .unwrap();
    fs::remove_file(&seen).unwrap();
    manager.restart(&slug).await.unwrap();
    assert_eq!(wait_seen().await, "segundo|construido");
    let first_release = manager.status(&slug).await.unwrap().release.unwrap();

    // Una versión nueva y, después, rollback a la anterior: sin clonar ni preparar nada.
    let define = |setup: &str| shared::UpdateDeployment {
        repository: Some(shared::RepositoryRef {
            owner: "acme".into(),
            name: "demo".into(),
            branch: "main".into(),
        }),
        access: shared::RepositoryAccess::Account,
        setup_commands: vec![setup.into()],
        start_command: "sh app.sh".into(),
        publish_dir: None,
    };
    manager
        .set_deployment(&slug, define("echo v2 > build.txt"))
        .await
        .unwrap();
    fs::remove_file(&seen).unwrap();
    manager.deploy(&slug).await.unwrap();
    assert_eq!(wait_run().await.status, shared::DeployStatus::Succeeded);
    assert_eq!(wait_seen().await, "segundo|v2");
    let status = manager.status(&slug).await.unwrap();
    assert_eq!(
        status.previous_release.as_deref(),
        Some(first_release.as_str())
    );

    fs::remove_file(&seen).unwrap();
    let rollback = manager.rollback(&slug).await.unwrap();
    assert_eq!(rollback.kind, shared::DeployKind::Rollback);
    assert_eq!(rollback.release, first_release);
    assert_eq!(rollback.branch, "main");
    assert_eq!(wait_run().await.status, shared::DeployStatus::Succeeded);
    assert_eq!(wait_seen().await, "segundo|construido");
    assert_eq!(
        manager.status(&slug).await.unwrap().release,
        Some(first_release)
    );

    // El repositorio ya no se puede cambiar; la rama sí.
    let locked = manager
        .set_deployment(
            &slug,
            shared::UpdateDeployment {
                repository: Some(shared::RepositoryRef {
                    owner: "otro".into(),
                    name: "demo".into(),
                    branch: "main".into(),
                }),
                access: shared::RepositoryAccess::Account,
                setup_commands: vec![],
                start_command: "sh app.sh".into(),
                publish_dir: None,
            },
        )
        .await;
    assert!(matches!(
        locked,
        Err(agent::services::ServiceError::RepositoryLocked)
    ));

    // Con llave de despliegue: sin llave no se despliega; con ella, sí (con `file://` git no usa
    // SSH, pero la llave tiene que llegar a la unidad).
    let mut with_key = define("echo v3 > build.txt");
    with_key.access = shared::RepositoryAccess::DeployKey;
    manager.set_deployment(&slug, with_key).await.unwrap();
    assert!(matches!(
        manager.deploy(&slug).await,
        Err(agent::services::ServiceError::DeployKeyMissing)
    ));
    let key = manager.create_deploy_key(&slug).await.unwrap();
    assert!(key.public_key.starts_with("ssh-ed25519 "));
    assert_eq!(manager.deploy_key(&slug).await.unwrap(), Some(key));
    fs::remove_file(&seen).unwrap();
    manager.deploy(&slug).await.unwrap();
    let run = wait_run().await;
    assert_eq!(
        run.status,
        shared::DeployStatus::Succeeded,
        "{:?}",
        run.detail
    );
    assert_eq!(wait_seen().await, "segundo|v3");
    assert!(
        !paths.deployment_dir(&slug).join("deploy-key").exists(),
        "la copia de la llave sigue en disco"
    );
}
