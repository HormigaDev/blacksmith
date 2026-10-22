//! Consumo de un proceso (RSS y CPU) con `sysinfo`.
//!
//! El porcentaje de CPU necesita dos muestras: milisegundos de CPU que el proceso consumió entre
//! ambas (`accumulated_cpu_time`, de `/proc/<pid>/stat`), divididos por el tiempo real
//! transcurrido. Se calcula aquí y no con `Process::cpu_usage`, que depende del contador global
//! del `System` (desfasado si se miden varios PID en momentos distintos) y da 0 en la primera
//! medición de un proceso recién creado.
//!
//! Eficiencia:
//! - Un único `System` y solo se refresca el PID pedido: se leen `/proc/<pid>/stat` y `statm`,
//!   nada de hilos, entorno ni rutas.
//! - Con una muestra reciente (consultas periódicas de la interfaz) basta una lectura, sin
//!   esperar. Sin ella, se toma una de referencia y se espera [`MIN_INTERVAL`] con
//!   `tokio::time::sleep`, sin ocupar ningún hilo.
//! - Las lecturas de `/proc` (síncronas) van en `spawn_blocking`.
//! - Como mucho [`MAX_TRACKED`] procesos en memoria; los que no se consultan se olvidan.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use rustix::process::Pid;
use shared::ProcessMetrics;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

/// Ventana mínima entre muestras. El kernel cuenta la CPU en ticks de 10 ms: con 200 ms el
/// error es de ±5 puntos; con consultas periódicas (cada segundo o más), mucho menor.
pub const MIN_INTERVAL: Duration = Duration::from_millis(200);
/// Más allá, el porcentaje sería la media de un intervalo largo, no el uso actual.
const MAX_SAMPLE_AGE: Duration = Duration::from_secs(30);
/// Un proceso sin consultas durante este tiempo se olvida.
const FORGET_AFTER: Duration = Duration::from_secs(300);
const MAX_TRACKED: usize = 64;
const BYTES_PER_MIB: f64 = 1024.0 * 1024.0;

#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("el estado de las métricas quedó inutilizable tras un pánico")]
    Poisoned,
    #[error("la tarea de medición se interrumpió")]
    Task(#[from] tokio::task::JoinError),
}

/// Última lectura de un proceso.
struct Sample {
    at: Instant,
    cpu_ms: u64,
    /// Hora de arranque del proceso: si cambia, el PID se reutilizó para otro proceso.
    start_time: u64,
    /// `None` tras la muestra de referencia: aún no hay porcentaje de CPU.
    metrics: Option<ProcessMetrics>,
}

struct Tracker {
    system: System,
    samples: HashMap<u32, Sample>,
}

impl Default for Tracker {
    fn default() -> Self {
        Self {
            system: System::new(),
            samples: HashMap::new(),
        }
    }
}

enum Step {
    /// Medición lista; `None` si el proceso ya no existe.
    Done(Option<ProcessMetrics>),
    /// Se tomó la muestra de referencia: falta la segunda.
    Baseline,
}

/// Barato de clonar: comparte el estado entre peticiones.
#[derive(Clone, Default)]
pub struct ProcessSampler {
    tracker: Arc<Mutex<Tracker>>,
}

impl std::fmt::Debug for ProcessSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessSampler").finish_non_exhaustive()
    }
}

impl ProcessSampler {
    /// RSS y CPU actual del proceso, o `None` si no existe (o terminó durante la medición).
    pub async fn sample(&self, pid: Pid) -> Result<Option<ProcessMetrics>, MetricsError> {
        // Un `Pid` es positivo: la conversión no pierde nada.
        let pid = pid.as_raw_nonzero().get().unsigned_abs();
        if let Step::Done(metrics) = self.step(pid).await? {
            return Ok(metrics);
        }
        tokio::time::sleep(MIN_INTERVAL).await;
        match self.step(pid).await? {
            Step::Done(metrics) => Ok(metrics),
            // Solo si el PID pasó a otro proceso justo en medio: no hay dato fiable.
            Step::Baseline => Ok(None),
        }
    }

