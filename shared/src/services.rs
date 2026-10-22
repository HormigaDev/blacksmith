//! Servicios gestionados: estado, métricas y acciones (`/api/v1/services/{name}/…`).
//!
//! Los campos viajan en `camelCase`: la interfaz (TypeScript) recibe el JSON tal cual.

use serde::{Deserialize, Serialize};

/// Ruta base de los servicios. `{name}` es el identificador corto del servicio registrado.
pub const SERVICES_PATH: &str = "/api/v1/services";

/// `LoadState` de systemd: si la unidad está instalada y es válida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoadState {
    Loaded,
    /// Registrada en el agente, pero sin archivo de unidad en systemd.
    NotFound,
    BadSetting,
    Error,
    Masked,
    Merged,
    Stub,
}

/// `ActiveState` de systemd (systemd 255).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActiveState {
    Active,
    Reloading,
    Inactive,
    Failed,
    Activating,
    Deactivating,
    Maintenance,
    Refreshing,
}

/// Consumo del proceso principal del servicio.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessMetrics {
    /// Memoria residente en bytes (para formatearla en la interfaz).
    pub rss_bytes: u64,
    /// Memoria residente en megabytes (MiB, 1024 × 1024), con un decimal.
    pub rss_mb: f64,
    /// CPU usada desde la muestra anterior, con un decimal. 100 equivale a un núcleo completo:
    /// en una máquina con 4 núcleos puede llegar a 400, como en `top`.
    pub cpu_percent: f64,
}

/// Respuesta de `GET /api/v1/services/{name}/status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatusReport {
    pub name: String,
    pub unit: String,
    pub load_state: LoadState,
    pub active_state: ActiveState,
    /// Subestado específico del tipo de unidad (`running`, `dead`, `exited`…). Texto libre.
    pub sub_state: String,
    /// `None` si no hay proceso principal (detenido, fallido…).
    pub pid: Option<u32>,
    /// `None` sin proceso principal, o si el proceso terminó justo durante la medición.
    pub metrics: Option<ProcessMetrics>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServiceOutcome {
    Started,
    Stopped,
    Restarted,
}

/// Respuesta de `POST /api/v1/services/{name}/{start,stop,restart}`. Se devuelve cuando systemd
/// terminó el trabajo: `started` significa que el servicio arrancó.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceActionReport {
    pub name: String,
    pub unit: String,
    pub outcome: ServiceOutcome,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_del_estado() {
        let report = ServiceStatusReport {
            name: "api".into(),
            unit: "bbel-api.service".into(),
            load_state: LoadState::NotFound,
            active_state: ActiveState::Inactive,
            sub_state: "dead".into(),
            pid: None,
            metrics: None,
        };
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            r#"{"name":"api","unit":"bbel-api.service","loadState":"not-found","activeState":"inactive","subState":"dead","pid":null,"metrics":null}"#
        );
        let metrics = ProcessMetrics {
            rss_bytes: 1_048_576,
            rss_mb: 1.0,
            cpu_percent: 12.5,
        };
        assert_eq!(
            serde_json::to_string(&metrics).unwrap(),
            r#"{"rssBytes":1048576,"rssMb":1.0,"cpuPercent":12.5}"#
        );
    }

    #[test]
    fn json_de_la_accion() {
        let report = ServiceActionReport {
            name: "api".into(),
            unit: "bbel-api.service".into(),
            outcome: ServiceOutcome::Restarted,
        };
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            r#"{"name":"api","unit":"bbel-api.service","outcome":"restarted"}"#
        );
    }
}
