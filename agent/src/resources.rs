//! Consumo de un servicio a partir de los contadores del cgroup de su unidad (systemd): todos sus
//! procesos, no solo el principal (`npm start` → `node`, un servidor con trabajadores…).
//!
//! - El porcentaje de CPU necesita dos lecturas de `CPUUsageNSec`: la interfaz consulta cada
//!   pocos segundos, así que suele bastar con la anterior; si no hay una reciente, se toma otra
//!   tras [`SECOND_READING`] (con `tokio::time::sleep`, sin ocupar hilos).
//! - Memoria: el *working set* (como cAdvisor/Kubernetes), `MemoryCurrent` menos la caché de
//!   archivos inactiva (`inactive_file` de `memory.stat`), que el kernel libera cuando hace falta.
//!   `MemoryCurrent` a secas la incluye y puede triplicar lo que el servicio usa de verdad.
//! - El tiempo en marcha usa el reloj monotónico, el mismo de `ActiveEnterTimestampMonotonic`.
//! - La capacidad del servidor (núcleos, RAM, swap) se lee una sola vez.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use shared::{HostCapacity, ServiceResources};

use crate::systemd::{Systemctl, UnitName, UnitResources};

const SECOND_READING: Duration = Duration::from_millis(250);
/// Más allá, el porcentaje sería la media de un intervalo largo, no el uso actual.
const MAX_SAMPLE_AGE: Duration = Duration::from_secs(30);
const MAX_TRACKED: usize = 256;

/// Última lectura de CPU de cada unidad.
#[derive(Debug, Clone, Default)]
pub struct CgroupCpu {
    samples: Arc<Mutex<HashMap<String, (Instant, u64)>>>,
}

impl CgroupCpu {
    /// Consumo actual del cgroup de `unit`, con `resources` leídos de systemd en `read_at`.
    pub async fn resources(
        &self,
        systemctl: &Systemctl,
        unit: &UnitName,
        resources: UnitResources,
        read_at: Instant,
        control_group: Option<&str>,
        running: bool,
    ) -> ServiceResources {
        let memory = match (running, resources.memory) {
            (true, Some(current)) => Some(match control_group {
                Some(group) => current.saturating_sub(inactive_file(group).await.unwrap_or(0)),
                None => current,
            }),
            _ => None,
        };
        let cpu_percent = match (running, resources.cpu_nsec) {
            (true, Some(nsec)) => self.cpu_percent(systemctl, unit, read_at, nsec).await,
            _ => None,
        };
        ServiceResources {
            memory_bytes: memory,
            memory_peak_bytes: resources.memory_peak,
            memory_limit_bytes: resources.memory_max,
            swap_bytes: resources.swap.filter(|_| running),
            swap_limit_bytes: resources.swap_max,
            cpu_percent,
            // 1 000 000 µs por segundo = un núcleo = 100 %.
            cpu_limit_percent: resources
                .cpu_quota_us
                .map(|us| (us as f64 / 1_000.0).round() / 10.0),
            tasks: resources.tasks.filter(|_| running),
            tasks_limit: resources.tasks_max,
            restarts: resources.restarts,
            uptime_secs: resources
                .active_since_us
                .filter(|_| running)
                .and_then(|since| monotonic_us().checked_sub(since))
                .map(|elapsed| elapsed / 1_000_000),
        }
    }

    async fn cpu_percent(
        &self,
        systemctl: &Systemctl,
        unit: &UnitName,
        read_at: Instant,
        nsec: u64,
    ) -> Option<f64> {
        let previous = self.record(unit, read_at, nsec);
        if let Some(percent) = previous.and_then(|(at, before)| percent(at, before, read_at, nsec))
        {
            return Some(percent);
        }
        tokio::time::sleep(SECOND_READING).await;
        let later = systemctl.cpu_usage_nsec(unit).await.ok().flatten()?;
        let at = Instant::now();
        self.record(unit, at, later);
        percent(read_at, nsec, at, later)
    }

