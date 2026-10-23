//! Agente de Blacksmith: servidor HTTP local que administra servicios `systemd`.
//!
//! Solo escucha en loopback. La administración llega por túnel SSH y los webhooks de GitHub por
//! Nginx (ver `deploy/`).

pub mod config;
pub mod db;
pub mod deploy;
pub mod envfile;
pub mod error;
pub mod github;
pub mod host;
pub mod http;
pub mod limits;
pub mod nginx;
pub mod resources;
pub mod routes;
pub mod server;
pub mod services;
pub mod state;
pub mod systemd;
pub mod telemetry;

/// Segundos Unix (UTC): las fechas de la base de datos, del contrato y de las versiones.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}
