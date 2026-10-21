//! Error único de la capa HTTP. Todo lo que sale del agente hacia un cliente pasa por aquí.
//!
//! - El cuerpo siempre es un [`ApiErrorBody`] con un [`ErrorCode`] estable del crate `shared`.
//! - Los errores internos se registran con su cadena completa de causas, pero el cliente solo
//!   recibe un mensaje genérico: nunca rutas, salidas de comandos ni detalles del sistema.

use std::{borrow::Cow, error::Error as StdError, fmt};

use axum::{
    Json,
    extract::rejection::JsonRejection,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use shared::{ApiErrorBody, ErrorCode};

pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;
pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("solicitud inválida: {0}")]
    BadRequest(Cow<'static, str>),

    #[error("cuerpo JSON rechazado")]
    InvalidJson(#[from] JsonRejection),

    #[error("no autenticado")]
    Unauthorized,

    #[error("acceso denegado")]
    Forbidden,

    #[error("recurso no encontrado")]
    NotFound,

    #[error("método no permitido")]
    MethodNotAllowed,

    #[error("conflicto: {0}")]
    Conflict(Cow<'static, str>),

    #[error("tiempo de espera agotado")]
    Timeout,

    #[error("agente saturado")]
    Overloaded,

    #[error("servicio no disponible: {0}")]
    Unavailable(Cow<'static, str>),

    /// `context` dice qué se intentaba hacer; `source` es la causa real, solo para el registro.
    #[error("{context}")]
    Internal {
        context: &'static str,
        #[source]
        source: BoxError,
    },
}

impl AppError {
    pub fn internal(context: &'static str, source: impl Into<BoxError>) -> Self {
        Self::Internal {
            context,
            source: source.into(),
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::InvalidJson(rejection) => rejection.status(),
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Timeout => StatusCode::REQUEST_TIMEOUT,
            Self::Overloaded | Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
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
            Self::Unauthorized => (ErrorCode::Unauthorized, "autenticación requerida".into()),
            Self::Forbidden => (ErrorCode::Forbidden, "acceso denegado".into()),
            Self::NotFound => (ErrorCode::NotFound, "recurso no encontrado".into()),
            Self::MethodNotAllowed => (ErrorCode::MethodNotAllowed, "método no permitido".into()),
            Self::Conflict(message) => (ErrorCode::Conflict, message),
            Self::Timeout => (ErrorCode::RequestTimeout, "tiempo de espera agotado".into()),
            Self::Overloaded => (
                ErrorCode::Overloaded,
                "agente saturado; reintente en breve".into(),
            ),
            Self::Unavailable(message) => (ErrorCode::Unavailable, message),
            Self::Internal { .. } => (ErrorCode::Internal, "error interno del agente".into()),
        };
        ApiErrorBody { code, message }
    }
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

/// Convierte cualquier error en [`AppError::Internal`] indicando qué se intentaba hacer.
pub trait ResultExt<T> {
    fn or_internal(self, context: &'static str) -> AppResult<T>;
}

impl<T, E: Into<BoxError>> ResultExt<T> for Result<T, E> {
    fn or_internal(self, context: &'static str) -> AppResult<T> {
        self.map_err(|error| AppError::internal(context, error))
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
        let error = Err::<(), _>(io::Error::other("/etc/shadow: permiso denegado"))
            .or_internal("leyendo la unidad")
            .unwrap_err();
        let (status, body) = body(error).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.code, ErrorCode::Internal);
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
        let error = AppError::internal("arrancando", io::Error::other("disco lleno"));
        assert_eq!(DisplayChain(&error).to_string(), "arrancando: disco lleno");
    }
}
