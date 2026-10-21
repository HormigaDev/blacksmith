//! Bóveda de credenciales SSH.
//!
//! Cifrado por envoltura: el llavero del sistema (Secret Service en Linux, Administrador de
//! credenciales en Windows) guarda solo una clave maestra aleatoria de 32 bytes por instalación;
//! cada credencial se cifra con AES-256-GCM y se guarda en el directorio de datos de la app
//! (`0600`). Así se evita el límite de tamaño del llavero de Windows (2560 bytes: una llave RSA de
//! 4096 bits no cabe) y nada sensible queda en disco en claro: sin el llavero de la sesión del
//! usuario, los archivos son ilegibles.
//!
//! Formato de cada archivo: `BSV1` ‖ nonce (12 bytes) ‖ texto cifrado + etiqueta GCM. El id de la
//! credencial va como dato autenticado: un archivo copiado sobre el de otro servidor no descifra.

use std::{fs, path::PathBuf, sync::Mutex};

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Key, Nonce,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{error::CommandError, ssh::host_keys::write_private_file};

const MAGIC: &[u8; 4] = b"BSV1";
const NONCE_LEN: usize = 12;
const AAD_PREFIX: &str = "blacksmith-vault-v1:";

/// Origen de la clave maestra. En producción, el llavero del sistema; en tests, memoria.
pub trait MasterKeyStore: Send + Sync {
    fn load(&self) -> Result<Option<Zeroizing<[u8; 32]>>, CommandError>;
    fn store(&self, key: &[u8; 32]) -> Result<(), CommandError>;
}

/// Lo que se guarda por servidor. `Debug` no muestra el material secreto.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credential {
    pub private_key: Zeroizing<String>,
    /// Solo si el usuario pidió recordarla.
    pub passphrase: Option<Zeroizing<String>>,
    pub summary: crate::ssh::key::KeySummary,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("private_key", &"[REDACTED]")
            .field(
                "passphrase",
                &self.passphrase.as_ref().map(|_| "[REDACTED]"),
            )
            .field("summary", &self.summary)
            .finish()
    }
}

pub struct Vault {
    dir: PathBuf,
    master: Box<dyn MasterKeyStore>,
    /// Se obtiene del llavero una sola vez por sesión.
    cached: Mutex<Option<Zeroizing<[u8; 32]>>>,
}

impl Vault {
    pub fn new(dir: PathBuf, master: Box<dyn MasterKeyStore>) -> Self {
        Self {
            dir,
            master,
            cached: Mutex::new(None),
        }
    }

