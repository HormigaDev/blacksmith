// Los helpers no son `#[test]`, así que `allow-unwrap-in-tests` no los cubre.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use agent::{config::Config, db::Database, http, state::AppState};
use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use shared::{AgentStatus, ApiErrorBody, ErrorCode};
use tower::ServiceExt;

fn app() -> Router {
    let config = Config::from_vars([("BLACKSMITH_ENV", "production")]).unwrap();
    http::router(AppState::new(
        Arc::new(config),
        Database::open_in_memory().unwrap(),
    ))
}

fn request(method: Method, uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::empty()).unwrap()
}

fn get(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    request(Method::GET, uri, headers)
}

async fn send(request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = app().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    (
        parts.status,
        parts.headers,
        body.collect().await.unwrap().to_bytes().to_vec(),
    )
}

async fn error(request: Request<Body>) -> (StatusCode, ErrorCode) {
    let (status, _, body) = send(request).await;
    let body: ApiErrorBody = serde_json::from_slice(&body).unwrap();
    (status, body.code)
}

const TUNNEL: (&str, &str) = ("host", "127.0.0.1:4040");

#[tokio::test]
async fn health_por_el_tunel() {
    let (status, headers, body) = send(get("/api/v1/health", &[TUNNEL])).await;
    assert_eq!(status, StatusCode::OK);
    let health: AgentStatus = serde_json::from_slice(&body).unwrap();
    assert_eq!(health.name, "blacksmith-agent");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(headers.contains_key("x-request-id"));
}

#[tokio::test]
async fn el_puerto_local_del_tunel_es_libre() {
    for host in ["localhost:14040", "[::1]:4040", "127.0.0.1"] {
        let (status, _, _) = send(get("/api/v1/health", &[("host", host)])).await;
        assert_eq!(status, StatusCode::OK, "{host}");
    }
}

#[tokio::test]
async fn propaga_el_request_id_recibido() {
    let (_, headers, _) = send(get("/api/v1/health", &[TUNNEL, ("x-request-id", "abc123")])).await;
    assert_eq!(headers["x-request-id"], "abc123");
}

#[tokio::test]
async fn rechaza_navegadores() {
    for browser in [
        ("origin", "https://evil.example"),
        ("sec-fetch-site", "cross-site"),
    ] {
        assert_eq!(
            error(get("/api/v1/health", &[TUNNEL, browser])).await,
            (StatusCode::FORBIDDEN, ErrorCode::Forbidden),
            "{browser:?}"
        );
    }
}

#[tokio::test]
async fn rechaza_dns_rebinding() {
    for host in ["evil.example:4040", "127.0.0.1.nip.io:4040"] {
        assert_eq!(
            error(get("/api/v1/health", &[("host", host)])).await,
            (StatusCode::FORBIDDEN, ErrorCode::Forbidden),
            "{host}"
        );
    }
    assert_eq!(
        error(get("/api/v1/health", &[])).await,
        (StatusCode::FORBIDDEN, ErrorCode::Forbidden)
    );
}

#[tokio::test]
async fn oculta_la_administracion_al_trafico_del_proxy() {
    for forwarded in [
        ("x-forwarded-for", "203.0.113.7"),
        ("x-real-ip", "203.0.113.7"),
        ("forwarded", "for=203.0.113.7"),
    ] {
        assert_eq!(
            error(get("/api/v1/health", &[TUNNEL, forwarded])).await,
            (StatusCode::NOT_FOUND, ErrorCode::NotFound),
            "{forwarded:?}"
        );
    }
}

#[tokio::test]
async fn rutas_y_metodos_desconocidos_responden_json() {
    assert_eq!(
        error(get("/no-existe", &[TUNNEL])).await,
        (StatusCode::NOT_FOUND, ErrorCode::NotFound)
    );
    assert_eq!(
        error(request(Method::DELETE, "/api/v1/health", &[TUNNEL])).await,
        (StatusCode::METHOD_NOT_ALLOWED, ErrorCode::MethodNotAllowed)
    );
}
