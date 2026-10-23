//! Límites de recursos de un servicio, aplicados con `systemctl set-property` sobre su unidad.
//!
//! - systemd los guarda como drop-ins en `/etc/systemd/system.control/` (persisten tras
//!   reiniciar) y los aplica al cgroup al momento, sin reiniciar el servicio.
//! - polkit solo deja `set-property` sobre `blacksmith@*.service`, y systemd solo acepta ahí
//!   propiedades de control de recursos (no `ExecStart=` ni similares) en una unidad que no es
//!   transitoria. Aun así, el agente envía siempre la lista cerrada de [`properties`], con
//!   valores numéricos ya validados: nunca texto del cliente.
//! - Se envían todas a la vez, también las que no tienen límite (`infinity` o vacío): así se
//!   borra cualquier límite anterior.

use shared::{HostCapacity, ResourceLimits};

/// Por debajo, casi ningún proceso arranca: sería un servicio roto, no limitado.
pub const MIN_MEMORY_BYTES: u64 = 32 * 1024 * 1024;
pub const MIN_TASKS: u32 = 16;
/// `pid_max` máximo de Linux.
pub const MAX_TASKS: u32 = 4_194_304;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct InvalidLimits(String);

/// Comprueba los límites contra la capacidad del servidor.
pub fn validate(
    limits: ResourceLimits,
    host: HostCapacity,
) -> Result<ResourceLimits, InvalidLimits> {
    let invalid = |message: String| Err(InvalidLimits(message));
    if let Some(cpu) = limits.cpu_percent {
        let max = host.cpu_cores.saturating_mul(100).max(100);
        if cpu == 0 || cpu > max {
            return invalid(format!(
                "CPU entre 1 % y {max} % ({} núcleos)",
                host.cpu_cores
            ));
        }
    }
    if let Some(memory) = limits.memory_max_bytes
        && (memory < MIN_MEMORY_BYTES || (host.memory_bytes > 0 && memory > host.memory_bytes))
    {
        return invalid(format!(
            "memoria entre {} MiB y la RAM del servidor ({} MiB)",
            MIN_MEMORY_BYTES / 1024 / 1024,
            host.memory_bytes / 1024 / 1024
        ));
    }
    if let Some(swap) = limits.swap_max_bytes
        && swap > host.swap_bytes
    {
        return invalid(format!(
            "swap hasta {} MiB (la del servidor); 0 para no usarla",
            host.swap_bytes / 1024 / 1024
        ));
    }
    if let Some(tasks) = limits.tasks_max
        && !(MIN_TASKS..=MAX_TASKS).contains(&tasks)
    {
        return invalid(format!("procesos entre {MIN_TASKS} y {MAX_TASKS}"));
    }
    Ok(limits)
}

/// Propiedades de `systemctl set-property`, siempre las mismas y en el mismo orden.
pub fn properties(limits: &ResourceLimits) -> Vec<String> {
    let bytes = |value: Option<u64>| value.map_or_else(|| "infinity".to_owned(), |v| v.to_string());
    let weight = limits.priority.weight();
    vec![
        // Vacío: sin cuota.
        format!(
            "CPUQuota={}",
            limits
                .cpu_percent
                .map_or_else(String::new, |cpu| format!("{cpu}%"))
        ),
        format!("CPUWeight={weight}"),
        format!("IOWeight={weight}"),
        format!("MemoryMax={}", bytes(limits.memory_max_bytes)),
        format!("MemorySwapMax={}", bytes(limits.swap_max_bytes)),
        format!(
            "TasksMax={}",
            limits
                .tasks_max
                .map_or_else(|| "infinity".to_owned(), |tasks| tasks.to_string())
        ),
    ]
}

#[cfg(test)]
mod tests {
    use shared::Priority;

    use super::*;

    const HOST: HostCapacity = HostCapacity {
        cpu_cores: 2,
        memory_bytes: 4 * 1024 * 1024 * 1024,
        swap_bytes: 1024 * 1024 * 1024,
    };

    #[test]
    fn sin_limites_lo_borra_todo() {
        assert_eq!(
            properties(&ResourceLimits::default()),
            [
                "CPUQuota=",
                "CPUWeight=100",
                "IOWeight=100",
                "MemoryMax=infinity",
                "MemorySwapMax=infinity",
                "TasksMax=infinity"
            ]
        );
    }

    #[test]
    fn con_limites() {
        let limits = ResourceLimits {
            cpu_percent: Some(150),
            priority: Priority::Low,
            memory_max_bytes: Some(536_870_912),
            swap_max_bytes: Some(0),
            tasks_max: Some(256),
        };
        assert_eq!(validate(limits, HOST), Ok(limits));
        assert_eq!(
            properties(&limits),
            [
                "CPUQuota=150%",
                "CPUWeight=25",
                "IOWeight=25",
                "MemoryMax=536870912",
                "MemorySwapMax=0",
                "TasksMax=256"
            ]
        );
    }

    #[test]
    fn limites_fuera_de_rango() {
        let base = ResourceLimits::default();
        for limits in [
            ResourceLimits {
                cpu_percent: Some(0),
                ..base
            },
            // Dos núcleos: hasta 200 %.
            ResourceLimits {
                cpu_percent: Some(201),
                ..base
            },
            ResourceLimits {
                memory_max_bytes: Some(MIN_MEMORY_BYTES - 1),
                ..base
            },
            ResourceLimits {
                memory_max_bytes: Some(HOST.memory_bytes + 1),
                ..base
            },
            ResourceLimits {
                swap_max_bytes: Some(HOST.swap_bytes + 1),
                ..base
            },
            ResourceLimits {
                tasks_max: Some(MIN_TASKS - 1),
                ..base
            },
        ] {
            assert!(validate(limits, HOST).is_err(), "{limits:?}");
        }
        // Sin swap en el servidor, solo «sin swap» o sin límite.
        let no_swap = HostCapacity {
            swap_bytes: 0,
            ..HOST
        };
        assert!(
            validate(
                ResourceLimits {
                    swap_max_bytes: Some(0),
                    ..base
                },
                no_swap
            )
            .is_ok()
        );
    }
}
