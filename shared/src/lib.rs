//! Tipos compartidos entre la aplicación de escritorio y el agente servidor.
//!
//! Este crate es el contrato de la API: cualquier cambio aquí debe romper la compilación del
//! cliente en lugar de fallar en tiempo de ejecución.

mod error;

pub use error::{ApiErrorBody, ErrorCode};

use serde::{Deserialize, Serialize};

/// Estado básico reportado por el agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStatus {
    pub name: String,
    pub version: String,
    pub status: ServiceStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Ready,
}
