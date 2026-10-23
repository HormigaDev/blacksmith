//! Invocación segura de `systemctl`: ciclo de vida de los servicios y consulta de su PID.
//!
//! - Sin shell: el binario y cada argumento se pasan por separado con [`std::process::Command`],
//!   así que ningún carácter del nombre de unidad se interpreta.
//! - Ruta absoluta y entorno vacío: ni `PATH` ni variables del agente influyen en `systemctl`.
//!   `LC_ALL=C` fija el idioma de los mensajes, que es lo que permite clasificar los errores.
//! - `--no-ask-password`: sin autorización falla al momento en lugar de esperar una contraseña.
//! - `--` antes del nombre: aunque [`UnitName`] ya lo impide, nada puede leerse como opción.
//! - Salidas acotadas y tiempo máximo: si se agota, el proceso se mata.
//!
//! El código de salida no basta para clasificar: polkit rechaza antes de mirar si la unidad
//! existe y casi todos los fallos salen con 1. Se clasifica por el mensaje (verificado contra
//! systemd 255) y el código solo se usa como respaldo (5 = unidad no encontrada).

use std::{
    fmt, io,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};

use rustix::process::Pid;
use shared::{ActiveState, LoadState};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Child,
};

use super::UnitName;

/// Ruta de `systemctl` en Debian/Ubuntu (con `/usr` fusionado) y derivadas de RHEL.
pub const SYSTEMCTL_PATH: &str = "/usr/bin/systemctl";

/// Mayor que el `DefaultTimeoutStartSec` de systemd (90 s): normalmente systemd da el trabajo por
/// fallido antes de que se agote este tiempo.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Las consultas no esperan a ningún trabajo de systemd: si tardan más, algo va mal.
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Bytes que se conservan de cada salida; el resto se descarta sin almacenarlo.
const OUTPUT_LIMIT: u64 = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Start,
    Stop,
    Restart,
}

impl Action {
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
        Operation::from(*self).fmt(f)
    }
}

/// Lo que se estaba haciendo cuando falló `systemctl`: una acción o una consulta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    Start,
    Stop,
    Restart,
    /// `show -p LoadState -p ActiveState -p SubState -p MainPID`.
    ShowStatus,
    /// `show -p EnvironmentFiles`.
    ShowEnvironmentFiles,
    /// `set-property`: límites de recursos del cgroup.
    SetProperty,
}

impl Operation {
    fn verb(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::ShowStatus | Self::ShowEnvironmentFiles => "show",
            Self::SetProperty => "set-property",
        }
    }
}

impl From<Action> for Operation {
    fn from(action: Action) -> Self {
        match action {
            Action::Start => Self::Start,
            Action::Stop => Self::Stop,
            Action::Restart => Self::Restart,
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.verb())
    }
}

/// Resultado de una acción completada: `systemctl` espera a que systemd termine el trabajo, así
/// que `Started` significa que el servicio arrancó (o ya estaba activo), no solo que se pidió.
pub type Outcome = shared::ServiceOutcome;

/// Gestor de servicios al que se dirigen las acciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// Servicios del sistema (`/etc/systemd/system`). Las acciones requieren autorización de
    /// polkit; las consultas no.
    #[default]
    System,
    /// Servicios del propio usuario (`systemctl --user`). Necesita su bus de sesión.
    User,
}

#[derive(Debug, thiserror::Error)]
pub enum SystemctlError {
    #[error("la unidad {unit} no existe")]
    UnitNotFound { unit: UnitName },

    #[error("sin permisos para {operation} {unit}")]
    PermissionDenied {
        operation: Operation,
        unit: UnitName,
    },

    #[error("la unidad {unit} está enmascarada")]
    UnitMasked { unit: UnitName },

    /// `RefuseManualStart=` / `RefuseManualStop=`: solo se activa como dependencia.
    #[error("la unidad {unit} no admite {operation} manual")]
    ManualOperationRefused {
        operation: Operation,
        unit: UnitName,
    },

