use std::io::{self, IsTerminal};

use tracing_subscriber::{EnvFilter, fmt};

use crate::{
    config::{Config, LogFormat},
    error::BoxError,
};

#[derive(Debug, thiserror::Error)]
#[error("no se pudo inicializar el registro de eventos")]
pub struct TelemetryError(#[source] BoxError);

/// Los eventos van a stderr; bajo systemd terminan en journald.
pub fn init(config: &Config) -> Result<(), TelemetryError> {
    let filter =
        EnvFilter::try_new(&config.log_filter).map_err(|error| TelemetryError(error.into()))?;
    let builder = fmt()
        .with_env_filter(filter)
        .with_writer(io::stderr)
        .with_ansi(io::stderr().is_terminal());

    match config.log_format {
        LogFormat::Json => builder
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(false)
            .try_init(),
        LogFormat::Pretty => builder.pretty().try_init(),
        LogFormat::Compact => builder.compact().try_init(),
    }
    .map_err(TelemetryError)
}
