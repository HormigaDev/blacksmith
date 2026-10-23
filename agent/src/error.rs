//! Error único de la capa HTTP. Todo lo que sale del agente hacia un cliente pasa por aquí.
//!
//! - El cuerpo siempre es un [`ApiErrorBody`] con un [`ErrorCode`] estable del crate `shared`.
//!   El cliente decide por `code`; `message` es diagnóstico.
//! - Los errores 5xx se registran con su cadena completa de causas.
//! - [`AppError::Service`] solo lo producen las rutas de administración, que únicamente se
//!   alcanzan por el túnel SSH (el usuario ya está autenticado): su mensaje es el diagnóstico
//!   real (p. ej. por qué systemd no pudo arrancar el servicio) para depurar desde el cliente.
//! - [`AppError::Internal`] solo muestra su contexto (texto fijo del código), nunca la causa:
//!   es el que usan las rutas públicas (webhooks), donde no debe salir ningún detalle.

use std::{borrow::Cow, error::Error as StdError, fmt};

use axum::{
    Json,
    extract::{
        rejection::{JsonRejection, PathRejection},
        ws::rejection::WebSocketUpgradeRejection,
    },
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use shared::{ApiErrorBody, ErrorCode};

use crate::{
    envfile::EnvFileError,
    github::GitHubAccessError,
    nginx::SiteError,
    services::ServiceError,
    systemd::{JournalError, SystemctlError},
};

pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;
pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("solicitud inválida: {0}")]
    BadRequest(Cow<'static, str>),

    #[error("cuerpo JSON rechazado")]
    InvalidJson(#[from] JsonRejection),

    #[error("parámetro de ruta inválido")]
    InvalidPath(#[from] PathRejection),

    #[error(transparent)]
    Service(#[from] ServiceError),

    #[error(transparent)]
    Site(#[from] SiteError),

    #[error("se requiere un handshake WebSocket")]
    WebSocketRequired(#[from] WebSocketUpgradeRejection),

    #[error("máximo de flujos de logs simultáneos alcanzado ({0})")]
    LogStreamLimit(usize),

    #[error("no se pudo leer el journal")]
    Journal(#[from] JournalError),

    #[error("no autenticado")]
    Unauthorized,

    #[error("acceso denegado")]
    Forbidden,

    #[error("recurso no encontrado")]
    NotFound,

    #[error("método no permitido")]
    MethodNotAllowed,

    #[error("tiempo de espera agotado")]
    Timeout,

    #[error("agente saturado")]
    Overloaded,

    /// `context` dice qué se intentaba hacer; `source` es la causa real, solo para el registro.
    #[error("{context}")]
    Internal {
        context: &'static str,
        #[source]
        source: BoxError,
    },
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::InvalidJson(rejection) => rejection.status(),
            Self::InvalidPath(_) => StatusCode::BAD_REQUEST,
            Self::Service(error) => service_error(error).0,
            Self::Site(error) => site_error(error).0,
            Self::WebSocketRequired(_) => StatusCode::UPGRADE_REQUIRED,
            Self::LogStreamLimit(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Journal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::Timeout => StatusCode::REQUEST_TIMEOUT,
            Self::Overloaded => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn into_body(self) -> ApiErrorBody {
        let (code, message): (ErrorCode, Cow<'static, str>) = match self {
            Self::BadRequest(message) => (ErrorCode::BadRequest, message),
            Self::InvalidJson(rejection) => {
                let code = match rejection.status() {
                    StatusCode::PAYLOAD_TOO_LARGE => ErrorCode::PayloadTooLarge,
                    StatusCode::UNSUPPORTED_MEDIA_TYPE => ErrorCode::UnsupportedMediaType,
                    _ => ErrorCode::InvalidJson,
                };
                (code, rejection.body_text().into())
            }
            Self::InvalidPath(rejection) => (ErrorCode::BadRequest, rejection.body_text().into()),
            Self::Service(error) => {
                let (_, code, message) = service_error(&error);
                (code, message.into())
            }
            Self::Site(error) => {
                let (_, code) = site_error(&error);
                (code, DisplayChain(&error).to_string().into())
            }
            Self::WebSocketRequired(rejection) => (
                ErrorCode::WebSocketRequired,
                format!(
                    "se requiere un handshake WebSocket: {}",
                    rejection.body_text()
                )
                .into(),
            ),
            error @ Self::LogStreamLimit(_) => {
                (ErrorCode::LogStreamLimit, error.to_string().into())
            }
            Self::Journal(error) => (
                ErrorCode::SystemdFailed,
                DisplayChain(&error).to_string().into(),
            ),
            Self::Unauthorized => (ErrorCode::Unauthorized, "autenticación requerida".into()),
            Self::Forbidden => (ErrorCode::Forbidden, "acceso denegado".into()),
            Self::NotFound => (ErrorCode::NotFound, "recurso no encontrado".into()),
            Self::MethodNotAllowed => (ErrorCode::MethodNotAllowed, "método no permitido".into()),
            Self::Timeout => (ErrorCode::RequestTimeout, "tiempo de espera agotado".into()),
            Self::Overloaded => (
                ErrorCode::Overloaded,
                "agente saturado; reintente en breve".into(),
            ),
            Self::Internal { context, .. } => (
                ErrorCode::Internal,
                format!("error interno del agente: {context}").into(),
            ),
        };
        ApiErrorBody { code, message }
    }
}

/// Estado, código y mensaje de diagnóstico de un error de servicio.
fn site_error(error: &SiteError) -> (StatusCode, ErrorCode) {
    match error {
        SiteError::NotRegistered(_) => (StatusCode::NOT_FOUND, ErrorCode::ServiceNotFound),
        SiteError::Invalid(_) => (StatusCode::BAD_REQUEST, ErrorCode::InvalidSite),
        // Una plantilla del administrador mal escrita: nada que corregir desde el cliente.
        SiteError::Template(_) => (StatusCode::UNPROCESSABLE_ENTITY, ErrorCode::InvalidSite),
        SiteError::DomainInUse(_) => (StatusCode::CONFLICT, ErrorCode::DomainInUse),
        SiteError::Rejected(_) => (StatusCode::UNPROCESSABLE_ENTITY, ErrorCode::NginxRejected),
        SiteError::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, ErrorCode::NginxUnavailable),
        SiteError::File(_) | SiteError::Database(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, ErrorCode::Internal)
        }
    }
}

fn service_error(error: &ServiceError) -> (StatusCode, ErrorCode, String) {
    use SystemctlError as E;

    let message = DisplayChain(error).to_string();
    let (status, code, message) = match error {
        ServiceError::NotRegistered(_) => {
            (StatusCode::NOT_FOUND, ErrorCode::ServiceNotFound, message)
        }
        ServiceError::Systemctl(error) => match error {
            E::UnitNotFound { .. } => (
                StatusCode::CONFLICT,
                ErrorCode::UnitNotFound,
                format!("{message}: el servicio está registrado, pero systemd no tiene su unidad"),
            ),
            E::UnitMasked { .. } => (StatusCode::CONFLICT, ErrorCode::UnitMasked, message),
            E::ManualOperationRefused { .. } => (
                StatusCode::CONFLICT,
                ErrorCode::UnitRefusesManualOperation,
                message,
            ),
            E::JobFailed { unit, .. } => (
                StatusCode::BAD_GATEWAY,
                ErrorCode::ServiceJobFailed,
                format!("{message} (detalles: journalctl -u {unit})"),
            ),
            E::PermissionDenied { .. } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::SystemdPermissionDenied,
                format!(
                    "{message}: polkit no autoriza al agente; instale deploy/polkit/10-blacksmith.rules"
                ),
            ),
            E::Timeout { .. } => (
                StatusCode::GATEWAY_TIMEOUT,
                ErrorCode::SystemdTimeout,
                message,
            ),
            E::Io(_) | E::UnexpectedOutput { .. } | E::Failed { .. } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::SystemdFailed,
                message,
            ),
        },
        ServiceError::Invalid(_) => (StatusCode::BAD_REQUEST, ErrorCode::BadRequest, message),
        ServiceError::AlreadyExists(_) => (
            StatusCode::CONFLICT,
            ErrorCode::ServiceAlreadyExists,
            message,
        ),
        ServiceError::Env(EnvFileError::Invalid(_)) => (
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidEnvironment,
            message,
        ),
        ServiceError::RepositoryLocked => {
            (StatusCode::CONFLICT, ErrorCode::RepositoryLocked, message)
        }
        ServiceError::DeploymentIncomplete(_) => (
            StatusCode::CONFLICT,
            ErrorCode::DeploymentIncomplete,
            message,
        ),
        ServiceError::DeployKeyMissing => {
            (StatusCode::CONFLICT, ErrorCode::DeployKeyMissing, message)
        }
        ServiceError::DeployInProgress => {
            (StatusCode::CONFLICT, ErrorCode::DeployInProgress, message)
        }
        ServiceError::NotDeployed => (StatusCode::CONFLICT, ErrorCode::NotDeployed, message),
        ServiceError::NothingToRun => (StatusCode::CONFLICT, ErrorCode::NothingToRun, message),
        ServiceError::NothingToRollBack => {
            (StatusCode::CONFLICT, ErrorCode::NothingToRollBack, message)
        }
        ServiceError::GitHub(GitHubAccessError::InvalidLogin | GitHubAccessError::InvalidToken) => {
            (StatusCode::BAD_REQUEST, ErrorCode::BadRequest, message)
        }
        ServiceError::File(_) | ServiceError::GitHub(_) | ServiceError::DeployKey(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
            message,
        ),
        ServiceError::InvalidDeployment(_) => (
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidDeployment,
            message,
        ),
        ServiceError::Env(EnvFileError::Insecure { .. } | EnvFileError::Malformed { .. }) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::EnvironmentFileInvalid,
            message,
        ),
        ServiceError::InvalidLimits(_) => {
            (StatusCode::BAD_REQUEST, ErrorCode::InvalidLimits, message)
        }
        ServiceError::Env(EnvFileError::Io { .. } | EnvFileError::Task(_))
        | ServiceError::Task(_)
        | ServiceError::Database(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
            message,
        ),
    };
    (status, code, message)
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            tracing::error!(status = status.as_u16(), error = %DisplayChain(&self), "solicitud fallida");
        } else {
            tracing::debug!(status = status.as_u16(), error = %self, "solicitud rechazada");
        }

        let overloaded = matches!(self, Self::Overloaded);
        let mut response = (status, Json(self.into_body())).into_response();
        if overloaded {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        }
        response
    }
}

