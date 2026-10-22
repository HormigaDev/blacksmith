//! Comprobación del agente: `GET /api/v1/health` contra una dirección local (el extremo del túnel
//! o, en un servidor local, el propio agente). Que el puerto esté abierto no basta: la respuesta
//! debe ser un `AgentStatus` con el nombre del agente (contrato del crate `shared`).
//!
//! HTTP/1.1 mínimo y a mano, sin un cliente HTTP como dependencia: una sola petición, con
//! `Connection: close`, límite de tamaño y timeout.

use std::{net::SocketAddr, time::Duration};

use shared::{AgentStatus, AGENT_NAME, HEALTH_PATH};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::error::CommandError;

pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;

/// `Ok` con el estado del agente; `AgentUnreachable` si no hay nadie escuchando o no contesta;
/// `AgentInvalid` si contesta algo que no es el agente.
pub async fn probe(addr: SocketAddr, timeout: Duration) -> Result<AgentStatus, CommandError> {
    tokio::time::timeout(timeout, exchange(addr))
        .await
        .map_err(|_| CommandError::AgentUnreachable)?
}

async fn exchange(addr: SocketAddr) -> Result<AgentStatus, CommandError> {
    let mut stream = TcpStream::connect(addr)
        .await
        .map_err(|_| CommandError::AgentUnreachable)?;
    // Sin `Origin` y con `Host` loopback: lo que exige la guarda del agente.
    let request = format!(
        "GET {HEALTH_PATH} HTTP/1.1\r\nHost: {addr}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|_| CommandError::AgentUnreachable)?;

    // Lectura incremental: no se espera a que el otro extremo cierre (un servicio que no es HTTP
    // podría no cerrar nunca); se decide en cuanto hay datos suficientes.
    let mut response = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|_| CommandError::AgentUnreachable)?;
        if read == 0 {
            // Se aceptó la conexión y se cerró sin responder (p. ej. el túnel no llegó a nada).
            return if response.is_empty() {
                Err(CommandError::AgentUnreachable)
            } else {
                parse(&response)
            };
        }
        response.extend_from_slice(chunk.get(..read).unwrap_or_default());
        if response.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(CommandError::AgentInvalid);
        }
        let prefix = response.get(..response.len().min(5)).unwrap_or_default();
        if !b"HTTP/".starts_with(prefix) {
            return Err(CommandError::AgentInvalid);
        }
        if is_complete(&response) {
            return parse(&response);
        }
    }
}

/// Cabeceras recibidas y, si hay `content-length`, el cuerpo entero.
fn is_complete(response: &[u8]) -> bool {
    let Some(separator) = find_separator(response) else {
        return false;
    };
    let head =
        String::from_utf8_lossy(response.get(..separator).unwrap_or_default()).to_ascii_lowercase();
    let length = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok());
    match length {
        Some(length) => response.len() >= separator + 4 + length,
        // Sin longitud declarada, el cuerpo termina al cerrar la conexión.
        None => false,
    }
}

fn find_separator(response: &[u8]) -> Option<usize> {
    response.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse(response: &[u8]) -> Result<AgentStatus, CommandError> {
    let separator = find_separator(response).ok_or(CommandError::AgentInvalid)?;
    let (head, body) = response.split_at(separator);
    let body = body.get(4..).ok_or(CommandError::AgentInvalid)?;
    let head = std::str::from_utf8(head).map_err(|_| CommandError::AgentInvalid)?;
    let status_line = head.lines().next().unwrap_or_default();
    let status_ok =
        status_line.starts_with("HTTP/1.") && status_line.split_whitespace().nth(1) == Some("200");
    if !status_ok {
        return Err(CommandError::AgentInvalid);
    }
    let agent: AgentStatus =
        serde_json::from_slice(body).map_err(|_| CommandError::AgentInvalid)?;
    if agent.name != AGENT_NAME {
        return Err(CommandError::AgentInvalid);
    }
    Ok(agent)
}

#[cfg(test)]
pub(crate) mod tests {
    use tokio::net::TcpListener;

    use super::*;

    pub const HEALTH_BODY: &str =
        r#"{"name":"blacksmith-agent","version":"1.0.0","status":"ready"}"#;

    pub fn health_response(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    /// Servidor que responde siempre lo mismo y cierra.
    async fn serve(reply: String) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let reply = reply.clone();
                tokio::spawn(async move {
                    let mut buffer = [0u8; 1024];
                    let _ = socket.read(&mut buffer).await;
                    let _ = socket.write_all(reply.as_bytes()).await;
                });
            }
        });
        addr
    }

    /// Contra un agente real en marcha: `BLACKSMITH_TEST_AGENT=127.0.0.1:4040 cargo test -p client
    /// -- --ignored agente_real`.
    #[tokio::test]
    #[ignore = "necesita un agente en marcha"]
    async fn agente_real() {
        let addr = std::env::var("BLACKSMITH_TEST_AGENT")
            .unwrap()
            .parse()
            .unwrap();
        let agent = probe(addr, PROBE_TIMEOUT).await.unwrap();
        assert_eq!(agent.name, AGENT_NAME);
    }

    #[tokio::test]
    async fn reconoce_al_agente() {
        let addr = serve(health_response(HEALTH_BODY)).await;
        let agent = probe(addr, PROBE_TIMEOUT).await.unwrap();
        assert_eq!(
            (agent.name.as_str(), agent.version.as_str()),
            (AGENT_NAME, "1.0.0")
        );
    }

    #[tokio::test]
    async fn nadie_escuchando_es_agente_inalcanzable() {
        let addr = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();
        assert!(matches!(
            probe(addr, PROBE_TIMEOUT).await,
            Err(CommandError::AgentUnreachable)
        ));
    }

    #[tokio::test]
    async fn otro_servicio_en_el_puerto_no_es_el_agente() {
        for reply in [
            health_response(r#"{"name":"otro","version":"1","status":"ready"}"#),
            health_response("<html>nginx</html>"),
            "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n".to_owned(),
            "SSH-2.0-OpenSSH_9.6\r\n".to_owned(),
        ] {
            let addr = serve(reply.clone()).await;
            assert!(
                matches!(
                    probe(addr, PROBE_TIMEOUT).await,
                    Err(CommandError::AgentInvalid)
                ),
                "{reply}"
            );
        }
    }

    #[tokio::test]
    async fn un_servicio_que_no_es_http_y_no_cierra_se_descarta_al_momento() {
        // Eco: devuelve la petición y no cierra nunca. No debe esperarse al timeout.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (mut read, mut write) = socket.split();
                    let _ = tokio::io::copy(&mut read, &mut write).await;
                });
            }
        });
        let started = std::time::Instant::now();
        assert!(matches!(
            probe(addr, PROBE_TIMEOUT).await,
            Err(CommandError::AgentInvalid)
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn un_servicio_mudo_agota_el_tiempo() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        let result = probe(addr, Duration::from_millis(200)).await;
        assert!(
            matches!(result, Err(CommandError::AgentUnreachable)),
            "{result:?}"
        );
    }
}
