use std::sync::Arc;

use crate::{config::Config, db::Database, services::ServiceManager, systemd::Systemctl};

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
        let services = ServiceManager::new(db.clone(), Systemctl::default());
        Self {
            config,
            db,
            services,
        }
    }
}
