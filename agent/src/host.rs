//! Datos del servidor (`GET /api/v1/host`): sistema, hardware y su uso actual.
//!
//! - Lo que no cambia mientras corre el agente (SO, CPU, GPU, equipo, virtualización) se lee una
//!   sola vez; el uso (memoria, discos, red, carga) en cada petición.
//! - Solo lecturas de `/proc`, `/sys` y `/etc`, en `spawn_blocking`, más `systemd-detect-virt`
//!   una vez. Nada se guarda entre peticiones salvo lo fijo: el agente no crece en memoria.
//! - Lo que no se puede leer (permisos, sandbox, otra distribución) se omite, nunca falla.

use std::{
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use shared::{
    CpuInfo, DiskInfo, DiskKind, GpuInfo, HostInfo, MemoryInfo, NetworkInterface, OperatingSystem,
};
use sysinfo::{CpuRefreshKind, Disks, Networks, System};
use tokio::sync::OnceCell;

const SYSTEMD_DETECT_VIRT: &str = "/usr/bin/systemd-detect-virt";
const DETECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Base de datos de nombres PCI (paquetes `pciutils`/`hwdata`), por orden de preferencia.
const PCI_IDS: [&str; 2] = ["/usr/share/misc/pci.ids", "/usr/share/hwdata/pci.ids"];
const PCI_DEVICES: &str = "/sys/bus/pci/devices";
const MAX_GPUS: usize = 8;
const MAX_DISKS: usize = 16;
const MAX_INTERFACES: usize = 16;
/// Interfaces virtuales de contenedores y máquinas virtuales locales: no son la red del servidor.
const VIRTUAL_INTERFACES: [&str; 5] = ["docker", "br-", "veth", "virbr", "cni"];
/// Sistemas de archivos virtuales o de paquetes: no son discos.
const PSEUDO_FILE_SYSTEMS: [&str; 9] = [
    "tmpfs", "devtmpfs", "squashfs", "overlay", "ramfs", "efivarfs", "proc", "sysfs", "nsfs",
];

/// Lo que no cambia mientras corre el agente.
#[derive(Debug, Clone)]
struct Fixed {
    os: OperatingSystem,
    machine: Option<String>,
    virtualization: Option<String>,
    cpu: CpuInfo,
    gpus: Vec<GpuInfo>,
}

/// Datos del servidor. Barato de clonar.
#[derive(Debug, Clone, Default)]
pub struct HostProbe {
    fixed: std::sync::Arc<OnceCell<Fixed>>,
}

impl HostProbe {
    pub async fn info(&self) -> HostInfo {
        let fixed = self
            .fixed
            .get_or_init(|| async {
                let virtualization = detect_virtualization().await;
                tokio::task::spawn_blocking(move || read_fixed(virtualization))
                    .await
                    .unwrap_or_else(|_| read_fixed(None))
            })
            .await
            .clone();
        tokio::task::spawn_blocking(move || read_current(fixed))
            .await
            .unwrap_or_else(|_| read_current_fallback())
    }
}

fn read_fixed(virtualization: Option<String>) -> Fixed {
    let mut system = System::new();
    system.refresh_cpu_list(CpuRefreshKind::nothing());
    let first = system.cpus().first();
    let text = |value: &str| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    };
    let logical = u32::try_from(system.cpus().len())
        .ok()
        .filter(|cores| *cores > 0)
        .unwrap_or_else(|| crate::resources::host_capacity().cpu_cores);
    Fixed {
        os: OperatingSystem {
            name: os_pretty_name(),
            id: System::distribution_id(),
            kernel: System::kernel_version(),
            architecture: System::cpu_arch(),
        },
        machine: machine(Path::new("/sys/class/dmi/id")),
        virtualization,
        cpu: CpuInfo {
            model: first.and_then(|cpu| text(cpu.brand())),
            vendor: first.and_then(|cpu| text(cpu.vendor_id())),
            physical_cores: System::physical_core_count().and_then(|n| u32::try_from(n).ok()),
            logical_cores: logical,
            frequency_mhz: None,
        },
        gpus: gpus(Path::new(PCI_DEVICES)),
    }
}

