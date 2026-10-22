//! Túnel SSH con reenvío de puerto local (equivalente a `ssh -N -L <local>:127.0.0.1:4040 vps`).
//!
//! Orden de `Tunnel::open`, pensado para fallar pronto y con un error preciso:
//! 1. Reserva el puerto local (127.0.0.1): si está ocupado no se llega a abrir SSH.
//! 2. Conecta con timeout y verifica la llave del servidor (`HostKeyStore`).
//! 3. Se autentica con la llave del usuario.
//! 4. Empieza a reenviar cada conexión local por un canal `direct-tcpip`.
//! 5. Comprueba que el agente responde a través del túnel (`GET /api/v1/health`): así se
//!    distingue «servidor inalcanzable» de «agente caído» y de «otro servicio en el puerto». Si no
//!    responde, el túnel se cierra.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use russh::{
    client::{self, AuthResult, DisconnectReason, Handle, Handler},
    keys::{Algorithm, PrivateKey, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate},
    Disconnect,
};
use serde::Serialize;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{watch, Semaphore},
    task::{JoinHandle, JoinSet},
};

use shared::AgentStatus;

use super::{
    host_keys::{HostKeyStore, Verdict},
    target::SshTarget,
};
use crate::{agent_probe, error::CommandError};

/// El agente escucha solo en loopback del VPS (ver CLAUDE.md, arquitectura Zero-Trust).
pub const AGENT_HOST: &str = "127.0.0.1";
pub const AGENT_PORT: u16 = 4040;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
/// Keepalives sin respuesta antes de dar la conexión por perdida (~45 s).
const KEEPALIVE_MAX: usize = 3;
/// Conexiones locales simultáneas por túnel.
const MAX_CONNECTIONS: usize = 64;

pub struct TunnelConfig {
    pub target: SshTarget,
    /// 0 = puerto aleatorio libre.
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub connect_timeout: Duration,
}

impl TunnelConfig {
    /// Túnel hacia el agente: `127.0.0.1:<local>` → `127.0.0.1:4040` del VPS.
    pub fn to_agent(target: SshTarget, local_port: u16) -> Self {
        Self {
            target,
            local_port,
            remote_host: AGENT_HOST.to_owned(),
            remote_port: AGENT_PORT,
            connect_timeout: CONNECT_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CloseReason {
    /// El usuario lo cerró.
    Requested,
    /// Se perdió la conexión (red caída, servidor reiniciado, keepalive agotado).
    ConnectionLost,
}

struct ClientHandler {
    target: SshTarget,
    host_keys: HostKeyStore,
    /// Veredicto cuando la llave no es de confianza, para traducir el fallo de `connect`.
    verdict: Arc<Mutex<Option<Result<Verdict, String>>>>,
    lost: watch::Sender<bool>,
}

impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        // Un certificado de host se fija por su llave, igual que una llave simple.
        let key = match server_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(certificate) => {
                PublicKey::new(certificate.public_key().clone(), "")
            }
        };
        let verdict = self
            .host_keys
            .verify(&self.target, &key)
            .map_err(|error| error.to_string());
        let trusted = matches!(verdict, Ok(Verdict::Trusted));
        if let Ok(mut slot) = self.verdict.lock() {
            *slot = Some(verdict);
        }
        Ok(trusted)
    }

    async fn disconnected(
        &mut self,
        reason: DisconnectReason<Self::Error>,
    ) -> Result<(), Self::Error> {
        let _ = self.lost.send(true);
        match reason {
            DisconnectReason::ReceivedDisconnect(_) => Ok(()),
            DisconnectReason::Error(error) => Err(error),
        }
    }
}

pub struct Tunnel {
    local_addr: SocketAddr,
    handle: Arc<Handle<ClientHandler>>,
    stop: watch::Sender<bool>,
    lost: watch::Receiver<bool>,
    forwarder: JoinHandle<()>,
    agent: Option<AgentStatus>,
}

impl std::fmt::Debug for Tunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tunnel")
            .field("local_addr", &self.local_addr)
            .finish_non_exhaustive()
    }
}

