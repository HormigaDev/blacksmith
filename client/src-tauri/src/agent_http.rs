//! Peticiones HTTP al agente desde Rust. La webview no puede llegar al agente (la CSP solo permite
//! IPC y el agente rechaza cualquier petición con `Origin`): `agent_request` pasa por aquí.
//!
//! HTTP/1.1 mínimo y a mano, como `agent_probe`: una petición por conexión (`Connection: close`),
//! límites de tamaño, timeout y solo rutas de la API (`/api/v1/…`). La dirección no la elige la
//! webview: es la del túnel (o el agente local) del servidor indicado.

use std::{net::SocketAddr, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::error::{CommandError, CommandResult};

/// Mayor que el máximo del agente (`BLACKSMITH_REQUEST_TIMEOUT_SECS` ≤ 300 s): quien corta antes
/// es el agente, con su propio error.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(310);
pub const MAX_REQUEST_BODY: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_PATH: usize = 2048;
const API_PREFIX: &str = "/api/v1/";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

/// Respuesta del agente, tal cual: la interfaz la convierte en un `Response` de Fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub retry_after: Option<String>,
    pub request_id: Option<String>,
    pub body: String,
}

/// Ruta con query, relativa al agente. Solo la API, sin escapar de ella ni romper la petición.
pub fn validate_path(path: &str) -> CommandResult<()> {
    let valid = path.starts_with(API_PREFIX)
        && path.len() <= MAX_PATH
        && path.bytes().all(|byte| byte.is_ascii_graphic())
        && !path.contains("..")
        && !path.contains("//")
        && !path.contains('\\')
        && !path.contains('#');
    if valid {
        Ok(())
    } else {
        Err(CommandError::InvalidTarget("path"))
    }
}

/// Identificador de petición (`x-request-id`): opcional, corto y sin caracteres especiales.
fn validate_request_id(id: &str) -> CommandResult<()> {
    if !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        Ok(())
    } else {
        Err(CommandError::InvalidTarget("requestId"))
    }
}

pub async fn request(
    addr: SocketAddr,
    method: Method,
    path: &str,
    body: Option<&str>,
    request_id: Option<&str>,
    timeout: Duration,
) -> CommandResult<AgentResponse> {
    validate_path(path)?;
    if let Some(id) = request_id {
        validate_request_id(id)?;
    }
    if body.is_some_and(|body| body.len() > MAX_REQUEST_BODY) {
        return Err(CommandError::InvalidTarget("body"));
    }
    tokio::time::timeout(timeout, exchange(addr, method, path, body, request_id))
        .await
        .map_err(|_| CommandError::ConnectionTimeout)?
}

async fn exchange(
    addr: SocketAddr,
    method: Method,
    path: &str,
    body: Option<&str>,
    request_id: Option<&str>,
) -> CommandResult<AgentResponse> {
    let mut stream = TcpStream::connect(addr)
        .await
        .map_err(|_| CommandError::AgentUnreachable)?;

    // Sin `Origin` y con `Host` loopback: lo que exige la guarda del agente.
    let mut head = format!(
        "{} {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: application/json\r\nConnection: close\r\n",
        method.as_str()
    );
    if let Some(id) = request_id {
        head.push_str(&format!("x-request-id: {id}\r\n"));
    }
    if let Some(body) = body {
        head.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .await
        .map_err(|_| CommandError::AgentUnreachable)?;
    if let Some(body) = body {
        stream
            .write_all(body.as_bytes())
            .await
            .map_err(|_| CommandError::AgentUnreachable)?;
    }

    let mut response = Vec::new();
    (&mut stream)
        .take(MAX_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut response)
        .await
        .map_err(|_| CommandError::AgentUnreachable)?;
    if response.is_empty() {
        return Err(CommandError::AgentUnreachable);
    }
    if response.len() > MAX_RESPONSE_BYTES {
        return Err(CommandError::AgentInvalid);
    }
    parse(&response, method)
}

fn parse(response: &[u8], method: Method) -> CommandResult<AgentResponse> {
    let separator = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(CommandError::AgentInvalid)?;
    let head = std::str::from_utf8(response.get(..separator).unwrap_or_default())
        .map_err(|_| CommandError::AgentInvalid)?;
    let raw_body = response.get(separator + 4..).unwrap_or_default();

    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let mut parts = status_line.splitn(3, ' ');
    let (Some(version), Some(code)) = (parts.next(), parts.next()) else {
        return Err(CommandError::AgentInvalid);
    };
    if !version.starts_with("HTTP/1.") {
        return Err(CommandError::AgentInvalid);
    }
    let status: u16 = code.parse().map_err(|_| CommandError::AgentInvalid)?;

    let mut content_type = None;
    let mut retry_after = None;
    let mut request_id = None;
    let mut content_length = None;
    let mut chunked = false;
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(CommandError::AgentInvalid)?;
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "content-type" => content_type = Some(value.to_owned()),
            "retry-after" => retry_after = Some(value.to_owned()),
            "x-request-id" => request_id = Some(value.to_owned()),
            "content-length" => {
                content_length = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| CommandError::AgentInvalid)?,
                );
            }
            "transfer-encoding" => chunked = value.eq_ignore_ascii_case("chunked"),
            _ => {}
        }
    }

    let body = if method == Method::Head || matches!(status, 204 | 304) {
        Vec::new()
    } else if chunked {
        dechunk(raw_body).ok_or(CommandError::AgentInvalid)?
    } else if let Some(length) = content_length {
        raw_body
            .get(..length)
            .ok_or(CommandError::AgentInvalid)?
            .to_vec()
    } else {
        raw_body.to_vec()
    };
    Ok(AgentResponse {
        status,
        content_type,
        retry_after,
        request_id,
        body: String::from_utf8(body).map_err(|_| CommandError::AgentInvalid)?,
    })
}

