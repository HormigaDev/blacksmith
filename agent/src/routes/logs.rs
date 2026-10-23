//! `GET /api/v1/services/{name}/logs/ws`: logs del servicio en tiempo real por WebSocket.
//!
//! Antes del handshake se comprueba todo lo que puede fallar (nombre, registro, cupo y arranque
//! de `journalctl`), para responder con el error JSON de siempre. Después, cada mensaje de texto
//! es un [`LogEvent`]: primero las últimas [`LOG_BACKLOG_LINES`] entradas y luego las nuevas.
//!
//! - `journalctl` se lee en su propia tarea y pasa las entradas por un canal acotado: si el
//!   cliente lee despacio, el canal se llena, la tarea deja de leer y `journalctl` se bloquea.
//!   La memoria no crece; si un envío tarda más de 10 s, la sesión se corta.
//! - Ping cada 20 s; sin noticias del cliente en 45 s se da por muerto ([`LogTimings`]). Los
//!   Ping del cliente los responde la propia biblioteca.
//! - El cliente no envía órdenes: sus mensajes solo cuentan como señal de vida.
//! - Al terminar, por la causa que sea, se suelta el flujo (mata `journalctl`) y el cupo.

use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{
        Query, State,
        rejection::QueryRejection,
        ws::{
            CloseFrame, Message, WebSocket, WebSocketUpgrade, close_code,
            rejection::WebSocketUpgradeRejection,
        },
    },
    response::Response,
};
use shared::{LOG_BACKLOG_LINES, LogEndReason, LogEntry, LogEvent};
use tokio::{
    sync::{OwnedSemaphorePermit, mpsc, watch},
    time::Instant,
};

use crate::{
    error::{AppError, AppResult, DisplayChain},
    http::Path,
    services::ServiceSlug,
    state::AppState,
    systemd::{JournalError, LogStream, UnitName},
};

/// Tiempos de la sesión. Los valores por defecto son los de producción.
#[derive(Debug, Clone, Copy)]
pub struct LogTimings {
    pub ping_interval: Duration,
    /// Sin ningún mensaje del cliente (ni Pong) durante este tiempo, se da por muerto.
    pub peer_timeout: Duration,
    /// Máximo para enviar un mensaje; si se supera, el cliente no lee lo bastante rápido.
    pub send_timeout: Duration,
}

impl Default for LogTimings {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(20),
            peer_timeout: Duration::from_secs(45),
            send_timeout: Duration::from_secs(10),
        }
    }
}
/// Entradas en tránsito entre `journalctl` y el socket.
const CHANNEL_CAPACITY: usize = 64;
/// El cliente no tiene nada que enviar salvo señales de vida.
const MAX_INCOMING: usize = 4 * 1024;
/// Lo que puede acumularse sin enviar antes de que el envío falle.
const MAX_WRITE_BUFFER: usize = 1024 * 1024;

/// `?source=deploy`: los logs de `blacksmith-deploy@<nombre>` (salida de los despliegues) en lugar de
/// los del servicio.
#[derive(Debug, Default, serde::Deserialize)]
pub struct LogsQuery {
    #[serde(default)]
    source: LogSource,
    /// Solo desde este instante (segundos Unix): p. ej. el inicio del último despliegue.
    since: Option<u64>,
}

/// Con `since`, más margen que el historial normal: la salida entera de un despliegue (el visor
/// del cliente guarda como mucho 2000 líneas).
const SINCE_MAX_LINES: u16 = 2000;

#[derive(Debug, Default, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogSource {
    #[default]
    Service,
    Deploy,
}

pub async fn stream(
    State(state): State<AppState>,
    Path(slug): Path<ServiceSlug>,
    query: Result<Query<LogsQuery>, QueryRejection>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> AppResult<Response> {
    let upgrade = upgrade?;
    let Query(query) =
        query.map_err(|rejection| AppError::BadRequest(rejection.body_text().into()))?;
    state.services.unit_of(&slug).await?;
    // Un despliegue son dos unidades: el clon y la preparación.
    let units = match query.source {
        LogSource::Service => vec![slug.unit()],
        LogSource::Deploy => vec![slug.fetch_unit(), slug.deploy_unit()],
    };
    let permit = state
        .log_streams
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::LogStreamLimit(state.config.max_log_streams))?;
    let stream = match query.since {
        Some(since) => state.journal.follow_since(&units, since, SINCE_MAX_LINES)?,
        None => state.journal.follow(&units, LOG_BACKLOG_LINES)?,
    };
    let shutdown = state.shutdown.subscribe();
    let timings = state.log_timings;

    Ok(upgrade
        .max_message_size(MAX_INCOMING)
        .max_frame_size(MAX_INCOMING)
        .max_write_buffer_size(MAX_WRITE_BUFFER)
        .on_failed_upgrade(|error| tracing::debug!(%error, "handshake WebSocket fallido"))
        .on_upgrade(move |socket| session(socket, stream, shutdown, permit, units, timings)))
}

