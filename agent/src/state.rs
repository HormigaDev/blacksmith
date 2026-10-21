use std::sync::Arc;

use crate::config::Config;

/// Estado compartido por los handlers. Clonarlo solo copia punteros.
#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
}