impl Tunnel {
    pub async fn open(
        config: TunnelConfig,
        key: PrivateKey,
        host_keys: HostKeyStore,
    ) -> Result<Self, CommandError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.local_port))
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::AddrInUse => CommandError::PortInUse {
                    port: config.local_port,
                },
                _ => CommandError::internal("reservando el puerto local", error),
            })?;
        let local_addr = listener
            .local_addr()
            .map_err(|error| CommandError::internal("leyendo el puerto local", error))?;

        let verdict = Arc::new(Mutex::new(None));
        let (lost_tx, lost_rx) = watch::channel(false);
        let handler = ClientHandler {
            target: config.target.clone(),
            host_keys,
            verdict: Arc::clone(&verdict),
            lost: lost_tx,
        };
        let ssh_config = Arc::new(client::Config {
            keepalive_interval: Some(KEEPALIVE_INTERVAL),
            keepalive_max: KEEPALIVE_MAX,
            inactivity_timeout: None,
            nodelay: true,
            ..Default::default()
        });

        let address = (config.target.host.as_str(), config.target.port);
        let mut handle = match tokio::time::timeout(
            config.connect_timeout,
            client::connect(ssh_config, address, handler),
        )
        .await
        {
            Err(_) => return Err(CommandError::ConnectionTimeout),
            Ok(Err(error)) => return Err(connect_error(error, &verdict)),
            Ok(Ok(handle)) => handle,
        };

        let authenticated = tokio::time::timeout(
            config.connect_timeout,
            authenticate(&mut handle, &config.target, key),
        )
        .await
        .map_err(|_| CommandError::ConnectionTimeout)??;
        if !authenticated {
            return Err(CommandError::AuthRejected);
        }

        let handle = Arc::new(handle);
        let (stop_tx, stop_rx) = watch::channel(false);
        let forwarder = tokio::spawn(forward_loop(
            listener,
            Arc::clone(&handle),
            config.remote_host,
            config.remote_port,
            stop_rx,
            lost_rx.clone(),
        ));

        let mut tunnel = Self {
            local_addr,
            handle,
            stop: stop_tx,
            lost: lost_rx,
            forwarder,
            agent: None,
        };

        // El túnel solo cuenta como abierto si el agente responde a través de él. Si no, se cierra:
        // el servidor sigue guardado y el usuario puede reintentar cuando el agente esté en marcha.
        match agent_probe::probe(local_addr, config.connect_timeout).await {
            Ok(agent) => {
                tunnel.agent = Some(agent);
                Ok(tunnel)
            }
            Err(error) => {
                tunnel.close().await;
                Err(error)
            }
        }
    }

    /// Estado que devolvió el agente al abrir el túnel.
    pub fn agent(&self) -> Option<&AgentStatus> {
        self.agent.as_ref()
    }

    pub fn local_port(&self) -> u16 {
        self.local_addr.port()
    }

    /// Se resuelve cuando la conexión SSH se pierde (no cuando se cierra a propósito).
    pub fn lost(&self) -> watch::Receiver<bool> {
        self.lost.clone()
    }

    /// Cierra el túnel: deja de aceptar conexiones, corta las abiertas y libera el puerto local.
    pub async fn close(self) {
        let _ = self.stop.send(true);
        let _ = self
            .handle
            .disconnect(Disconnect::ByApplication, "", "es")
            .await;
        let _ = self.forwarder.await;
    }
}

async fn authenticate(
    handle: &mut Handle<ClientHandler>,
    target: &SshTarget,
    key: PrivateKey,
) -> Result<bool, CommandError> {
    // RSA: firmar con SHA-2 si el servidor lo admite (ssh-rsa con SHA-1 está desactivado en OpenSSH moderno).
    let hash = if matches!(key.algorithm(), Algorithm::Rsa { .. }) {
        handle
            .best_supported_rsa_hash()
            .await
            .map_err(session_error)?
            .flatten()
    } else {
        None
    };
    let result = handle
        .authenticate_publickey(
            target.username.clone(),
            PrivateKeyWithHashAlg::new(Arc::new(key), hash),
        )
        .await
        .map_err(session_error)?;
    Ok(matches!(result, AuthResult::Success))
}

async fn forward_loop(
    listener: TcpListener,
    handle: Arc<Handle<ClientHandler>>,
    remote_host: String,
    remote_port: u16,
    mut stop: watch::Receiver<bool>,
    mut lost: watch::Receiver<bool>,
) {
    let limit = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _ = stop.changed() => break,
            _ = lost.changed() => break,
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
            accepted = listener.accept() => {
                let Ok((socket, peer)) = accepted else { continue };
                // Sin cupo se rechaza la conexión en vez de encolarla.
                let Ok(permit) = Arc::clone(&limit).try_acquire_owned() else { continue };
                let handle = Arc::clone(&handle);
                let remote_host = remote_host.clone();
                connections.spawn(async move {
                    let _permit = permit;
                    forward(&handle, socket, peer, remote_host, remote_port).await;
                });
            }
        }
    }
    // Al salir se sueltan el listener (el puerto queda libre) y las conexiones en curso.
    connections.abort_all();
}

async fn forward(
    handle: &Handle<ClientHandler>,
    mut socket: TcpStream,
    peer: SocketAddr,
    host: String,
    port: u16,
) {
    let _ = socket.set_nodelay(true);
    let Ok(channel) = handle
        .channel_open_direct_tcpip(
            host,
            u32::from(port),
            peer.ip().to_string(),
            u32::from(peer.port()),
        )
        .await
    else {
        return;
    };
    let mut stream = channel.into_stream();
    let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
}

fn connect_error(
    error: russh::Error,
    verdict: &Mutex<Option<Result<Verdict, String>>>,
) -> CommandError {
    let verdict = verdict.lock().ok().and_then(|mut slot| slot.take());
    match (verdict, error) {
        (Some(Ok(Verdict::Unknown(key))), _) => CommandError::HostKeyUnknown {
            algorithm: key.algorithm,
            fingerprint: key.fingerprint,
            public_key: key.openssh,
        },
        (Some(Ok(Verdict::Changed(key))), _) => CommandError::HostKeyChanged {
            algorithm: key.algorithm,
            fingerprint: key.fingerprint,
        },
        (Some(Err(message)), _) => {
            CommandError::internal("verificando la llave del servidor", message)
        }
        (_, error) => session_error(error),
    }
}

fn session_error(error: russh::Error) -> CommandError {
    match error {
        russh::Error::IO(io) if io.kind() == std::io::ErrorKind::TimedOut => {
            CommandError::ConnectionTimeout
        }
        // Rechazo, host o red inalcanzables, DNS que no resuelve, conexión cortada al negociar.
        russh::Error::IO(_) | russh::Error::Disconnect | russh::Error::HUP => {
            CommandError::HostUnreachable
        }
        russh::Error::ConnectionTimeout
        | russh::Error::KeepaliveTimeout
        | russh::Error::InactivityTimeout => CommandError::ConnectionTimeout,
        other => CommandError::internal("negociando la conexión SSH", other),
    }
}

#[cfg(test)]
mod tests;
