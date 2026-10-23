//! Comandos para hablar con el agente de un servidor: HTTP (`agent_request`) y logs en tiempo
//! real (`logs_follow` / `logs_stop`). Contrato en TypeScript: client/src/lib/agent/.
//!
//! La webview elige el servidor (por su id) y la ruta de la API; la dirección la resuelve Rust
//! (túnel o agente local), así que estos comandos no sirven para llegar a ninguna otra parte.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use futures_util::StreamExt;
use serde::Deserialize;
use shared::{LogEndReason, LogEvent};
use tauri::{ipc::Channel, State};
use tokio::sync::{oneshot, Mutex};
use tokio_tungstenite::tungstenite::{self, protocol::WebSocketConfig, Message};

use crate::{
    agent_http::{self, AgentResponse, Method},
    commands::AppState,
    error::{CommandError, CommandResult},
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Una entrada del journal ocupa como mucho ~256 KiB en el agente; con su JSON, menos de 1 MiB.
const MAX_LOG_MESSAGE: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRequest {
    server_id: String,
    method: Method,
    /// Ruta con query, p. ej. `/api/v1/services`.
    path: String,
    body: Option<String>,
    request_id: Option<String>,
}

/// Petición HTTP al agente del servidor. Los errores HTTP del agente no son errores del comando:
/// la respuesta vuelve tal cual (estado y cuerpo) y la interpreta el cliente HTTP de la interfaz.
#[tauri::command]
pub async fn agent_request(
    state: State<'_, AppState>,
    request: AgentRequest,
) -> CommandResult<AgentResponse> {
    let addr = state.agent_addr(&request.server_id).await?;
    agent_http::request(
        addr,
        request.method,
        &request.path,
        request.body.as_deref(),
        request.request_id.as_deref(),
        agent_http::REQUEST_TIMEOUT,
    )
    .await
}

/// Visores de logs abiertos: id → aviso de parada.
#[derive(Default)]
pub struct LogFollows {
    next: AtomicU32,
    stops: Arc<Mutex<HashMap<u32, oneshot::Sender<()>>>>,
}

impl LogFollows {
    /// Para todos los visores. Al recargar la webview nadie llama a `logs_stop`, y enviar a un
    /// canal de la página anterior no falla: sin esto, cada recarga dejaría un WebSocket abierto
    /// que ocupa uno de los cupos de flujos del agente.
    pub async fn stop_all(&self) {
        for (_, stop) in self.stops.lock().await.drain() {
            let _ = stop.send(());
        }
    }
}

/// Abre el WebSocket de logs del servicio y reenvía cada evento a `on_event`. Devuelve el id para
/// `logs_stop`. Si el agente rechaza el handshake (servicio no registrado, cupo lleno…), falla
/// aquí con `AGENT_REJECTED` y el código del agente.
#[tauri::command]
pub async fn logs_follow(
    state: State<'_, AppState>,
    server_id: String,
    service: String,
    deploy: Option<bool>,
    since: Option<u64>,
    on_event: Channel<LogEvent>,
) -> CommandResult<u32> {
    // El agente valida el nombre; esto solo impide componer otra ruta.
    if service.is_empty()
        || service.len() > 48
        || !service
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(CommandError::InvalidTarget("service"));
    }
    let addr = state.agent_addr(&server_id).await?;
    // `deploy`: la salida de los despliegues (`blacksmith-deploy@<servicio>`) en lugar del servicio.
    // `since` (segundos Unix): solo desde ese instante, p. ej. el último despliegue.
    let mut query: Vec<String> = Vec::new();
    if deploy.unwrap_or(false) {
        query.push("source=deploy".into());
    }
    if let Some(since) = since {
        query.push(format!("since={since}"));
    }
    let query = if query.is_empty() {
        String::new()
    } else {
        format!("?{}", query.join("&"))
    };
    let url = format!("ws://{addr}/api/v1/services/{service}/logs/ws{query}");
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_LOG_MESSAGE))
        .max_frame_size(Some(MAX_LOG_MESSAGE));
    let connect = tokio_tungstenite::connect_async_with_config(url, Some(config), false);
    let socket = match tokio::time::timeout(CONNECT_TIMEOUT, connect).await {
        Err(_) => return Err(CommandError::ConnectionTimeout),
        Ok(Err(tungstenite::Error::Http(response))) => return Err(rejected(*response)),
        Ok(Err(_)) => return Err(CommandError::AgentUnreachable),
        Ok(Ok((socket, _))) => socket,
    };

    let id = state.log_follows.next.fetch_add(1, Ordering::Relaxed);
    let (stop, stopped) = oneshot::channel();
    let stops = Arc::clone(&state.log_follows.stops);
    stops.lock().await.insert(id, stop);
    tauri::async_runtime::spawn(async move {
        forward(socket, on_event, stopped).await;
        stops.lock().await.remove(&id);
    });
    Ok(id)
}

