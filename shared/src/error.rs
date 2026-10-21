use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// Código estable de error. El cliente decide qué hacer a partir de `code`, nunca de `message`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    BadRequest,
    InvalidJson,
    UnsupportedMediaType,
    PayloadTooLarge,
    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    Conflict,
    RequestTimeout,
    Overloaded,
    Unavailable,
    Internal,
    /// Código emitido por un agente más reciente que este cliente. El agente nunca lo envía.
    #[serde(other)]
    Unknown,
}

/// Cuerpo de toda respuesta de error del agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: ErrorCode,
    /// Texto para registro o diagnóstico; no está pensado para mostrarse tal cual.
    pub message: Cow<'static, str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializa_codigos_estables() {
        let body = ApiErrorBody {
            code: ErrorCode::NotFound,
            message: "x".into(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"code":"NOT_FOUND","message":"x"}"#
        );
    }

    #[test]
    fn tolera_codigos_desconocidos() {
        let body: ApiErrorBody =
            serde_json::from_str(r#"{"code":"FUTURE_CODE","message":"x"}"#).unwrap();
        assert_eq!(body.code, ErrorCode::Unknown);
    }
}