    pub fn put(&self, id: &str, credential: &Credential) -> Result<(), CommandError> {
        let plaintext = Zeroizing::new(
            serde_json::to_vec(credential)
                .map_err(|error| CommandError::internal("serializando la credencial", error))?,
        );
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce)
            .map_err(|error| CommandError::internal("generando el nonce", error))?;
        let ciphertext = self
            .cipher()?
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: &plaintext,
                    aad: aad(id).as_bytes(),
                },
            )
            .map_err(|_| CommandError::internal("cifrando la credencial", "fallo de AES-GCM"))?;

        let mut file = Vec::with_capacity(MAGIC.len() + NONCE_LEN + ciphertext.len());
        file.extend_from_slice(MAGIC);
        file.extend_from_slice(&nonce);
        file.extend_from_slice(&ciphertext);
        write_private_file(&self.path(id), &file)
            .map_err(|error| CommandError::internal("guardando la credencial", error))
    }

    pub fn get(&self, id: &str) -> Result<Option<Credential>, CommandError> {
        let file = match fs::read(self.path(id)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(CommandError::internal("leyendo la credencial", error)),
        };
        let body = file
            .strip_prefix(MAGIC.as_slice())
            .ok_or(CommandError::VaultUnreadable)?;
        if body.len() <= NONCE_LEN {
            return Err(CommandError::VaultUnreadable);
        }
        let (nonce, ciphertext) = body.split_at(NONCE_LEN);
        let nonce: [u8; NONCE_LEN] = nonce
            .try_into()
            .map_err(|_| CommandError::VaultUnreadable)?;
        // Falla si se alteró el archivo, si es de otro servidor o si la clave maestra cambió.
        let plaintext = Zeroizing::new(
            self.cipher()?
                .decrypt(
                    &Nonce::from(nonce),
                    Payload {
                        msg: ciphertext,
                        aad: aad(id).as_bytes(),
                    },
                )
                .map_err(|_| CommandError::VaultUnreadable)?,
        );
        serde_json::from_slice(&plaintext)
            .map(Some)
            .map_err(|_| CommandError::VaultUnreadable)
    }

    pub fn delete(&self, id: &str) -> Result<bool, CommandError> {
        match fs::remove_file(self.path(id)) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(CommandError::internal("borrando la credencial", error)),
        }
    }

    /// El nombre del archivo no revela el servidor: es el SHA-256 de su id.
    fn path(&self, id: &str) -> PathBuf {
        let digest = Sha256::digest(id.as_bytes());
        self.dir
            .join(format!("{}.bin", data_encoding::HEXLOWER.encode(&digest)))
    }

    fn cipher(&self) -> Result<Aes256Gcm, CommandError> {
        let mut cached = self
            .cached
            .lock()
            .map_err(|_| CommandError::internal("bóveda", "mutex envenenado"))?;
        if cached.is_none() {
            let key = match self.master.load()? {
                Some(key) => key,
                None => {
                    let mut key = Zeroizing::new([0u8; 32]);
                    getrandom::fill(key.as_mut()).map_err(|error| {
                        CommandError::internal("generando la clave maestra", error)
                    })?;
                    self.master.store(&key)?;
                    key
                }
            };
            *cached = Some(key);
        }
        let key = cached
            .as_ref()
            .ok_or_else(|| CommandError::internal("bóveda", "sin clave maestra"))?;
        Ok(Aes256Gcm::new(&Key::<Aes256Gcm>::from(**key)))
    }
}

fn aad(id: &str) -> String {
    format!("{AAD_PREFIX}{id}")
}

/// Clave maestra en el llavero del sistema operativo.
pub struct KeyringMasterKey;

const KEYRING_SERVICE: &str = "com.blacksmith.client";
const KEYRING_ACCOUNT: &str = "vault-master-key";

impl KeyringMasterKey {
    /// Registra el llavero del sistema como almacén por defecto. Se llama una vez al arrancar.
    pub fn install() -> Result<Self, CommandError> {
        #[cfg(target_os = "linux")]
        let store = zbus_secret_service_keyring_store::Store::new()
            .map_err(|_| CommandError::KeyringUnavailable)?;
        #[cfg(windows)]
        let store = windows_native_keyring_store::Store::new()
            .map_err(|_| CommandError::KeyringUnavailable)?;
        #[cfg(any(target_os = "linux", windows))]
        keyring_core::set_default_store(store);
        Ok(Self)
    }

    fn entry() -> Result<keyring_core::Entry, CommandError> {
        keyring_core::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .map_err(|_| CommandError::KeyringUnavailable)
    }
}

impl MasterKeyStore for KeyringMasterKey {
    fn load(&self) -> Result<Option<Zeroizing<[u8; 32]>>, CommandError> {
        match Self::entry()?.get_secret() {
            Ok(secret) => {
                let secret = Zeroizing::new(secret);
                let key: [u8; 32] = secret
                    .as_slice()
                    .try_into()
                    .map_err(|_| CommandError::VaultUnreadable)?;
                Ok(Some(Zeroizing::new(key)))
            }
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(_) => Err(CommandError::KeyringUnavailable),
        }
    }