    /// systemd ejecutó el trabajo y el servicio falló (proceso que termina con error, timeout de
    /// arranque…). Los detalles están en el journal de la unidad.
    #[error("{operation} de {unit} falló: {detail}")]
    JobFailed {
        operation: Operation,
        unit: UnitName,
        detail: String,
    },

    /// En una acción, el trabajo puede seguir en curso en systemd: matar `systemctl` no lo
    /// cancela.
    #[error("{operation} de {unit} no terminó en {after:?}")]
    Timeout {
        operation: Operation,
        unit: UnitName,
        after: Duration,
    },

    #[error("no se pudo ejecutar systemctl")]
    Io(#[source] io::Error),

    /// `systemctl` terminó bien pero su salida no tiene el formato esperado.
    #[error("{operation} de {unit} devolvió una salida inesperada: {output:?}")]
    UnexpectedOutput {
        operation: Operation,
        unit: UnitName,
        output: String,
    },

    /// Cualquier otro fallo, con el código de salida (`None` si terminó por una señal).
    #[error("systemctl terminó con {status:?} al ejecutar {operation} de {unit}: {detail}")]
    Failed {
        operation: Operation,
        unit: UnitName,
        status: Option<i32>,
        detail: String,
    },
}

/// Estado de una unidad según systemd.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitStatus {
    pub load_state: LoadState,
    pub active_state: ActiveState,
    pub sub_state: String,
    /// `None` si no hay proceso principal: detenido, fallido, `Type=oneshot` ya terminado… o una
    /// unidad que no existe (systemd responde `0` en todos esos casos).
    pub main_pid: Option<Pid>,
    /// Contadores del cgroup de la unidad (todos sus procesos, no solo el principal).
    pub resources: UnitResources,
    /// Ruta del cgroup (`ControlGroup`, relativa a `/sys/fs/cgroup`), si está en marcha.
    pub control_group: Option<String>,
}

/// Contadores del cgroup de una unidad según systemd. `None` cuando systemd no los tiene (unidad
/// parada, contabilidad desactivada o sin límite).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnitResources {
    /// `MemoryCurrent`: memoria del cgroup (incluye la caché de páginas de sus archivos).
    pub memory: Option<u64>,
    /// `MemoryPeak`: máximo desde que arrancó.
    pub memory_peak: Option<u64>,
    /// `MemoryMax`: límite configurado.
    pub memory_max: Option<u64>,
    /// `MemorySwapCurrent`.
    pub swap: Option<u64>,
    /// `CPUUsageNSec`: CPU consumida desde que arrancó, en nanosegundos.
    pub cpu_nsec: Option<u64>,
    /// `TasksCurrent`: procesos e hilos.
    pub tasks: Option<u64>,
    /// `CPUQuotaPerSecUSec`: CPU permitida por segundo, en µs (1 000 000 = un núcleo).
    pub cpu_quota_us: Option<u64>,
    /// `MemorySwapMax`.
    pub swap_max: Option<u64>,
    /// `TasksMax`.
    pub tasks_max: Option<u64>,
    /// `NRestarts`: reinicios automáticos (`Restart=`) desde el último arranque manual.
    pub restarts: Option<u64>,
    /// `ActiveEnterTimestampMonotonic`: cuándo pasó a activa (µs del reloj monotónico).
    pub active_since_us: Option<u64>,
}

/// Propiedades que se piden para [`UnitResources`].
const RESOURCE_PROPERTIES: [&str; 11] = [
    "MemoryCurrent",
    "MemoryPeak",
    "MemoryMax",
    "MemorySwapCurrent",
    "CPUUsageNSec",
    "TasksCurrent",
    "NRestarts",
    "ActiveEnterTimestampMonotonic",
    "CPUQuotaPerSecUSec",
    "MemorySwapMax",
    "TasksMax",
];

