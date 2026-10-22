//! Invocación segura de `systemctl` para el ciclo de vida de los servicios.
//!
//! - Sin shell: el binario y cada argumento se pasan por separado con [`std::process::Command`],
//!   así que ningún carácter del nombre de unidad se interpreta.
//! - Ruta absoluta y entorno vacío: ni `PATH` ni variables del agente influyen en `systemctl`.
//!   `LC_ALL=C` fija el idioma de los mensajes, que es lo que permite clasificar los errores.
//! - `--no-ask-password`: sin autorización falla al momento en lugar de esperar una contraseña.
//! - `--` antes del nombre: aunque [`UnitName`] ya lo impide, nada puede leerse como opción.
//! - Salida de error acotada y tiempo máximo: si se agota, el proceso se mata.
//!
//! El código de salida no basta para clasificar: polkit rechaza antes de mirar si la unidad
//! existe y casi todos los fallos salen con 1. Se clasifica por el mensaje (verificado contra
//! systemd 255) y el código solo se usa como respaldo (5 = unidad no encontrada).

use std::{
    fmt, io,
    path::PathBuf,
    process::{ExitStatus, Stdio},
    time::Duration,
};

use tokio::io::AsyncReadExt;

use super::UnitName;

/// Ruta de `systemctl` en Debian/Ubuntu (con `/usr` fusionado) y derivadas de RHEL.
pub const SYSTEMCTL_PATH: &str = "/usr/bin/systemctl";

/// Mayor que el `DefaultTimeoutStartSec` de systemd (90 s): normalmente systemd da el trabajo por
/// fallido antes de que se agote este tiempo.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Bytes de la salida de error que se conservan; el resto se descarta sin almacenarlo.
const STDERR_LIMIT: u64 = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Start,
    Stop,
    Restart,
}

impl Action {
    fn verb(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }

    fn outcome(self) -> Outcome {
        match self {
            Self::Start => Outcome::Started,
            Self::Stop => Outcome::Stopped,
            Self::Restart => Outcome::Restarted,
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.verb())
    }
}

/// Resultado de una acción completada: `systemctl` espera a que systemd termine el trabajo, así
/// que `Started` significa que el servicio arrancó (o ya estaba activo), no solo que se pidió.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    Started,
    Stopped,
    Restarted,
}

/// Gestor de servicios al que se dirigen las acciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// Servicios del sistema (`/etc/systemd/system`). Requiere autorización de polkit.
    #[default]
    System,
    /// Servicios del propio usuario (`systemctl --user`). Necesita su bus de sesión.
    User,
}

#[derive(Debug, thiserror::Error)]
pub enum SystemctlError {
    #[error("la unidad {unit} no existe")]
    UnitNotFound { unit: UnitName },

    #[error("sin permisos para {action} {unit}")]
    PermissionDenied { action: Action, unit: UnitName },

    #[error("la unidad {unit} está enmascarada")]
    UnitMasked { unit: UnitName },

    /// `RefuseManualStart=` / `RefuseManualStop=`: solo se activa como dependencia.
    #[error("la unidad {unit} no admite {action} manual")]
    ManualOperationRefused { action: Action, unit: UnitName },

    /// systemd ejecutó el trabajo y el servicio falló (proceso que termina con error, timeout de
    /// arranque…). Los detalles están en el journal de la unidad.
    #[error("{action} de {unit} falló: {detail}")]
    JobFailed {
        action: Action,
        unit: UnitName,
        detail: String,
    },

    /// El trabajo puede seguir en curso en systemd: matar `systemctl` no lo cancela.
    #[error("{action} de {unit} no terminó en {after:?}")]
    Timeout {
        action: Action,
        unit: UnitName,
        after: Duration,
    },

    #[error("no se pudo ejecutar systemctl")]
    Io(#[source] io::Error),

    /// Cualquier otro fallo, con el código de salida (`None` si terminó por una señal).
    #[error("systemctl terminó con {status:?} al ejecutar {action} de {unit}: {detail}")]
    Failed {
        action: Action,
        unit: UnitName,
        status: Option<i32>,
        detail: String,
    },
}

/// Cliente de `systemctl`. Es barato de clonar y no guarda estado entre llamadas.
#[derive(Debug, Clone)]
pub struct Systemctl {
    program: PathBuf,
    scope: Scope,
    timeout: Duration,
}

impl Default for Systemctl {
    fn default() -> Self {
        Self::new(SYSTEMCTL_PATH, Scope::System, DEFAULT_TIMEOUT)
    }
}

impl Systemctl {
    /// `program` debe ser una ruta absoluta: no se busca en `PATH`.
    pub fn new(program: impl Into<PathBuf>, scope: Scope, timeout: Duration) -> Self {
        Self {
            program: program.into(),
            scope,
            timeout,
        }
    }