/// Cierra el visor. `false` si ya no estaba abierto.
#[tauri::command]
pub async fn logs_stop(state: State<'_, AppState>, id: u32) -> CommandResult<bool> {
    let stop = state.log_follows.stops.lock().await.remove(&id);
    Ok(stop.is_some_and(|stop| stop.send(()).is_ok()))
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Reenvía hasta que el agente cierre, se pida parar o la webview deje de escuchar. Los Ping del
/// agente los responde la biblioteca al leer.
async fn forward(
    mut socket: Socket,
    events: Channel<LogEvent>,
    mut stopped: oneshot::Receiver<()>,
) {
    let mut ended = false;
    loop {
        let message = tokio::select! {
            _ = &mut stopped => {
                let _ = socket.close(None).await;
                return;
            }
            message = socket.next() => message,
        };
        match message {
            Some(Ok(Message::Text(text))) => {
                let Ok(event) = serde_json::from_str::<LogEvent>(&text) else {
                    continue;
                };
                ended |= matches!(event, LogEvent::End { .. });
                if events.send(event).is_err() {
                    // La webview ya no escucha (recarga, ventana cerrada).
                    let _ = socket.close(None).await;
                    return;
                }
            }
            Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                if !ended {
                    let _ = events.send(LogEvent::End {
                        reason: LogEndReason::ConnectionLost,
                        detail: None,
                    });
                }
                return;
            }
            Some(Ok(_)) => {}
        }
    }
}

fn rejected(response: tungstenite::http::Response<Option<Vec<u8>>>) -> CommandError {
    let status = response.status().as_u16();
    CommandError::agent_rejected(status, response.body().as_deref().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use futures_util::SinkExt;
    use shared::{ErrorCode, LogEntry};
    use tauri::ipc::InvokeResponseBody;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::http;

    use super::*;

    /// Canal que guarda lo recibido, como lo vería la webview.
    fn collecting() -> (Channel<LogEvent>, Arc<StdMutex<Vec<LogEvent>>>) {
        let received = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&received);
        let channel = Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                sink.lock()
                    .unwrap()
                    .push(serde_json::from_str(&json).unwrap());
            }
            Ok(())
        });
        (channel, received)
    }

    fn entry(message: &str) -> LogEvent {
        LogEvent::Entry(LogEntry {
            timestamp_us: 1,
            priority: Some(6),
            pid: None,
            identifier: None,
            message: message.into(),
        })
    }

    /// Agente falso: envía `events` y, si `close`, cierra; si no, se queda esperando el cierre
    /// del cliente y lo avisa por `closed_by_client`.
    async fn agent(events: Vec<LogEvent>, close: bool) -> (Socket, oneshot::Receiver<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (closed_by_client, notified) = oneshot::channel();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            for event in events {
                let text = serde_json::to_string(&event).unwrap();
                socket.send(Message::Text(text.into())).await.unwrap();
            }
            if close {
                let _ = socket.close(None).await;
            } else {
                while let Some(Ok(message)) = socket.next().await {
                    if matches!(message, Message::Close(_)) {
                        let _ = closed_by_client.send(());
                        break;
                    }
                }
            }
        });
        let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/"))
            .await
            .unwrap();
        (socket, notified)
    }

    /// Al recargar la webview se paran todos los visores (nadie llamará a `logs_stop`).
    #[tokio::test]
    async fn recargar_la_webview_para_los_visores() {
        let follows = LogFollows::default();
        let (stop, stopped) = oneshot::channel();
        follows.stops.lock().await.insert(1, stop);
        follows.stop_all().await;
        assert!(stopped.await.is_ok());
        assert!(follows.stops.lock().await.is_empty());
    }

    #[tokio::test]
    async fn reenvia_y_avisa_si_se_corta_sin_fin() {
        let (socket, _) = agent(vec![entry("uno"), entry("dos")], true).await;
        let (channel, received) = collecting();
        let (_stop, stopped) = oneshot::channel();
        forward(socket, channel, stopped).await;
        assert_eq!(
            *received.lock().unwrap(),
            [
                entry("uno"),
                entry("dos"),
                LogEvent::End {
                    reason: LogEndReason::ConnectionLost,
                    detail: None
                }
            ]
        );
    }

    #[tokio::test]
    async fn con_fin_del_agente_no_inventa_otro() {
        let end = LogEvent::End {
            reason: LogEndReason::AgentShutdown,
            detail: None,
        };
        let (socket, _) = agent(vec![entry("uno"), end.clone()], true).await;
        let (channel, received) = collecting();
        let (_stop, stopped) = oneshot::channel();
        forward(socket, channel, stopped).await;
        assert_eq!(*received.lock().unwrap(), [entry("uno"), end]);
    }

    #[tokio::test]
    async fn parar_cierra_el_websocket() {
        let (socket, closed_by_client) = agent(vec![entry("uno")], false).await;
        let (channel, received) = collecting();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(forward(socket, channel, stopped));
        tokio::time::sleep(Duration::from_millis(100)).await;
        stop.send(()).unwrap();
        task.await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), closed_by_client)
            .await
            .expect("el agente debe recibir el cierre")
            .unwrap();
        assert_eq!(*received.lock().unwrap(), [entry("uno")]);
    }

    #[test]
    fn rechazo_del_handshake_con_el_codigo_del_agente() {
        let response = http::Response::builder()
            .status(503)
            .body(Some(
                br#"{"code":"LOG_STREAM_LIMIT","message":"cupo lleno"}"#.to_vec(),
            ))
            .unwrap();
        match rejected(response) {
            CommandError::AgentRejected {
                status: 503,
                code: ErrorCode::LogStreamLimit,
                message,
            } => assert_eq!(message, "cupo lleno"),
            other => panic!("{other:?}"),
        }
        let response = http::Response::builder().status(502).body(None).unwrap();
        assert!(matches!(
            rejected(response),
            CommandError::AgentRejected {
                status: 502,
                code: ErrorCode::Unknown,
                ..
            }
        ));
    }
}