    async fn step(&self, pid: u32) -> Result<Step, MetricsError> {
        let tracker = Arc::clone(&self.tracker);
        tokio::task::spawn_blocking(move || {
            let mut tracker = tracker.lock().map_err(|_| MetricsError::Poisoned)?;
            Ok(tracker.step(pid, Instant::now()))
        })
        .await?
    }
}

impl Tracker {
    fn step(&mut self, pid: u32, now: Instant) -> Step {
        self.forget(now);

        if let Some(previous) = self.samples.get(&pid)
            && now.duration_since(previous.at) < MIN_INTERVAL
        {
            // Demasiado pronto para una muestra nueva: sirve la última, si ya tiene CPU.
            return match previous.metrics {
                Some(metrics) => Step::Done(Some(metrics)),
                None => Step::Baseline,
            };
        }

        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
            true,
            ProcessRefreshKind::nothing().with_memory().with_cpu(),
        );
        let Some(process) = self.system.process(sysinfo::Pid::from_u32(pid)) else {
            self.samples.remove(&pid);
            return Step::Done(None);
        };
        let (cpu_ms, start_time, rss_bytes) = (
            process.accumulated_cpu_time(),
            process.start_time(),
            process.memory(),
        );

        let metrics = self.samples.get(&pid).and_then(|previous| {
            let elapsed = now.duration_since(previous.at);
            (previous.start_time == start_time && elapsed <= MAX_SAMPLE_AGE).then(|| {
                let used = cpu_ms.saturating_sub(previous.cpu_ms) as f64;
                ProcessMetrics {
                    rss_bytes,
                    rss_mb: one_decimal(rss_bytes as f64 / BYTES_PER_MIB),
                    cpu_percent: one_decimal(used / elapsed.as_secs_f64() / 10.0),
                }
            })
        });

        if !self.samples.contains_key(&pid) && self.samples.len() >= MAX_TRACKED {
            self.forget_oldest();
        }
        self.samples.insert(
            pid,
            Sample {
                at: now,
                cpu_ms,
                start_time,
                metrics,
            },
        );
        match metrics {
            Some(metrics) => Step::Done(Some(metrics)),
            None => Step::Baseline,
        }
    }

    /// Descarta las muestras viejas y, si el `System` acumula procesos que ya no se siguen,
    /// lo reinicia: `sysinfo` no permite quitar procesos vivos de su lista.
    fn forget(&mut self, now: Instant) {
        self.samples
            .retain(|_, sample| now.duration_since(sample.at) < FORGET_AFTER);
        if self.system.processes().len() > MAX_TRACKED * 2 {
            self.system = System::new();
            self.samples.clear();
        }
    }

    fn forget_oldest(&mut self) {
        if let Some(oldest) = self
            .samples
            .iter()
            .min_by_key(|(_, sample)| sample.at)
            .map(|(pid, _)| *pid)
        {
            self.samples.remove(&oldest);
        }
    }
}

