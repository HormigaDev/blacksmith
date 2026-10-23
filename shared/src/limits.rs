//! Límites de recursos de un servicio (`/api/v1/services/{name}/limits`).
//!
//! Son los del cgroup de su unidad (systemd, cgroups v2): se aplican al momento, sin reiniciar, y
//! cubren todos sus procesos. Sin límite, el servicio usa lo que el servidor tenga libre.

use serde::{Deserialize, Serialize};

use crate::HostCapacity;

/// Prioridad frente a los demás servicios cuando compiten por CPU y disco (`CPUWeight=` e
/// `IOWeight=`). No limita: solo reparte cuando no hay para todos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
}

impl Priority {
    /// Peso de systemd (1-10000; 100 es el de cualquier unidad sin configurar).
    pub fn weight(self) -> u16 {
        match self {
            Self::Low => 25,
            Self::Normal => 100,
            Self::High => 400,
        }
    }
}

/// Límites de un servicio. `None` = sin límite.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLimits {
    /// CPU máxima (`CPUQuota=`): 100 = un núcleo completo; 250 = dos núcleos y medio.
    pub cpu_percent: Option<u32>,
    #[serde(default)]
    pub priority: Priority,
    /// Memoria máxima (`MemoryMax=`). Si la supera, el kernel termina el proceso (OOM) y systemd
    /// lo reinicia.
    pub memory_max_bytes: Option<u64>,
    /// Swap máxima (`MemorySwapMax=`). `Some(0)` = sin swap.
    pub swap_max_bytes: Option<u64>,
    /// Procesos e hilos (`TasksMax=`).
    pub tasks_max: Option<u32>,
}

/// Respuesta de `GET`/`PUT /api/v1/services/{name}/limits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLimits {
    pub limits: ResourceLimits,
    /// Capacidad del servidor: los límites no pueden superarla.
    pub host: HostCapacity,
    /// `None` si nunca se configuraron (sin límites).
    pub updated_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_de_los_limites() {
        let limits = ResourceLimits {
            cpu_percent: Some(150),
            priority: Priority::High,
            memory_max_bytes: Some(536_870_912),
            swap_max_bytes: Some(0),
            tasks_max: None,
        };
        assert_eq!(
            serde_json::to_string(&limits).unwrap(),
            r#"{"cpuPercent":150,"priority":"high","memoryMaxBytes":536870912,"swapMaxBytes":0,"tasksMax":null}"#
        );
        // Sin prioridad, la normal.
        let parsed: ResourceLimits = serde_json::from_str(
            r#"{"cpuPercent":null,"memoryMaxBytes":null,"swapMaxBytes":null,"tasksMax":null}"#,
        )
        .unwrap();
        assert_eq!(parsed, ResourceLimits::default());
    }
}