/// Duración de `systemctl show` (`infinity`, `500ms`, `1.500000s`, `1min 30s`…) en µs.
/// `None` sin límite o con un formato desconocido.
fn parse_timespan_us(raw: &str) -> Option<u64> {
    if raw == "infinity" || raw.is_empty() {
        return None;
    }
    let mut total = 0.0_f64;
    for part in raw.split_whitespace() {
        let split = part
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(part.len());
        let (number, unit) = part.split_at_checked(split)?;
        let factor = match unit {
            "us" | "µs" => 1.0,
            "ms" => 1_000.0,
            "s" => 1_000_000.0,
            "min" => 60_000_000.0,
            _ => return None,
        };
        total += number.parse::<f64>().ok()? * factor;
    }
    // Redondeado al µs: es lo que guarda systemd.
    Some(total.round() as u64)
}

/// Ruta de cgroup que se puede unir a `/sys/fs/cgroup` sin salir de él: absoluta, sin `..` y
/// con los caracteres que usa systemd (los demás los escapa como `\x2d`).
fn valid_control_group(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 512
        && path.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b'.' | b'_' | b'-' | b'@' | b':' | b'\\')
        })
        && !path.split('/').any(|part| part == "..")
}

/// Valor numérico de systemd, o `None` si no tiene (`[not set]`, `infinity`, vacío, `0` en las
/// marcas de tiempo o `u64::MAX`, su «sin valor» interno).
fn parse_counter(raw: &str) -> Option<u64> {
    raw.parse::<u64>().ok().filter(|value| *value != u64::MAX)
}

/// Salida de una ejecución de `systemctl`, ya acotada.
struct Execution {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
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

    /// El mismo programa contra el systemd del sistema (nginx y certbot lo son también cuando
    /// los servicios van en el del usuario).
    pub fn system(&self) -> Self {
        Self {
            scope: Scope::System,
            ..self.clone()
        }
    }

    pub async fn run(&self, action: Action, unit: &UnitName) -> Result<Outcome, SystemctlError> {
        let result = self.run_unlogged(action, unit).await;
        match &result {
            Ok(outcome) => {
                tracing::info!(%unit, %action, ?outcome, "acción de servicio completada")
            }
            Err(error) => tracing::warn!(%unit, %action, %error, "acción de servicio fallida"),
        }
        result
    }

    /// Como [`Systemctl::run`], sin registrar el resultado (para la comprobación de permisos al
    /// arrancar, cuyo «fallo» esperado no debe aparecer como una acción fallida).
    pub(crate) async fn run_unlogged(
        &self,
        action: Action,
        unit: &UnitName,
    ) -> Result<Outcome, SystemctlError> {
        let operation = Operation::from(action);
        let execution = self
            .execute(operation, &[], unit, &[], self.timeout)
            .await?;
        if execution.status.success() {
            Ok(action.outcome())
        } else {
            Err(classify(
                operation,
                unit,
                execution.status,
                &execution.stderr,
            ))
        }
    }

    /// Estado de la unidad en una sola llamada; una unidad inexistente es `LoadState::NotFound`.
    /// No requiere polkit.
    pub async fn status(&self, unit: &UnitName) -> Result<UnitStatus, SystemctlError> {
        let mut options = Vec::with_capacity(2 * (5 + RESOURCE_PROPERTIES.len()));
        for property in [
            "LoadState",
            "ActiveState",
            "SubState",
            "MainPID",
            "ControlGroup",
        ]
        .into_iter()
        .chain(RESOURCE_PROPERTIES)
        {
            options.extend(["-p", property]);
        }
        self.show(Operation::ShowStatus, &options, unit, parse_status)
            .await
    }

    /// CPU consumida por el cgroup de la unidad (`CPUUsageNSec`), para medir el uso entre dos
    /// lecturas. `None` si systemd no la tiene.
    pub async fn cpu_usage_nsec(&self, unit: &UnitName) -> Result<Option<u64>, SystemctlError> {
        let options = ["-p", "CPUUsageNSec", "--value"];
        self.show(Operation::ShowStatus, &options, unit, |stdout| {
            Some(
                std::str::from_utf8(stdout)
                    .ok()
                    .and_then(|value| parse_counter(value.trim())),
            )
        })
        .await
    }

