use std::{sync::Arc, time::Duration};

use crate::{
    config::Config,
    db::Database,
    services::ServiceManager,
    systemd::{DEFAULT_TIMEOUT, SYSTEMCTL_PATH, Scope, Systemctl},
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
}

impl AppState {
    pub fn new(config: Arc<Config>, db: Database) -> Self {
        let timeout = config
            .request_timeout
            .saturating_sub(ACTION_MARGIN)
            .clamp(Duration::from_secs(1), DEFAULT_TIMEOUT);
        let systemctl = Systemctl::new(SYSTEMCTL_PATH, Scope::System, timeout);
        Self::with_systemctl(config, db, systemctl)
    }

    /// Con otro `systemctl` (tests).
    pub fn with_systemctl(config: Arc<Config>, db: Database, systemctl: Systemctl) -> Self {
        let services = ServiceManager::new(db.clone(), systemctl);
        Self {
            config,
            db,
            services,
        }
    }
}
