//! Tests de extremo a extremo del túnel contra un servidor SSH real en proceso (russh) que hace
//! de VPS, y un servidor de eco que hace de agente.

use std::{sync::Arc, time::Duration};

use russh::{
    keys::{decode_secret_key, PublicKey},
    server::{self, Auth, ChannelOpenHandle, Msg, Session},
    Channel,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

use super::*;
use crate::ssh::{
    host_keys::HostKeyStore,
    key::fixtures::{
        CLIENT_ED25519, CLIENT_RSA, HOST_ED25519, HOST_IMPOSTOR_ED25519, INTRUDER_ED25519,
    },
};

const USER: &str = "deploy";

#[derive(Clone)]
struct VpsHandler {
    authorized: Vec<PublicKey>,
}

impl server::Handler for VpsHandler {
    type Error = russh::Error;

    async fn auth_publickey_offered(
        &mut self,
        user: &str,
        key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        Ok(self.decide(user, key))
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        Ok(self.decide(user, key))
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Como sshd: reenvía al destino pedido si se puede conectar; si no, rechaza el canal.
        match TcpStream::connect((host, port as u16)).await {
            Ok(mut upstream) => {
                reply.accept().await;
                tokio::spawn(async move {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut upstream, &mut stream).await;
                });
            }
            Err(_) => reply.reject(russh::ChannelOpenFailure::ConnectFailed).await,
        }
        Ok(())
    }
}

impl VpsHandler {
    fn decide(&self, user: &str, key: &PublicKey) -> Auth {
        if user == USER
            && self
                .authorized
                .iter()
                .any(|authorized| authorized.key_data() == key.key_data())
        {
            Auth::Accept
        } else {
            Auth::reject()
        }
    }
}

/// VPS de test: acepta las llaves de `CLIENT_ED25519` y `CLIENT_RSA`.
///
/// Entre el cliente y la sesión SSH hay un "cable" (un proxy en memoria) que se puede cortar:
/// `crash` simula una caída de red o del servidor cerrando las conexiones de golpe.
struct Vps {
    port: u16,
    task: JoinHandle<()>,
}

impl Vps {
    async fn start(host_key_pem: &str) -> Self {
        let config = Arc::new(server::Config {
            keys: vec![decode_secret_key(host_key_pem, None).unwrap()],
            auth_rejection_time: Duration::from_millis(10),
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        });
        let handler = VpsHandler {
            authorized: [CLIENT_ED25519, CLIENT_RSA]
                .iter()
                .map(|pem| decode_secret_key(pem, None).unwrap().public_key().clone())
                .collect(),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let mut cables = JoinSet::new();
            while let Ok((mut socket, _)) = listener.accept().await {
                let (mut near, far) = tokio::io::duplex(64 * 1024);
                let config = Arc::clone(&config);
                let handler = handler.clone();
                tokio::spawn(async move {
                    if let Ok(session) = server::run_stream(config, far, handler).await {
                        let _ = session.await;
                    }
                });
                // Al abortar esta tarea se suelta el socket: el cliente ve la conexión cortada.
                cables.spawn(async move {
                    let _ = tokio::io::copy_bidirectional(&mut socket, &mut near).await;
                });
            }
        });
        Self { port, task }
    }

    /// Corta todas las conexiones de golpe, como una caída de red o del servidor.
    fn crash(self) {
        self.task.abort();
    }
}

/// "Agente" de test: devuelve lo que recibe.
async fn echo_agent() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let (mut read, mut write) = socket.split();
                let _ = tokio::io::copy(&mut read, &mut write).await;
            });
        }
    });
    port
}

struct Fixture {
    _dir: tempfile::TempDir,
    host_keys: HostKeyStore,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let host_keys = HostKeyStore::new(dir.path().join("known_hosts.json"), None);
        Self {
            _dir: dir,
            host_keys,
        }
    }

    fn config(&self, ssh_port: u16, agent_port: u16) -> TunnelConfig {
        TunnelConfig {
            target: SshTarget::new("127.0.0.1", ssh_port, USER).unwrap(),
            local_port: 0,
            remote_host: "127.0.0.1".into(),
            remote_port: agent_port,
            connect_timeout: Duration::from_secs(5),
        }
    }

    async fn open(
        &self,
        ssh_port: u16,
        agent_port: u16,
        key_pem: &str,
    ) -> Result<Tunnel, CommandError> {
        let key = decode_secret_key(key_pem, None).unwrap();
        Tunnel::open(
            self.config(ssh_port, agent_port),
            key,
            self.host_keys.clone(),
        )
        .await
    }

    /// Primera conexión: llave desconocida → el usuario la confirma.
    async fn trust_host(&self, ssh_port: u16, agent_port: u16) {
        match self.open(ssh_port, agent_port, CLIENT_ED25519).await {
            Err(CommandError::HostKeyUnknown {
                public_key,
                algorithm,
                fingerprint,
            }) => {
                let presented = crate::ssh::host_keys::PresentedKey {
                    algorithm,
                    fingerprint,
                    openssh: public_key,
                };
                self.host_keys
                    .trust(
                        &SshTarget::new("127.0.0.1", ssh_port, USER).unwrap(),
                        &presented,
                    )
                    .unwrap();
            }
            Err(other) => panic!("esperaba HOST_KEY_UNKNOWN, llegó {other:?}"),
            Ok(_) => panic!("una llave desconocida nunca debe aceptarse sola"),
        }
    }
}

async fn roundtrip(port: u16, message: &[u8]) -> Vec<u8> {
    let mut socket = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    socket.write_all(message).await.unwrap();
    let mut buffer = vec![0; message.len()];
    tokio::time::timeout(Duration::from_secs(5), socket.read_exact(&mut buffer))
        .await
        .unwrap()
        .unwrap();
    buffer
}

