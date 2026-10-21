//! Comandos de Tauri: la frontera con la webview. Todo lo que llega se valida; nada secreto vuelve.
//! Contrato en TypeScript: client/src/lib/ssh/api.ts.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use zeroize::Zeroizing;

use crate::{
    error::{CommandError, CommandResult},
    secrets::{Credential, Vault},
    servers::{ServerProfile, ServerStore},
    ssh::{
        host_keys::{HostKeyStore, PresentedKey},
        key::{self, KeySource, KeySummary},
        target::{SshTarget, DEFAULT_SSH_PORT},
        tunnel::{CloseReason, Tunnel, TunnelConfig, AGENT_PORT},
    },
};

/// Evento emitido cuando un túnel se cierra o se pierde.
pub const TUNNEL_CLOSED_EVENT: &str = "tunnel://closed";

pub struct AppState {
    vault: Arc<Vault>,
    servers: Arc<ServerStore>,
    host_keys: HostKeyStore,
    home: Option<PathBuf>,
    tunnels: Mutex<HashMap<String, OpenTunnel>>,
    /// Llaves de servidor presentadas y aún sin confirmar, por `host:puerto`.
    pending_host_keys: std::sync::Mutex<HashMap<String, PresentedKey>>,
}

struct OpenTunnel {
    tunnel: Tunnel,
    /// Distingue este túnel de uno posterior al mismo destino.
    generation: u64,
}

