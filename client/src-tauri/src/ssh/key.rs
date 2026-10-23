//! Llaves privadas SSH: de un archivo elegido por el usuario, del contenido pegado o de las llaves
//! por defecto de `~/.ssh`. Se validan aquí; el material nunca vuelve a la webview.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use russh::keys::{decode_secret_key, Algorithm, HashAlg, PrivateKey};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::CommandError;

/// Una llave privada en texto (OpenSSH, PKCS#1, PKCS#8 o PuTTY) nunca pasa de unos pocos KB.
pub const MAX_KEY_BYTES: u64 = 64 * 1024;

/// Orden de OpenSSH para las llaves por defecto (las más modernas primero).
pub const DEFAULT_KEY_FILES: [&str; 3] = ["id_ed25519", "id_ecdsa", "id_rsa"];

/// De dónde viene la llave que el usuario quiere usar.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum KeySource {
    /// La primera de `~/.ssh/{id_ed25519,id_ecdsa,id_rsa}` que exista.
    Default,
    /// Ruta elegida con el FilePicker.
    File { path: PathBuf },
    /// Contenido pegado en el textarea.
    Inline { content: Zeroizing<String> },
}

/// Lo único que se muestra de una llave: nunca el material privado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySummary {
    pub algorithm: String,
    /// Huella SHA-256 de la llave pública, en el formato de OpenSSH (`SHA256:…`).
    pub fingerprint: String,
    /// Si la llave está protegida con passphrase.
    pub encrypted: bool,
}

/// Lee el texto de la llave según su origen.
pub fn read_source(
    source: KeySource,
    home: Option<&Path>,
) -> Result<Zeroizing<String>, CommandError> {
    match source {
        KeySource::Inline { content } => {
            if content.len() as u64 > MAX_KEY_BYTES {
                return Err(CommandError::KeyTooLarge);
            }
            Ok(content)
        }
        KeySource::File { path } => read_key_file(&path),
        KeySource::Default => {
            let ssh_dir = home.ok_or(CommandError::KeyNotFound)?.join(".ssh");
            let path = DEFAULT_KEY_FILES
                .iter()
                .map(|name| ssh_dir.join(name))
                .find(|path| path.is_file())
                .ok_or(CommandError::KeyNotFound)?;
            read_key_file(&path)
        }
    }
}

fn read_key_file(path: &Path) -> Result<Zeroizing<String>, CommandError> {
    let file = File::open(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => CommandError::KeyNotFound,
        _ => CommandError::KeyFileUnreadable,
    })?;
    let metadata = file
        .metadata()
        .map_err(|_| CommandError::KeyFileUnreadable)?;
    if !metadata.is_file() {
        return Err(CommandError::KeyFileUnreadable);
    }
    if metadata.len() > MAX_KEY_BYTES {
        return Err(CommandError::KeyTooLarge);
    }
    // Reservado de antemano: si la cadena creciera al leer, la copia anterior quedaría sin borrar.
    let mut content = Zeroizing::new(String::with_capacity(
        usize::try_from(metadata.len()).unwrap_or(0) + 1,
    ));
    file.take(MAX_KEY_BYTES)
        .read_to_string(&mut content)
        .map_err(|_| CommandError::KeyInvalid)?;
    Ok(content)
}

/// Decodifica la llave, descifrándola si hace falta, y distingue cada fallo para la interfaz.
pub fn decode(pem: &str, passphrase: Option<&str>) -> Result<PrivateKey, CommandError> {
    let trimmed = pem.trim_start();
    if looks_like_public_key(trimmed) {
        return Err(CommandError::KeyIsPublic);
    }
    match decode_secret_key(pem, None) {
        Ok(key) => Ok(key),
        Err(russh::keys::Error::KeyIsEncrypted) => match passphrase.filter(|p| !p.is_empty()) {
            None => Err(CommandError::PassphraseRequired),
            Some(passphrase) => decode_secret_key(pem, Some(passphrase))
                .map_err(|_| CommandError::PassphraseInvalid),
        },
        Err(
            russh::keys::Error::UnsupportedKeyType { .. } | russh::keys::Error::UnknownAlgorithm(_),
        ) => Err(CommandError::KeyUnsupported),
        Err(_) => Err(CommandError::KeyInvalid),
    }
}

/// `true` si la llave necesita passphrase (sin intentar descifrarla).
pub fn is_encrypted(pem: &str) -> bool {
    matches!(
        decode_secret_key(pem, None),
        Err(russh::keys::Error::KeyIsEncrypted)
    )
}

pub fn summary(key: &PrivateKey, encrypted: bool) -> KeySummary {
    KeySummary {
        algorithm: algorithm_name(&key.algorithm()),
        fingerprint: key.public_key().fingerprint(HashAlg::Sha256).to_string(),
        encrypted,
    }
}

pub fn algorithm_name(algorithm: &Algorithm) -> String {
    algorithm.as_str().to_owned()
}

/// Un error frecuente: pegar o elegir el `.pub` en lugar de la llave privada.
fn looks_like_public_key(text: &str) -> bool {
    text.starts_with("ssh-")
        || text.starts_with("ecdsa-sha2-")
        || text.starts_with("sk-")
        || text.starts_with("-----BEGIN SSH2 PUBLIC KEY-----")
        || text.starts_with("-----BEGIN PUBLIC KEY-----")
}

