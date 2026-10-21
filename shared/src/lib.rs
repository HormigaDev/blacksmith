//! Tipos compartidos entre la aplicación de escritorio y el agente servidor.

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
