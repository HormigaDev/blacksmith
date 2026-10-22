//! `Systemctl` contra procesos reales.
//!
//! - Un `systemctl` falso (script de shell) verifica argumentos, entorno, tiempo máximo y salida
//!   acotada sin tocar systemd.
//! - Los tests `#[ignore]` usan systemd de verdad: `cargo test -p agent -- --ignored`.
//!   El de usuario crea unidades temporales en `$XDG_RUNTIME_DIR/systemd/user` y las borra.

// Los helpers no son `#[test]`, así que `allow-unwrap-in-tests` no los cubre.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
    time::{Duration, Instant},
};

use agent::systemd::{Action, Outcome, Scope, Systemctl, SystemctlError, UnitName};

/// El comportamiento depende del último argumento (la unidad).
const FAKE: &str = r#"#!/bin/sh
for unit; do :; done
case "$unit" in
    ok.service) exit 0 ;;
    lento.service) exec /bin/sleep 30 ;;
    ruidoso.service) /usr/bin/head -c 1000000 /dev/zero | /usr/bin/tr '\0' x >&2; exit 1 ;;
    eco.service) echo "ARGS: $* ENV: $(/usr/bin/env | /usr/bin/sort | /usr/bin/tr '\n' ' ')" >&2; exit 1 ;;
esac
echo "Failed to start $unit: Unit $unit not found." >&2
exit 5
"#;

/// Se escribe una sola vez antes de cualquier ejecución: escribir un ejecutable mientras otro
/// hilo hace `fork` puede dar `ETXTBSY` al lanzarlo.
fn fake() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("systemctl-falso");
        fs::write(&path, FAKE).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    })
}

fn systemctl(timeout: Duration) -> Systemctl {
    Systemctl::new(fake(), Scope::System, timeout)
}

fn unit(name: &str) -> UnitName {
    UnitName::new(name).unwrap()
}