    fn store(&self, key: &[u8; 32]) -> Result<(), CommandError> {
        Self::entry()?
            .set_secret(key)
            .map_err(|_| CommandError::KeyringUnavailable)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::ssh::key::KeySummary;

    /// Clave maestra en memoria, para tests.
    #[derive(Default)]
    pub struct MemoryMasterKey(pub Mutex<Option<[u8; 32]>>);

    impl MasterKeyStore for MemoryMasterKey {
        fn load(&self) -> Result<Option<Zeroizing<[u8; 32]>>, CommandError> {
            Ok(self.0.lock().unwrap().map(Zeroizing::new))
        }
        fn store(&self, key: &[u8; 32]) -> Result<(), CommandError> {
            *self.0.lock().unwrap() = Some(*key);
            Ok(())
        }
    }

    fn credential() -> Credential {
        Credential {
            private_key: Zeroizing::new(
                "-----BEGIN OPENSSH PRIVATE KEY-----\nsecreto\n".to_owned(),
            ),
            passphrase: Some(Zeroizing::new("passphrase".to_owned())),
            summary: KeySummary {
                algorithm: "ssh-ed25519".into(),
                fingerprint: "SHA256:x".into(),
                encrypted: true,
            },
        }
    }

    #[test]
    fn guarda_cifrado_y_recupera() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::new(dir.path().to_path_buf(), Box::<MemoryMasterKey>::default());
        assert!(vault.get("deploy@vps:22").unwrap().is_none());
        vault.put("deploy@vps:22", &credential()).unwrap();

        let stored = vault.get("deploy@vps:22").unwrap().unwrap();
        assert_eq!(
            stored.private_key.as_str(),
            credential().private_key.as_str()
        );
        assert_eq!(stored.passphrase.unwrap().as_str(), "passphrase");

        // En disco no aparece nada en claro, ni el nombre del servidor.
        let entry = fs::read_dir(dir.path()).unwrap().next().unwrap().unwrap();
        assert!(!entry.file_name().to_string_lossy().contains("vps"));
        let bytes = fs::read(entry.path()).unwrap();
        for needle in ["secreto", "passphrase", "OPENSSH", "ssh-ed25519"] {
            assert!(
                !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "{needle} en claro"
            );
        }
        assert!(vault.delete("deploy@vps:22").unwrap());
        assert!(vault.get("deploy@vps:22").unwrap().is_none());
    }

    #[test]
    fn detecta_manipulacion_y_archivos_cambiados_de_servidor() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::new(dir.path().to_path_buf(), Box::<MemoryMasterKey>::default());
        vault.put("a@host:22", &credential()).unwrap();
        vault.put("b@host:22", &credential()).unwrap();

        // Un bit alterado.
        let path_a = vault.path("a@host:22");
        let mut bytes = fs::read(&path_a).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&path_a, &bytes).unwrap();
        assert!(matches!(
            vault.get("a@host:22"),
            Err(CommandError::VaultUnreadable)
        ));

        // El archivo de b copiado en el lugar de a: el dato autenticado no coincide.
        fs::copy(vault.path("b@host:22"), &path_a).unwrap();
        assert!(matches!(
            vault.get("a@host:22"),
            Err(CommandError::VaultUnreadable)
        ));
    }

    #[test]
    fn otra_clave_maestra_no_descifra() {
        let dir = tempfile::tempdir().unwrap();
        Vault::new(dir.path().to_path_buf(), Box::<MemoryMasterKey>::default())
            .put("a@h:22", &credential())
            .unwrap();
        let other = Vault::new(dir.path().to_path_buf(), Box::<MemoryMasterKey>::default());
        assert!(matches!(
            other.get("a@h:22"),
            Err(CommandError::VaultUnreadable)
        ));
    }

    /// Contra el llavero REAL del sistema (escribe y borra una entrada de test). Ignorado por
    /// defecto: `cargo test -p client -- --ignored llavero_real`.
    #[test]
    #[ignore = "usa el llavero real del sistema"]
    fn llavero_real_ida_y_vuelta() {
        KeyringMasterKey::install().expect("llavero del sistema disponible");
        let entry = keyring_core::Entry::new("com.blacksmith.client.test", "roundtrip").unwrap();
        let secret = [7u8; 32];
        entry.set_secret(&secret).unwrap();
        assert_eq!(entry.get_secret().unwrap(), secret);
        entry.delete_credential().unwrap();
        assert!(matches!(
            entry.get_secret(),
            Err(keyring_core::Error::NoEntry)
        ));
    }

    #[test]
    fn debug_no_muestra_secretos() {
        let shown = format!("{:?}", credential());
        assert!(!shown.contains("secreto") && !shown.contains("passphrase\""));
        assert!(shown.contains("[REDACTED]"));
    }
}
