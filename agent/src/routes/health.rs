use shared::{AgentStatus, ServiceStatus};

use crate::http::Json;

pub async fn status() -> Json<AgentStatus> {
    Json(AgentStatus {
        name: "blacksmith-agent".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        status: ServiceStatus::Ready,
    })
}
