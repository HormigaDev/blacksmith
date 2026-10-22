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
    http,
    services::{PermissionCheck, ServiceManager, ServiceName, ServiceSlug},
    state::AppState,
    systemd::{Scope, Systemctl},
};
use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use shared::{
    ActiveState, ApiErrorBody, ErrorCode, LoadState, ServiceActionReport, ServiceOutcome,
    ServiceStatusReport,
};
use tower::ServiceExt;

/// `$3` es el verbo (tras `--no-ask-password --no-pager`) y el último argumento, la unidad.
/// `MainPID=$PPID`: el proceso del test, que está vivo y se puede medir de verdad.
const FAKE: &str = r#"#!/bin/sh
for unit; do :; done
echo "$3 $unit" >> "__LOG__"
case "$3:$unit" in
    show:bbel-api.service)
        echo "MainPID=$PPID"; echo "LoadState=loaded"; echo "ActiveState=active"; echo "SubState=running" ;;
    show:bbel-parado.service)
        echo "MainPID=0"; echo "LoadState=loaded"; echo "ActiveState=inactive"; echo "SubState=dead" ;;
    show:bbel-sin-unidad.service)
        echo "MainPID=0"; echo "LoadState=not-found"; echo "ActiveState=inactive"; echo "SubState=dead" ;;
    show:bbel-raro.service) echo "MainPID=abc" ;;
    start:bbel-api.service|stop:bbel-api.service|restart:bbel-api.service) exit 0 ;;
    start:bbel-falla.service)
        echo "Job for bbel-falla.service failed because the control process exited with error code." >&2
        echo "See \"systemctl status bbel-falla.service\" for details." >&2
        exit 1 ;;
    start:bbel-sin-permiso.service)
        echo "Failed to start bbel-sin-permiso.service: Access denied" >&2; exit 1 ;;
    start:bbel-sin-unidad.service)
        echo "Failed to start bbel-sin-unidad.service: Unit bbel-sin-unidad.service not found." >&2; exit 5 ;;
    start:bbel-lento.service) exec /bin/sleep 30 ;;
    stop:bbel-agent-permission-check.service)
        echo "Failed to stop bbel-agent-permission-check.service: Unit bbel-agent-permission-check.service not loaded." >&2
        exit 5 ;;
    *) echo "inesperado: $*" >&2; exit 99 ;;
esac
"#;

struct Fake {
    program: PathBuf,
    log: PathBuf,
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
        fs::write(&program, FAKE.replace("__LOG__", log.to_str().unwrap())).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        Fake { program, log }
    })
}

fn invocations() -> String {
    fs::read_to_string(&fake().log).unwrap_or_default()
}

async fn app() -> Router {
    let config = Config::from_vars([("BLACKSMITH_ENV", "production")]).unwrap();
    let db = Database::open_in_memory().unwrap();
    for slug in [
        "api",
        "parado",
        "sin-unidad",
        "raro",
        "falla",
        "sin-permiso",
        "lento",
    ] {
        db.insert_service(
            ServiceSlug::new(slug).unwrap(),
            ServiceName::new(slug).unwrap(),
        )
        .await
        .unwrap();
    }
    let systemctl = Systemctl::new(&fake().program, Scope::System, Duration::from_millis(500));
    http::router(AppState::with_systemctl(Arc::new(config), db, systemctl))
}

async fn send(method: Method, uri: &str) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:4040")
        .body(Body::empty())
        .unwrap();
    let response = app().await.oneshot(request).await.unwrap();
    let status = response.status();
    let content_type = response.headers().get("content-type").cloned();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    assert_eq!(content_type.unwrap(), "application/json", "{uri}");
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
    assert_eq!(report.unit, "bbel-api.service");
    assert_eq!(report.load_state, LoadState::Loaded);
    assert_eq!(report.active_state, ActiveState::Active);
    assert_eq!(report.sub_state, "running");
    assert_eq!(report.pid, Some(std::process::id()));
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
        assert_eq!(report.unit, "bbel-api.service");
        assert!(invocations().contains(&format!("{action} bbel-api.service")));
    }
}

#[tokio::test]
async fn un_servicio_no_registrado_no_llega_a_systemd() {
    // `bbel-ajeno.service` podría existir en systemd: para el cliente no existe.
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
    for name in ["API", "-x", "a.b", "a%20b", "bbel-api.service"] {
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
        body.message.contains("journalctl -u bbel-falla.service"),
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
    );
    assert!(matches!(
        manager.check_permissions().await,
        PermissionCheck::Authorized
    ));
    assert!(invocations().contains("stop bbel-agent-permission-check.service"));
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
    let manager = ServiceManager::new(Database::open_in_memory().unwrap(), Systemctl::default());
    assert!(matches!(
        manager.check_permissions().await,
        PermissionCheck::Denied
    ));
}
