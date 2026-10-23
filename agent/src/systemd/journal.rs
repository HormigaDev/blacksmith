//! Lectura continua del journal de una o varias unidades con `journalctl`.
//!
//! `journalctl --unit=<unidad> --lines=100 --follow --output=json` en un subproceso de tokio;
//! su stdout se lee línea a línea y cada una se convierte en un [`LogEntry`].
//!
//! - Mismas protecciones que `systemctl`: sin shell, ruta absoluta, entorno vacío y la unidad
//!   como un único argumento (`--unit=…`) ya validado por [`UnitName`] (sin comodines).
//! - Memoria acotada: una línea de más de [`MAX_LINE_BYTES`] se descarta sin almacenarla entera
//!   (un mensaje binario llega como array de números, unas 4 veces su tamaño) y la salida de
//!   error se limita a [`STDERR_LIMIT`].
//! - Fin y fallos: cuando stdout se cierra se recoge el código de salida y el error de
//!   `journalctl`; un fallo de lectura también termina el flujo. Ambos se devuelven como
//!   [`JournalError`] y después [`LogStream::next`] solo devuelve `None`.
//! - Limpieza: al soltar el [`LogStream`] el proceso se mata y se espera en una tarea aparte,
//!   así que no queda como zombi. `kill_on_drop` por sí solo no basta: tokio recoge los procesos
//!   huérfanos solo cuando ocurre otro evento de procesos, que en un agente en reposo puede no
//!   llegar nunca.
//!
//! Memoria de cada `journalctl`: ~1 MB propio. Su RSS parece mucho mayor porque mapea los
//! archivos del journal, pero esas páginas son caché de disco limpia y compartida que el kernel
//! recupera antes de limitar la unidad. Aun así, el número de flujos simultáneos se limita
//! (`BLACKSMITH_MAX_LOG_STREAMS`): cuentan para el `MemoryMax=` de la unidad del agente.

use std::{io, path::PathBuf, process::Stdio, time::Duration};

use serde::Deserialize;
use serde_json::Value;
use shared::LogEntry;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, BufReader},
    process::{Child, ChildStdout},
    task::JoinHandle,
};

use super::{Scope, UnitName, systemctl::base_command};

/// Ruta de `journalctl` en Debian/Ubuntu (con `/usr` fusionado) y derivadas de RHEL.
pub const JOURNALCTL_PATH: &str = "/usr/bin/journalctl";

/// Una línea más larga se descarta.
pub const MAX_LINE_BYTES: usize = 256 * 1024;
/// Tras una línea grande, el búfer vuelve a este tamaño para no retener la memoria.
const LINE_BUFFER: usize = 8 * 1024;
/// Leer el final del journal no espera a nada: si tarda más, algo va mal.
const TAIL_TIMEOUT: Duration = Duration::from_secs(5);
const STDERR_LIMIT: u64 = 4 * 1024;

