//! Datos del servidor (`GET /api/v1/host`): sistema, hardware y su uso actual.
//!
//! Todo es informativo y de solo lectura. Lo que el agente no puede averiguar va como `None`
//! (o una lista vacía), nunca como un error.

use serde::{Deserialize, Serialize};

/// Ruta de los datos del servidor.
pub const HOST_PATH: &str = "/api/v1/host";

/// Respuesta de `GET /api/v1/host`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    pub hostname: Option<String>,
    pub os: OperatingSystem,
    /// Fabricante y modelo del equipo (DMI), p. ej. «Hetzner vServer» o «QEMU Standard PC».
    pub machine: Option<String>,
    /// Tecnología de virtualización (`systemd-detect-virt`: `kvm`, `xen`, `lxc`…); `None` en
    /// una máquina física o si no se sabe.
    pub virtualization: Option<String>,
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    pub network: Vec<NetworkInterface>,
    /// Segundos desde el arranque del servidor.
    pub uptime_secs: u64,
    /// Arranque del servidor, en segundos Unix.
    pub boot_time: u64,
    /// Carga media de 1, 5 y 15 minutos (procesos ejecutándose o esperando CPU).
    pub load_average: [f64; 3],
    /// Zona horaria del sistema (`Europe/Madrid`), si se puede leer.
    pub timezone: Option<String>,
    /// Versión del agente.
    pub agent_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatingSystem {
    /// Nombre completo (`PRETTY_NAME` de os-release), p. ej. «Ubuntu 24.04.1 LTS».
    pub name: Option<String>,
    /// Identificador de la distribución (`ubuntu`, `debian`, `rhel`…).
    pub id: String,
    pub kernel: Option<String>,
    /// Arquitectura (`x86_64`, `aarch64`…).
    pub architecture: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    /// Modelo, p. ej. «AMD EPYC 7763 64-Core Processor».
    pub model: Option<String>,
    pub vendor: Option<String>,
    /// Núcleos físicos, si se conocen.
    pub physical_cores: Option<u32>,
    /// Hilos (CPU lógicas) disponibles.
    pub logical_cores: u32,
    /// Frecuencia actual del primer núcleo, en MHz.
    pub frequency_mhz: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    /// Nombre según la base de datos PCI (`pci.ids`), o los identificadores si no está.
    pub name: String,
    pub vendor: Option<String>,
    /// Controlador del kernel en uso (`nvidia`, `amdgpu`, `i915`, `virtio-pci`…).
    pub driver: Option<String>,
    /// Memoria de vídeo, si el controlador la publica.
    pub memory_bytes: Option<u64>,
    /// Dirección PCI (`0000:00:02.0`).
    pub address: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_bytes: u64,
    /// Disponible para nuevos procesos sin usar swap (`MemAvailable`).
    pub available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_free_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiskKind {
    Ssd,
    Hdd,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub mount_point: String,
    /// Dispositivo (`/dev/sda1`).
    pub device: String,
    pub file_system: String,
    pub kind: DiskKind,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    pub name: String,
    /// Direcciones con su prefijo (`203.0.113.7/24`, `2001:db8::1/64`), sin las de enlace local.
    pub addresses: Vec<String>,
    /// Bytes recibidos y enviados desde el arranque.
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_del_servidor() {
        let host = HostInfo {
            hostname: Some("vps-1".into()),
            os: OperatingSystem {
                name: Some("Ubuntu 24.04.1 LTS".into()),
                id: "ubuntu".into(),
                kernel: Some("6.8.0-45-generic".into()),
                architecture: "x86_64".into(),
            },
            machine: None,
            virtualization: Some("kvm".into()),
            cpu: CpuInfo {
                model: Some("AMD EPYC".into()),
                vendor: None,
                physical_cores: None,
                logical_cores: 2,
                frequency_mhz: None,
            },
            gpus: vec![],
            memory: MemoryInfo {
                total_bytes: 4,
                available_bytes: 3,
                swap_total_bytes: 2,
                swap_free_bytes: 1,
            },
            disks: vec![DiskInfo {
                mount_point: "/".into(),
                device: "/dev/sda1".into(),
                file_system: "ext4".into(),
                kind: DiskKind::Ssd,
                total_bytes: 10,
                available_bytes: 5,
                read_only: false,
            }],
            network: vec![],
            uptime_secs: 60,
            boot_time: 1,
            load_average: [0.5, 0.25, 0.0],
            timezone: None,
            agent_version: "0.1.0".into(),
        };
        let json = serde_json::to_string(&host).unwrap();
        assert!(json.contains(r#""os":{"name":"Ubuntu 24.04.1 LTS","id":"ubuntu","kernel":"6.8.0-45-generic","architecture":"x86_64"}"#), "{json}");
        assert!(json.contains(r#""kind":"ssd""#), "{json}");
        assert!(json.contains(r#""loadAverage":[0.5,0.25,0.0]"#), "{json}");
        assert_eq!(serde_json::from_str::<HostInfo>(&json).unwrap(), host);
    }
}