fn read_current(mut fixed: Fixed) -> HostInfo {
    let mut system = System::new();
    system.refresh_memory();
    system.refresh_cpu_list(CpuRefreshKind::nothing().with_frequency());
    fixed.cpu.frequency_mhz = system
        .cpus()
        .first()
        .map(sysinfo::Cpu::frequency)
        .filter(|mhz| *mhz > 0);
    let load = System::load_average();
    let round = |value: f64| (value * 100.0).round() / 100.0;
    HostInfo {
        hostname: System::host_name(),
        os: fixed.os,
        machine: fixed.machine,
        virtualization: fixed.virtualization,
        cpu: fixed.cpu,
        gpus: fixed.gpus,
        memory: MemoryInfo {
            total_bytes: system.total_memory(),
            available_bytes: system.available_memory(),
            swap_total_bytes: system.total_swap(),
            swap_free_bytes: system.free_swap(),
        },
        disks: disks(),
        network: network(),
        uptime_secs: System::uptime(),
        boot_time: System::boot_time(),
        load_average: [round(load.one), round(load.five), round(load.fifteen)],
        timezone: timezone(),
        agent_version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

/// Si la tarea de lectura falla (no debería), al menos lo básico.
fn read_current_fallback() -> HostInfo {
    read_current(Fixed {
        os: OperatingSystem {
            name: None,
            id: System::distribution_id(),
            kernel: None,
            architecture: System::cpu_arch(),
        },
        machine: None,
        virtualization: None,
        cpu: CpuInfo {
            model: None,
            vendor: None,
            physical_cores: None,
            logical_cores: crate::resources::host_capacity().cpu_cores,
            frequency_mhz: None,
        },
        gpus: Vec::new(),
    })
}

/// `PRETTY_NAME` de os-release.
fn os_pretty_name() -> Option<String> {
    ["/etc/os-release", "/usr/lib/os-release"]
        .into_iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .and_then(|text| parse_os_release(&text))
}

fn parse_os_release(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .filter(|value| !value.is_empty())
}

/// Fabricante y modelo (DMI), sin los textos de relleno de las placas genéricas.
fn machine(dmi: &Path) -> Option<String> {
    let read = |name: &str| {
        fs::read_to_string(dmi.join(name))
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| {
                !value.is_empty()
                    && ![
                        "To be filled by O.E.M.",
                        "System Product Name",
                        "System manufacturer",
                        "Default string",
                        "Not Specified",
                    ]
                    .contains(&value.as_str())
            })
    };
    match (read("sys_vendor"), read("product_name")) {
        (Some(vendor), Some(product)) if product.starts_with(&vendor) => Some(product),
        (Some(vendor), Some(product)) => Some(format!("{vendor} {product}")),
        (vendor, product) => vendor.or(product),
    }
}

/// `systemd-detect-virt`: `kvm`, `xen`, `lxc`…; `None` en una máquina física o si falla.
async fn detect_virtualization() -> Option<String> {
    let output = tokio::time::timeout(
        DETECT_TIMEOUT,
        tokio::process::Command::new(SYSTEMD_DETECT_VIRT)
            .env_clear()
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    // Sale con 1 y escribe «none» en una máquina física.
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success()
        && value != "none"
        && !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'))
    .then_some(value)
}

/// Tarjetas gráficas: dispositivos PCI de clase `0x03` (pantalla, 3D).
fn gpus(devices: &Path) -> Vec<GpuInfo> {
    let Ok(entries) = fs::read_dir(devices) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf, String, String)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let class = fs::read_to_string(path.join("class")).ok()?;
            if !class.trim().starts_with("0x03") {
                return None;
            }
            let id = |name: &str| {
                fs::read_to_string(path.join(name))
                    .ok()
                    .map(|value| value.trim().trim_start_matches("0x").to_ascii_lowercase())
            };
            Some((
                entry.file_name().to_string_lossy().into_owned(),
                path.clone(),
                id("vendor")?,
                id("device")?,
            ))
        })
        .collect();
    found.sort();
    found.truncate(MAX_GPUS);
    if found.is_empty() {
        return Vec::new();
    }
    let ids = PCI_IDS
        .into_iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .unwrap_or_default();
    found
        .into_iter()
        .map(|(address, path, vendor, device)| {
            let (vendor_name, device_name) = pci_names(&ids, &vendor, &device);
            let name = nvidia_model(&address)
                .or(device_name)
                .unwrap_or_else(|| format!("{vendor}:{device}"));
            GpuInfo {
                name,
                vendor: vendor_name,
                driver: fs::read_link(path.join("driver")).ok().and_then(|link| {
                    link.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                }),
                // amdgpu la publica; otros controladores, no.
                memory_bytes: fs::read_to_string(path.join("mem_info_vram_total"))
                    .ok()
                    .and_then(|value| value.trim().parse().ok()),
                address,
            }
        })
        .collect()
}

