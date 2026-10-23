//! `/api/v1/services/{name}/…`: estado y ciclo de vida de los servicios registrados.
//!
//! `{name}` es el identificador corto del registro (`ServiceSlug`), nunca un nombre de unidad:
//! un servicio no registrado responde `SERVICE_NOT_FOUND` sin llegar a systemd.

use axum::{extract::State, http::StatusCode};
use shared::{
    DeployKey, DeployRun, GitHubAccess, GlobalEnvironment, RegisterService, ResourceLimits,
    ServiceActionReport, ServiceDeployment, ServiceEnvironment, ServiceLimits, ServiceStatusReport,
    ServiceSummary, SetGitHubAccess, Site, SiteConfig, UpdateDeployment, UpdateEnvironment,
};

use crate::{
    error::{AppError, AppResult},
    http::{Json, Path},
    services::ServiceSlug,
    state::AppState,
    systemd::Action,
};

/// `GET /api/v1/services`: los servicios registrados. Nunca unidades de systemd que no lo estén.
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<ServiceSummary>>> {
    Ok(Json(state.services.list().await?))
}

/// `POST /api/v1/services`: registra un servicio (solo datos; no toca systemd).
pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterService>,
) -> AppResult<(StatusCode, Json<ServiceSummary>)> {
    Ok((
        StatusCode::CREATED,
        Json(state.services.register(request).await?),
    ))
}

/// `DELETE /api/v1/services/{name}`: lo quita del registro y borra su archivo de variables.
pub async fn unregister(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<StatusCode> {
    let slug = slug(&name)?;
    state.services.unregister(&slug).await?;
    // Su sitio deja de servirse (los datos del sitio se borraron con el servicio).
    state.sites.forget(&slug).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/env`: variables comunes a todos los servicios del servidor.
pub async fn global_environment(
    State(state): State<AppState>,
) -> AppResult<Json<GlobalEnvironment>> {
    Ok(Json(state.services.global_environment().await?))
}

/// `PUT /api/v1/env`: sustituye las variables globales.
pub async fn set_global_environment(
    State(state): State<AppState>,
    Json(update): Json<UpdateEnvironment>,
) -> AppResult<Json<GlobalEnvironment>> {
    Ok(Json(
        state
            .services
            .set_global_environment(update.variables)
            .await?,
    ))
}

/// `GET /api/v1/services/{name}/limits`: límites de recursos (sin límites si nunca se pusieron).
pub async fn limits(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<ServiceLimits>> {
    Ok(Json(state.services.limits(&slug(&name)?).await?))
}

/// `PUT /api/v1/services/{name}/limits`: los aplica al momento (sin reiniciar) y los guarda.
pub async fn set_limits(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(limits): Json<ResourceLimits>,
) -> AppResult<Json<ServiceLimits>> {
    Ok(Json(
        state.services.set_limits(&slug(&name)?, limits).await?,
    ))
}

/// `GET /api/v1/services/{name}/deployment`.
pub async fn deployment(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<ServiceDeployment>> {
    Ok(Json(state.services.deployment(&slug(&name)?).await?))
}

/// `PUT /api/v1/services/{name}/deployment`: sustituye la definición (solo se guarda).
pub async fn set_deployment(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(update): Json<UpdateDeployment>,
) -> AppResult<Json<ServiceDeployment>> {
    Ok(Json(
        state.services.set_deployment(&slug(&name)?, update).await?,
    ))
}

/// `GET /api/v1/services/{name}/deploy-key`: parte pública de la llave, o `null` si no hay.
pub async fn deploy_key(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<Option<DeployKey>>> {
    Ok(Json(state.services.deploy_key(&slug(&name)?).await?))
}

/// `POST /api/v1/services/{name}/deploy-key`: la genera si no existe (idempotente).
pub async fn create_deploy_key(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<DeployKey>> {
    Ok(Json(state.services.create_deploy_key(&slug(&name)?).await?))
}

/// `GET /api/v1/services/{name}/site`: sitio de nginx del servicio, o `null`.
pub async fn site(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<Option<Site>>> {
    Ok(Json(state.sites.get(&slug(&name)?).await?))
}

/// `PUT /api/v1/services/{name}/site`: lo guarda y lo aplica en nginx (si nginx lo rechaza, se
/// mantiene el anterior). Con HTTPS, el certificado se pide en segundo plano.
pub async fn set_site(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(config): Json<SiteConfig>,
) -> AppResult<Json<Site>> {
    Ok(Json(state.sites.set(&slug(&name)?, config).await?))
}

/// `DELETE /api/v1/services/{name}/site`: lo retira de nginx.
pub async fn remove_site(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<StatusCode> {
    state.sites.remove(&slug(&name)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/services/{name}/site/certificate`: vuelve a pedir el certificado.
pub async fn retry_certificate(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<(StatusCode, Json<Site>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(state.sites.retry_certificate(&slug(&name)?).await?),
    ))
}

/// `GET /api/v1/github`: cuenta de GitHub del servidor (nunca el token), o `null`.
pub async fn github_access(State(state): State<AppState>) -> AppResult<Json<Option<GitHubAccess>>> {
    Ok(Json(state.services.github_access().await?))
}

/// `PUT /api/v1/github`: el cliente deja la cuenta y el token con los que el servidor descarga
/// repositorios privados.
pub async fn set_github_access(
    State(state): State<AppState>,
    Json(access): Json<SetGitHubAccess>,
) -> AppResult<Json<GitHubAccess>> {
    Ok(Json(state.services.set_github_access(access).await?))
}

/// `DELETE /api/v1/github`: el servidor olvida la cuenta.
pub async fn remove_github_access(State(state): State<AppState>) -> AppResult<StatusCode> {
    state.services.remove_github_access().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/services/{name}/deploy`: lanza un despliegue (202: sigue en segundo plano).
pub async fn deploy(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<(StatusCode, Json<DeployRun>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(state.services.deploy(&slug(&name)?).await?),
    ))
}

/// `POST /api/v1/services/{name}/rollback`: vuelve a la versión anterior (202: sigue en segundo
/// plano, como un despliegue).
pub async fn rollback(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<(StatusCode, Json<DeployRun>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(state.services.rollback(&slug(&name)?).await?),
    ))
}

/// `GET /api/v1/services/{name}/deploys`: últimos despliegues, del más reciente al más antiguo.
pub async fn deploy_runs(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<Vec<DeployRun>>> {
    Ok(Json(state.services.deploy_runs(&slug(&name)?).await?))
}

/// `GET /api/v1/services/{name}/env`.
pub async fn environment(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> AppResult<Json<ServiceEnvironment>> {
    Ok(Json(state.services.environment(&slug(&name)?).await?))
}

/// `PUT /api/v1/services/{name}/env`: sustituye todas las variables. Se aplican al siguiente
/// arranque o reinicio del servicio.
pub async fn set_environment(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(update): Json<UpdateEnvironment>,
) -> AppResult<Json<ServiceEnvironment>> {
    Ok(Json(
        state
            .services
            .set_environment(&slug(&name)?, update.variables)
            .await?,
    ))
}

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
pub(crate) fn slug(name: &str) -> AppResult<ServiceSlug> {
    ServiceSlug::new(name).map_err(|error| {
        AppError::BadRequest(format!("nombre de servicio inválido: {error}").into())
    })
}
