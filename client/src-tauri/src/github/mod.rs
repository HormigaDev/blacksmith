//! GitHub en la aplicación de escritorio: conectar la cuenta (Device Flow de una GitHub App propia)
//! y listar los repositorios y ramas a los que da acceso. Contrato en TypeScript:
//! client/src/lib/github/.
//!
//! - Configuración: el *Client ID* y el *slug* de la App (públicos), en `github.json` (`0600`).
//! - Token: en la bóveda cifrada (llave maestra en el llavero del sistema), nunca en claro ni hacia
//!   la webview. Si se cambia de App, el token anterior se borra: pertenece a la otra.
//! - Todas las peticiones salen de Rust: la webview no tiene acceso a internet (CSP).

pub mod client;

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::{Mutex, Notify};
use zeroize::Zeroizing;

use self::client::{GitHubClient, GitHubRepository, GitHubUser, Poll};
use crate::{
    agent_http::{self, Method},
    commands::AppState,
    error::{CommandError, CommandResult},
    ssh::host_keys::write_private_file,
};

/// GitHub App del proyecto, compartida por todos los usuarios: el *Client ID* es público y el
/// Device Flow no necesita secretos. Quien prefiera la suya la configura con `github_configure`.
pub const DEFAULT_APP: (&str, &str) = ("Iv23liTl5M3dN9eydoJk", "bbel-blacksmith");

/// Id del token en la bóveda. No puede coincidir con un servidor (`usuario@host:puerto`,
/// `local:<puerto>`), y además va como dato autenticado del cifrado.
const TOKEN_ID: &str = "github:token";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubSettings {
    pub client_id: String,
    pub app_slug: String,
}

#[derive(Serialize, Deserialize)]
struct StoredToken {
    token: Zeroizing<String>,
    /// App con la que se obtuvo.
    client_id: String,
}

struct PendingLogin {
    client_id: String,
    device_code: Zeroizing<String>,
    interval: Duration,
    expires_at: Instant,
    cancel: Arc<Notify>,
}

pub struct GitHubState {
    settings_path: PathBuf,
    login: Mutex<Option<PendingLogin>>,
}

impl GitHubState {
    pub fn new(settings_path: PathBuf) -> Self {
        Self {
            settings_path,
            login: Mutex::new(None),
        }
    }

    /// La App en uso: la propia si se configuró, si no la del proyecto.
    fn settings(&self) -> CommandResult<Option<GitHubSettings>> {
        Ok(Some(self.custom()?.unwrap_or_else(default_settings)))
    }