    pub async fn start(&self, unit: &UnitName) -> Result<Outcome, SystemctlError> {
        self.run(Action::Start, unit).await
    }

    pub async fn stop(&self, unit: &UnitName) -> Result<Outcome, SystemctlError> {
        self.run(Action::Stop, unit).await
    }

    pub async fn restart(&self, unit: &UnitName) -> Result<Outcome, SystemctlError> {
        self.run(Action::Restart, unit).await
    }

    pub async fn run(&self, action: Action, unit: &UnitName) -> Result<Outcome, SystemctlError> {
        let mut command = self.command(action, unit);
        let mut child = command.spawn().map_err(SystemctlError::Io)?;
        let stderr = child.stderr.take();

        // `async move`: si se agota el tiempo, el futuro se descarta con el hijo dentro y
        // `kill_on_drop` lo mata.
        let wait = async move {
            let mut captured = Vec::new();
            if let Some(stderr) = stderr {
                let mut limited = stderr.take(STDERR_LIMIT);
                limited.read_to_end(&mut captured).await?;
                // Se vacía el resto para que el hijo no se bloquee con la tubería llena.
                tokio::io::copy(&mut limited.into_inner(), &mut tokio::io::sink()).await?;
            }
            let status = child.wait().await?;
            Ok::<_, io::Error>((status, captured))
        };

        let (status, stderr) = tokio::time::timeout(self.timeout, wait)
            .await
            .map_err(|_| SystemctlError::Timeout {
                action,
                unit: unit.clone(),
                after: self.timeout,
            })?
            .map_err(SystemctlError::Io)?;

        let result = interpret(action, unit, status, &stderr);
        match &result {
            Ok(outcome) => {
                tracing::info!(%unit, %action, ?outcome, "acción de servicio completada")
            }
            Err(error) => tracing::warn!(%unit, %action, %error, "acción de servicio fallida"),
        }
        result
    }