/// Nombres del fabricante y del dispositivo en `pci.ids`: `vvvv  Fabricante` sin sangría y,
/// debajo, `\tdddd  Dispositivo`.
fn pci_names(ids: &str, vendor: &str, device: &str) -> (Option<String>, Option<String>) {
    let mut lines = ids.lines();
    let vendor_name = lines.by_ref().find_map(|line| {
        line.strip_prefix(vendor)
            .filter(|rest| rest.starts_with("  "))
            .map(|rest| rest.trim().to_owned())
    });
    if vendor_name.is_none() {
        return (None, None);
    }
    let device_name = lines
        .take_while(|line| line.starts_with('\t') || line.starts_with('#') || line.is_empty())
        .find_map(|line| {
            line.strip_prefix('\t')
                .filter(|rest| !rest.starts_with('\t'))
                .and_then(|rest| rest.strip_prefix(device))
                .filter(|rest| rest.starts_with("  "))
                .map(|rest| rest.trim().to_owned())
        });
    (vendor_name, device_name)
}

/// El controlador propietario de NVIDIA publica el modelo exacto.
fn nvidia_model(address: &str) -> Option<String> {
    let text =
        fs::read_to_string(format!("/proc/driver/nvidia/gpus/{address}/information")).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("Model:"))
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty())
}

fn disks() -> Vec<DiskInfo> {
    let mut list: Vec<DiskInfo> = Disks::new_with_refreshed_list()
        .list()
        .iter()
        .filter_map(|disk| {
            let file_system = disk.file_system().to_string_lossy().into_owned();
            let mount_point = disk.mount_point().to_string_lossy().into_owned();
            if disk.total_space() == 0
                || PSEUDO_FILE_SYSTEMS.contains(&file_system.as_str())
                || file_system.starts_with("fuse")
                || mount_point.starts_with("/snap/")
                // Particiones de arranque: no son espacio para los servicios.
                || mount_point == "/boot"
                || mount_point.starts_with("/boot/")
            {
                return None;
            }
            Some(DiskInfo {
                mount_point,
                device: disk.name().to_string_lossy().into_owned(),
                file_system,
                kind: match disk.kind() {
                    sysinfo::DiskKind::SSD => DiskKind::Ssd,
                    sysinfo::DiskKind::HDD => DiskKind::Hdd,
                    sysinfo::DiskKind::Unknown(_) => DiskKind::Unknown,
                },
                total_bytes: disk.total_space(),
                available_bytes: disk.available_space(),
                read_only: disk.is_read_only(),
            })
        })
        .collect();
    // Un mismo dispositivo montado varias veces (bind mounts): el punto más corto.
    list.sort_by(|a, b| {
        a.device
            .cmp(&b.device)
            .then(a.mount_point.len().cmp(&b.mount_point.len()))
    });
    list.dedup_by(|later, first| later.device == first.device);
    list.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    list.truncate(MAX_DISKS);
    list
}

fn network() -> Vec<NetworkInterface> {
    let networks = Networks::new_with_refreshed_list();
    let mut list: Vec<NetworkInterface> = networks
        .list()
        .iter()
        .filter(|(name, _)| {
            name.as_str() != "lo"
                && !VIRTUAL_INTERFACES
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
        })
        .filter_map(|(name, data)| {
            let addresses: Vec<String> = data
                .ip_networks()
                .iter()
                .filter(|network| !link_local(&network.addr) && !network.addr.is_loopback())
                .map(|network| format!("{}/{}", network.addr, network.prefix))
                .collect();
            (!addresses.is_empty()).then(|| NetworkInterface {
                name: name.clone(),
                addresses,
                received_bytes: data.total_received(),
                transmitted_bytes: data.total_transmitted(),
            })
        })
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list.truncate(MAX_INTERFACES);
    list
}

