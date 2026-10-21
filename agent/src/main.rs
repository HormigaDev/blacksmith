use axum::{routing::get, Json, Router};
use shared::{AgentStatus, ServiceStatus};

async fn health() -> Json<AgentStatus> {
    Json(AgentStatus {
        name: "blacksmith-agent".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        status: ServiceStatus::Ready,
    })
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/health", get(health));
    let address = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("failed to bind agent server");

    println!("Blacksmith agent listening on http://{address}");
    axum::serve(listener, app)
        .await
        .expect("agent server failed");
}