    /// Archivos `EnvironmentFile=` que systemd cargará al arrancar la unidad (de la unidad y de
    /// sus drop-ins, con los especificadores como `%N` ya resueltos). Vacío si no tiene ninguno
    /// o la unidad no existe. No requiere polkit.
    pub async fn environment_files(&self, unit: &UnitName) -> Result<Vec<PathBuf>, SystemctlError> {
        let options = ["-p", "EnvironmentFiles"];
        self.show(
            Operation::ShowEnvironmentFiles,
            &options,
            unit,
            parse_environment_files,
        )
        .await
    }

    /// `systemctl show`, con su propio tiempo máximo (una consulta no espera a ningún trabajo).
    /// `parse` devuelve `None` si no entiende la salida.
    async fn show<T>(
        &self,
        operation: Operation,
        options: &[&str],
        unit: &UnitName,
        parse: impl FnOnce(&[u8]) -> Option<T>,
    ) -> Result<T, SystemctlError> {
        let execution = self
            .execute(
                operation,
                options,
                unit,
                &[],
                self.timeout.min(QUERY_TIMEOUT),
            )
            .await?;
        if !execution.status.success() {
            return Err(classify(
                operation,
                unit,
                execution.status,
                &execution.stderr,
            ));
        }
        parse(&execution.stdout).ok_or_else(|| SystemctlError::UnexpectedOutput {
            operation,
            unit: unit.clone(),
            output: String::from_utf8_lossy(&execution.stdout).into_owned(),
        })
    }

    /// Aplica propiedades de control de recursos (`systemctl set-property`), al momento y de forma
    /// persistente. Requiere polkit (verbo `set-property`). Ver [`crate::limits`].
    pub async fn set_properties(
        &self,
        unit: &UnitName,
        properties: &[String],
    ) -> Result<(), SystemctlError> {
        let operation = Operation::SetProperty;
        let after: Vec<&str> = properties.iter().map(String::as_str).collect();
        let execution = self
            .execute(
                operation,
                &[],
                unit,
                &after,
                self.timeout.min(QUERY_TIMEOUT),
            )
            .await?;
        if execution.status.success() {
            tracing::info!(%unit, ?properties, "límites de recursos aplicados");
            Ok(())
        } else {
            let error = classify(operation, unit, execution.status, &execution.stderr);
            tracing::warn!(%unit, %error, "no se pudieron aplicar los límites");
            Err(error)
        }
    }

    /// Como [`Systemctl::run`] con otro tiempo máximo: para `Type=oneshot` largos (despliegues),
    /// `start` espera a que el trabajo termine y su resultado es el del trabajo.
    pub async fn run_with_timeout(
        &self,
        action: Action,
        unit: &UnitName,
        timeout: Duration,
    ) -> Result<Outcome, SystemctlError> {
        Self {
            timeout,
            ..self.clone()
        }
        .run(action, unit)
        .await
    }

    /// `after`: argumentos tras la unidad (las propiedades de `set-property`).
    async fn execute(
        &self,
        operation: Operation,
        options: &[&str],
        unit: &UnitName,
        after: &[&str],
        timeout: Duration,
    ) -> Result<Execution, SystemctlError> {
        let mut child = self
            .command(operation, options, unit, after)
            .spawn()
            .map_err(SystemctlError::Io)?;
        match tokio::time::timeout(timeout, collect(&mut child)).await {
            Ok(execution) => execution.map_err(SystemctlError::Io),
            Err(_) => {
                // Se mata y se recoge aquí: soltado sin más, quedaría zombi hasta que tokio lo
                // recogiera en segundo plano.
                let _ = child.kill().await;
                Err(SystemctlError::Timeout {
                    operation,
                    unit: unit.clone(),
                    after: timeout,
                })
            }
        }
    }

