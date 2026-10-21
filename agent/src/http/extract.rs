use axum::{
    extract::FromRequest,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::error::AppError;

/// `axum::Json` con los rechazos traducidos a [`AppError`], para que todo error salga con el
/// mismo formato. Úsese siempre este en lugar de `axum::Json`.
#[derive(Debug, Clone, Copy, Default, FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct Json<T>(pub T);

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}