/// Solo los campos que se usan: `journalctl` no serializa el resto.
const OUTPUT_FIELDS: &str = "--output-fields=MESSAGE,PRIORITY,_PID,SYSLOG_IDENTIFIER";

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("no se pudo ejecutar journalctl")]
    Spawn(#[source] io::Error),

    #[error("fallo al leer la salida de journalctl")]
    Read(#[source] io::Error),

    /// `journalctl` terminó (con `--follow` solo debería pasar si falla). `status` es `None` si
    /// terminó por una señal.
    #[error("journalctl terminó con {status:?}: {detail}")]
    Exited { status: Option<i32>, detail: String },
}

/// Cliente de `journalctl`. Es barato de clonar y no guarda estado.
#[derive(Debug, Clone)]
pub struct Journalctl {
    program: PathBuf,
    scope: Scope,
}

impl Default for Journalctl {
    fn default() -> Self {
        Self::new(JOURNALCTL_PATH, Scope::System)
    }
}

impl Journalctl {
    /// `program` debe ser una ruta absoluta: no se busca en `PATH`.
    pub fn new(program: impl Into<PathBuf>, scope: Scope) -> Self {
        Self {
            program: program.into(),
            scope,
        }
    }

    fn unit_flag(&self) -> &'static str {
        match self.scope {
            Scope::System => "--unit",
            Scope::User => "--user-unit",
        }
    }

    /// Las últimas líneas de la unidad desde `since` (segundos Unix), sin vacías: el motivo de
    /// un fallo. Vacío si no se pudo leer.
    pub async fn tail(&self, unit: &UnitName, since: i64) -> String {
        const READ: u16 = 40;
        const KEEP: usize = 12;
        const MAX_CHARS: usize = 2000;
        let mut command = tokio::process::Command::from(base_command(&self.program, self.scope));
        command
            .args(["--no-pager", "--quiet", "--output=cat"])
            .arg(format!("--lines={READ}"))
            .arg(format!("--since=@{since}"))
            .arg(format!("{}={unit}", self.unit_flag()))
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let Ok(Ok(output)) = tokio::time::timeout(TAIL_TIMEOUT, command.output()).await else {
            return String::new();
        };
        let text = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        lines
            .get(lines.len().saturating_sub(KEEP)..)
            .unwrap_or_default()
            .join("\n")
            .chars()
            .take(MAX_CHARS)
            .collect()
    }

    /// Las últimas `lines` entradas de las unidades (mezcladas por fecha) y, después, las nuevas
    /// según llegan.
    pub fn follow(&self, units: &[UnitName], lines: u16) -> Result<LogStream, JournalError> {
        self.spawn(units, lines, None)
    }

    /// Como [`Journalctl::follow`], pero solo desde `since` (segundos Unix): p. ej. la salida
    /// del último despliegue, sin la de los anteriores.
    pub fn follow_since(
        &self,
        units: &[UnitName],
        since: u64,
        lines: u16,
    ) -> Result<LogStream, JournalError> {
        self.spawn(units, lines, Some(since))
    }

    fn spawn(
        &self,
        units: &[UnitName],
        lines: u16,
        since: Option<u64>,
    ) -> Result<LogStream, JournalError> {
        let mut command = base_command(&self.program, self.scope);
        let unit_flag = self.unit_flag();
        command
            .args([
                "--no-pager",
                "--quiet",
                "--output=json",
                OUTPUT_FIELDS,
                "--follow",
                // Con `--follow`, journalctl solo mira el arranque actual: la salida de un
                // despliegue anterior a reiniciar el servidor (o los logs previos de un servicio)
                // no aparecería. `--merge` lee todos los journals disponibles.
                "--merge",
            ])
            .arg(format!("--lines={lines}"))
            .args(since.map(|since| format!("--since=@{since}")))
            .args(units.iter().map(|unit| format!("{unit_flag}={unit}")))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut command = tokio::process::Command::from(command);
        command.kill_on_drop(true);
        let mut child = command.spawn().map_err(JournalError::Spawn)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| JournalError::Spawn(io::Error::other("sin stdout")))?;
        // La salida de error se lee aparte: si nadie la vaciara, `journalctl` podría bloquearse.
        let stderr = child.stderr.take().map(|stderr| {
            tokio::spawn(async move {
                let mut captured = Vec::new();
                let mut limited = stderr.take(STDERR_LIMIT);
                let _ = limited.read_to_end(&mut captured).await;
                let _ = tokio::io::copy(&mut limited.into_inner(), &mut tokio::io::sink()).await;
                captured
            })
        });
        tracing::debug!(?units, pid = child.id(), "journalctl iniciado");
        Ok(LogStream {
            child: Some(child),
            stdout: BufReader::with_capacity(LINE_BUFFER, stdout),
            stderr,
            line: Vec::with_capacity(LINE_BUFFER),
            finished: false,
        })
    }
}

/// Flujo de entradas de un `journalctl` en marcha. Soltarlo mata el proceso.
pub struct LogStream {
    /// `None` solo durante el `Drop`.
    child: Option<Child>,
    stdout: BufReader<ChildStdout>,
    stderr: Option<JoinHandle<Vec<u8>>>,
    line: Vec<u8>,
    finished: bool,
}

