//! Servicios gestionados: estado, métricas y acciones (`/api/v1/services/{name}/…`).
//!
//! Los campos viajan en `camelCase`: la interfaz (TypeScript) recibe el JSON tal cual.

use serde::{Deserialize, Serialize};

/// Ruta base de los servicios. `{name}` es el identificador corto del servicio registrado.
pub const SERVICES_PATH: &str = "/api/v1/services";

/// Un servicio registrado (`GET /api/v1/services`). Datos informativos: registrarlo no crea ni
/// toca nada en systemd.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSummary {
    /// Identificador corto (`[a-z0-9-]`, 1-48): define la unidad `blacksmith-<name>.service`.
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub unit: String,
    /// Segundos Unix (UTC).
    pub created_at: i64,
}

/// Cuerpo de `POST /api/v1/services`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterService {
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
}

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
    /// Hay una versión activa (un despliegue terminó bien). Sin ella la unidad existe, por la
    /// plantilla, pero no puede arrancar: systemd la omitiría.
    pub deployed: bool,
    /// Versión activa y la anterior, a la que vuelve un rollback.
    pub release: Option<String>,
    pub previous_release: Option<String>,
    /// Consumo del servicio entero (su cgroup: todos sus procesos, no solo el principal).
    pub resources: ServiceResources,
    /// Capacidad del servidor, para poner el consumo en proporción.
    pub host: HostCapacity,
}

/// Consumo del cgroup del servicio según systemd. `None` si no se conoce (servicio parado,
/// contabilidad desactivada o sin límite configurado).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceResources {
    /// Memoria en uso (incluye la caché de páginas de sus archivos), en bytes.
    pub memory_bytes: Option<u64>,
    /// Máximo de memoria desde que arrancó.
    pub memory_peak_bytes: Option<u64>,
    /// Límite de memoria configurado (`MemoryMax=`).
    pub memory_limit_bytes: Option<u64>,
    pub swap_bytes: Option<u64>,
    /// Límite de swap (`MemorySwapMax=`); `Some(0)` = sin swap.
    pub swap_limit_bytes: Option<u64>,
    /// CPU desde la lectura anterior, con un decimal. 100 = un núcleo completo (como `top`).
    pub cpu_percent: Option<f64>,
    /// Límite de CPU (`CPUQuota=`), en la misma escala (100 = un núcleo).
    pub cpu_limit_percent: Option<f64>,
    /// Procesos e hilos.
    pub tasks: Option<u64>,
    /// Límite de procesos e hilos (`TasksMax=`).
    pub tasks_limit: Option<u64>,
    /// Reinicios automáticos tras un fallo (`Restart=`).
    pub restarts: Option<u64>,
    /// Segundos en marcha desde el último arranque.
    pub uptime_secs: Option<u64>,
}

/// Capacidad del servidor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCapacity {
    pub cpu_cores: u32,
    pub memory_bytes: u64,
    pub swap_bytes: u64,
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
            unit: "blacksmith@api.service".into(),
            load_state: LoadState::NotFound,
            active_state: ActiveState::Inactive,
            sub_state: "dead".into(),
            pid: None,
            metrics: None,
            deployed: false,
            release: None,
            previous_release: None,
            resources: ServiceResources::default(),
            host: HostCapacity::default(),
        };
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            r#"{"name":"api","unit":"blacksmith@api.service","loadState":"not-found","activeState":"inactive","subState":"dead","pid":null,"metrics":null,"deployed":false,"release":null,"previousRelease":null,"resources":{"memoryBytes":null,"memoryPeakBytes":null,"memoryLimitBytes":null,"swapBytes":null,"swapLimitBytes":null,"cpuPercent":null,"cpuLimitPercent":null,"tasks":null,"tasksLimit":null,"restarts":null,"uptimeSecs":null},"host":{"cpuCores":0,"memoryBytes":0,"swapBytes":0}}"#
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
            unit: "blacksmith-api.service".into(),
            outcome: ServiceOutcome::Restarted,
        };
        assert_eq!(
            serde_json::to_string(&report).unwrap(),
            r#"{"name":"api","unit":"blacksmith-api.service","outcome":"restarted"}"#
        );
    }
}
