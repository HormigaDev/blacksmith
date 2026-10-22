//! Punto de entrada. Los fallos de arranque salen con códigos de `sysexits.h` para que systemd
//! no reintente lo que no se arregla solo (`RestartPreventExitStatus=` en deploy/).

use std::{process::ExitCode, sync::Arc};

use agent::{config::Config, db::Database, error::DisplayChain, server, telemetry};

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

    match runtime.block_on(server::run(config, db)) {
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