/// Traduce los errores de la pila de `tower` (timeout, carga) al formato común.
pub async fn from_middleware(error: BoxError) -> AppError {
    if error.is::<tower::timeout::error::Elapsed>() {
        AppError::Timeout
    } else if error.is::<tower::load_shed::error::Overloaded>() {
        AppError::Overloaded
    } else {
        AppError::Internal {
            context: "fallo en la pila de middleware",
            source: error,
        }
    }
}

/// Muestra un error seguido de todas sus causas: `a: b: c`.
pub struct DisplayChain<'a>(pub &'a (dyn StdError + 'static));

impl fmt::Display for DisplayChain<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)?;
        let mut source = self.0.source();
        while let Some(cause) = source {
            write!(f, ": {cause}")?;
            source = cause.source();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use http_body_util::BodyExt;

    use super::*;

    async fn body(error: AppError) -> (StatusCode, ApiErrorBody) {
        let response = error.into_response();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn los_errores_internos_no_filtran_detalles() {
        let error = AppError::Internal {
            context: "leyendo la unidad",
            source: io::Error::other("/etc/shadow: permiso denegado").into(),
        };
        let (status, body) = body(error).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.code, ErrorCode::Internal);
        assert_eq!(body.message, "error interno del agente: leyendo la unidad");
        assert!(!body.message.contains("shadow"));
    }

    #[tokio::test]
    async fn saturacion_indica_reintento() {
        let response = AppError::Overloaded.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[header::RETRY_AFTER], "1");
    }

    #[test]
    fn cadena_de_causas() {
        let error = AppError::Internal {
            context: "arrancando",
            source: io::Error::other("disco lleno").into(),
        };
        assert_eq!(DisplayChain(&error).to_string(), "arrancando: disco lleno");
    }
}