fn link_local(addr: &IpAddr) -> bool {
    match addr {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => (v6.segments().first().copied().unwrap_or(0) & 0xffc0) == 0xfe80,
    }
}

/// Zona horaria del enlace `/etc/localtime` (`…/zoneinfo/Europe/Madrid`) o de `/etc/timezone`.
fn timezone() -> Option<String> {
    fs::read_link("/etc/localtime")
        .ok()
        .and_then(|target| {
            let target = target.to_string_lossy().into_owned();
            target
                .split_once("zoneinfo/")
                .map(|(_, zone)| zone.to_owned())
        })
        .or_else(|| {
            fs::read_to_string("/etc/timezone")
                .ok()
                .map(|zone| zone.trim().to_owned())
        })
        .filter(|zone| !zone.is_empty() && zone.len() <= 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombre_del_sistema() {
        let text = "NAME=\"Ubuntu\"\nPRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\nID=ubuntu\n";
        assert_eq!(
            parse_os_release(text).as_deref(),
            Some("Ubuntu 24.04.1 LTS")
        );
        assert_eq!(parse_os_release("ID=x\n"), None);
    }

    #[test]
    fn nombres_pci() {
        let ids = "# comentario\n\
                   1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\
                   \t73bf  Navi 21 [Radeon RX 6800/6800 XT / 6900 XT]\n\
                   8086  Intel Corporation\n\
                   \t0042  Core Processor Integrated Graphics Controller\n\
                   \t\t1028 040a  Latitude E6410\n\
                   \t46a8  Alder Lake-P GT2 [Iris Xe Graphics]\n\
                   80ee  InnoTek Systemberatung GmbH\n\
                   \t46a8  Otro\n";
        assert_eq!(
            pci_names(ids, "8086", "46a8"),
            (
                Some("Intel Corporation".to_owned()),
                Some("Alder Lake-P GT2 [Iris Xe Graphics]".to_owned())
            )
        );
        // El dispositivo de otro fabricante no cuenta.
        assert_eq!(pci_names(ids, "8086", "9999").1, None);
        assert_eq!(pci_names(ids, "1234", "0001"), (None, None));
    }

    #[test]
    fn equipo_sin_textos_de_relleno() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("sys_vendor"), "Hetzner\n").unwrap();
        fs::write(dir.path().join("product_name"), "vServer\n").unwrap();
        assert_eq!(machine(dir.path()).as_deref(), Some("Hetzner vServer"));
        fs::write(dir.path().join("product_name"), "To be filled by O.E.M.\n").unwrap();
        assert_eq!(machine(dir.path()).as_deref(), Some("Hetzner"));
        fs::write(dir.path().join("sys_vendor"), "QEMU\n").unwrap();
        fs::write(
            dir.path().join("product_name"),
            "QEMU Standard PC (Q35 + ICH9, 2009)\n",
        )
        .unwrap();
        assert_eq!(
            machine(dir.path()).as_deref(),
            Some("QEMU Standard PC (Q35 + ICH9, 2009)")
        );
    }

    #[test]
    fn enlace_local() {
        assert!(link_local(&"fe80::1".parse().unwrap()));
        assert!(link_local(&"169.254.1.1".parse().unwrap()));
        assert!(!link_local(&"2001:db8::1".parse().unwrap()));
        assert!(!link_local(&"203.0.113.7".parse().unwrap()));
    }

    /// Contra la máquina real: lo esencial siempre está.
    #[tokio::test]
    async fn datos_de_esta_maquina() {
        let info = HostProbe::default().info().await;
        assert!(info.cpu.logical_cores >= 1);
        assert!(info.memory.total_bytes > 0);
        assert!(info.uptime_secs > 0);
        assert!(!info.os.architecture.is_empty());
        assert!(
            info.disks.iter().any(|disk| disk.mount_point == "/"),
            "{:?}",
            info.disks
        );
    }
}
