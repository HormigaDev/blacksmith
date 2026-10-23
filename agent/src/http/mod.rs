//! Pila HTTP del agente: rutas, guardas y middleware transversal.

mod extract;
mod guard;

pub use extract::{Json, Path};

use axum::{
    Router,
    error_handling::HandleErrorLayer,
    extract::{DefaultBodyLimit, Request},
    http::{HeaderName, HeaderValue, header},
    middleware,
    routing::{delete, get, post},
};
use tower::{ServiceBuilder, limit::GlobalConcurrencyLimitLayer};
use tower_http::{
    LatencyUnit,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::{Level, Span};

use crate::{
    error::{self, AppError},
    routes,
    state::AppState,
};

const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

pub fn router(state: AppState) -> Router {
    let config = &state.config;

    // Superficie de administración: solo alcanzable a través del túnel SSH.
    let admin = Router::new()
        .route("/health", get(routes::health::status))
        .route("/host", get(routes::health::host))
        .route(
            "/services",
            get(routes::services::list).post(routes::services::register),
        )
        .route("/services/{name}", delete(routes::services::unregister))
        .route(
            "/env",
            get(routes::services::global_environment).put(routes::services::set_global_environment),
        )
        .route(
            "/github",
            get(routes::services::github_access)
                .put(routes::services::set_github_access)
                .delete(routes::services::remove_github_access),
        )
        .route(
            "/services/{name}/deploy-key",
            get(routes::services::deploy_key).post(routes::services::create_deploy_key),
        )
        .route(
            "/services/{name}/site",
            get(routes::services::site)
                .put(routes::services::set_site)
                .delete(routes::services::remove_site),
        )
        .route(
            "/services/{name}/site/certificate",
            post(routes::services::retry_certificate),
        )
        .route("/services/{name}/deploy", post(routes::services::deploy))
        .route(
            "/services/{name}/rollback",
            post(routes::services::rollback),
        )
        .route(
            "/services/{name}/deploys",
            get(routes::services::deploy_runs),
        )
        .route(
            "/services/{name}/deployment",
            get(routes::services::deployment).put(routes::services::set_deployment),
        )
        .route(
            "/services/{name}/limits",
            get(routes::services::limits).put(routes::services::set_limits),
        )
        .route(
            "/services/{name}/env",
            get(routes::services::environment).put(routes::services::set_environment),
        )
        .route("/services/{name}/status", get(routes::services::status))
        .route("/services/{name}/start", post(routes::services::start))
        .route("/services/{name}/stop", post(routes::services::stop))
        .route("/services/{name}/restart", post(routes::services::restart))
        .route("/services/{name}/logs/ws", get(routes::logs::stream))
        .route_layer(middleware::from_fn(guard::tunnel_only));

    // `Router::layer` aplica la pila a cada ruta por separado: por eso el límite de
    // concurrencia es `Global…`, que comparte un único semáforo entre todas.
    let stack = ServiceBuilder::new()
        .layer(SetRequestIdLayer::new(REQUEST_ID, MakeRequestUuid))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(request_span)
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros),
                )
                // Los 5xx ya los registra `AppError` con su causa.
                .on_failure(()),
        )
        .layer(PropagateRequestIdLayer::new(REQUEST_ID))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(HandleErrorLayer::new(error::from_middleware))
        // Sin cola: si no hay cupo se responde 503 de inmediato en vez de acumular memoria.
        .load_shed()
        .layer(GlobalConcurrencyLimitLayer::new(config.max_concurrency))
        .timeout(config.request_timeout)
        // Lo respetan los extractores (`Json`, `Bytes`, `String`). Un handler que lea el
        // `Body` en crudo debe imponer su propio límite.
        .layer(DefaultBodyLimit::max(config.body_limit));

    Router::new()
        .nest("/api/v1", admin)
        // Fuera de `tunnel_only`: es la única superficie que expone Nginx y se autentica con la
        // firma HMAC de GitHub.
        .route("/api/webhooks/github", post(routes::webhooks::github))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(stack)
        .with_state(state)
}

/// Solo la ruta, nunca la query: podría llevar datos sensibles.
fn request_span(request: &Request) -> Span {
    let request_id = request
        .headers()
        .get(&REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() <= 64)
        .unwrap_or("-");
    tracing::info_span!(
        "http",
        method = %request.method(),
        path = request.uri().path(),
        request_id,
    )
}

async fn not_found() -> AppError {
    AppError::NotFound
}

async fn method_not_allowed() -> AppError {
    AppError::MethodNotAllowed
}
