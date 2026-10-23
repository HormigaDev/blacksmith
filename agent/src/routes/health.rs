use axum::extract::State;
use shared::{AGENT_NAME, AgentStatus, HostInfo, ServiceStatus};

use crate::{http::Json, state::AppState};

pub async fn status() -> Json<AgentStatus> {
    Json(AgentStatus {
        name: AGENT_NAME.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        status: ServiceStatus::Ready,
    })
}

/// `GET /api/v1/host`: sistema, hardware y uso actual del servidor.
pub async fn host(State(state): State<AppState>) -> Json<HostInfo> {
    Json(state.host.info().await)
}