#[tokio::test]
async fn exito_y_unidad_inexistente() {
    let systemctl = systemctl(Duration::from_secs(5));
    assert_eq!(
        systemctl.restart(&unit("ok")).await.unwrap(),
        Outcome::Restarted
    );
    let error = systemctl.start(&unit("otra")).await.unwrap_err();
    assert!(
        matches!(error, SystemctlError::UnitNotFound { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn argumentos_separados_y_entorno_vacio() {
    let error = systemctl(Duration::from_secs(5))
        .stop(&unit("eco"))
        .await
        .unwrap_err();
    let SystemctlError::Failed { detail, status, .. } = error else {
        panic!("se esperaba Failed: {error:?}");
    };
    assert_eq!(status, Some(1));
    let (args, env) = detail.split_once(" ENV: ").unwrap();
    // Cada argumento por separado, con `--` antes de la unidad.
    assert_eq!(
        args,
        "ARGS: --no-ask-password --no-pager stop -- eco.service"
    );
    // Nada del entorno del agente (HOME, PATH, secretos…): solo lo que fija `Systemctl` más lo
    // que añade el propio shell.
    let vars: Vec<_> = env
        .split_whitespace()
        .filter_map(|pair| pair.split_once('=').map(|(key, _)| key))
        .filter(|key| !matches!(*key, "PWD" | "SHLVL" | "_"))
        .collect();
    assert_eq!(vars, ["LC_ALL", "SYSTEMD_COLORS"], "{env}");
    assert!(env.contains("LC_ALL=C"), "{env}");
}

#[tokio::test]
async fn el_tiempo_maximo_mata_el_proceso() {
    let start = Instant::now();
    let error = systemctl(Duration::from_millis(200))
        .start(&unit("lento"))
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            SystemctlError::Timeout {
                action: Action::Start,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(start.elapsed() < Duration::from_secs(5));
    // El `sleep 30` no queda huérfano.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let ps = Command::new("pgrep")
        .args(["-f", "^/bin/sleep 30$"])
        .output()
        .unwrap();
    assert!(
        ps.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&ps.stdout)
    );
}

#[tokio::test]
async fn salida_de_error_enorme_acotada_sin_bloquear() {
    let error = systemctl(Duration::from_secs(10))
        .start(&unit("ruidoso"))
        .await
        .unwrap_err();
    let SystemctlError::Failed { detail, .. } = error else {
        panic!("se esperaba Failed: {error:?}");
    };
    assert!(detail.len() <= 4 * 1024, "{}", detail.len());
}

#[tokio::test]
async fn programa_inexistente() {
    let error = Systemctl::new(
        "/no/existe/systemctl",
        Scope::System,
        Duration::from_secs(1),
    )
    .start(&unit("ok"))
    .await
    .unwrap_err();
    assert!(matches!(error, SystemctlError::Io(_)), "{error:?}");
}

// --- systemd real -------------------------------------------------------------------------

/// Sin regla de polkit, un usuario sin privilegios no puede gestionar servicios del sistema.
/// No cambia nada: polkit rechaza antes de mirar la unidad.
#[tokio::test]
#[ignore = "usa el systemd real; ejecutar como usuario sin privilegios ni regla de polkit"]
async fn sistema_sin_autorizacion() {
    let error = Systemctl::default()
        .start(&unit("blacksmith-prueba-inexistente"))
        .await
        .unwrap_err();
    assert!(
        matches!(error, SystemctlError::PermissionDenied { .. }),
        "{error:?}"
    );
}

struct UserUnits {
    dir: PathBuf,
    names: Vec<&'static str>,
}

impl UserUnits {
    fn create(units: &[(&'static str, &str)]) -> Self {
        let runtime = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR");
        let dir = Path::new(&runtime).join("systemd/user");
        fs::create_dir_all(&dir).unwrap();
        for (name, body) in units {
            fs::write(dir.join(name), body).unwrap();
        }
        let units = Self {
            dir,
            names: units.iter().map(|(name, _)| *name).collect(),
        };
        user_systemctl(&["daemon-reload"]);
        units
    }
}

impl Drop for UserUnits {
    fn drop(&mut self) {
        for name in &self.names {
            user_systemctl(&["stop", name]);
            let _ = fs::remove_file(self.dir.join(name));
        }
        user_systemctl(&["daemon-reload"]);
        for name in &self.names {
            user_systemctl(&["reset-failed", name]);
        }
    }
}

fn user_systemctl(args: &[&str]) {
    let _ = Command::new("/usr/bin/systemctl")
        .arg("--user")
        .args(args)
        .output();
}

fn is_active(name: &str) -> bool {
    Command::new("/usr/bin/systemctl")
        .args(["--user", "is-active", "--quiet", name])
        .status()
        .unwrap()
        .success()
}

#[tokio::test]
#[ignore = "usa el systemd de usuario real (crea y borra unidades temporales)"]
async fn ciclo_de_vida_real_de_usuario() {
    let _units = UserUnits::create(&[
        (
            "blacksmith-prueba-ok.service",
            "[Service]\nExecStart=/bin/sleep 300\n",
        ),
        (
            "blacksmith-prueba-falla.service",
            "[Service]\nType=oneshot\nExecStart=/bin/false\n",
        ),
        (
            "blacksmith-prueba-manual.service",
            "[Unit]\nRefuseManualStart=yes\n[Service]\nExecStart=/bin/sleep 300\n",
        ),
    ]);
    let systemctl = Systemctl::new("/usr/bin/systemctl", Scope::User, Duration::from_secs(30));
    let ok = unit("blacksmith-prueba-ok");

    assert_eq!(systemctl.start(&ok).await.unwrap(), Outcome::Started);
    assert!(is_active(ok.as_str()));
    assert_eq!(systemctl.restart(&ok).await.unwrap(), Outcome::Restarted);
    assert!(is_active(ok.as_str()));
    assert_eq!(systemctl.stop(&ok).await.unwrap(), Outcome::Stopped);
    assert!(!is_active(ok.as_str()));
    // Parar lo ya parado no es un error.
    assert_eq!(systemctl.stop(&ok).await.unwrap(), Outcome::Stopped);

    let error = systemctl
        .start(&unit("blacksmith-prueba-falla"))
        .await
        .unwrap_err();
    assert!(
        matches!(error, SystemctlError::JobFailed { .. }),
        "{error:?}"
    );

    // Sin archivo de unidad: `mask --runtime` crea el enlace a /dev/null en el mismo directorio
    // temporal y falla si ya hay un archivo con ese nombre.
    user_systemctl(&["mask", "--runtime", "blacksmith-prueba-enmascarada.service"]);
    let error = systemctl
        .start(&unit("blacksmith-prueba-enmascarada"))
        .await;
    user_systemctl(&[
        "unmask",
        "--runtime",
        "blacksmith-prueba-enmascarada.service",
    ]);
    assert!(
        matches!(error, Err(SystemctlError::UnitMasked { .. })),
        "{error:?}"
    );

    let error = systemctl
        .start(&unit("blacksmith-prueba-manual"))
        .await
        .unwrap_err();
    assert!(
        matches!(error, SystemctlError::ManualOperationRefused { .. }),
        "{error:?}"
    );

    for action in [Action::Start, Action::Stop, Action::Restart] {
        let error = systemctl
            .run(action, &unit("blacksmith-prueba-inexistente"))
            .await
            .unwrap_err();
        assert!(
            matches!(error, SystemctlError::UnitNotFound { .. }),
            "{action}: {error:?}"
        );
    }
}
