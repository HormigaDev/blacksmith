//! `GET /api/v1/services/{name}/logs/ws` de extremo a extremo: servidor real en un puerto TCP,
//! cliente WebSocket real y un `journalctl` falso.

// Los helpers no son `#[test]`, así que `allow-unwrap-in-tests` no los cubre.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{
    fs,
    net::SocketAddr,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Duration,
};

use agent::{
    config::Config,
    db::Database,
    http,
    routes::logs::LogTimings,
    services::{ServiceName, ServiceSlug},
    state::{AppState, Shutdown},
    systemd::{Journalctl, Scope, Systemctl},
};
use futures_util::{SinkExt, StreamExt};
use shared::{ApiErrorBody, ErrorCode, LogEndReason, LogEvent};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{self, Message, client::IntoClientRequest, protocol::frame::coding::CloseCode},
};

/// La primera entrada lleva el PID de `journalctl` para comprobar después que ya no existe
/// (`exec` conserva el PID).
const FAKE: &str = r#"#!/bin/sh
for unit; do :; done
entry() { echo "{\"__REALTIME_TIMESTAMP\":\"$1\",\"MESSAGE\":\"$2\",\"PRIORITY\":\"6\"}"; }
case "$unit" in
    --unit=bbel-api.service)
        entry 1 "pid=$$"; entry 2 dos; entry 3 tres
        exec /bin/sleep 30 ;;
    --unit=bbel-fin.service) entry 1 ultima; exit 0 ;;
    --unit=bbel-falla.service) echo "Failed to open journal: Permission denied" >&2; exit 1 ;;
esac
exit 99
"#;

fn fake() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("journalctl-falso-ws");
        fs::write(&path, FAKE).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    })
}

struct Agent {
    addr: SocketAddr,
    shutdown: Shutdown,
}

/// Agente con 2 flujos como máximo y tiempos cortos.
async fn agent(timings: LogTimings) -> Agent {
    let config = Config::from_vars([
        ("BLACKSMITH_ENV", "production"),
        ("BLACKSMITH_MAX_LOG_STREAMS", "2"),
    ])
    .unwrap();
    let db = Database::open_in_memory().unwrap();
    for slug in ["api", "fin", "falla"] {
        db.insert_service(
            ServiceSlug::new(slug).unwrap(),
            ServiceName::new(slug).unwrap(),
        )
        .await
        .unwrap();
    }
    let mut state = AppState::with_commands(
        Arc::new(config),
        db,
        Systemctl::new("/no/se/usa", Scope::System, Duration::from_secs(1)),
        Journalctl::new(fake(), Scope::System),
    );
    state.log_timings = timings;
    let shutdown = state.shutdown.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, http::router(state)).await });
    Agent { addr, shutdown }
}

fn fast() -> LogTimings {
    LogTimings {
        ping_interval: Duration::from_millis(100),
        peer_timeout: Duration::from_millis(400),
        send_timeout: Duration::from_secs(2),
    }
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn connect(agent: &Agent, name: &str) -> Result<Socket, tungstenite::Error> {
    let url = format!("ws://{}/api/v1/services/{name}/logs/ws", agent.addr);
    connect_async(url).await.map(|(socket, _)| socket)
}

/// Siguiente evento (los Ping del servidor los responde la biblioteca al leer).
async fn event(socket: &mut Socket) -> LogEvent {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("sin mensaje a tiempo")
            .expect("conexión cerrada")
            .unwrap();
        match message {
            Message::Text(text) => return serde_json::from_str(&text).unwrap(),
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("mensaje inesperado: {other:?}"),
        }
    }
}

async fn message(socket: &mut Socket) -> String {
    match event(socket).await {
        LogEvent::Entry(entry) => entry.message,
        other => panic!("se esperaba una entrada: {other:?}"),
    }
}

/// Lee hasta el cierre y devuelve su código.
async fn close_code(socket: &mut Socket) -> CloseCode {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), socket.next()).await {
            Ok(Some(Ok(Message::Close(Some(frame))))) => return frame.code,
            Ok(Some(Ok(_))) => {}
            other => panic!("se esperaba un cierre: {other:?}"),
        }
    }
}

