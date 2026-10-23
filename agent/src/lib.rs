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
pub mod metrics;
pub mod nginx;
pub mod resources;
pub mod routes;
pub mod server;
pub mod services;
pub mod state;
pub mod systemd;
pub mod telemetry;