    fn command(
        &self,
        operation: Operation,
        options: &[&str],
        unit: &UnitName,
        after: &[&str],
    ) -> tokio::process::Command {
        let mut command = base_command(&self.program, self.scope);
        if self.scope == Scope::User {
            command.arg("--user");
        }
        command
            .args(["--no-ask-password", "--no-pager", operation.verb()])
            .args(options)
            .arg("--")
            .arg(unit.as_str())
            .args(after)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut command = tokio::process::Command::from(command);
        command.kill_on_drop(true);
        command
    }
}

/// `systemctl` o `journalctl` sin el entorno del agente: solo un idioma fijo (los mensajes se
/// clasifican por su texto) y, con el gestor de usuario, las variables de su bus de sesión.
pub(super) fn base_command(program: &Path, scope: Scope) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    command
        .env_clear()
        .env("LC_ALL", "C")
        .env("SYSTEMD_COLORS", "0");
    if scope == Scope::User {
        for key in ["XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
    }
    command
}

/// Lee ambas salidas a la vez (si se leyeran por turnos, el hijo podría bloquearse escribiendo
/// en la otra tubería llena) y espera a que termine.
async fn collect(child: &mut Child) -> io::Result<Execution> {
    let (stdout, stderr) = tokio::try_join!(
        read_limited(child.stdout.take()),
        read_limited(child.stderr.take()),
    )?;
    Ok(Execution {
        status: child.wait().await?,
        stdout,
        stderr,
    })
}

/// Conserva como mucho [`OUTPUT_LIMIT`] bytes y vacía el resto.
async fn read_limited(pipe: Option<impl AsyncRead + Unpin>) -> io::Result<Vec<u8>> {
    let mut captured = Vec::new();
    if let Some(pipe) = pipe {
        let mut limited = pipe.take(OUTPUT_LIMIT);
        limited.read_to_end(&mut captured).await?;
        tokio::io::copy(&mut limited.into_inner(), &mut tokio::io::sink()).await?;
    }
    Ok(captured)
}

/// Valor de `MainPID`: un entero decimal. `0` o vacío significan que no hay proceso principal.
/// Cualquier otra cosa es `None` del `Option` externo (salida inesperada).
fn parse_main_pid(stdout: &[u8]) -> Option<Option<Pid>> {
    let value = stdout.strip_suffix(b"\n").unwrap_or(stdout);
    if value.is_empty() {
        return Some(None);
    }
    if !value.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let pid: i32 = std::str::from_utf8(value).ok()?.parse().ok()?;
    // `from_raw` devuelve `None` para 0; los negativos no llegan aquí (solo dígitos).
    Some(Pid::from_raw(pid))
}

/// Salida de `show -p …` sin `--value`: una línea `Clave=Valor` por propiedad, en el orden
/// interno de systemd (no en el de los argumentos). Cada clave pedida debe aparecer una vez.
fn parse_status(stdout: &[u8]) -> Option<UnitStatus> {
    let text = std::str::from_utf8(stdout).ok()?;
    const STATE: [&str; 5] = [
        "LoadState",
        "ActiveState",
        "SubState",
        "MainPID",
        "ControlGroup",
    ];
    let mut fields = std::collections::HashMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=')?;
        // Solo lo pedido, y cada clave una vez: otra cosa es una salida que no se entiende.
        let known = STATE.contains(&key) || RESOURCE_PROPERTIES.contains(&key);
        if !known || fields.insert(key, value).is_some() {
            return None;
        }
    }
    let field = |name: &str| fields.get(name).copied();
    // Los contadores son opcionales: un systemd antiguo puede no tener alguno.
    let counter = |name: &str| field(name).and_then(parse_counter);
    let resources = UnitResources {
        memory: counter("MemoryCurrent"),
        memory_peak: counter("MemoryPeak"),
        memory_max: counter("MemoryMax"),
        swap: counter("MemorySwapCurrent"),
        cpu_nsec: counter("CPUUsageNSec"),
        tasks: counter("TasksCurrent"),
        restarts: counter("NRestarts"),
        active_since_us: counter("ActiveEnterTimestampMonotonic").filter(|value| *value > 0),
        cpu_quota_us: field("CPUQuotaPerSecUSec").and_then(parse_timespan_us),
        swap_max: counter("MemorySwapMax"),
        tasks_max: counter("TasksMax"),
    };
    let (load, active, sub, pid, group) = (
        field("LoadState"),
        field("ActiveState"),
        field("SubState"),
        field("MainPID"),
        field("ControlGroup"),
    );
    let sub_state = sub?;
    if sub_state.is_empty()
        || !sub_state
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return None;
    }
    Some(UnitStatus {
        load_state: parse_state(load?)?,
        active_state: parse_state(active?)?,
        sub_state: sub_state.to_owned(),
        main_pid: parse_main_pid(pid?.as_bytes())?,
        resources,
        control_group: group
            .filter(|path| valid_control_group(path))
            .map(str::to_owned),
    })
}