impl std::fmt::Debug for LogStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogStream")
            .field("pid", &self.pid())
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl LogStream {
    /// PID de `journalctl`, si sigue vivo.
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }
    /// Siguiente entrada. `Some(Err(…))` una sola vez si `journalctl` termina o falla la
    /// lectura; desde entonces, `None`. No es seguro cancelarlo a medias (`select!`): úsese
    /// desde una única tarea.
    pub async fn next(&mut self) -> Option<Result<LogEntry, JournalError>> {
        while !self.finished {
            match read_line(&mut self.stdout, &mut self.line).await {
                Ok(Line::Complete) => {
                    let entry = parse_entry(&self.line);
                    if self.line.capacity() > LINE_BUFFER {
                        self.line = Vec::with_capacity(LINE_BUFFER);
                    }
                    match entry {
                        Some(entry) => return Some(Ok(entry)),
                        None => tracing::warn!("línea de journalctl con formato inesperado"),
                    }
                }
                Ok(Line::TooLong) => {
                    tracing::warn!(
                        limit = MAX_LINE_BYTES,
                        "entrada del journal demasiado larga; omitida"
                    );
                }
                Ok(Line::End) => {
                    self.finished = true;
                    return Some(Err(self.exit_error().await));
                }
                Err(error) => {
                    self.finished = true;
                    if let Some(child) = &mut self.child {
                        let _ = child.start_kill();
                    }
                    return Some(Err(JournalError::Read(error)));
                }
            }
        }
        None
    }

    async fn exit_error(&mut self) -> JournalError {
        let status = match &mut self.child {
            Some(child) => match child.wait().await {
                Ok(status) => status.code(),
                Err(error) => return JournalError::Read(error),
            },
            None => None,
        };
        let stderr = match self.stderr.take() {
            Some(task) => task.await.unwrap_or_default(),
            None => Vec::new(),
        };
        let text = String::from_utf8_lossy(&stderr);
        JournalError::Exited {
            status,
            detail: text
                .lines()
                .next()
                .unwrap_or("sin mensaje de error")
                .to_owned(),
        }
    }
}

impl Drop for LogStream {
    fn drop(&mut self) {
        if let Some(task) = self.stderr.take() {
            task.abort();
        }
        let Some(mut child) = self.child.take() else {
            return;
        };
        // Matar y esperar: sin el `wait`, el proceso quedaría como zombi hasta que tokio
        // atendiera otro evento de procesos.
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn(async move {
                    let _ = child.kill().await;
                });
            }
            // Fuera de un runtime solo queda `kill_on_drop`.
            Err(_) => {
                let _ = child.start_kill();
            }
        }
    }
}

enum Line {
    /// `buf` contiene una línea completa, sin el salto final.
    Complete,
    /// La línea superaba el límite y se descartó.
    TooLong,
    /// Fin del flujo.
    End,
}

/// Lee hasta el siguiente `\n` sin acumular más de [`MAX_LINE_BYTES`]. Si la línea es más
/// larga, consume y descarta el resto sin guardarlo.
async fn read_line<R: AsyncBufRead + Unpin>(reader: &mut R, buf: &mut Vec<u8>) -> io::Result<Line> {
    buf.clear();
    let mut discarding = false;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            // Una última línea sin salto final también cuenta.
            return Ok(if buf.is_empty() || discarding {
                Line::End
            } else {
                Line::Complete
            });
        }
        let (chunk, found) = match available.iter().position(|byte| *byte == b'\n') {
            Some(newline) => (newline, true),
            None => (available.len(), false),
        };
        if !discarding {
            if buf.len() + chunk > MAX_LINE_BYTES {
                discarding = true;
                buf.clear();
            } else {
                buf.extend_from_slice(available.get(..chunk).unwrap_or_default());
            }
        }
        reader.consume(if found { chunk + 1 } else { chunk });
        if found {
            return Ok(if discarding {
                Line::TooLong
            } else {
                Line::Complete
            });
        }
    }
}

/// Campos de una línea de `journalctl --output=json`. Todos los valores llegan como texto,
/// salvo los que no son UTF-8 o tienen caracteres de control (array de bytes) y los campos
/// repetidos (array de valores).
#[derive(Deserialize)]
struct RawEntry {
    #[serde(rename = "__REALTIME_TIMESTAMP")]
    timestamp: Option<Value>,
    #[serde(rename = "MESSAGE")]
    message: Option<Value>,
    #[serde(rename = "PRIORITY")]
    priority: Option<Value>,
    #[serde(rename = "_PID")]
    pid: Option<Value>,
    #[serde(rename = "SYSLOG_IDENTIFIER")]
    identifier: Option<Value>,
}

fn parse_entry(line: &[u8]) -> Option<LogEntry> {
    let raw: RawEntry = serde_json::from_slice(line).ok()?;
    Some(LogEntry {
        timestamp_us: text(raw.timestamp.as_ref()?)?.parse().ok()?,
        priority: raw
            .priority
            .as_ref()
            .and_then(text)
            .and_then(|value| value.parse().ok())
            .filter(|priority| *priority <= 7),
        pid: raw
            .pid
            .as_ref()
            .and_then(text)
            .and_then(|value| value.parse().ok()),
        identifier: raw.identifier.as_ref().and_then(text),
        message: raw.message.as_ref().and_then(text).unwrap_or_default(),
    })
}

