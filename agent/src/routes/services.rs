//! `/api/v1/services/{name}/…`: estado y ciclo de vida de los servicios registrados.
//!
//! `{name}` es el identificador corto del registro (`ServiceSlug`), nunca un nombre de unidad:
//! un servicio no registrado responde `SERVICE_NOT_FOUND` sin llegar a systemd.

use axum::extract::State;
use shared::{ServiceActionReport, ServiceStatusReport};

use crate::{
    error::{AppError, AppResult},
    http::{Json, Path},
    services::ServiceSlug,
    state::AppState,
    systemd::Action,
};

pub async fn status(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<ServiceStatusReport>> {
    Ok(Json(state.services.status(&slug(&name)?).await?))
}

pub async fn start(
    state: State<AppState>,
    name: Path<String>,
) -> AppResult<Json<ServiceActionReport>> {
    act(Action::Start, state, name).await
}

pub async fn stop(
    state: State<AppState>,
    name: Path<String>,
) -> AppResult<Json<ServiceActionReport>> {
    act(Action::Stop, state, name).await
}

pub async fn restart(
    state: State<AppState>,
    name: Path<String>,
) -> AppResult<Json<ServiceActionReport>> {
    act(Action::Restart, state, name).await
}

async fn act(
    action: Action,
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<ServiceActionReport>> {
    Ok(Json(state.services.run(action, &slug(&name)?).await?))
}

/// El nombre no se repite en el mensaje: viene del cliente y puede ser arbitrariamente largo.
fn slug(name: &str) -> AppResult<ServiceSlug> {
    ServiceSlug::new(name).map_err(|error| {
        AppError::BadRequest(format!("nombre de servicio inválido: {error}").into())
    })
}
