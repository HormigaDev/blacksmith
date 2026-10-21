//! Destino SSH (`usuario@host:puerto`) validado antes de tocar la red o el disco: lo que llega de
//! la webview se trata como no confiable.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::error::CommandError;

pub const DEFAULT_SSH_PORT: u16 = 22;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SshTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
}

impl SshTarget {
    /// Normaliza y valida. El host se guarda en minúsculas y sin corchetes IPv6.
    pub fn new(host: &str, port: u16, username: &str) -> Result<Self, CommandError> {
        let host = normalize_host(host).ok_or(CommandError::InvalidTarget("host"))?;
        if port == 0 {
            return Err(CommandError::InvalidTarget("port"));
        }
        if !is_valid_username(username) {
            return Err(CommandError::InvalidTarget("username"));
        }
        Ok(Self {
            host,
            port,
            username: username.to_owned(),
        })
    }

    /// Identificador estable del destino: clave de las credenciales y de los túneles abiertos.
    pub fn id(&self) -> String {
        format!("{}@{}", self.username, self.host_port())
    }

    /// `host:puerto`, con corchetes para IPv6 (formato de OpenSSH).
    pub fn host_port(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn normalize_host(raw: &str) -> Option<String> {
    let host = raw.trim();
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if host.parse::<IpAddr>().is_ok() {
        return Some(host.to_ascii_lowercase());
    }
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    let valid = !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });
    valid.then_some(host)
}

/// Nombres de usuario POSIX portables (más el punto, habitual en la práctica). Nunca empiezan por
/// `-`, para que no puedan interpretarse como una opción.
fn is_valid_username(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_hosts_validos() {
        let target = SshTarget::new(" VPS.Example.com. ", 22, "deploy").unwrap();
        assert_eq!(target.host, "vps.example.com");
        assert_eq!(target.id(), "deploy@vps.example.com:22");
        assert_eq!(
            SshTarget::new("203.0.113.7", 2222, "root")
                .unwrap()
                .host_port(),
            "203.0.113.7:2222"
        );
        assert_eq!(
            SshTarget::new("[2001:DB8::1]", 22, "ops")
                .unwrap()
                .host_port(),
            "[2001:db8::1]:22"
        );
    }

    #[test]
    fn rechaza_entradas_peligrosas_o_mal_formadas() {
        for host in [
            "",
            "a b",
            "-evil.com",
            "evil-.com",
            "x..y",
            "host;rm -rf",
            "user@host",
            &"a".repeat(254),
        ] {
            assert!(SshTarget::new(host, 22, "deploy").is_err(), "{host}");
        }
        for user in ["", "-oProxyCommand=x", "a b", "root;id", &"u".repeat(33)] {
            assert!(SshTarget::new("host", 22, user).is_err(), "{user}");
        }
        assert!(SshTarget::new("host", 0, "deploy").is_err());
    }
}
