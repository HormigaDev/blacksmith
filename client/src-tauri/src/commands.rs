//! Comandos de Tauri: la frontera con la webview. Todo lo que llega se valida; nada secreto vuelve.
//! Contrato en TypeScript: client/src/lib/ssh/api.ts.

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use shared::AgentStatus;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use zeroize::Zeroizing;

use crate::{
    agent_probe,
    error::{CommandError, CommandResult},
    secrets::{Credential, Vault},
    servers::{ServerKind, ServerProfile, ServerStore},
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
    pub(crate) vault: Arc<Vault>,
    servers: Arc<ServerStore>,
    host_keys: HostKeyStore,
    home: Option<PathBuf>,
    tunnels: Mutex<HashMap<String, OpenTunnel>>,
    /// Llaves de servidor presentadas y aún sin confirmar, por `host:puerto`.
    pending_host_keys: std::sync::Mutex<HashMap<String, PresentedKey>>,
    /// Visores de logs abiertos (ver `agent_api`).
    pub(crate) log_follows: crate::agent_api::LogFollows,
    pub(crate) github: crate::github::GitHubState,
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
        github: crate::github::GitHubState,
    ) -> Self {
        Self {
            vault: Arc::new(vault),
            servers: Arc::new(servers),
            host_keys,
            home,
            tunnels: Mutex::new(HashMap::new()),
            pending_host_keys: std::sync::Mutex::new(HashMap::new()),
            log_follows: crate::agent_api::LogFollows::default(),
            github,
        }
    }

    /// Dirección del agente de un servidor: el extremo local de su túnel o, si es este equipo,
    /// el propio agente. La webview solo elige el servidor, nunca la dirección.
    pub(crate) async fn agent_addr(&self, server_id: &str) -> CommandResult<SocketAddr> {
        let servers = Arc::clone(&self.servers);
        let id = server_id.to_owned();
        let profile = blocking(move || servers.get(&id))
            .await?
            .ok_or(CommandError::ServerNotFound)?;
        let port = match profile.kind {
            ServerKind::Local => profile.port,
            ServerKind::Ssh => self
                .tunnels
                .lock()
                .await
                .get(server_id)
                .map(|open| open.tunnel.local_port())
                .ok_or(CommandError::NotConnected)?,
        };
        Ok(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
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
    /// Lo que respondió el agente al abrir el túnel.
    agent: Option<AgentStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelClosed {
    target_id: String,
    reason: CloseReason,
}

/// Las operaciones del llavero y del disco son bloqueantes: fuera del runtime asíncrono.
/// Trabajo de archivos, llavero o cifrado en un hilo aparte, para no bloquear el runtime.
pub(crate) async fn blocking<T: Send + 'static>(
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
        let server = servers.upsert(ServerProfile::from(&target))?;
        Ok(SavedServer {
            server,
            key: summary,
        })
    })
    .await
}

/// Servidores configurados en el orden guardado.
#[tauri::command]
pub async fn servers_list(state: State<'_, AppState>) -> CommandResult<Vec<ServerProfile>> {
    let servers = Arc::clone(&state.servers);
    blocking(move || servers.list()).await
}

/// Guarda el nuevo orden de la lista. Devuelve la lista ya ordenada.
#[tauri::command]
pub async fn servers_reorder(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> CommandResult<Vec<ServerProfile>> {
    let servers = Arc::clone(&state.servers);
    blocking(move || servers.reorder(&ids)).await
}

/// Registra este mismo equipo como servidor: sin túnel ni llave, se habla con el agente
/// directamente en `127.0.0.1:<puerto>` (4040 por defecto). Para probar el cliente con un agente
/// local.
#[tauri::command]
pub async fn server_add_local(
    state: State<'_, AppState>,
    port: Option<u16>,
) -> CommandResult<ServerProfile> {
    let port = agent_port(port)?;
    let servers = Arc::clone(&state.servers);
    blocking(move || servers.upsert(ServerProfile::local(port))).await
}

/// Estado del agente local. Solo consulta `127.0.0.1`: el comando no acepta otro host, así que no
/// sirve para sondear otras máquinas.
#[tauri::command]
pub async fn local_agent_status(port: Option<u16>) -> CommandResult<AgentStatus> {
    let port = agent_port(port)?;
    agent_probe::probe(
        SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        agent_probe::PROBE_TIMEOUT,
    )
    .await
}

fn agent_port(port: Option<u16>) -> CommandResult<u16> {
    match port.unwrap_or(AGENT_PORT) {
        0 => Err(CommandError::InvalidTarget("port")),
        port => Ok(port),
    }
}

/// Quita el servidor. Si es SSH, cierra su túnel y borra su llave; la llave del servidor (huella
/// confirmada) se conserva, para que siga protegido contra suplantaciones si se vuelve a añadir.
/// Acción destructiva: la interfaz la confirma antes.
#[tauri::command]
pub async fn server_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<bool> {
    let servers = Arc::clone(&state.servers);
    let lookup = id.clone();
    let Some(profile) = blocking(move || servers.get(&lookup)).await? else {
        return Ok(false);
    };
    if profile.kind == ServerKind::Ssh {
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
    }
    let vault = Arc::clone(&state.vault);
    let servers = Arc::clone(&state.servers);
    blocking(move || {
        if profile.kind == ServerKind::Ssh {
            vault.delete(&id)?;
        }
        servers.remove(&id)
    })
    .await
}

/// Abre el túnel hacia el agente. Si ya está abierto, devuelve el existente.
#[tauri::command]
pub async fn tunnel_open(
    app: AppHandle,
    state: State<'_, AppState>,
    target: TargetInput,
    passphrase: Option<Zeroizing<String>>,
) -> CommandResult<TunnelInfo> {
    let target = target.validate()?;
    let id = target.id();
    // El mutex no se mantiene mientras se conecta (hasta 10 s): bloquearía estado y cierres.
    if let Some(open) = state.tunnels.lock().await.get(&id) {
        return Ok(info(&id, &open.tunnel));
    }

    let vault = Arc::clone(&state.vault);
    let credential_id = id.clone();
    // Descifrar una llave con passphrase (bcrypt) tarda hasta un segundo: fuera del runtime.
    let private_key = blocking(move || {
        let credential = vault
            .get(&credential_id)?
            .ok_or(CommandError::CredentialsMissing)?;
        let passphrase = passphrase.or(credential.passphrase);
        key::decode(
            &credential.private_key,
            passphrase.as_deref().map(String::as_str),
        )
    })
    .await?;

    // Puerto local libre que elige el sistema: la interfaz no decide dónde escucha nada.
    let config = TunnelConfig::to_agent(target.clone(), 0);
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

    // El servidor pudo quitarse mientras se conectaba: su túnel no debe quedar abierto.
    let servers = Arc::clone(&state.servers);
    let lookup = id.clone();
    if blocking(move || servers.get(&lookup)).await?.is_none() {
        tunnel.close().await;
        return Err(CommandError::ServerNotFound);
    }
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

fn info(id: &str, tunnel: &Tunnel) -> TunnelInfo {
    TunnelInfo {
        target_id: id.to_owned(),
        local_port: tunnel.local_port(),
        remote_port: AGENT_PORT,
        agent: tunnel.agent().cloned(),
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
        // Si el aviso se cierra sin llegar (la sesión terminó sin pasar por `disconnected`), el
        // túnel tampoco sirve ya: se retira igual. Uno cerrado a propósito ya no está en el mapa.
        let _ = lost.wait_for(|lost| *lost).await;
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