#[cfg(test)]
pub(crate) mod fixtures {
    pub const CLIENT_ED25519: &str = include_str!("fixtures/client_ed25519");
    pub const CLIENT_ED25519_PUB: &str = include_str!("fixtures/client_ed25519.pub");
    pub const CLIENT_ED25519_ENCRYPTED: &str = include_str!("fixtures/client_ed25519_encrypted");
    pub const PASSPHRASE: &str = "correct horse battery staple";
    pub const CLIENT_RSA: &str = include_str!("fixtures/client_rsa");
    pub const CLIENT_RSA_PKCS1: &str = include_str!("fixtures/client_rsa_pkcs1");
    pub const CLIENT_ECDSA: &str = include_str!("fixtures/client_ecdsa");
    pub const INTRUDER_ED25519: &str = include_str!("fixtures/intruder_ed25519");
    pub const HOST_ED25519: &str = include_str!("fixtures/host_ed25519");
    pub const HOST_IMPOSTOR_ED25519: &str = include_str!("fixtures/host_impostor_ed25519");
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{fixtures::*, *};

    #[test]
    fn decodifica_los_formatos_habituales() {
        for (name, pem, algorithm) in [
            ("ed25519", CLIENT_ED25519, "ssh-ed25519"),
            ("rsa openssh", CLIENT_RSA, "ssh-rsa"),
            ("rsa pkcs1", CLIENT_RSA_PKCS1, "ssh-rsa"),
            ("ecdsa", CLIENT_ECDSA, "ecdsa-sha2-nistp256"),
        ] {
            let key = decode(pem, None).unwrap_or_else(|error| panic!("{name}: {error}"));
            let info = summary(&key, false);
            assert_eq!(info.algorithm, algorithm, "{name}");
            assert!(info.fingerprint.starts_with("SHA256:"), "{name}");
        }
    }

    #[test]
    fn distingue_passphrase_ausente_e_incorrecta() {
        assert!(is_encrypted(CLIENT_ED25519_ENCRYPTED));
        assert!(!is_encrypted(CLIENT_ED25519));
        assert!(matches!(
            decode(CLIENT_ED25519_ENCRYPTED, None),
            Err(CommandError::PassphraseRequired)
        ));
        assert!(matches!(
            decode(CLIENT_ED25519_ENCRYPTED, Some("")),
            Err(CommandError::PassphraseRequired)
        ));
        assert!(matches!(
            decode(CLIENT_ED25519_ENCRYPTED, Some("incorrecta")),
            Err(CommandError::PassphraseInvalid)
        ));
        assert!(decode(CLIENT_ED25519_ENCRYPTED, Some(PASSPHRASE)).is_ok());
    }

    #[test]
    fn detecta_llaves_publicas_y_basura() {
        assert!(matches!(
            decode(CLIENT_ED25519_PUB, None),
            Err(CommandError::KeyIsPublic)
        ));
        assert!(matches!(
            decode("hola", None),
            Err(CommandError::KeyInvalid)
        ));
        assert!(matches!(
            decode(
                "-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----",
                None
            ),
            Err(CommandError::KeyInvalid)
        ));
    }

    #[test]
    fn busca_la_llave_por_defecto_en_el_orden_de_openssh() {
        let home = tempfile::tempdir().unwrap();
        assert!(matches!(
            read_source(KeySource::Default, Some(home.path())),
            Err(CommandError::KeyNotFound)
        ));
        assert!(matches!(
            read_source(KeySource::Default, None),
            Err(CommandError::KeyNotFound)
        ));

        let ssh = home.path().join(".ssh");
        fs::create_dir(&ssh).unwrap();
        fs::write(ssh.join("id_rsa"), CLIENT_RSA).unwrap();
        assert_eq!(
            read_source(KeySource::Default, Some(home.path()))
                .unwrap()
                .as_str(),
            CLIENT_RSA
        );
        fs::write(ssh.join("id_ed25519"), CLIENT_ED25519).unwrap();
        assert_eq!(
            read_source(KeySource::Default, Some(home.path()))
                .unwrap()
                .as_str(),
            CLIENT_ED25519
        );
    }

    #[test]
    fn limita_el_tamaño_y_valida_el_archivo() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big");
        fs::write(&big, vec![b'a'; MAX_KEY_BYTES as usize + 1]).unwrap();
        assert!(matches!(
            read_source(KeySource::File { path: big }, None),
            Err(CommandError::KeyTooLarge)
        ));
        assert!(matches!(
            read_source(
                KeySource::File {
                    path: dir.path().join("no-existe")
                },
                None
            ),
            Err(CommandError::KeyNotFound)
        ));
        assert!(matches!(
            read_source(
                KeySource::File {
                    path: dir.path().to_path_buf()
                },
                None
            ),
            Err(CommandError::KeyFileUnreadable)
        ));
        let inline = Zeroizing::new("a".repeat(MAX_KEY_BYTES as usize + 1));
        assert!(matches!(
            read_source(KeySource::Inline { content: inline }, None),
            Err(CommandError::KeyTooLarge)
        ));
    }
}
