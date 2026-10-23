//! Punto de entrada. Los fallos de arranque salen con códigos de `sysexits.h` para que systemd
//! no reintente lo que no se arregla solo (`RestartPreventExitStatus=` en deploy/).

use std::{process::ExitCode, sync::Arc};

use agent::{
    config::Config,
    db::Database,
    deploy::{Paths, runner, units},
    error::DisplayChain,
    server,
    systemd::SYSTEMCTL_PATH,
    telemetry,
};

const EX_USAGE: u8 = 64;
const EX_SOFTWARE: u8 = 70;
const EX_CANTCREAT: u8 = 73;
const EX_NOPERM: u8 = 77;
const EX_CONFIG: u8 = 78;

fn main() -> ExitCode {
    if rustix::process::geteuid().is_root() {
        eprintln!(
            "blacksmith-agent: no debe ejecutarse como root; use el usuario de servicio `blacksmith`"
        );
        return ExitCode::from(EX_NOPERM);
    }

    // Lo que ejecutan las plantillas de systemd (deploy/systemd/): sin configuración del agente.
    match std::env::args().nth(1).as_deref() {
        Some("run-deploy") => return runner::run_deploy(),
        Some("run-service") => return runner::run_service(),
        Some("write-units") => return write_units(std::env::args().nth(2)),
        Some(other) => {
            eprintln!(
                "blacksmith-agent: orden desconocida «{other}» (run-deploy, run-service, write-units <dir>)"
            );
            return ExitCode::from(EX_USAGE);
        }
        None => {}
    }

    // El entorno se carga antes de crear el runtime: modificar variables con otros hilos vivos
    // no es seguro.
    #[cfg(debug_assertions)]
    if let Err(error) = load_dotenv() {
        eprintln!("blacksmith-agent: no se pudo cargar .env: {error}");
        return ExitCode::from(EX_CONFIG);
    }

    let config = match Config::from_env() {
        Ok(config) => Arc::new(config),
        Err(error) => {
            eprintln!("blacksmith-agent: {error}");
            return ExitCode::from(EX_CONFIG);
        }
    };

    if let Err(error) = telemetry::init(&config) {
        eprintln!("blacksmith-agent: {}", DisplayChain(&error));
        return ExitCode::from(EX_CONFIG);
    }

    // Antes del runtime: abrir es síncrono y, si falla, no hay nada que servir.
    let db = match Database::open(&config.database_path) {
        Ok(db) => db,
        Err(error) => {
            tracing::error!(
                path = %config.database_path.display(),
                error = %DisplayChain(&error),
                "no se pudo abrir la base de datos"
            );
            return ExitCode::from(if error.is_configuration() {
                EX_CONFIG
            } else {
                EX_CANTCREAT
            });
        }
    };

    let paths = match Paths::for_scope(config.systemd_scope) {
        Ok(paths) => paths,
        Err(error) => {
            tracing::error!(%error, "no se pudieron determinar las rutas de los servicios");
            return ExitCode::from(EX_CONFIG);
        }
    };
    if let Some(units_dir) = &paths.units_dir {
        install_user_units(units_dir);
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.worker_threads)
        .max_blocking_threads(config.max_blocking_threads)
        .thread_name("blacksmith-rt")
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "no se pudo crear el runtime de tokio");
            return ExitCode::from(EX_SOFTWARE);
        }
    };

    match runtime.block_on(server::run(config, db, paths)) {
        Ok(()) => {
            tracing::info!("agente detenido");
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(error = %DisplayChain(&error), "el agente terminó con error");
            ExitCode::FAILURE
        }
    }
}

/// Solo en builds de desarrollo, y solo el `.env` del directorio actual: `dotenvy::dotenv()`
/// también buscaría en los directorios padre. Las variables ya exportadas tienen prioridad.
/// En release no se lee ningún `.env`: el entorno lo pone systemd (`EnvironmentFile=`).
#[cfg(debug_assertions)]
fn load_dotenv() -> Result<(), dotenvy::Error> {
    match dotenvy::from_path(".env") {
        Err(error) if error.not_found() => Ok(()),
        result => result,
    }
}

/// Modo local: instala las plantillas en el systemd del usuario apuntando a este binario. Un fallo
/// no impide arrancar (estado, logs y registro funcionan igual), pero se registra.
fn install_user_units(units_dir: &std::path::Path) {
    let binary = match std::env::current_exe() {
        Ok(binary) => binary,
        Err(error) => {
            tracing::error!(%error, "no se pudo localizar el binario del agente");
            return;
        }
    };
    match units::install_user_units(units_dir, &binary) {
        Ok(false) => {}
        Ok(true) => {
            let reloaded = std::process::Command::new(SYSTEMCTL_PATH)
                .args(["--user", "daemon-reload"])
                .status();
            match reloaded {
                Ok(status) if status.success() => {
                    tracing::info!(dir = %units_dir.display(), "plantillas de systemd instaladas (modo local)");
                }
                other => tracing::error!(?other, "no se pudo recargar el systemd del usuario"),
            }
        }
        Err(error) => {
            tracing::error!(%error, dir = %units_dir.display(), "no se pudieron instalar las plantillas")
        }
    }
}

/// `write-units <dir>`: escribe las plantillas del servidor (las de `deploy/systemd/`).
fn write_units(dir: Option<String>) -> ExitCode {
    let Some(dir) = dir else {
        eprintln!("blacksmith-agent: uso: write-units <directorio>");
        return ExitCode::from(EX_USAGE);
    };
    for (name, content) in units::render(agent::systemd::Scope::System, units::SYSTEM_BINARY) {
        let path = std::path::Path::new(&dir).join(name);
        if let Err(error) = std::fs::write(&path, content) {
            eprintln!("blacksmith-agent: {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
