//! Configuración del agente: se lee una sola vez al arrancar y se valida de forma estricta.
//!
//! Reglas:
//! - Solo se leen variables `BLACKSMITH_*`. Una variable con ese prefijo que no se reconoce es
//!   un error: un error tipográfico nunca debe degradar en silencio a un valor por defecto.
//! - Se informan todos los problemas de una vez, no solo el primero.
//! - El agente solo escucha en loopback. No hay forma de configurarlo de otra manera.
//! - Los secretos nunca viajan en variables de entorno, porque las heredan los procesos hijos
//!   (`systemctl`, `journalctl`…). Se leen de archivos `0600`/`0400`, idealmente credenciales de
//!   systemd (`LoadCredential=`, ver `deploy/blacksmith.service`).

use std::{
    collections::HashMap,
    ffi::OsString,
    fmt,
    fs::File,
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::RangeInclusive,
    os::unix::fs::PermissionsExt,
    str::FromStr,
    time::Duration,
};

use secrecy::SecretString;
use tracing_subscriber::EnvFilter;

pub const ENV_PREFIX: &str = "BLACKSMITH_";

/// Nombres de las variables reconocidas.
pub mod vars {
    pub const ENV: &str = "BLACKSMITH_ENV";
    pub const BIND_ADDR: &str = "BLACKSMITH_BIND_ADDR";
    pub const LOG: &str = "BLACKSMITH_LOG";
    pub const LOG_FORMAT: &str = "BLACKSMITH_LOG_FORMAT";
    pub const WORKER_THREADS: &str = "BLACKSMITH_WORKER_THREADS";
    pub const MAX_BLOCKING_THREADS: &str = "BLACKSMITH_MAX_BLOCKING_THREADS";
    pub const REQUEST_TIMEOUT_SECS: &str = "BLACKSMITH_REQUEST_TIMEOUT_SECS";
    pub const SHUTDOWN_TIMEOUT_SECS: &str = "BLACKSMITH_SHUTDOWN_TIMEOUT_SECS";
    pub const BODY_LIMIT_BYTES: &str = "BLACKSMITH_BODY_LIMIT_BYTES";
    pub const MAX_CONCURRENCY: &str = "BLACKSMITH_MAX_CONCURRENCY";
    pub const GITHUB_WEBHOOK_SECRET_FILE: &str = "BLACKSMITH_GITHUB_WEBHOOK_SECRET_FILE";

    pub(super) const ALL: &[&str] = &[
        ENV,
        BIND_ADDR,
        LOG,
        LOG_FORMAT,
        WORKER_THREADS,
        MAX_BLOCKING_THREADS,
        REQUEST_TIMEOUT_SECS,
        SHUTDOWN_TIMEOUT_SECS,
        BODY_LIMIT_BYTES,
        MAX_CONCURRENCY,
        GITHUB_WEBHOOK_SECRET_FILE,
    ];
}

pub const DEFAULT_BIND_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4040);

/// GitHub nunca envía cuerpos de más de 25 MB; no hay motivo para aceptar más.
const MAX_BODY_LIMIT: usize = 25 * 1024 * 1024;
const SECRET_LEN: RangeInclusive<usize> = 32..=256;
const MAX_SECRET_FILE_BYTES: u64 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

impl Environment {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "development" => Ok(Self::Development),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "«{other}» no es válido; use `development` o `production`"
            )),
        }
    }

    fn default_log_filter(self) -> &'static str {
        match self {
            Self::Development => "info,agent=debug,tower_http=debug",
            Self::Production => "info",
        }
    }

    fn default_log_format(self) -> LogFormat {
        match self {
            Self::Development => LogFormat::Pretty,
            Self::Production => LogFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Compact,
    Json,
}

impl LogFormat {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "pretty" => Ok(Self::Pretty),
            "compact" => Ok(Self::Compact),
            "json" => Ok(Self::Json),
            other => Err(format!(
                "«{other}» no es válido; use `pretty`, `compact` o `json`"
            )),
        }
    }
}