/// Una línea por archivo: `EnvironmentFiles=/ruta (ignore_errors=yes)`. Sin archivos, nada.
fn parse_environment_files(stdout: &[u8]) -> Option<Vec<PathBuf>> {
    std::str::from_utf8(stdout)
        .ok()?
        .lines()
        .map(|line| {
            let value = line.strip_prefix("EnvironmentFiles=")?;
            let path = value
                .strip_suffix(" (ignore_errors=yes)")
                .or_else(|| value.strip_suffix(" (ignore_errors=no)"))?;
            path.starts_with('/').then(|| PathBuf::from(path))
        })
        .collect()
}

/// Estados de systemd (`loaded`, `not-found`, `active`…): los mismos nombres que en el contrato
/// (`shared`, en kebab-case).
fn parse_state<T: serde::de::DeserializeOwned>(raw: &str) -> Option<T> {
    T::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(raw)).ok()
}

/// Clasifica un fallo por la salida de error (`LC_ALL=C`) y el código de salida.
fn classify(
    operation: Operation,
    unit: &UnitName,
    status: ExitStatus,
    stderr: &[u8],
) -> SystemctlError {
    let text = String::from_utf8_lossy(stderr);
    let detail = text.trim();
    let unit = unit.clone();
    let code = status.code();

    // Mensajes de systemd 255 (src/systemctl/systemctl-start-unit.c y bus-wait-for-jobs.c).
    if detail.contains("Interactive authentication required") || detail.contains("Access denied") {
        SystemctlError::PermissionDenied { operation, unit }
    } else if detail.contains(" not found.") || detail.contains(" not loaded.") || code == Some(5) {
        SystemctlError::UnitNotFound { unit }
    } else if detail.contains(" is masked.") {
        SystemctlError::UnitMasked { unit }
    } else if detail.contains("Operation refused") {
        SystemctlError::ManualOperationRefused { operation, unit }
    } else if detail.starts_with("Job for ") {
        SystemctlError::JobFailed {
            operation,
            unit,
            detail: first_line(detail),
        }
    } else {
        SystemctlError::Failed {
            operation,
            unit,
            status: code,
            detail: first_line(detail),
        }
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
        Err(classify(
            Operation::Start,
            &unit(),
            exit(code),
            stderr.as_bytes(),
        ))
    }

    #[test]
    fn exito_segun_la_accion() {
        for (action, outcome) in [
            (Action::Start, Outcome::Started),
            (Action::Stop, Outcome::Stopped),
            (Action::Restart, Outcome::Restarted),
        ] {
            assert_eq!(action.outcome(), outcome);
        }
    }

    #[test]
    fn pid_principal() {
        let pid = |raw: i32| Some(Some(Pid::from_raw(raw).unwrap()));
        assert_eq!(parse_main_pid(b"85072\n"), pid(85072));
        assert_eq!(parse_main_pid(b"1\n"), pid(1));
        assert_eq!(parse_main_pid(b"350"), pid(350));
        // Detenido: systemd devuelve 0; también se admite vacío.
        assert_eq!(parse_main_pid(b"0\n"), Some(None));
        assert_eq!(parse_main_pid(b"\n"), Some(None));
        assert_eq!(parse_main_pid(b""), Some(None));
    }

    // Salidas reales de systemd 255 (el orden no es el de los argumentos).
    #[test]
    fn estado_de_la_unidad() {
        let running = parse_status(
            b"MainPID=94584\nLoadState=loaded\nActiveState=active\nSubState=running\n",
        )
        .unwrap();
        assert_eq!(running.load_state, LoadState::Loaded);
        assert_eq!(running.active_state, ActiveState::Active);
        assert_eq!(running.sub_state, "running");
        assert_eq!(running.main_pid, Pid::from_raw(94584));

        let missing =
            parse_status(b"MainPID=0\nLoadState=not-found\nActiveState=inactive\nSubState=dead\n")
                .unwrap();
        assert_eq!(missing.load_state, LoadState::NotFound);
        assert_eq!(missing.main_pid, None);
        assert_eq!(missing.resources, UnitResources::default());
    }

    // Salida real de systemd 255 con los contadores del cgroup (en marcha y parada).
    #[test]
    fn contadores_del_cgroup() {
        let running = parse_status(
            b"MainPID=2286\nNRestarts=2\nLoadState=loaded\nActiveState=active\nSubState=running\n\
              ActiveEnterTimestampMonotonic=35279981\nMemoryCurrent=9285632\nMemoryPeak=12783616\n\
              MemorySwapCurrent=1081344\nMemoryMax=infinity\nCPUUsageNSec=98258438000\nTasksCurrent=3\n\
              CPUQuotaPerSecUSec=1.500000s\nMemorySwapMax=0\nTasksMax=16604\n\
              ControlGroup=/user.slice/user-1000.slice/user@1000.service/app.slice/blacksmith@api.service\n",
        )
        .unwrap();
        assert_eq!(
            running.control_group.as_deref(),
            Some("/user.slice/user-1000.slice/user@1000.service/app.slice/blacksmith@api.service")
        );
        assert!(!valid_control_group("/a/../../etc"));
        assert!(!valid_control_group("relativa"));
        assert_eq!(
            running.resources,
            UnitResources {
                memory: Some(9_285_632),
                memory_peak: Some(12_783_616),
                memory_max: None,
                swap: Some(1_081_344),
                cpu_nsec: Some(98_258_438_000),
                tasks: Some(3),
                restarts: Some(2),
                active_since_us: Some(35_279_981),
                cpu_quota_us: Some(1_500_000),
                swap_max: Some(0),
                tasks_max: Some(16_604),
            }
        );
        let stopped = parse_status(
            b"MainPID=0\nNRestarts=0\nLoadState=loaded\nActiveState=inactive\nSubState=dead\n\
              ActiveEnterTimestampMonotonic=0\nMemoryCurrent=[not set]\nMemoryPeak=[not set]\n\
              MemorySwapCurrent=[not set]\nMemoryMax=1073741824\nCPUUsageNSec=[not set]\n\
              TasksCurrent=18446744073709551615\n",
        )
        .unwrap();
        assert_eq!(
            stopped.resources,
            UnitResources {
                memory_max: Some(1_073_741_824),
                restarts: Some(0),
                ..UnitResources::default()
            }
        );
    }

    #[test]
    fn estado_con_salida_inesperada() {
        for raw in [
            // Falta una propiedad.
            &b"MainPID=1\nLoadState=loaded\nActiveState=active\n"[..],
            // Repetida.
            b"MainPID=1\nMainPID=2\nLoadState=loaded\nActiveState=active\nSubState=running\n",
            // Desconocida.
            b"MainPID=1\nLoadState=loaded\nActiveState=active\nSubState=running\nOtra=x\n",
            // Valor fuera del enum.
            b"MainPID=1\nLoadState=loaded\nActiveState=dancing\nSubState=running\n",
            b"MainPID=1\nLoadState=loaded\nActiveState=active\nSubState=run ning\n",
            b"MainPID=1\nLoadState=loaded\nActiveState=active\nSubState=\n",
            b"MainPID=x\nLoadState=loaded\nActiveState=active\nSubState=running\n",
            b"sin igual\n",
        ] {
            assert_eq!(
                parse_status(raw),
                None,
                "{:?}",
                String::from_utf8_lossy(raw)
            );
        }
    }

    // Salidas reales de systemd 255.
    #[test]
    fn archivos_de_entorno() {
        assert_eq!(parse_environment_files(b""), Some(vec![]));
        assert_eq!(
            parse_environment_files(
                b"EnvironmentFiles=/etc/default/anacron (ignore_errors=no)\nEnvironmentFiles=/etc/blacksmith/env/blacksmith-api.env (ignore_errors=yes)\n"
            ),
            Some(vec![
                PathBuf::from("/etc/default/anacron"),
                PathBuf::from("/etc/blacksmith/env/blacksmith-api.env")
            ])
        );
        for raw in [
            &b"EnvironmentFiles=/x\n"[..],
            b"Otra=/x (ignore_errors=no)\n",
            b"EnvironmentFiles=relativa (ignore_errors=no)\n",
        ] {
            assert_eq!(
                parse_environment_files(raw),
                None,
                "{:?}",
                String::from_utf8_lossy(raw)
            );
        }
    }

    #[test]
    fn pid_principal_con_salida_inesperada() {
        for raw in [
            &b"-1\n"[..],
            b"+5\n",
            b" 5\n",
            b"5 \n",
            b"5\n\n",
            b"5\n6\n",
            b"MainPID=5\n",
            b"abc\n",
            b"\xff\n",
            b"2147483648\n",
            b"99999999999999999999\n",
        ] {
            assert_eq!(
                parse_main_pid(raw),
                None,
                "{:?}",
                String::from_utf8_lossy(raw)
            );
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
                operation: Operation::Start,
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
        let killed = classify(Operation::Stop, &unit(), ExitStatus::from_raw(9), b"");
        assert!(matches!(
            killed,
            SystemctlError::Failed { status: None, .. }
        ));
    }

    // Formatos reales de `CPUQuotaPerSecUSec` (systemd 255).
    #[test]
    fn duraciones_de_systemd() {
        assert_eq!(parse_timespan_us("1.500000s"), Some(1_500_000));
        assert_eq!(parse_timespan_us("500ms"), Some(500_000));
        assert_eq!(parse_timespan_us("2s"), Some(2_000_000));
        assert_eq!(parse_timespan_us("1min 30s"), Some(90_000_000));
        assert_eq!(parse_timespan_us("10ms 500us"), Some(10_500));
        assert_eq!(parse_timespan_us("infinity"), None);
        assert_eq!(parse_timespan_us("[not set]"), None);
        assert_eq!(parse_timespan_us("1h"), None);
    }

    #[test]
    fn set_property_va_tras_la_unidad() {
        let command = Systemctl::default().command(
            Operation::SetProperty,
            &[],
            &unit(),
            &["MemoryMax=infinity", "CPUQuota="],
        );
        let args: Vec<_> = command.as_std().get_args().collect();
        assert_eq!(
            args,
            [
                "--no-ask-password",
                "--no-pager",
                "set-property",
                "--",
                "api.service",
                "MemoryMax=infinity",
                "CPUQuota="
            ]
        );
    }

    #[test]
    fn argumentos_y_entorno() {
        let command = Systemctl::default().command(Operation::Restart, &[], &unit(), &[]);
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

        let command = Systemctl::default().command(
            Operation::ShowStatus,
            &["-p", "CPUUsageNSec", "--value"],
            &unit(),
            &[],
        );
        let args: Vec<_> = command.as_std().get_args().collect();
        assert_eq!(
            args,
            [
                "--no-ask-password",
                "--no-pager",
                "show",
                "-p",
                "CPUUsageNSec",
                "--value",
                "--",
                "api.service"
            ]
        );
    }
}