#[tokio::test]
async fn reenvia_el_trafico_al_agente_tras_confirmar_la_llave_del_servidor() {
    let vps = Vps::start(HOST_ED25519).await;
    let agent = echo_agent().await;
    let fixture = Fixture::new();
    fixture.trust_host(vps.port, agent).await;

    let tunnel = fixture.open(vps.port, agent, CLIENT_ED25519).await.unwrap();
    assert_ne!(tunnel.local_port(), 0);
    assert_eq!(
        roundtrip(tunnel.local_port(), b"GET /api/v1/health").await,
        b"GET /api/v1/health"
    );
    // Varias conexiones simultáneas por el mismo túnel.
    let (a, b) = tokio::join!(
        roundtrip(tunnel.local_port(), b"uno"),
        roundtrip(tunnel.local_port(), b"dos")
    );
    assert_eq!(
        (a.as_slice(), b.as_slice()),
        (b"uno".as_slice(), b"dos".as_slice())
    );

    let port = tunnel.local_port();
    tunnel.close().await;
    assert!(
        TcpStream::connect(("127.0.0.1", port)).await.is_err(),
        "el puerto local debe quedar libre"
    );
}

#[tokio::test]
async fn autentica_con_rsa_firmando_con_sha2() {
    let vps = Vps::start(HOST_ED25519).await;
    let agent = echo_agent().await;
    let fixture = Fixture::new();
    fixture.trust_host(vps.port, agent).await;
    let tunnel = fixture.open(vps.port, agent, CLIENT_RSA).await.unwrap();
    assert_eq!(roundtrip(tunnel.local_port(), b"rsa").await, b"rsa");
    tunnel.close().await;
}

#[tokio::test]
async fn bloquea_si_la_llave_del_servidor_cambia() {
    // El usuario confió en la llave legítima del servidor; ahora responde otro con otra llave
    // en la misma dirección: posible intermediario.
    let impostor = Vps::start(HOST_IMPOSTOR_ED25519).await;
    let agent = echo_agent().await;
    let fixture = Fixture::new();
    let legit = decode_secret_key(HOST_ED25519, None).unwrap();
    let pinned = crate::ssh::host_keys::PresentedKey::from_public(legit.public_key()).unwrap();
    let target = SshTarget::new("127.0.0.1", impostor.port, USER).unwrap();
    fixture.host_keys.trust(&target, &pinned).unwrap();

    let result = fixture.open(impostor.port, agent, CLIENT_ED25519).await;
    assert!(
        matches!(result, Err(CommandError::HostKeyChanged { .. })),
        "{result:?}"
    );
}

#[tokio::test]
async fn rechaza_una_llave_no_autorizada() {
    let vps = Vps::start(HOST_ED25519).await;
    let agent = echo_agent().await;
    let fixture = Fixture::new();
    fixture.trust_host(vps.port, agent).await;
    let result = fixture.open(vps.port, agent, INTRUDER_ED25519).await;
    assert!(
        matches!(result, Err(CommandError::AuthRejected)),
        "{result:?}"
    );
}

#[tokio::test]
async fn distingue_servidor_inalcanzable_de_agente_caido() {
    let fixture = Fixture::new();
    // Puerto sin nadie escuchando.
    let closed = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let result = fixture.open(closed, 4040, CLIENT_ED25519).await;
    assert!(
        matches!(result, Err(CommandError::HostUnreachable)),
        "{result:?}"
    );

    // SSH funciona, pero en el VPS no hay nadie en el puerto del agente.
    let vps = Vps::start(HOST_ED25519).await;
    let no_agent = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    fixture.trust_host(vps.port, no_agent).await;
    let result = fixture.open(vps.port, no_agent, CLIENT_ED25519).await;
    assert!(
        matches!(result, Err(CommandError::AgentUnreachable)),
        "{result:?}"
    );
}

#[tokio::test]
async fn agota_el_tiempo_si_el_servidor_no_habla_ssh() {
    // Acepta TCP pero nunca envía el saludo SSH.
    let silent = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = silent.local_addr().unwrap().port();
    let _keep = tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = silent.accept().await {
            held.push(socket);
        }
    });
    let fixture = Fixture::new();
    let mut config = fixture.config(port, 4040);
    config.connect_timeout = Duration::from_millis(300);
    let key = decode_secret_key(CLIENT_ED25519, None).unwrap();
    let result = Tunnel::open(config, key, fixture.host_keys.clone()).await;
    assert!(
        matches!(result, Err(CommandError::ConnectionTimeout)),
        "{result:?}"
    );
}

#[tokio::test]
async fn informa_si_el_puerto_local_esta_ocupado() {
    let busy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = busy.local_addr().unwrap().port();
    let fixture = Fixture::new();
    let mut config = fixture.config(22, 4040);
    config.local_port = port;
    let key = decode_secret_key(CLIENT_ED25519, None).unwrap();
    let result = Tunnel::open(config, key, fixture.host_keys.clone()).await;
    assert!(
        matches!(result, Err(CommandError::PortInUse { port: p }) if p == port),
        "{result:?}"
    );
}

#[tokio::test]
async fn detecta_la_perdida_de_la_conexion() {
    let vps = Vps::start(HOST_ED25519).await;
    let agent = echo_agent().await;
    let fixture = Fixture::new();
    fixture.trust_host(vps.port, agent).await;
    let tunnel = fixture.open(vps.port, agent, CLIENT_ED25519).await.unwrap();
    let mut lost = tunnel.lost();

    vps.crash();
    tokio::time::timeout(Duration::from_secs(10), lost.wait_for(|lost| *lost))
        .await
        .expect("la caída del servidor debe detectarse")
        .unwrap();
    let port = tunnel.local_port();
    tunnel.close().await;
    assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
}
