use std::{io, net::SocketAddr, sync::Arc};

use sd_notify::NotifyState;
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
    sync::watch,
};

use crate::{
    config::{Config, Environment},
    db::Database,
    error::DisplayChain,
    http,
    services::{PermissionCheck, ServiceManager},
    state::AppState,
};

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("no se pudo escuchar en {addr}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("no se pudieron instalar los manejadores de señales")]
    Signals(#[source] io::Error),
    #[error("el servidor HTTP terminó con error")]
    Serve(#[source] io::Error),
}

/// Sirve hasta recibir SIGTERM/SIGINT. Después drena las conexiones abiertas durante, como
/// mucho, `shutdown_timeout`; las que sigan vivas se cortan.
pub async fn run(config: Arc<Config>, db: Database) -> Result<(), ServerError> {
    // Los manejadores se instalan antes de aceptar tráfico para que un fallo aborte el arranque.
    let mut sigterm = signal(SignalKind::terminate()).map_err(ServerError::Signals)?;
    let mut sigint = signal(SignalKind::interrupt()).map_err(ServerError::Signals)?;

    let addr = config.bind_addr;
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|source| ServerError::Bind { addr, source })?;

    let (draining, drain_signal) = watch::channel(false);
    let state = AppState::new(Arc::clone(&config), db);
    let services = state.services.clone();
    let app = http::router(state);
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(wait_for_drain(drain_signal))
        .into_future();
    tokio::pin!(server);

    tracing::info!(
        %addr,
        environment = ?config.environment,
        webhooks = config.github_webhook_secret.is_some(),
        version = env!("CARGO_PKG_VERSION"),
        "agente escuchando"
    );
    notify_systemd(NotifyState::Ready);
    // En segundo plano: no retrasa el arranque ni lo impide (estado y métricas no necesitan
    // permisos), pero un fallo de permisos queda en el registro desde el primer momento.
    tokio::spawn(report_permissions(services, config.environment));

    let signal_name = tokio::select! {
        result = &mut server => return result.map_err(ServerError::Serve),
        _ = sigterm.recv() => "SIGTERM",
        _ = sigint.recv() => "SIGINT",
    };

    tracing::info!(signal = signal_name, grace = ?config.shutdown_timeout, "deteniendo; drenando conexiones");
    notify_systemd(NotifyState::Stopping);
    // Solo falla si el servidor ya soltó el receptor, es decir, si ya terminó.
    let _ = draining.send(true);

    match tokio::time::timeout(config.shutdown_timeout, server).await {
        Ok(result) => result.map_err(ServerError::Serve),
        Err(_) => {
            tracing::warn!("plazo de drenado agotado; se cortan las conexiones restantes");
            Ok(())
        }
    }
}

async fn report_permissions(services: ServiceManager, environment: Environment) {
    match services.check_permissions().await {
        PermissionCheck::Authorized => {
            tracing::info!("polkit autoriza al agente a gestionar sus servicios (bbel-*.service)");
        }
        PermissionCheck::Denied if environment == Environment::Development => tracing::warn!(
            "polkit no autoriza a este usuario: iniciar, parar o reiniciar servicios fallará \
             (normal en desarrollo; en el servidor, ver README «Permisos del agente»)"
        ),
        PermissionCheck::Denied => tracing::error!(
            "polkit NO autoriza al agente: iniciar, parar o reiniciar servicios fallará con \
             SYSTEMD_PERMISSION_DENIED. Instale deploy/polkit/10-blacksmith.rules y verifique \
             con deploy/polkit/verify.sh (README, «Permisos del agente»)"
        ),
        PermissionCheck::Inconclusive(error) => tracing::error!(
            error = %DisplayChain(&error),
            "no se pudo comprobar si polkit autoriza al agente; las acciones sobre servicios \
             pueden fallar (README, «Permisos del agente»)"
        ),
    }
}

async fn wait_for_drain(mut signal: watch::Receiver<bool>) {
    // Si el emisor desaparece también se drena: nunca queda un servidor sin forma de pararse.
    let _ = signal.wait_for(|draining| *draining).await;
}

/// Sin `NOTIFY_SOCKET` (fuera de systemd) no hace nada.
fn notify_systemd(state: NotifyState<'_>) {
    if let Err(error) = sd_notify::notify(false, &[state]) {
        tracing::warn!(%error, "no se pudo notificar a systemd");
    }
}