fn one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use std::process::{Child, Command};

    use super::*;

    struct Spawned(Child);

    impl Spawned {
        fn pid(&self) -> Pid {
            Pid::from_raw(i32::try_from(self.0.id()).unwrap()).unwrap()
        }
    }

    impl Drop for Spawned {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn spawn(script: &str) -> Spawned {
        Spawned(
            Command::new("/bin/sh")
                .args(["-c", script])
                .spawn()
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn proceso_ocioso() {
        let child = spawn("exec /bin/sleep 30");
        let sampler = ProcessSampler::default();
        let metrics = sampler.sample(child.pid()).await.unwrap().unwrap();
        assert!(metrics.rss_bytes > 0);
        assert!(metrics.rss_mb < 50.0, "{metrics:?}");
        assert!(metrics.cpu_percent < 5.0, "{metrics:?}");
    }

    #[tokio::test]
    async fn proceso_que_consume_un_nucleo() {
        let child = spawn("while :; do :; done");
        let sampler = ProcessSampler::default();
        // Primera consulta: referencia + 200 ms.
        let first = sampler.sample(child.pid()).await.unwrap().unwrap();
        assert!(first.cpu_percent > 50.0, "{first:?}");
        // Consulta periódica: una sola lectura, sin esperar.
        tokio::time::sleep(Duration::from_millis(300)).await;
        let start = Instant::now();
        let second = sampler.sample(child.pid()).await.unwrap().unwrap();
        assert!(start.elapsed() < MIN_INTERVAL, "{:?}", start.elapsed());
        assert!(
            second.cpu_percent > 50.0 && second.cpu_percent <= 110.0,
            "{second:?}"
        );
    }

    /// Medir otro PID en medio no altera el porcentaje (cada uno tiene su propio `System`).
    #[tokio::test]
    async fn procesos_independientes() {
        let busy = spawn("while :; do :; done");
        let idle = spawn("exec /bin/sleep 30");
        let sampler = ProcessSampler::default();
        sampler.sample(busy.pid()).await.unwrap();
        for _ in 0..3 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            sampler.sample(idle.pid()).await.unwrap();
        }
        let busy_metrics = sampler.sample(busy.pid()).await.unwrap().unwrap();
        assert!(
            busy_metrics.cpu_percent > 50.0 && busy_metrics.cpu_percent <= 110.0,
            "{busy_metrics:?}"
        );
    }

    #[tokio::test]
    async fn proceso_inexistente() {
        let mut child = spawn("exit 0");
        let pid = Pid::from_raw(i32::try_from(child.0.id()).unwrap()).unwrap();
        child.0.wait().unwrap();
        let sampler = ProcessSampler::default();
        assert_eq!(sampler.sample(pid).await.unwrap(), None);
        assert!(sampler.tracker.lock().unwrap().samples.is_empty());
    }

    /// Una segunda consulta justo después de la muestra de referencia no recibe un 0 % falso.
    #[tokio::test]
    async fn consultas_simultaneas() {
        let child = spawn("while :; do :; done");
        let sampler = ProcessSampler::default();
        let (a, b) = tokio::join!(sampler.sample(child.pid()), sampler.sample(child.pid()));
        for metrics in [a.unwrap().unwrap(), b.unwrap().unwrap()] {
            assert!(metrics.cpu_percent > 50.0, "{metrics:?}");
        }
    }

    #[test]
    fn olvida_procesos_y_respeta_el_limite() {
        let mut tracker = Tracker::default();
        let me = std::process::id();
        let start = Instant::now();
        assert!(matches!(tracker.step(me, start), Step::Baseline));
        // Tras FORGET_AFTER sin consultas, se descarta y vuelve a tomar referencia.
        let later = start + FORGET_AFTER + Duration::from_secs(1);
        assert!(matches!(tracker.step(me, later), Step::Baseline));
        assert_eq!(tracker.samples.len(), 1);

        for fake in 0..MAX_TRACKED {
            tracker.samples.insert(
                u32::MAX - u32::try_from(fake).unwrap(),
                Sample {
                    at: later,
                    cpu_ms: 0,
                    start_time: 0,
                    metrics: None,
                },
            );
        }
        tracker.samples.remove(&me);
        assert!(matches!(tracker.step(me, later), Step::Baseline));
        assert_eq!(tracker.samples.len(), MAX_TRACKED);
    }

    #[test]
    fn un_pid_reutilizado_no_mezcla_procesos() {
        let mut tracker = Tracker::default();
        let me = std::process::id();
        let start = Instant::now();
        tracker.step(me, start);
        // Simula que el PID era de otro proceso (otra hora de arranque).
        tracker.samples.get_mut(&me).unwrap().start_time += 1;
        let next = start + MIN_INTERVAL + Duration::from_millis(1);
        assert!(matches!(tracker.step(me, next), Step::Baseline));
    }
}
