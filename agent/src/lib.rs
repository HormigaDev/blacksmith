//! Agente de BBEL Blacksmith: servidor HTTP local que administra servicios `systemd`.
//!
//! Solo escucha en loopback. La administración llega por túnel SSH y los webhooks de GitHub por
//! Nginx (ver `deploy/`).

pub mod config;
pub mod error;
pub mod http;
pub mod routes;
pub mod server;
pub mod state;
pub mod telemetry;