impl AppState {
    pub fn new(
        vault: Vault,
        servers: ServerStore,
        host_keys: HostKeyStore,
        home: Option<PathBuf>,
    ) -> Self {
        Self {
            vault: Arc::new(vault),
            servers: Arc::new(servers),
            host_keys,
            home,
            tunnels: Mutex::new(HashMap::new()),
            pending_host_keys: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInput {
    host: String,
    port: Option<u16>,
    username: String,
}

impl TargetInput {
    fn validate(&self) -> CommandResult<SshTarget> {
        SshTarget::new(
            &self.host,
            self.port.unwrap_or(DEFAULT_SSH_PORT),
            &self.username,
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelInfo {
    target_id: String,
    local_port: u16,
    remote_port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelClosed {
    target_id: String,
    reason: CloseReason,
}

/// Las operaciones del llavero y del disco son bloqueantes: fuera del runtime asíncrono.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> CommandResult<T> + Send + 'static,
) -> CommandResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| CommandError::internal("ejecutando una operación bloqueante", error))?
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedServer {
    /// Perfil normalizado (host en minúsculas, puerto resuelto): úsese su `id` en adelante.
    server: ServerProfile,
    key: KeySummary,
}

/// Valida la llave (y la passphrase, si la tiene), la guarda cifrada y registra el servidor.
/// El servidor queda guardado aunque luego no se pueda conectar (p. ej. el agente aún no está
/// instalado): el usuario no tiene que volver a introducir la llave.
#[tauri::command]
pub async fn ssh_save_key(
    state: State<'_, AppState>,
    target: TargetInput,
    source: KeySource,
    passphrase: Option<Zeroizing<String>>,
    remember_passphrase: bool,
) -> CommandResult<SavedServer> {
    let target = target.validate()?;
    let home = state.home.clone();
    let vault = Arc::clone(&state.vault);
    let servers = Arc::clone(&state.servers);
    blocking(move || {
        let pem = key::read_source(source, home.as_deref())?;
        let encrypted = key::is_encrypted(&pem);
        let decoded = key::decode(&pem, passphrase.as_deref().map(String::as_str))?;
        let summary = key::summary(&decoded, encrypted);
        let credential = Credential {
            private_key: pem,
            passphrase: passphrase.filter(|_| encrypted && remember_passphrase),
            summary: summary.clone(),
        };
        vault.put(&target.id(), &credential)?;
        let server = servers.upsert(&target)?;
        Ok(SavedServer {
            server,
            key: summary,
        })
    })
    .await
}

/// Servidores configurados, en orden de alta.
#[tauri::command]
pub async fn servers_list(state: State<'_, AppState>) -> CommandResult<Vec<ServerProfile>> {
    let servers = Arc::clone(&state.servers);
    blocking(move || servers.list()).await
}

/// Quita el servidor: cierra su túnel si está abierto y borra su llave y su registro. La llave
/// del servidor (huella confirmada) se conserva: si se vuelve a añadir, sigue protegido contra
/// suplantaciones. Acción destructiva: la interfaz la confirma antes.
#[tauri::command]
pub async fn server_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    target: TargetInput,
) -> CommandResult<bool> {
    let target = target.validate()?;
    let id = target.id();
    if let Some(open) = state.tunnels.lock().await.remove(&id) {
        open.tunnel.close().await;
        let _ = app.emit(
            TUNNEL_CLOSED_EVENT,
            TunnelClosed {
                target_id: id.clone(),
                reason: CloseReason::Requested,
            },
        );
    }
    let vault = Arc::clone(&state.vault);
    let servers = Arc::clone(&state.servers);
    blocking(move || {
        vault.delete(&id)?;
        servers.remove(&target)
    })
    .await
}

/// Resumen de la llave guardada para este servidor (nunca la llave).
#[tauri::command]
pub async fn ssh_key_info(
    state: State<'_, AppState>,
    target: TargetInput,
) -> CommandResult<Option<KeyInfo>> {
    let target = target.validate()?;
    let vault = Arc::clone(&state.vault);
    blocking(move || {
        Ok(vault.get(&target.id())?.map(|credential| KeyInfo {
            passphrase_saved: credential.passphrase.is_some(),
            key: credential.summary,
        }))
    })
    .await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    #[serde(flatten)]
    key: KeySummary,
    passphrase_saved: bool,
}

/// Borra la llave guardada para este servidor. La interfaz lo confirma antes.
#[tauri::command]
pub async fn ssh_forget_key(
    state: State<'_, AppState>,
    target: TargetInput,
) -> CommandResult<bool> {
    let target = target.validate()?;
    let vault = Arc::clone(&state.vault);
    blocking(move || vault.delete(&target.id())).await
}

/// Abre el túnel hacia el agente. Si ya está abierto, devuelve el existente.
#[tauri::command]
pub async fn tunnel_open(
    app: AppHandle,
    state: State<'_, AppState>,
    target: TargetInput,
    passphrase: Option<Zeroizing<String>>,
    local_port: Option<u16>,
) -> CommandResult<TunnelInfo> {
    let target = target.validate()?;
    let id = target.id();
    // El mutex no se mantiene mientras se conecta (hasta 10 s): bloquearía estado y cierres.
    if let Some(open) = state.tunnels.lock().await.get(&id) {
        return Ok(info(&id, &open.tunnel));
    }

    let vault = Arc::clone(&state.vault);
    let credential_id = id.clone();
    let credential = blocking(move || vault.get(&credential_id))
        .await?
        .ok_or(CommandError::CredentialsMissing)?;
    let passphrase = passphrase.or(credential.passphrase);
    let private_key = key::decode(
        &credential.private_key,
        passphrase.as_deref().map(String::as_str),
    )?;

    let config = TunnelConfig::to_agent(target.clone(), local_port.unwrap_or(0));
    let tunnel = match Tunnel::open(config, private_key, state.host_keys.clone()).await {
        Ok(tunnel) => tunnel,
        Err(CommandError::HostKeyUnknown {
            algorithm,
            fingerprint,
            public_key,
        }) => {
            // Se recuerda lo presentado: solo esa llave se podrá confirmar con `tunnel_trust_host`.
            if let Ok(mut pending) = state.pending_host_keys.lock() {
                pending.insert(
                    target.host_port(),
                    PresentedKey {
                        algorithm: algorithm.clone(),
                        fingerprint: fingerprint.clone(),
                        openssh: public_key.clone(),
                    },
                );
            }
            return Err(CommandError::HostKeyUnknown {
                algorithm,
                fingerprint,
                public_key,
            });
        }
        Err(error) => return Err(error),
    };

    let mut tunnels = state.tunnels.lock().await;
    if let Some(open) = tunnels.get(&id) {
        // Otra llamada abrió el mismo destino mientras conectábamos: se conserva esa.
        let existing = info(&id, &open.tunnel);
        drop(tunnels);
        tunnel.close().await;
        return Ok(existing);
    }
    let generation = next_generation();
    let result = info(&id, &tunnel);
    watch_for_loss(app, id.clone(), generation, tunnel.lost());
    tunnels.insert(id, OpenTunnel { tunnel, generation });
    Ok(result)
}

/// Confirma la llave de servidor que se acaba de presentar. Rechaza cualquier otra huella.
#[tauri::command]
pub async fn tunnel_trust_host(
    state: State<'_, AppState>,
    target: TargetInput,
    fingerprint: String,
) -> CommandResult<()> {
    let target = target.validate()?;
    let presented = {
        let mut pending = state
            .pending_host_keys
            .lock()
            .map_err(|_| CommandError::internal("llaves pendientes", "mutex envenenado"))?;
        match pending.get(&target.host_port()) {
            Some(key) if key.fingerprint == fingerprint => pending.remove(&target.host_port()),
            _ => None,
        }
    }
    .ok_or(CommandError::HostKeyNotPending)?;
    let host_keys = state.host_keys.clone();
    blocking(move || host_keys.trust(&target, &presented)).await
}

/// Olvida la llave registrada del servidor (p. ej. tras reinstalarlo). La interfaz lo confirma antes.
#[tauri::command]
pub async fn ssh_forget_host(
    state: State<'_, AppState>,
    target: TargetInput,
) -> CommandResult<bool> {
    let target = target.validate()?;
    let host_keys = state.host_keys.clone();
    blocking(move || host_keys.forget(&target)).await
}

#[tauri::command]
pub async fn tunnel_close(
    app: AppHandle,
    state: State<'_, AppState>,
    target: TargetInput,
) -> CommandResult<bool> {
    let id = target.validate()?.id();
    let Some(open) = state.tunnels.lock().await.remove(&id) else {
        return Ok(false);
    };
    open.tunnel.close().await;
    let _ = app.emit(
        TUNNEL_CLOSED_EVENT,
        TunnelClosed {
            target_id: id,
            reason: CloseReason::Requested,
        },
    );
    Ok(true)
}

#[tauri::command]
pub async fn tunnel_status(
    state: State<'_, AppState>,
    target: TargetInput,
) -> CommandResult<Option<TunnelInfo>> {
    let id = target.validate()?.id();
    Ok(state
        .tunnels
        .lock()
        .await
        .get(&id)
        .map(|open| info(&id, &open.tunnel)))
}

fn info(id: &str, tunnel: &Tunnel) -> TunnelInfo {
    TunnelInfo {
        target_id: id.to_owned(),
        local_port: tunnel.local_port(),
        remote_port: AGENT_PORT,
    }
}

fn next_generation() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Si la conexión se pierde, retira el túnel y avisa a la interfaz.
fn watch_for_loss(
    app: AppHandle,
    id: String,
    generation: u64,
    mut lost: tokio::sync::watch::Receiver<bool>,
) {
    tauri::async_runtime::spawn(async move {
        if lost.wait_for(|lost| *lost).await.is_err() {
            return;
        }
        let state = tauri::Manager::state::<AppState>(&app);
        let removed = {
            let mut tunnels = state.tunnels.lock().await;
            match tunnels.get(&id) {
                Some(open) if open.generation == generation => tunnels.remove(&id),
                // Ya se cerró a propósito o hay otro túnel nuevo para el mismo destino.
                _ => None,
            }
        };
        if let Some(open) = removed {
            open.tunnel.close().await;
            let _ = app.emit(
                TUNNEL_CLOSED_EVENT,
                TunnelClosed {
                    target_id: id,
                    reason: CloseReason::ConnectionLost,
                },
            );
        }
    });
}