/// Texto de un campo: la cadena tal cual, un array de bytes como UTF-8 con sustituciones, o el
/// primer valor de un campo repetido.
fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => match items.first()? {
            Value::Number(_) => {
                let bytes: Option<Vec<u8>> = items
                    .iter()
                    .map(|item| item.as_u64().and_then(|byte| u8::try_from(byte).ok()))
                    .collect();
                Some(String::from_utf8_lossy(&bytes?).into_owned())
            }
            first => text(first),
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Líneas reales de journalctl 255.
    const ENTRY: &[u8] = br#"{"SYSLOG_IDENTIFIER":"polkitd","__SEQNUM":"4176288","MESSAGE":"Loading rules from directory /usr/share/polkit-1/rules.d","PRIORITY":"5","__REALTIME_TIMESTAMP":"1792671898654984","_PID":"1058"}"#;

    #[test]
    fn entrada_completa() {
        let entry = parse_entry(ENTRY).unwrap();
        assert_eq!(entry.timestamp_us, 1_792_671_898_654_984);
        assert_eq!(entry.priority, Some(5));
        assert_eq!(entry.pid, Some(1058));
        assert_eq!(entry.identifier.as_deref(), Some("polkitd"));
        assert_eq!(
            entry.message,
            "Loading rules from directory /usr/share/polkit-1/rules.d"
        );
    }

    #[test]
    fn mensajes_en_bytes() {
        // Con códigos de color ANSI (control): array de bytes.
        let entry = parse_entry(
            br#"{"__REALTIME_TIMESTAMP":"1","MESSAGE":[99,111,108,111,114,32,27,91,51,49,109,114,111,106,111,27,91,48,109]}"#,
        )
        .unwrap();
        assert_eq!(entry.message, "color \u{1b}[31mrojo\u{1b}[0m");
        // Latin-1: no es UTF-8.
        let entry = parse_entry(
            br#"{"__REALTIME_TIMESTAMP":"1","MESSAGE":[108,97,116,105,110,49,32,233]}"#,
        )
        .unwrap();
        assert_eq!(entry.message, "latin1 \u{fffd}");
        // Campo repetido: el primero.
        let entry =
            parse_entry(br#"{"__REALTIME_TIMESTAMP":"1","MESSAGE":["uno","dos"]}"#).unwrap();
        assert_eq!(entry.message, "uno");
        // Sin MESSAGE.
        assert_eq!(
            parse_entry(br#"{"__REALTIME_TIMESTAMP":"1"}"#)
                .unwrap()
                .message,
            ""
        );
    }

    #[test]
    fn entradas_invalidas() {
        for line in [
            &b"no es json"[..],
            br#"{"MESSAGE":"sin fecha"}"#,
            br#"{"__REALTIME_TIMESTAMP":"ayer","MESSAGE":"x"}"#,
        ] {
            assert!(
                parse_entry(line).is_none(),
                "{}",
                String::from_utf8_lossy(line)
            );
        }
        // Un mensaje mal formado no descarta la entrada: queda vacío.
        let entry = parse_entry(br#"{"__REALTIME_TIMESTAMP":"1","MESSAGE":[300]}"#).unwrap();
        assert_eq!(entry.message, "");
        // Prioridad fuera de rango: se ignora, la entrada no.
        let entry = parse_entry(br#"{"__REALTIME_TIMESTAMP":"1","PRIORITY":"9"}"#).unwrap();
        assert_eq!(entry.priority, None);
    }

    #[tokio::test]
    async fn lineas_acotadas() {
        let long = vec![b'x'; MAX_LINE_BYTES + 10];
        let mut input = Vec::new();
        input.extend_from_slice(b"uno\n");
        input.extend_from_slice(&long);
        input.extend_from_slice(b"\ndos\ntres");
        // Búfer pequeño: la línea larga llega en muchos trozos.
        let mut reader = BufReader::with_capacity(64, input.as_slice());
        let mut buf = Vec::new();
        let mut lines = Vec::new();
        loop {
            match read_line(&mut reader, &mut buf).await.unwrap() {
                Line::Complete => lines.push(String::from_utf8(buf.clone()).unwrap()),
                Line::TooLong => lines.push("<larga>".to_owned()),
                Line::End => break,
            }
            assert!(buf.len() <= MAX_LINE_BYTES);
        }
        assert_eq!(lines, ["uno", "<larga>", "dos", "tres"]);
    }
}