/// `Transfer-Encoding: chunked`: `<tamaño hex>\r\n<datos>\r\n` … `0\r\n\r\n`.
fn dechunk(mut raw: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::with_capacity(raw.len());
    loop {
        let line_end = raw.windows(2).position(|window| window == b"\r\n")?;
        let size_text = std::str::from_utf8(raw.get(..line_end)?).ok()?;
        let size_text = size_text.split(';').next()?.trim();
        let size = usize::from_str_radix(size_text, 16).ok()?;
        raw = raw.get(line_end + 2..)?;
        if size == 0 {
            return Some(body);
        }
        body.extend_from_slice(raw.get(..size)?);
        raw = raw.get(size..)?.strip_prefix(b"\r\n")?;
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    /// Servidor que guarda la petición recibida y responde `reply`.
    async fn serve(reply: &'static str) -> (SocketAddr, tokio::sync::oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (sender, received) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0u8; 8192];
            let read = socket.read(&mut buffer).await.unwrap();
            let _ = sender.send(String::from_utf8_lossy(&buffer[..read]).into_owned());
            socket.write_all(reply.as_bytes()).await.unwrap();
        });
        (addr, received)
    }

    #[tokio::test]
    async fn peticion_y_respuesta() {
        let (addr, received) = serve(
            "HTTP/1.1 201 Created\r\ncontent-type: application/json\r\ncontent-length: 11\r\nx-request-id: abc\r\n\r\n{\"ok\":true}",
        )
        .await;
        let response = request(
            addr,
            Method::Post,
            "/api/v1/services",
            Some(r#"{"name":"api"}"#),
            Some("req-1"),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert_eq!(response.status, 201);
        assert_eq!(response.body, r#"{"ok":true}"#);
        assert_eq!(response.content_type.as_deref(), Some("application/json"));
        assert_eq!(response.request_id.as_deref(), Some("abc"));

        let sent = received.await.unwrap();
        assert!(
            sent.starts_with("POST /api/v1/services HTTP/1.1\r\n"),
            "{sent}"
        );
        assert!(sent.contains(&format!("Host: {addr}\r\n")));
        assert!(sent.contains("x-request-id: req-1\r\n"));
        assert!(sent.contains("Content-Length: 14\r\n"));
        assert!(sent.ends_with(r#"{"name":"api"}"#));
        assert!(!sent.to_ascii_lowercase().contains("origin"));
    }

    #[tokio::test]
    async fn cuerpo_troceado_y_sin_contenido() {
        let (addr, _) = serve(
            "HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n4\r\n{\"a\"\r\n3\r\n:1}\r\n0\r\n\r\n",
        )
        .await;
        let response = request(
            addr,
            Method::Get,
            "/api/v1/x",
            None,
            None,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert_eq!(response.body, r#"{"a":1}"#);

        let (addr, _) = serve("HTTP/1.1 204 No Content\r\n\r\n").await;
        let response = request(
            addr,
            Method::Delete,
            "/api/v1/services/api",
            None,
            None,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert_eq!((response.status, response.body.as_str()), (204, ""));
    }

    #[test]
    fn solo_rutas_de_la_api() {
        for path in [
            "/api/v1/services",
            "/api/v1/services/api/env",
            "/api/v1/services?x=1&y=%20",
        ] {
            assert!(validate_path(path).is_ok(), "{path}");
        }
        for path in [
            "/",
            "/api/v2/x",
            "api/v1/x",
            "/api/v1/../admin",
            "/api/v1//x",
            "/api/v1/x y",
            "/api/v1/x\r\nHost: evil",
            "/api/v1/x#frag",
            "/api/v1/ñ",
            "http://evil/api/v1/x",
        ] {
            assert!(validate_path(path).is_err(), "{path:?}");
        }
        assert!(validate_request_id("abc-123").is_ok());
        assert!(validate_request_id("a\r\nb").is_err());
    }

    #[tokio::test]
    async fn respuestas_invalidas_o_sin_agente() {
        let (addr, _) = serve("SSH-2.0-OpenSSH_9.6\r\n").await;
        assert!(matches!(
            request(
                addr,
                Method::Get,
                "/api/v1/x",
                None,
                None,
                Duration::from_secs(5)
            )
            .await,
            Err(CommandError::AgentInvalid)
        ));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed = listener.local_addr().unwrap();
        drop(listener);
        assert!(matches!(
            request(
                closed,
                Method::Get,
                "/api/v1/x",
                None,
                None,
                Duration::from_secs(5)
            )
            .await,
            Err(CommandError::AgentUnreachable)
        ));
        let big = "x".repeat(MAX_REQUEST_BODY + 1);
        assert!(matches!(
            request(
                closed,
                Method::Post,
                "/api/v1/x",
                Some(&big),
                None,
                Duration::from_secs(5)
            )
            .await,
            Err(CommandError::InvalidTarget("body"))
        ));
    }
}
