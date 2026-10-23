use std::{sync::Arc, time::Duration};

use tokio::sync::{Semaphore, watch};

use crate::{
    config::Config,
    db::Database,
    deploy::Paths,
    host::HostProbe,
    nginx::SiteManager,
    routes::logs::LogTimings,
    services::ServiceManager,
    systemd::{DEFAULT_TIMEOUT, JOURNALCTL_PATH, Journalctl, SYSTEMCTL_PATH, Scope, Systemctl},
};

/// Margen para que una acción que se agota responda `SYSTEMD_TIMEOUT` (con su diagnóstico)
/// antes de que el límite de la petición corte con un `REQUEST_TIMEOUT` genérico.
const ACTION_MARGIN: Duration = Duration::from_secs(2);

/// Estado compartido por los handlers. Clonarlo solo copia punteros.
#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Database,
    /// Única vía para actuar sobre servicios: solo los registrados en `db`.
    pub services: ServiceManager,
    /// Sitios de nginx de los servicios.
    pub sites: SiteManager,
    /// Datos del servidor (sistema, hardware y uso).
    pub host: HostProbe,
    pub journal: Journalctl,
    /// Cupos de flujos de logs simultáneos (`BLACKSMITH_MAX_LOG_STREAMS`).
    pub log_streams: Arc<Semaphore>,
    pub log_timings: LogTimings,
    pub shutdown: Shutdown,
}

impl AppState {
    pub fn new(config: Arc<Config>, db: Database, paths: Paths) -> Self {
        let timeout = config
            .request_timeout
            .saturating_sub(ACTION_MARGIN)
            .clamp(Duration::from_secs(1), DEFAULT_TIMEOUT);
        let scope = config.systemd_scope;
        let systemctl = Systemctl::new(SYSTEMCTL_PATH, scope, timeout);
        let journal = Journalctl::new(JOURNALCTL_PATH, scope);
        Self::with_commands(config, db, systemctl, journal, paths)
    }

    /// Con otros `systemctl`, `journalctl` y rutas (tests).
    pub fn with_commands(
        config: Arc<Config>,
        db: Database,
        systemctl: Systemctl,
        journal: Journalctl,
        paths: Paths,
    ) -> Self {
        // nginx y certbot son del systemd del sistema, también en modo local.
        let sites = SiteManager::new(
            db.clone(),
            systemctl.system(),
            Journalctl::new(JOURNALCTL_PATH, Scope::System),
            paths.clone(),
            config.bind_addr.port(),
        );
        let services = ServiceManager::new(db.clone(), systemctl, paths);
        let log_streams = Arc::new(Semaphore::new(config.max_log_streams));
        Self {
            config,
            db,
            services,
            sites,
            host: HostProbe::default(),
            journal,
            log_streams,
            log_timings: LogTimings::default(),
            shutdown: Shutdown::default(),
        }
    }
}

/// Aviso de apagado para las conexiones que viven más allá de una petición (WebSocket): el
/// apagado ordenado de HTTP no las espera ni las cierra.
#[derive(Debug, Clone)]
pub struct Shutdown(Arc<watch::Sender<bool>>);

impl Default for Shutdown {
    fn default() -> Self {
        Self(Arc::new(watch::Sender::new(false)))
    }
}

impl Shutdown {
    pub fn trigger(&self) {
        self.0.send_replace(true);
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.0.subscribe()
    }
}