async fn assert_reaped(pid: u32) {
    for _ in 0..100 {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("journalctl {pid} sigue existiendo");
}

fn http_error(error: tungstenite::Error) -> (u16, ApiErrorBody) {
    match error {
        tungstenite::Error::Http(response) => {
            let status = response.status().as_u16();
            let body = response.into_body().expect("cuerpo del error");
            (status, serde_json::from_slice(&body).unwrap())
        }
        other => panic!("se esperaba un error HTTP: {other:?}"),
    }
}

#[tokio::test]
async fn transmite_las_entradas_en_orden() {
    let agent = agent(fast()).await;
    let mut socket = connect(&agent, "api").await.unwrap();
    assert!(message(&mut socket).await.starts_with("pid="));
    assert_eq!(message(&mut socket).await, "dos");
    assert_eq!(message(&mut socket).await, "tres");
}

#[tokio::test]
async fn cerrar_el_socket_mata_journalctl_y_libera_el_cupo() {
    let agent = agent(fast()).await;
    let mut first = connect(&agent, "api").await.unwrap();
    let pid: u32 = message(&mut first).await["pid=".len()..].parse().unwrap();
    let _second = connect(&agent, "api").await.unwrap();

    // Cupo lleno (2): el tercero no llega a hacer el handshake.
    let (status, body) = http_error(connect(&agent, "api").await.unwrap_err());
    assert_eq!((status, body.code), (503, ErrorCode::LogStreamLimit));

    first.close(None).await.unwrap();
    assert_reaped(pid).await;
    // El cupo vuelve.
    let mut again = connect(&agent, "api").await.unwrap();
    assert!(message(&mut again).await.starts_with("pid="));
}

#[tokio::test]
async fn un_cliente_que_no_responde_al_ping_se_desconecta() {
    let agent = agent(fast()).await;
    let mut socket = connect(&agent, "api").await.unwrap();
    let pid: u32 = message(&mut socket).await["pid=".len()..].parse().unwrap();
    // Sin leer, el cliente no contesta a los Ping: tras `peer_timeout` el agente lo suelta.
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert_reaped(pid).await;
}

#[tokio::test]
async fn un_cliente_que_responde_sigue_conectado() {
    let agent = agent(fast()).await;
    let mut socket = connect(&agent, "api").await.unwrap();
    for _ in 0..3 {
        message(&mut socket).await;
    }
    // Leer responde a los Ping: durante tres veces `peer_timeout`, la conexión sigue abierta.
    let until = tokio::time::Instant::now() + Duration::from_millis(1200);
    let mut pings = 0;
    while let Ok(next) = tokio::time::timeout_at(until, socket.next()).await {
        match next {
            Some(Ok(Message::Ping(_))) => pings += 1,
            other => panic!("la conexión no debería cerrarse: {other:?}"),
        }
    }
    assert!(pings >= 5, "{pings} pings");
    socket.send(Message::Ping("hola".into())).await.unwrap();
}

#[tokio::test]
async fn fin_y_fallo_de_journalctl() {
    let agent = agent(fast()).await;
    let mut socket = connect(&agent, "fin").await.unwrap();
    assert_eq!(message(&mut socket).await, "ultima");
    match event(&mut socket).await {
        LogEvent::End {
            reason: LogEndReason::JournalClosed,
            detail: Some(detail),
        } => assert!(detail.contains("Some(0)"), "{detail}"),
        other => panic!("se esperaba el fin: {other:?}"),
    }
    assert_eq!(close_code(&mut socket).await, CloseCode::Error);

    let mut socket = connect(&agent, "falla").await.unwrap();
    match event(&mut socket).await {
        LogEvent::End {
            reason: LogEndReason::JournalClosed,
            detail: Some(detail),
        } => assert!(detail.contains("Failed to open journal"), "{detail}"),
        other => panic!("se esperaba el fallo: {other:?}"),
    }
}

#[tokio::test]
async fn el_apagado_cierra_las_sesiones() {
    let agent = agent(fast()).await;
    let mut socket = connect(&agent, "api").await.unwrap();
    let pid: u32 = message(&mut socket).await["pid=".len()..].parse().unwrap();
    agent.shutdown.trigger();
    loop {
        match event(&mut socket).await {
            LogEvent::End { reason, .. } => {
                assert_eq!(reason, LogEndReason::AgentShutdown);
                break;
            }
            LogEvent::Entry(_) => {}
        }
    }
    assert_eq!(close_code(&mut socket).await, CloseCode::Away);
    assert_reaped(pid).await;
}

#[tokio::test]
async fn errores_antes_del_handshake() {
    let agent = agent(fast()).await;
    let (status, body) = http_error(connect(&agent, "no-registrado").await.unwrap_err());
    assert_eq!((status, body.code), (404, ErrorCode::ServiceNotFound));
    let (status, body) = http_error(connect(&agent, "Mal").await.unwrap_err());
    assert_eq!((status, body.code), (400, ErrorCode::BadRequest));

    // Una petición HTTP normal no abre nada.
    let response = reqwest_like_get(agent.addr, "/api/v1/services/api/logs/ws").await;
    assert!(response.starts_with("HTTP/1.1 426"), "{response}");
    assert!(response.contains("WEB_SOCKET_REQUIRED"), "{response}");
}

/// Una página web no puede abrir el socket contra el túnel local (cross-site WebSocket
/// hijacking): el navegador siempre envía `Origin`.
#[tokio::test]
async fn rechaza_websockets_de_navegador() {
    let agent = agent(fast()).await;
    let mut request = format!("ws://{}/api/v1/services/api/logs/ws", agent.addr)
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());
    let (status, body) = http_error(connect_async(request).await.unwrap_err());
    assert_eq!((status, body.code), (403, ErrorCode::Forbidden));
}

/// GET HTTP/1.1 mínimo, sin dependencias de cliente HTTP.
async fn reqwest_like_get(addr: SocketAddr, path: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}