    fn command(&self, action: Action, unit: &UnitName) -> tokio::process::Command {
        let mut command = std::process::Command::new(&self.program);
        command
            .env_clear()
            .env("LC_ALL", "C")
            .env("SYSTEMD_COLORS", "0");
        if self.scope == Scope::User {
            // El gestor de usuario se alcanza por el bus de sesión: son las únicas variables
            // que se heredan, y solo en este modo.
            for key in ["XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"] {
                if let Some(value) = std::env::var_os(key) {
                    command.env(key, value);
                }
            }
            command.arg("--user");
        }
        command
            .args(["--no-ask-password", "--no-pager", action.verb(), "--"])
            .arg(unit.as_str())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let mut command = tokio::process::Command::from(command);
        command.kill_on_drop(true);
        command
    }
}

/// Traduce el código de salida y la salida de error (`LC_ALL=C`) a un resultado tipado.
fn interpret(
    action: Action,
    unit: &UnitName,
    status: ExitStatus,
    stderr: &[u8],
) -> Result<Outcome, SystemctlError> {
    if status.success() {
        return Ok(action.outcome());
    }

    let text = String::from_utf8_lossy(stderr);
    let detail = text.trim();
    let unit = unit.clone();
    let code = status.code();

    // Mensajes de systemd 255 (src/systemctl/systemctl-start-unit.c y bus-wait-for-jobs.c).
    if detail.contains("Interactive authentication required") || detail.contains("Access denied") {
        Err(SystemctlError::PermissionDenied { action, unit })
    } else if detail.contains(" not found.") || detail.contains(" not loaded.") || code == Some(5) {
        Err(SystemctlError::UnitNotFound { unit })
    } else if detail.contains(" is masked.") {
        Err(SystemctlError::UnitMasked { unit })
    } else if detail.contains("Operation refused") {
        Err(SystemctlError::ManualOperationRefused { action, unit })
    } else if detail.starts_with("Job for ") {
        Err(SystemctlError::JobFailed {
            action,
            unit,
            detail: first_line(detail),
        })
    } else {
        Err(SystemctlError::Failed {
            action,
            unit,
            status: code,
            detail: first_line(detail),
        })
    }
}

/// La primera línea es el diagnóstico; las siguientes solo remiten a `systemctl status`.
fn first_line(detail: &str) -> String {
    detail.lines().next().unwrap_or_default().to_owned()
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;

    use super::*;

    fn exit(code: i32) -> ExitStatus {
        ExitStatus::from_raw(code << 8)
    }

    fn unit() -> UnitName {
        UnitName::new("api").unwrap()
    }

    fn check(code: i32, stderr: &str) -> Result<Outcome, SystemctlError> {
        interpret(Action::Start, &unit(), exit(code), stderr.as_bytes())
    }

    #[test]
    fn exito_segun_la_accion() {
        for (action, outcome) in [
            (Action::Start, Outcome::Started),
            (Action::Stop, Outcome::Stopped),
            (Action::Restart, Outcome::Restarted),
        ] {
            assert_eq!(interpret(action, &unit(), exit(0), b"").unwrap(), outcome);
        }
    }

    // Mensajes reales capturados de systemd 255 con `LC_ALL=C`.
    #[test]
    fn sin_permisos() {
        let error = check(
            1,
            "Failed to start api.service: Interactive authentication required.\n\
             See system logs and 'systemctl status api.service' for details.\n",
        );
        assert!(matches!(
            error,
            Err(SystemctlError::PermissionDenied {
                action: Action::Start,
                ..
            })
        ));
        // Denegación de la regla de polkit (deploy/polkit/verify.sh).
        let error = check(1, "Failed to start api.service: Access denied\n");
        assert!(matches!(
            error,
            Err(SystemctlError::PermissionDenied { .. })
        ));
    }

    #[test]
    fn unidad_inexistente() {
        let error = check(
            5,
            "Failed to start api.service: Unit api.service not found.\n",
        );
        assert!(matches!(error, Err(SystemctlError::UnitNotFound { .. })));
        // `stop` dice «not loaded».
        let error = check(
            5,
            "Failed to stop api.service: Unit api.service not loaded.\n",
        );
        assert!(matches!(error, Err(SystemctlError::UnitNotFound { .. })));
        // Código 5 aunque el mensaje cambie en otra versión.
        assert!(matches!(
            check(5, ""),
            Err(SystemctlError::UnitNotFound { .. })
        ));
    }

    #[test]
    fn enmascarada_y_rechazo_manual() {
        let error = check(
            1,
            "Failed to start api.service: Unit api.service is masked.\n",
        );
        assert!(matches!(error, Err(SystemctlError::UnitMasked { .. })));
        let error = check(
            4,
            "Failed to start api.service: Operation refused, unit api.service may be requested by \
             dependency only (it is configured to refuse manual start/stop).\n",
        );
        assert!(matches!(
            error,
            Err(SystemctlError::ManualOperationRefused { .. })
        ));
    }

    #[test]
    fn el_servicio_falla_al_arrancar() {
        let error = check(
            1,
            "Job for api.service failed because the control process exited with error code.\n\
             See \"systemctl status api.service\" and \"journalctl -xeu api.service\" for details.\n",
        );
        match error {
            Err(SystemctlError::JobFailed { detail, .. }) => assert_eq!(
                detail,
                "Job for api.service failed because the control process exited with error code."
            ),
            other => panic!("se esperaba JobFailed: {other:?}"),
        }
    }

    #[test]
    fn otros_fallos_conservan_codigo_y_detalle() {
        match check(3, "Something unexpected\nmore") {
            Err(SystemctlError::Failed { status, detail, .. }) => {
                assert_eq!(status, Some(3));
                assert_eq!(detail, "Something unexpected");
            }
            other => panic!("se esperaba Failed: {other:?}"),
        }
        // Terminado por una señal: sin código.
        let killed = interpret(Action::Stop, &unit(), ExitStatus::from_raw(9), b"");
        assert!(matches!(
            killed,
            Err(SystemctlError::Failed { status: None, .. })
        ));
    }

    #[test]
    fn argumentos_y_entorno() {
        let command = Systemctl::default().command(Action::Restart, &unit());
        let std = command.as_std();
        assert_eq!(std.get_program(), SYSTEMCTL_PATH);
        let args: Vec<_> = std.get_args().collect();
        assert_eq!(
            args,
            [
                "--no-ask-password",
                "--no-pager",
                "restart",
                "--",
                "api.service"
            ]
        );
        let envs: Vec<_> = std.get_envs().collect();
        assert!(envs.contains(&("LC_ALL".as_ref(), Some("C".as_ref()))));
        assert!(
            !envs
                .iter()
                .any(|(key, _)| *key == "DBUS_SESSION_BUS_ADDRESS")
        );
    }
}
