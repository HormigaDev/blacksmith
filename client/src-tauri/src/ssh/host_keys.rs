//! Verificación de la llave del servidor (protección contra intermediarios).
//!
//! 1. `~/.ssh/known_hosts` del usuario: si OpenSSH ya confía en el servidor, la app también; si la
//!    llave registrada allí es otra, la conexión se bloquea.
//! 2. Registro propio de la app (`known_hosts.json` en su directorio de datos): llaves que el
//!    usuario confirmó la primera vez que se conectó (TOFU, como OpenSSH).
//!
//! Se compara la llave pública completa, no solo su huella. Cualquier diferencia con lo
//! registrado, incluido otro algoritmo, se trata como un cambio: así nadie puede forzar al cliente
//! a aceptar otro tipo de llave.

use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::files::write_private_file;

use russh::keys::{known_hosts::known_host_keys_path, HashAlg, PublicKey};
use serde::{Deserialize, Serialize};

use super::{key::algorithm_name, target::SshTarget};
use crate::error::CommandError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentedKey {
    pub algorithm: String,
    pub fingerprint: String,
    /// Llave pública en formato OpenSSH (`ssh-ed25519 AAAA…`).
    pub openssh: String,
}

impl PresentedKey {
    pub fn from_public(key: &PublicKey) -> Result<Self, CommandError> {
        Ok(Self {
            algorithm: algorithm_name(&key.algorithm()),
            fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
            openssh: key.to_openssh().map_err(|error| {
                CommandError::internal("codificando la llave del servidor", error)
            })?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Trusted,
    /// Nunca visto: el usuario debe confirmar la huella.
    Unknown(PresentedKey),
    /// Distinto de lo registrado: posible ataque. Nunca se acepta automáticamente.
    Changed(PresentedKey),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoreFile {
    version: u32,
    /// `host:puerto` → llave pública OpenSSH.
    hosts: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct HostKeyStore {
    path: PathBuf,
    /// `~/.ssh/known_hosts`, si existe un directorio personal.
    openssh_known_hosts: Option<PathBuf>,
    /// Confirmar y olvidar leen, cambian y escriben el registro: sin esto, dos a la vez perderían
    /// uno de los cambios.
    lock: Arc<Mutex<()>>,
}

impl HostKeyStore {
    pub fn new(path: PathBuf, openssh_known_hosts: Option<PathBuf>) -> Self {
        Self {
            path,
            openssh_known_hosts,
            lock: Arc::default(),
        }
    }

    fn guard(&self) -> Result<std::sync::MutexGuard<'_, ()>, CommandError> {
        self.lock.lock().map_err(|_| {
            CommandError::internal(
                "registro de llaves de servidor",
                "bloqueo envenenado por un pánico",
            )
        })
    }

    pub fn verify(&self, target: &SshTarget, key: &PublicKey) -> Result<Verdict, CommandError> {
        let presented = PresentedKey::from_public(key)?;

        if let Some(known_hosts) = self
            .openssh_known_hosts
            .as_ref()
            .filter(|path| path.is_file())
        {
            // Un host puede tener varias llaves (una por algoritmo): basta con que coincida una. Si
            // tiene alguna y ninguna coincide, también de otro algoritmo, es un cambio: así nadie
            // fuerza al cliente a aceptar otro tipo de llave como si el servidor fuera nuevo. Un
            // `known_hosts` ilegible se ignora y decide el registro propio.
            if let Ok(recorded) = known_host_keys_path(&target.host, target.port, known_hosts) {
                if recorded
                    .iter()
                    .any(|(_, recorded)| recorded.key_data() == key.key_data())
                {
                    return Ok(Verdict::Trusted);
                }
                if !recorded.is_empty() {
                    return Ok(Verdict::Changed(presented));
                }
            }
        }

        Ok(match self.load()?.hosts.get(&target.host_port()) {
            None => Verdict::Unknown(presented),
            Some(pinned) if same_key(pinned, &presented.openssh) => Verdict::Trusted,
            Some(_) => Verdict::Changed(presented),
        })
    }

    pub fn trust(&self, target: &SshTarget, key: &PresentedKey) -> Result<(), CommandError> {
        let _guard = self.guard()?;
        let mut store = self.load()?;
        store.hosts.insert(target.host_port(), key.openssh.clone());
        self.save(&store)
    }

    /// Olvida la llave registrada (p. ej. tras reinstalar el servidor). Acción destructiva: la
    /// interfaz la confirma antes.
    pub fn forget(&self, target: &SshTarget) -> Result<bool, CommandError> {
        let _guard = self.guard()?;
        let mut store = self.load()?;
        let removed = store.hosts.remove(&target.host_port()).is_some();
        if removed {
            self.save(&store)?;
        }
        Ok(removed)
    }

    fn load(&self) -> Result<StoreFile, CommandError> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                CommandError::internal("leyendo el registro de llaves de servidor", error)
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(StoreFile {
                version: 1,
                ..Default::default()
            }),
            Err(error) => Err(CommandError::internal(
                "leyendo el registro de llaves de servidor",
                error,
            )),
        }
    }

    fn save(&self, store: &StoreFile) -> Result<(), CommandError> {
        let bytes = serde_json::to_vec_pretty(store).map_err(|error| {
            CommandError::internal("serializando el registro de llaves de servidor", error)
        })?;
        write_private_file(&self.path, &bytes).map_err(|error| {
            CommandError::internal("guardando el registro de llaves de servidor", error)
        })
    }
}

/// Compara dos llaves OpenSSH ignorando el comentario final.
fn same_key(a: &str, b: &str) -> bool {
    fn head(key: &str) -> impl Iterator<Item = &str> {
        key.split_whitespace().take(2)
    }
    head(a).eq(head(b))
}

#[cfg(test)]
mod tests {
    use russh::keys::decode_secret_key;

    use super::*;
    use crate::ssh::key::fixtures::{CLIENT_ECDSA, HOST_ED25519, HOST_IMPOSTOR_ED25519};

    /// Como la presenta un servidor real: sin comentario (russh compara también el comentario).
    fn public(pem: &str) -> PublicKey {
        PublicKey::new(
            decode_secret_key(pem, None)
                .unwrap()
                .public_key()
                .key_data()
                .clone(),
            "",
        )
    }

    fn target() -> SshTarget {
        SshTarget::new("vps.example.com", 2222, "deploy").unwrap()
    }

    #[test]
    fn tofu_desconocida_confirmada_y_cambiada() {
        let dir = tempfile::tempdir().unwrap();
        let store = HostKeyStore::new(dir.path().join("known_hosts.json"), None);
        let host = public(HOST_ED25519);

        let Verdict::Unknown(presented) = store.verify(&target(), &host).unwrap() else {
            panic!("debería ser desconocida")
        };
        assert!(presented.fingerprint.starts_with("SHA256:"));
        store.trust(&target(), &presented).unwrap();
        assert_eq!(store.verify(&target(), &host).unwrap(), Verdict::Trusted);

        assert!(matches!(
            store
                .verify(&target(), &public(HOST_IMPOSTOR_ED25519))
                .unwrap(),
            Verdict::Changed(_)
        ));
        // Otro puerto es otro servidor.
        let other_port = SshTarget::new("vps.example.com", 22, "deploy").unwrap();
        assert!(matches!(
            store.verify(&other_port, &host).unwrap(),
            Verdict::Unknown(_)
        ));

        assert!(store.forget(&target()).unwrap());
        assert!(matches!(
            store.verify(&target(), &host).unwrap(),
            Verdict::Unknown(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn el_registro_es_privado() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("known_hosts.json");
        let store = HostKeyStore::new(path.clone(), None);
        let Verdict::Unknown(presented) = store.verify(&target(), &public(HOST_ED25519)).unwrap()
        else {
            panic!()
        };
        store.trust(&target(), &presented).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn respeta_known_hosts_de_openssh() {
        let dir = tempfile::tempdir().unwrap();
        let known_hosts = dir.path().join("known_hosts");
        let host = public(HOST_ED25519);
        fs::write(
            &known_hosts,
            format!("[vps.example.com]:2222 {}\n", host.to_openssh().unwrap()),
        )
        .unwrap();
        let store = HostKeyStore::new(dir.path().join("app.json"), Some(known_hosts));

        assert_eq!(store.verify(&target(), &host).unwrap(), Verdict::Trusted);
        assert!(matches!(
            store
                .verify(&target(), &public(HOST_IMPOSTOR_ED25519))
                .unwrap(),
            Verdict::Changed(_)
        ));
    }

    /// `known_hosts` registra otra llave de otro algoritmo: no es un servidor nuevo (TOFU), sino
    /// un cambio que se bloquea.
    #[test]
    fn otro_algoritmo_en_known_hosts_es_un_cambio() {
        let dir = tempfile::tempdir().unwrap();
        let known_hosts = dir.path().join("known_hosts");
        let recorded = public(CLIENT_ECDSA);
        fs::write(
            &known_hosts,
            format!(
                "[vps.example.com]:2222 {}\n",
                recorded.to_openssh().unwrap()
            ),
        )
        .unwrap();
        let store = HostKeyStore::new(dir.path().join("app.json"), Some(known_hosts));
        assert!(matches!(
            store.verify(&target(), &public(HOST_ED25519)).unwrap(),
            Verdict::Changed(_)
        ));
    }
}