    /// Guarda la lectura y devuelve la anterior.
    fn record(&self, unit: &UnitName, at: Instant, nsec: u64) -> Option<(Instant, u64)> {
        let mut samples = self.samples.lock().ok()?;
        if samples.len() >= MAX_TRACKED {
            samples.retain(|_, (sampled, _)| sampled.elapsed() < MAX_SAMPLE_AGE);
        }
        samples.insert(unit.as_str().to_owned(), (at, nsec))
    }
}

/// CPU entre dos lecturas (100 = un núcleo), con un decimal. `None` si el intervalo no sirve o el
/// contador retrocedió (la unidad se reinició entre medias).
fn percent(from: Instant, before: u64, to: Instant, after: u64) -> Option<f64> {
    let elapsed = to.checked_duration_since(from)?;
    if elapsed < Duration::from_millis(100) || elapsed > MAX_SAMPLE_AGE || after < before {
        return None;
    }
    let used = (after - before) as f64 / elapsed.as_nanos() as f64 * 100.0;
    Some((used * 10.0).round() / 10.0)
}

/// Raíz de la jerarquía unificada de cgroups (v2).
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Caché de archivos inactiva del cgroup (`inactive_file` de su `memory.stat`, legible por
/// cualquier usuario). `None` si no se puede leer (cgroups v1, permisos…).
async fn inactive_file(group: &str) -> Option<u64> {
    let path = format!("{CGROUP_ROOT}{group}/memory.stat");
    let stat = tokio::task::spawn_blocking(move || std::fs::read_to_string(path))
        .await
        .ok()?
        .ok()?;
    parse_inactive_file(&stat)
}

fn parse_inactive_file(stat: &str) -> Option<u64> {
    stat.lines()
        .find_map(|line| line.strip_prefix("inactive_file "))
        .and_then(|value| value.trim().parse().ok())
}

fn monotonic_us() -> u64 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    u64::try_from(now.tv_sec).unwrap_or(0) * 1_000_000
        + u64::try_from(now.tv_nsec).unwrap_or(0) / 1_000
}

/// Núcleos, RAM y swap del servidor (no cambian mientras corre el agente).
pub fn host_capacity() -> HostCapacity {
    static HOST: OnceLock<HostCapacity> = OnceLock::new();
    *HOST.get_or_init(|| {
        let mut system = sysinfo::System::new();
        system.refresh_memory();
        HostCapacity {
            cpu_cores: std::thread::available_parallelism()
                .map_or(1, |cores| u32::try_from(cores.get()).unwrap_or(u32::MAX)),
            memory_bytes: system.total_memory(),
            swap_bytes: system.total_swap(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcentaje_entre_lecturas() {
        let from = Instant::now();
        let to = from + Duration::from_secs(2);
        // 1 s de CPU en 2 s de reloj: medio núcleo.
        assert_eq!(percent(from, 0, to, 1_000_000_000), Some(50.0));
        // Cuatro núcleos llenos: 400 %, como en top.
        assert_eq!(percent(from, 0, to, 8_000_000_000), Some(400.0));
        // Intervalo demasiado corto o largo, o contador reiniciado.
        assert_eq!(percent(from, 0, from + Duration::from_millis(10), 5), None);
        assert_eq!(percent(from, 0, from + Duration::from_secs(60), 5), None);
        assert_eq!(percent(from, 10, to, 5), None);
    }

    #[test]
    fn cache_inactiva_del_cgroup() {
        let stat = "anon 52428800\nfile 284164096\nkernel 1048576\ninactive_anon 0\n\
                    active_anon 52428800\ninactive_file 270532608\nactive_file 13631488\n";
        assert_eq!(parse_inactive_file(stat), Some(270_532_608));
        assert_eq!(parse_inactive_file("anon 1\n"), None);
    }

    #[test]
    fn capacidad_del_servidor() {
        let host = host_capacity();
        assert!(host.cpu_cores >= 1);
        assert!(host.memory_bytes > 0);
    }

    #[test]
    fn reloj_monotonico() {
        let first = monotonic_us();
        assert!(first > 0);
        assert!(monotonic_us() >= first);
    }
}
