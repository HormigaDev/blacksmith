mod agent_api;
mod agent_http;
mod agent_probe;
mod commands;
mod error;
mod files;
mod github;
mod secrets;
mod servers;
mod ssh;

use tauri::Manager;

use crate::{
    commands::AppState,
    error::CommandError,
    secrets::{KeyringMasterKey, MasterKeyStore, Vault},
    servers::ServerStore,
    ssh::host_keys::HostKeyStore,
};

/// Sin llavero del sistema (p. ej. un Linux sin GNOME Keyring ni KWallet) la app arranca igual:
/// las operaciones con credenciales devuelven `KEYRING_UNAVAILABLE`.
struct UnavailableKeyring;

impl MasterKeyStore for UnavailableKeyring {
    fn load(&self) -> Result<Option<zeroize::Zeroizing<[u8; 32]>>, CommandError> {
        Err(CommandError::KeyringUnavailable)
    }
    fn store(&self, _key: &[u8; 32]) -> Result<(), CommandError> {
        Err(CommandError::KeyringUnavailable)
    }
}

fn app_state(app: &tauri::App) -> Result<AppState, Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    let home = app.path().home_dir().ok();
    let master: Box<dyn MasterKeyStore> = match KeyringMasterKey::install() {
        Ok(keyring) => Box::new(keyring),
        Err(_) => Box::new(UnavailableKeyring),
    };
    let vault = Vault::new(data_dir.join("credentials"), master);
    let host_keys = HostKeyStore::new(
        data_dir.join("known_hosts.json"),
        home.as_ref()
            .map(|home| home.join(".ssh").join("known_hosts")),
    );
    let servers = ServerStore::new(data_dir.join("servers.json"));
    let github = github::GitHubState::new(data_dir.join("github.json"));
    Ok(AppState::new(vault, servers, host_keys, home, github))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // TLS de reqwest (GitHub) con ring, la misma criptografía que usa russh.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(app_state(app)?);
            Ok(())
        })
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                let app = webview.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(state) = app.try_state::<commands::AppState>() {
                        state.log_follows.stop_all().await;
                    }
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::ssh_save_key,
            commands::ssh_forget_host,
            commands::tunnel_open,
            commands::tunnel_trust_host,
            commands::tunnel_close,
            commands::servers_list,
            commands::servers_reorder,
            commands::server_remove,
            commands::server_add_local,
            commands::local_agent_status,
            agent_api::agent_request,
            agent_api::logs_follow,
            agent_api::logs_stop,
            github::github_status,
            github::github_configure,
            github::github_use_default_app,
            github::github_share_with_server,
            github::github_add_deploy_key,
            github::github_login_start,
            github::github_login_wait,
            github::github_login_cancel,
            github::github_logout,
            github::github_repositories,
            github::github_branches,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("blacksmith: la aplicación terminó con error: {error}");
        std::process::exit(1);
    }
}