    /// La App propia, si se configuró una.
    fn custom(&self) -> CommandResult<Option<GitHubSettings>> {
        match std::fs::read(&self.settings_path) {
            Ok(bytes) => Ok(serde_json::from_slice::<GitHubSettings>(&bytes)
                .ok()
                .filter(|settings| settings.client_id != DEFAULT_APP.0)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(CommandError::internal(
                "leyendo la configuración de GitHub",
                error,
            )),
        }
    }
}

fn default_settings() -> GitHubSettings {
    GitHubSettings {
        client_id: DEFAULT_APP.0.to_owned(),
        app_slug: DEFAULT_APP.1.to_owned(),
    }
}

/// Client ID de una GitHub App (`Iv23li…`, antes `Iv1.…`): solo letras, dígitos y puntos.
fn validate_client_id(client_id: &str) -> CommandResult<()> {
    let valid = (8..=64).contains(&client_id.len())
        && client_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.');
    valid
        .then_some(())
        .ok_or(CommandError::InvalidTarget("clientId"))
}

/// Slug de la App (`github.com/apps/<slug>`): minúsculas, dígitos y guiones.
fn validate_app_slug(slug: &str) -> CommandResult<()> {
    let valid = (1..=64).contains(&slug.len())
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !slug.starts_with('-')
        && !slug.ends_with('-');
    valid
        .then_some(())
        .ok_or(CommandError::InvalidTarget("appSlug"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubStatus {
    settings: Option<GitHubSettings>,
    /// `true` si se usa una App propia en lugar de la del proyecto.
    custom: bool,
    /// `None` si no hay cuenta conectada (o el token dejó de valer y se borró).
    user: Option<GitHubUser>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStart {
    user_code: String,
    verification_uri: String,
    expires_in: u64,
}

async fn blocking<T: Send + 'static>(
    task: impl FnOnce() -> CommandResult<T> + Send + 'static,
) -> CommandResult<T> {
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| CommandError::internal("tarea de GitHub", error))?
}

/// Token de la App configurada, o `GitHubNotConnected`.
async fn token(state: &AppState) -> CommandResult<Zeroizing<String>> {
    let settings = state
        .github
        .settings()?
        .ok_or(CommandError::GitHubNotConfigured)?;
    let vault = Arc::clone(&state.vault);
    let stored: Option<StoredToken> = blocking(move || vault.get_value(TOKEN_ID)).await?;
    match stored {
        Some(stored) if stored.client_id == settings.client_id => Ok(stored.token),
        _ => Err(CommandError::GitHubNotConnected),
    }
}

async fn forget_token(state: &AppState) -> CommandResult<bool> {
    let vault = Arc::clone(&state.vault);
    blocking(move || vault.delete(TOKEN_ID)).await
}

#[tauri::command]
pub async fn github_status(state: State<'_, AppState>) -> CommandResult<GitHubStatus> {
    let settings = state.github.settings()?;
    let user = match token(&state).await {
        Ok(token) => match GitHubClient::new()?.user(&token).await {
            Ok(user) => Some(user),
            // Revocado desde GitHub o la App cambió: se olvida.
            Err(CommandError::GitHubNotConnected) => {
                forget_token(&state).await?;
                None
            }
            Err(error) => return Err(error),
        },
        Err(CommandError::GitHubNotConnected | CommandError::GitHubNotConfigured) => None,
        Err(error) => return Err(error),
    };
    let custom = state.github.custom()?.is_some();
    Ok(GitHubStatus {
        settings,
        custom,
        user,
    })
}

/// Guarda la App que se usará. Si cambia, la cuenta conectada se desconecta.
#[tauri::command]
pub async fn github_configure(
    state: State<'_, AppState>,
    client_id: String,
    app_slug: String,
) -> CommandResult<GitHubSettings> {
    let settings = GitHubSettings {
        client_id: client_id.trim().to_owned(),
        app_slug: app_slug.trim().to_owned(),
    };
    validate_client_id(&settings.client_id)?;
    validate_app_slug(&settings.app_slug)?;
    if state
        .github
        .settings()?
        .is_some_and(|current| current.client_id != settings.client_id)
    {
        forget_token(&state).await?;
    }
    let path = state.github.settings_path.clone();
    let json = serde_json::to_vec_pretty(&settings).map_err(|error| {
        CommandError::internal("serializando la configuración de GitHub", error)
    })?;
    blocking(move || {
        write_private_file(&path, &json)
            .map_err(|error| CommandError::internal("guardando la configuración de GitHub", error))
    })
    .await?;
    Ok(settings)
}

/// Vuelve a la App del proyecto. Si se usaba otra, la cuenta conectada se desconecta.
#[tauri::command]
pub async fn github_use_default_app(state: State<'_, AppState>) -> CommandResult<GitHubSettings> {
    if state.github.custom()?.is_some() {
        forget_token(&state).await?;
    }
    let path = state.github.settings_path.clone();
    blocking(move || match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::internal(
            "borrando la configuración de GitHub",
            error,
        )),
    })
    .await?;
    Ok(default_settings())
}

/// Registra la llave de despliegue de un servicio en el repositorio (*deploy key* de solo
/// lectura). Solo funciona con una App propia con *Administration: write*; con la de Blacksmith (solo
/// lectura de contenido) falla con `GITHUB_PERMISSION_MISSING` y la interfaz ofrece hacerlo a mano.
#[tauri::command]
pub async fn github_add_deploy_key(
    state: State<'_, AppState>,
    owner: String,
    name: String,
    title: String,
    key: String,
) -> CommandResult<()> {
    let token = token(&state).await?;
    GitHubClient::new()?
        .add_deploy_key(&token, &owner, &name, &title, &key)
        .await
}

/// Deja en el servidor la cuenta conectada para que despliegue repositorios privados por su
/// cuenta (`PUT /api/v1/github` del agente). El token va de la bóveda al agente por el túnel, sin
/// pasar por la webview; el servidor lo usa solo para descargar código (la App solo lee).
#[tauri::command]
pub async fn github_share_with_server(
    state: State<'_, AppState>,
    server_id: String,
) -> CommandResult<shared::GitHubAccess> {
    let token = token(&state).await?;
    let user = GitHubClient::new()?.user(&token).await?;
    let body = Zeroizing::new(
        serde_json::to_string(&shared::SetGitHubAccess {
            login: user.login,
            token: token.to_string(),
        })
        .map_err(|error| CommandError::internal("preparando el acceso a GitHub", error))?,
    );
    let addr = state.agent_addr(&server_id).await?;
    let response = agent_http::request(
        addr,
        Method::Put,
        "/api/v1/github",
        Some(body.as_str()),
        None,
        agent_http::REQUEST_TIMEOUT,
    )
    .await?;
    if response.status != 200 {
        return Err(
            match serde_json::from_str::<shared::ApiErrorBody>(&response.body) {
                Ok(error) => CommandError::AgentRejected {
                    status: response.status,
                    code: error.code,
                    message: error.message.into_owned(),
                },
                Err(_) => CommandError::AgentRejected {
                    status: response.status,
                    code: shared::ErrorCode::Unknown,
                    message: format!("respuesta {} sin cuerpo de error", response.status),
                },
            },
        );
    }
    serde_json::from_str(&response.body)
        .map_err(|error| CommandError::internal("respuesta del agente inesperada", error))
}

/// Pide a GitHub un código para que el usuario lo confirme en `verification_uri`.
#[tauri::command]
pub async fn github_login_start(state: State<'_, AppState>) -> CommandResult<LoginStart> {
    let settings = state
        .github
        .settings()?
        .ok_or(CommandError::GitHubNotConfigured)?;
    let code = GitHubClient::new()?
        .device_code(&settings.client_id)
        .await?;
    let start = LoginStart {
        user_code: code.user_code.clone(),
        verification_uri: code.verification_uri.clone(),
        expires_in: code.expires_in,
    };
    let mut login = state.github.login.lock().await;
    if let Some(previous) = login.take() {
        previous.cancel.notify_one();
    }
    *login = Some(PendingLogin {
        client_id: settings.client_id,
        device_code: code.device_code,
        interval: Duration::from_secs(code.interval.max(5)),
        expires_at: Instant::now() + Duration::from_secs(code.expires_in),
        cancel: Arc::new(Notify::new()),
    });
    Ok(start)
}

/// Espera a que el usuario confirme el código (o lo rechace, caduque o se cancele) respetando el
/// intervalo de sondeo que pide GitHub. Al confirmarse, guarda el token y devuelve el usuario.
#[tauri::command]
pub async fn github_login_wait(state: State<'_, AppState>) -> CommandResult<GitHubUser> {
    let (client_id, device_code, mut interval, expires_at, cancel) = {
        let login = state.github.login.lock().await;
        let pending = login.as_ref().ok_or(CommandError::GitHubLoginCancelled)?;
        (
            pending.client_id.clone(),
            pending.device_code.clone(),
            pending.interval,
            pending.expires_at,
            Arc::clone(&pending.cancel),
        )
    };
    let client = GitHubClient::new()?;
    let token = loop {
        tokio::select! {
            () = cancel.notified() => return Err(CommandError::GitHubLoginCancelled),
            () = tokio::time::sleep(interval) => {}
        }
        if Instant::now() >= expires_at {
            state.github.login.lock().await.take();
            return Err(CommandError::GitHubCodeExpired);
        }
        match client.poll_token(&client_id, &device_code).await {
            Ok(Poll::Pending) => {}
            Ok(Poll::SlowDown(extra)) => interval += Duration::from_secs(extra.max(5)),
            Ok(Poll::Authorized(token)) => break token,
            Err(error) => {
                state.github.login.lock().await.take();
                return Err(error);
            }
        }
    };
    state.github.login.lock().await.take();

    let user = client.user(&token).await?;
    let vault = Arc::clone(&state.vault);
    blocking(move || vault.put_value(TOKEN_ID, &StoredToken { token, client_id })).await?;
    Ok(user)
}

#[tauri::command]
pub async fn github_login_cancel(state: State<'_, AppState>) -> CommandResult<bool> {
    let pending = state.github.login.lock().await.take();
    Ok(pending.map(|pending| pending.cancel.notify_one()).is_some())
}

/// Olvida el token en este equipo. Para revocar la autorización en GitHub: Settings →
/// Applications → Authorized GitHub Apps.
#[tauri::command]
pub async fn github_logout(state: State<'_, AppState>) -> CommandResult<bool> {
    forget_token(&state).await
}

#[tauri::command]
pub async fn github_repositories(
    state: State<'_, AppState>,
) -> CommandResult<Vec<GitHubRepository>> {
    let token = token(&state).await?;
    GitHubClient::new()?.repositories(&token).await
}

#[tauri::command]
pub async fn github_branches(
    state: State<'_, AppState>,
    owner: String,
    name: String,
) -> CommandResult<Vec<String>> {
    let token = token(&state).await?;
    GitHubClient::new()?.branches(&token, &owner, &name).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuracion_valida() {
        assert!(validate_client_id("Iv23liAbCdEf123456").is_ok());
        assert!(validate_client_id("Iv1.0123456789abcdef").is_ok());
        for bad in ["", "corto", "Iv23 li", "Iv23li/../x", &"a".repeat(65)] {
            assert!(validate_client_id(bad).is_err(), "{bad:?}");
        }
        assert!(validate_app_slug("bbel-blacksmith-hormiga").is_ok());
        for bad in ["", "Mayus", "-a", "a-", "a/b", "a b"] {
            assert!(validate_app_slug(bad).is_err(), "{bad:?}");
        }
    }
}
