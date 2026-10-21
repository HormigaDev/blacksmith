//! Guarda de la superficie de administración.
//!
//! El extremo local del túnel SSH (`127.0.0.1:<puerto>` en el escritorio) está al alcance de
//! cualquier página web abierta en el navegador del usuario. Esta guarda cierra ese vector y
//! otros dos más:
//! - Peticiones de navegador (CSRF): traen `Origin` o `Sec-Fetch-Site`. El cliente Tauri hace las
//!   peticiones desde Rust y no envía ninguna de las dos.
//! - DNS rebinding: el `Host` debe ser loopback literal; un dominio que resuelve a 127.0.0.1 no
//!   pasa.
//! - Tráfico que llega por Nginx: Nginx siempre añade cabeceras de reenvío. Si alguna vez se
//!   configura mal y reenvía más rutas que los webhooks, la administración sigue cerrada.

use std::net::IpAddr;

use axum::{
    extract::Request,
    http::{HeaderName, header, uri::Authority},
    middleware::Next,
    response::Response,
};

use crate::error::AppError;

const FORWARDING_HEADERS: [HeaderName; 3] = [
    header::FORWARDED,
    HeaderName::from_static("x-forwarded-for"),
    HeaderName::from_static("x-real-ip"),
];

const BROWSER_HEADERS: [HeaderName; 2] =
    [header::ORIGIN, HeaderName::from_static("sec-fetch-site")];

pub async fn tunnel_only(request: Request, next: Next) -> Result<Response, AppError> {
    let headers = request.headers();
    // 404 y no 403: hacia fuera no se revela que la ruta existe.
    if FORWARDING_HEADERS
        .iter()
        .any(|name| headers.contains_key(name))
    {
        return Err(AppError::NotFound);
    }
    if BROWSER_HEADERS
        .iter()
        .any(|name| headers.contains_key(name))
        || !host_is_loopback(&request)
    {
        return Err(AppError::Forbidden);
    }
    Ok(next.run(request).await)
}

fn host_is_loopback(request: &Request) -> bool {
    if let Some(authority) = request.uri().authority() {
        return is_loopback_host(authority.host());
    }
    request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<Authority>().ok())
        .is_some_and(|authority| is_loopback_host(authority.host()))
}

/// Cualquier puerto vale: el usuario elige el puerto local del túnel.
fn is_loopback_host(host: &str) -> bool {
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::is_loopback_host;

    #[test]
    fn solo_hosts_loopback_literales() {
        for host in ["127.0.0.1", "127.0.0.2", "localhost", "LOCALHOST", "[::1]"] {
            assert!(is_loopback_host(host), "{host}");
        }
        for host in [
            "evil.example",
            "localhost.evil.example",
            "0.0.0.0",
            "[::]",
            "10.0.0.1",
            "",
        ] {
            assert!(!is_loopback_host(host), "{host}");
        }
    }
}