/// `Debug` es seguro: `SecretString` se imprime como `[REDACTED]`.
#[derive(Debug)]
pub struct Config {
    pub environment: Environment,
    pub bind_addr: SocketAddr,
    /// Directiva de `EnvFilter`, ya validada.
    pub log_filter: String,
    pub log_format: LogFormat,
    pub worker_threads: usize,
    pub max_blocking_threads: usize,
    pub request_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub body_limit: usize,
    pub max_concurrency: usize,
    /// Si es `None`, los webhooks de GitHub quedan deshabilitados.
    pub github_webhook_secret: Option<SecretString>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_vars(std::env::vars_os())
    }

    pub fn from_vars<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        let mut reader = Reader::new(vars);

        // Si falta, el valor provisional no importa: el problema ya quedó registrado.
        let environment = reader
            .required(vars::ENV, Environment::parse)
            .unwrap_or(Environment::Production);
        if cfg!(not(debug_assertions)) && environment == Environment::Development {
            reader.invalid(
                vars::ENV,
                "un binario release solo admite `production`".to_owned(),
            );
        }

        let config = Self {
            environment,
            bind_addr: reader.optional(vars::BIND_ADDR, DEFAULT_BIND_ADDR, parse_bind_addr),
            log_filter: reader.optional(
                vars::LOG,
                environment.default_log_filter().to_owned(),
                parse_log_filter,
            ),
            log_format: reader.optional(
                vars::LOG_FORMAT,
                environment.default_log_format(),
                LogFormat::parse,
            ),
            worker_threads: reader.number(vars::WORKER_THREADS, 2, 1..=64),
            max_blocking_threads: reader.number(vars::MAX_BLOCKING_THREADS, 16, 1..=256),
            request_timeout: Duration::from_secs(reader.number(
                vars::REQUEST_TIMEOUT_SECS,
                30,
                1..=300,
            )),
            shutdown_timeout: Duration::from_secs(reader.number(
                vars::SHUTDOWN_TIMEOUT_SECS,
                10,
                1..=120,
            )),
            body_limit: reader.number(vars::BODY_LIMIT_BYTES, 1024 * 1024, 1024..=MAX_BODY_LIMIT),
            max_concurrency: reader.number(vars::MAX_CONCURRENCY, 128, 1..=4096),
            github_webhook_secret: reader.optional(
                vars::GITHUB_WEBHOOK_SECRET_FILE,
                None,
                |path| read_secret_file(path).map(Some),
            ),
        };

        reader.finish(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigIssue {
    #[error("{name} es obligatoria")]
    Missing { name: &'static str },
    #[error("{name}: {reason}")]
    Invalid { name: &'static str, reason: String },
    #[error("{name} no es una variable reconocida (¿error tipográfico?)")]
    Unknown { name: String },
    #[error("{name} contiene bytes que no son UTF-8")]
    NotUnicode { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub issues: Vec<ConfigIssue>,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "configuración inválida ({} problema(s)):",
            self.issues.len()
        )?;
        for issue in &self.issues {
            write!(f, "\n  - {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ConfigError {}

struct Reader {
    values: HashMap<&'static str, String>,
    issues: Vec<ConfigIssue>,
}

impl Reader {
    fn new<I, K, V>(vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        let mut reader = Self {
            values: HashMap::new(),
            issues: Vec::new(),
        };
        for (key, value) in vars {
            let (key, value): (OsString, OsString) = (key.into(), value.into());
            let Some(key_str) = key.to_str() else {
                if key.as_encoded_bytes().starts_with(ENV_PREFIX.as_bytes()) {
                    reader.issues.push(ConfigIssue::NotUnicode {
                        name: key.to_string_lossy().into_owned(),
                    });
                }
                continue;
            };
            if !key_str.starts_with(ENV_PREFIX) {
                continue;
            }
            let Some(name) = vars::ALL.iter().copied().find(|known| *known == key_str) else {
                reader.issues.push(ConfigIssue::Unknown {
                    name: key_str.to_owned(),
                });
                continue;
            };
            match value.into_string() {
                Ok(value) => {
                    reader.values.insert(name, value);
                }
                Err(_) => reader.issues.push(ConfigIssue::NotUnicode {
                    name: name.to_owned(),
                }),
            }
        }
        reader
    }

    /// Valor sin espacios alrededor; una variable vacía cuenta como no definida.
    fn take(&mut self, name: &'static str) -> Option<String> {
        let value = self.values.remove(name)?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else if trimmed.len() == value.len() {
            Some(value)
        } else {
            Some(trimmed.to_owned())
        }
    }

    fn invalid(&mut self, name: &'static str, reason: String) {
        self.issues.push(ConfigIssue::Invalid { name, reason });
    }

    fn required<T>(
        &mut self,
        name: &'static str,
        parse: impl FnOnce(&str) -> Result<T, String>,
    ) -> Option<T> {
        let Some(raw) = self.take(name) else {
            self.issues.push(ConfigIssue::Missing { name });
            return None;
        };
        parse(&raw)
            .map_err(|reason| self.invalid(name, reason))
            .ok()
    }

    fn optional<T>(
        &mut self,
        name: &'static str,
        default: T,
        parse: impl FnOnce(&str) -> Result<T, String>,
    ) -> T {
        let Some(raw) = self.take(name) else {
            return default;
        };
        match parse(&raw) {
            Ok(value) => value,
            Err(reason) => {
                self.invalid(name, reason);
                default
            }
        }
    }

    fn number<T>(&mut self, name: &'static str, default: T, range: RangeInclusive<T>) -> T
    where
        T: FromStr + PartialOrd + fmt::Display,
    {
        self.optional(name, default, |raw| match raw.parse::<T>() {
            Ok(value) if range.contains(&value) => Ok(value),
            Ok(_) => Err(format!(
                "debe estar entre {} y {}",
                range.start(),
                range.end()
            )),
            Err(_) => Err(format!("«{raw}» no es un entero sin signo")),
        })
    }

    fn finish(self, config: Config) -> Result<Config, ConfigError> {
        if self.issues.is_empty() {
            Ok(config)
        } else {
            Err(ConfigError {
                issues: self.issues,
            })
        }
    }
}

fn parse_bind_addr(raw: &str) -> Result<SocketAddr, String> {
    let addr: SocketAddr = raw
        .parse()
        .map_err(|_| format!("«{raw}» no es una dirección `ip:puerto` válida"))?;
    if !addr.ip().is_loopback() {
        return Err(format!(
            "{addr} no es loopback; la administración entra por túnel SSH y los webhooks por Nginx"
        ));
    }
    if addr.port() == 0 {
        return Err("el puerto no puede ser 0".to_owned());
    }
    Ok(addr)
}

fn parse_log_filter(raw: &str) -> Result<String, String> {
    EnvFilter::try_new(raw)
        .map(|_| raw.to_owned())
        .map_err(|error| format!("directiva de registro inválida: {error}"))
}

/// Lee un secreto de un archivo propio del usuario del servicio y sin permisos para nadie más.
/// Se admite un único salto de línea final (el que dejan `echo` y los editores).
fn read_secret_file(path: &str) -> Result<SecretString, String> {
    let fail = |reason: &dyn fmt::Display| format!("{path}: {reason}");

    // Los metadatos se toman del descriptor abierto, no de la ruta, para evitar carreras.
    let file = File::open(path).map_err(|error| fail(&error))?;
    let metadata = file.metadata().map_err(|error| fail(&error))?;
    if !metadata.is_file() {
        return Err(fail(&"no es un archivo regular"));
    }
    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(fail(&format_args!(
            "permisos {mode:03o} demasiado abiertos; use `chmod 600` o LoadCredential="
        )));
    }
    if metadata.len() > MAX_SECRET_FILE_BYTES {
        return Err(fail(&format_args!("supera {MAX_SECRET_FILE_BYTES} bytes")));
    }

    let mut secret = String::new();
    file.take(MAX_SECRET_FILE_BYTES)
        .read_to_string(&mut secret)
        .map_err(|error| fail(&error))?;
    // Se recorta en el mismo búfer para no dejar copias del secreto en memoria.
    if secret.ends_with('\n') {
        secret.pop();
        if secret.ends_with('\r') {
            secret.pop();
        }
    }
    let secret = SecretString::from(secret);

    let value = secrecy::ExposeSecret::expose_secret(&secret);
    if !SECRET_LEN.contains(&value.len()) {
        return Err(fail(&format_args!(
            "el secreto debe tener entre {} y {} bytes",
            SECRET_LEN.start(),
            SECRET_LEN.end()
        )));
    }
    if value.chars().any(|c| c.is_control()) || value.trim().len() != value.len() {
        return Err(fail(
            &"el secreto contiene espacios en los extremos o caracteres de control",
        ));
    }
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Write};

    use secrecy::ExposeSecret;

    use super::*;

    fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        Config::from_vars(vars.iter().copied())
    }

    fn issues(vars: &[(&str, &str)]) -> Vec<ConfigIssue> {
        load(vars).unwrap_err().issues
    }

    fn secret_file(contents: &str, mode: u32) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        fs::set_permissions(file.path(), fs::Permissions::from_mode(mode)).unwrap();
        file
    }

    #[test]
    fn valores_por_defecto_de_produccion() {
        let config = load(&[(vars::ENV, "production"), ("PATH", "/usr/bin")]).unwrap();
        assert_eq!(config.bind_addr, DEFAULT_BIND_ADDR);
        assert_eq!(config.log_format, LogFormat::Json);
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert!(config.github_webhook_secret.is_none());
    }

    #[test]
    fn entorno_obligatorio() {
        assert_eq!(issues(&[]), vec![ConfigIssue::Missing { name: vars::ENV }]);
        assert_eq!(
            issues(&[(vars::ENV, "  ")]),
            vec![ConfigIssue::Missing { name: vars::ENV }]
        );
        assert!(matches!(
            issues(&[(vars::ENV, "prod")]).as_slice(),
            [ConfigIssue::Invalid {
                name: vars::ENV,
                ..
            }]
        ));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn desarrollo_solo_en_debug() {
        let config = load(&[(vars::ENV, "development")]).unwrap();
        assert_eq!(config.log_format, LogFormat::Pretty);
    }

    #[test]
    fn informa_todos_los_problemas() {
        let found = issues(&[
            ("BLACKSMITH_BIND_ADRR", "127.0.0.1:4040"),
            (vars::REQUEST_TIMEOUT_SECS, "0"),
            (vars::BODY_LIMIT_BYTES, "-5"),
            (vars::LOG_FORMAT, "xml"),
        ]);
        assert_eq!(found.len(), 5, "{found:?}");
        assert!(found.contains(&ConfigIssue::Unknown {
            name: "BLACKSMITH_BIND_ADRR".to_owned()
        }));
        assert!(found.contains(&ConfigIssue::Missing { name: vars::ENV }));
    }

    #[test]
    fn solo_escucha_en_loopback() {
        for addr in [
            "0.0.0.0:4040",
            "192.168.1.10:4040",
            "[::]:4040",
            "[::ffff:127.0.0.1]:4040",
            "127.0.0.1:0",
        ] {
            assert!(
                matches!(
                    issues(&[(vars::ENV, "production"), (vars::BIND_ADDR, addr)]).as_slice(),
                    [ConfigIssue::Invalid {
                        name: vars::BIND_ADDR,
                        ..
                    }]
                ),
                "{addr} debería rechazarse"
            );
        }
        let config = load(&[(vars::ENV, "production"), (vars::BIND_ADDR, "[::1]:4141")]).unwrap();
        assert_eq!(config.bind_addr.port(), 4141);
    }

    #[test]
    fn rechaza_filtro_de_registro_invalido() {
        assert!(matches!(
            issues(&[
                (vars::ENV, "production"),
                (vars::LOG, "agent=nivel_inexistente")
            ])
            .as_slice(),
            [ConfigIssue::Invalid {
                name: vars::LOG,
                ..
            }]
        ));
    }

    #[test]
    fn secreto_desde_archivo_privado() {
        let secret = "s".repeat(40);
        let file = secret_file(&format!("{secret}\n"), 0o600);
        let config = load(&[
            (vars::ENV, "production"),
            (
                vars::GITHUB_WEBHOOK_SECRET_FILE,
                file.path().to_str().unwrap(),
            ),
        ])
        .unwrap();
        assert_eq!(
            config.github_webhook_secret.unwrap().expose_secret(),
            secret
        );
    }

    #[test]
    fn rechaza_secretos_inseguros() {
        let cases = [
            secret_file(&"s".repeat(40), 0o644),
            secret_file(&"s".repeat(40), 0o640),
            secret_file("corto", 0o600),
            secret_file(&format!(" {}", "s".repeat(40)), 0o600),
        ];
        for file in &cases {
            let found = issues(&[
                (vars::ENV, "production"),
                (
                    vars::GITHUB_WEBHOOK_SECRET_FILE,
                    file.path().to_str().unwrap(),
                ),
            ]);
            assert!(
                matches!(
                    found.as_slice(),
                    [ConfigIssue::Invalid {
                        name: vars::GITHUB_WEBHOOK_SECRET_FILE,
                        ..
                    }]
                ),
                "{found:?}"
            );
        }
    }

    #[test]
    fn el_error_nunca_muestra_el_secreto() {
        let secret = "x".repeat(40);
        let file = secret_file(&secret, 0o644);
        let message = load(&[
            (vars::ENV, "production"),
            (
                vars::GITHUB_WEBHOOK_SECRET_FILE,
                file.path().to_str().unwrap(),
            ),
        ])
        .unwrap_err()
        .to_string();
        assert!(!message.contains(&secret));
    }
}
