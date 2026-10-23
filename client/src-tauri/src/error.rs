//! Errores que cruzan a la webview. Igual que los del agente, cada uno tiene un `code` estable que
//! la interfaz traduce (`errors.tunnel.<CODE>` en client/src/lib/i18n/messages/es.ts) y, si hace
//! falta, `details` con datos para el mensaje. Nunca llevan material secreto.
//!
//! Debe mantenerse sincronizado con `TUNNEL_ERROR_CODES` (client/src/lib/ssh/errors.ts).

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("destino SSH inválido: {0}")]
    InvalidTarget(&'static str),

    #[error("no se encontró ninguna llave privada")]
    KeyNotFound,
    #[error("no se pudo leer el archivo de la llave")]
    KeyFileUnreadable,
    #[error("el archivo es demasiado grande para ser una llave privada")]
    KeyTooLarge,
    #[error("el contenido es una llave pública, no privada")]
    KeyIsPublic,
    #[error("el contenido no es una llave privada válida")]
    KeyInvalid,
    #[error("tipo de llave no soportado")]
    KeyUnsupported,
    #[error("la llave está protegida con passphrase")]
    PassphraseRequired,
    #[error("passphrase incorrecta")]
    PassphraseInvalid,
    #[error("no hay ninguna llave guardada para este servidor")]
    CredentialsMissing,

    #[error("el llavero del sistema no está disponible")]
    KeyringUnavailable,
    #[error("las credenciales guardadas no se pueden descifrar")]
    VaultUnreadable,

    #[error("no se pudo contactar con el servidor")]
    HostUnreachable,
    #[error("el servidor no respondió a tiempo")]
    ConnectionTimeout,
    #[error("la llave del servidor es desconocida")]
    HostKeyUnknown {
        algorithm: String,
        fingerprint: String,
        /// Llave completa (OpenSSH) para fijarla si el usuario confirma. No se envía a la webview.
        public_key: String,
    },
    #[error("la llave del servidor cambió")]
    HostKeyChanged {
        algorithm: String,
        fingerprint: String,
    },
    #[error("no hay ninguna llave de servidor pendiente de confirmar con esa huella")]
    HostKeyNotPending,
    #[error("el servidor rechazó la llave")]
    AuthRejected,
    #[error("el agente no responde")]
    AgentUnreachable,
    #[error("en el puerto del agente responde otro servicio")]
    AgentInvalid,
    #[error("el puerto local {port} está en uso")]
    PortInUse { port: u16 },

    #[error("el servidor no está en la lista")]
    ServerNotFound,
    #[error("el servidor no está conectado")]
    NotConnected,
    /// El agente rechazó la petición antes de abrir el WebSocket (servicio no registrado, cupo
    /// de visores lleno…). Trae el código del agente para traducirlo.
    #[error("falta configurar la GitHub App (Client ID y slug)")]
    GitHubNotConfigured,
    #[error("no hay ninguna cuenta de GitHub conectada")]
    GitHubNotConnected,
    #[error("se rechazó la autorización en GitHub")]
    GitHubAccessDenied,
    #[error("el código de GitHub caducó")]
    GitHubCodeExpired,
    #[error("se canceló la conexión con GitHub")]
    GitHubLoginCancelled,
    #[error("la GitHub App no tiene activado el Device Flow")]
    GitHubDeviceFlowDisabled,
    #[error("no se pudo contactar con GitHub")]
    GitHubUnreachable,
    #[error("se superó el límite de peticiones de GitHub")]
    GitHubRateLimited,
    #[error("la GitHub App no tiene permiso para registrar llaves en ese repositorio")]
    GitHubPermissionMissing,
    #[error("esa llave ya está registrada en otro repositorio")]
    GitHubKeyInUse,
    #[error("GitHub respondió {status}")]
    GitHub { status: u16 },
    #[error("el agente rechazó la petición ({status}): {message}")]
    AgentRejected {
        status: u16,
        code: shared::ErrorCode,
        message: String,
    },

    #[error("{context}")]
    Internal {
        context: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl CommandError {
    /// Respuesta de error del agente, con su código si trae el cuerpo de error habitual.
    pub fn agent_rejected(status: u16, body: &[u8]) -> Self {
        match serde_json::from_slice::<shared::ApiErrorBody>(body) {
            Ok(error) => Self::AgentRejected {
                status,
                code: error.code,
                message: error.message.into_owned(),
            },
            Err(_) => Self::AgentRejected {
                status,
                code: shared::ErrorCode::Unknown,
                message: format!("respuesta {status} sin cuerpo de error"),
            },
        }
    }

    pub fn internal(
        context: &'static str,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self::Internal {
            context,
            source: source.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidTarget(_) => "INVALID_TARGET",
            Self::KeyNotFound => "KEY_NOT_FOUND",
            Self::KeyFileUnreadable => "KEY_FILE_UNREADABLE",
            Self::KeyTooLarge => "KEY_TOO_LARGE",
            Self::KeyIsPublic => "KEY_IS_PUBLIC",
            Self::KeyInvalid => "KEY_INVALID",
            Self::KeyUnsupported => "KEY_UNSUPPORTED",
            Self::PassphraseRequired => "PASSPHRASE_REQUIRED",
            Self::PassphraseInvalid => "PASSPHRASE_INVALID",
            Self::CredentialsMissing => "CREDENTIALS_MISSING",
            Self::KeyringUnavailable => "KEYRING_UNAVAILABLE",
            Self::VaultUnreadable => "VAULT_UNREADABLE",
            Self::HostUnreachable => "HOST_UNREACHABLE",
            Self::ConnectionTimeout => "CONNECTION_TIMEOUT",
            Self::HostKeyUnknown { .. } => "HOST_KEY_UNKNOWN",
            Self::HostKeyChanged { .. } => "HOST_KEY_CHANGED",
            Self::HostKeyNotPending => "HOST_KEY_NOT_PENDING",
            Self::AuthRejected => "AUTH_REJECTED",
            Self::AgentUnreachable => "AGENT_UNREACHABLE",
            Self::AgentInvalid => "AGENT_INVALID",
            Self::PortInUse { .. } => "PORT_IN_USE",
            Self::ServerNotFound => "SERVER_NOT_FOUND",
            Self::NotConnected => "NOT_CONNECTED",
            Self::AgentRejected { .. } => "AGENT_REJECTED",
            Self::GitHubNotConfigured => "GITHUB_NOT_CONFIGURED",
            Self::GitHubNotConnected => "GITHUB_NOT_CONNECTED",
            Self::GitHubAccessDenied => "GITHUB_ACCESS_DENIED",
            Self::GitHubCodeExpired => "GITHUB_CODE_EXPIRED",
            Self::GitHubLoginCancelled => "GITHUB_LOGIN_CANCELLED",
            Self::GitHubDeviceFlowDisabled => "GITHUB_DEVICE_FLOW_DISABLED",
            Self::GitHubUnreachable => "GITHUB_UNREACHABLE",
            Self::GitHubRateLimited => "GITHUB_RATE_LIMITED",
            Self::GitHubPermissionMissing => "GITHUB_PERMISSION_MISSING",
            Self::GitHubKeyInUse => "GITHUB_KEY_IN_USE",
            Self::GitHub { .. } => "GITHUB_ERROR",
            Self::Internal { .. } => "INTERNAL",
        }
    }
}

/// Forma en la que llega a la webview: `{ code, message, details? }`.
#[derive(Serialize)]
struct Wire<'a> {
    code: &'static str,
    /// Solo para el registro; la interfaz muestra la traducción del `code`.
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Details<'a>>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Details<'a> {
    HostKey {
        algorithm: &'a str,
        fingerprint: &'a str,
    },
    Port {
        port: u16,
    },
    Field {
        field: &'a str,
    },
    Http {
        #[serde(rename = "httpStatus")]
        http_status: u16,
    },
    Agent {
        status: u16,
        #[serde(rename = "agentCode")]
        agent_code: shared::ErrorCode,
    },
}

impl Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Self::Internal { context, source } = self {
            // La causa real se queda en el registro de la app; la webview recibe solo el contexto.
            eprintln!("blacksmith: {context}: {source}");
        }
        let details = match self {
            Self::HostKeyUnknown {
                algorithm,
                fingerprint,
                ..
            }
            | Self::HostKeyChanged {
                algorithm,
                fingerprint,
            } => Some(Details::HostKey {
                algorithm,
                fingerprint,
            }),
            Self::PortInUse { port } => Some(Details::Port { port: *port }),
            Self::InvalidTarget(field) => Some(Details::Field { field }),
            Self::GitHub { status } => Some(Details::Http {
                http_status: *status,
            }),
            Self::AgentRejected { status, code, .. } => Some(Details::Agent {
                status: *status,
                agent_code: *code,
            }),
            _ => None,
        };
        Wire {
            code: self.code(),
            message: self.to_string(),
            details,
        }
        .serialize(serializer)
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