/// Cómo terminó la sesión.
enum Ending {
    Journal(Option<JournalError>),
    Shutdown,
    TooSlow,
    /// El cliente cerró, se cortó la conexión o dejó de responder: no hay a quién avisar.
    ClientGone(&'static str),
}

async fn session(
    mut socket: WebSocket,
    stream: LogStream,
    mut shutdown: watch::Receiver<bool>,
    _permit: OwnedSemaphorePermit,
    units: Vec<UnitName>,
    timings: LogTimings,
) {
    tracing::info!(?units, "flujo de logs abierto");
    let (entries, mut received) = mpsc::channel(CHANNEL_CAPACITY);
    let reader = tokio::spawn(read_journal(stream, entries));

    let mut ping = tokio::time::interval_at(
        Instant::now() + timings.ping_interval,
        timings.ping_interval,
    );
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_seen = Instant::now();

    let ending = loop {
        tokio::select! {
            item = received.recv() => match item {
                Some(Ok(entry)) => {
                    if !send(&mut socket, &LogEvent::Entry(entry), timings.send_timeout).await {
                        break Ending::TooSlow;
                    }
                }
                Some(Err(error)) => break Ending::Journal(Some(error)),
                None => break Ending::Journal(None),
            },
            message = socket.recv() => match message {
                Some(Ok(Message::Close(_))) | None => break Ending::ClientGone("cerrado por el cliente"),
                Some(Ok(_)) => last_seen = Instant::now(),
                Some(Err(_)) => break Ending::ClientGone("conexión rota"),
            },
            _ = ping.tick() => {
                if last_seen.elapsed() > timings.peer_timeout {
                    break Ending::ClientGone("sin respuesta al ping");
                }
                let sent = tokio::time::timeout(timings.send_timeout, socket.send(Message::Ping(Bytes::new()))).await;
                if !matches!(sent, Ok(Ok(()))) {
                    break Ending::TooSlow;
                }
            },
            () = stopping(&mut shutdown) => break Ending::Shutdown,
        }
    };

    // Suelta el flujo: mata `journalctl` aunque la tarea esté esperando una línea.
    reader.abort();

    let (reason, detail, code) = match ending {
        Ending::Journal(error) => (
            LogEndReason::JournalClosed,
            error.map(|error| DisplayChain(&error).to_string()),
            close_code::ERROR,
        ),
        Ending::Shutdown => (LogEndReason::AgentShutdown, None, close_code::AWAY),
        Ending::TooSlow => (LogEndReason::ClientTooSlow, None, close_code::POLICY),
        Ending::ClientGone(why) => {
            tracing::info!(?units, motivo = why, "flujo de logs cerrado");
            return;
        }
    };
    tracing::info!(
        ?units,
        ?reason,
        detail = detail.as_deref(),
        "flujo de logs cerrado"
    );
    // Mejor esfuerzo: si el cliente ya no lee, no se insiste.
    if !matches!(reason, LogEndReason::ClientTooSlow) {
        let _ = send(
            &mut socket,
            &LogEvent::End { reason, detail },
            timings.send_timeout,
        )
        .await;
    }
    let close = Message::Close(Some(CloseFrame {
        code,
        reason: "".into(),
    }));
    let _ = tokio::time::timeout(timings.send_timeout, socket.send(close)).await;
}

/// Lee `journalctl` hasta que termine, falle o la sesión deje de escuchar. Si la sesión se va,
/// `closed()` gana el `select!` y el flujo se suelta (matando el proceso).
async fn read_journal(
    mut stream: LogStream,
    entries: mpsc::Sender<Result<LogEntry, JournalError>>,
) {
    loop {
        let item = tokio::select! {
            () = entries.closed() => return,
            item = stream.next() => item,
        };
        let Some(item) = item else { return };
        let last = item.is_err();
        if entries.send(item).await.is_err() || last {
            return;
        }
    }
}

/// Espera al apagado. La guarda que devuelve `wait_for` no es `Send`: se suelta aquí dentro.
async fn stopping(shutdown: &mut watch::Receiver<bool>) {
    let _ = shutdown.wait_for(|stopping| *stopping).await;
}

/// `false` si no se pudo enviar a tiempo.
async fn send(socket: &mut WebSocket, event: &LogEvent, timeout: Duration) -> bool {
    let Ok(text) = serde_json::to_string(event) else {
        return true;
    };
    matches!(
        tokio::time::timeout(timeout, socket.send(Message::Text(text.into()))).await,
        Ok(Ok(()))
    )
}
