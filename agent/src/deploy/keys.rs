//! Llave de despliegue de un servicio (opcional, acceso `deployKey`): ed25519, generada en el
//! servidor con `ssh-keygen` y de solo lectura en GitHub. La privada nunca sale del servidor
//! (`0600`, en el directorio del agente) y solo la recibe `bbel-deploy@<nombre>.service`, como
//! credencial y durante el despliegue; la pública se muestra para añadirla al repositorio
//! (*Settings → Deploy keys*).

use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::Path,
    process::{Command, Stdio},
};

use shared::DeployKey;

use super::files::FileError;

const SSH_KEYGEN: &str = "/usr/bin/ssh-keygen";
pub const PRIVATE_KEY: &str = "deploy_key";
const PUBLIC_KEY: &str = "deploy_key.pub";

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("no se pudo generar la llave de despliegue: {0}")]
    Generate(String),
    #[error(transparent)]
    File(#[from] FileError),
}

fn file_error(path: &Path) -> impl FnOnce(std::io::Error) -> KeyError + '_ {
    move |source| {
        KeyError::File(FileError {
            path: path.to_owned(),
            source,
        })
    }
}

/// Llave existente en `dir`, si la hay.
pub fn read(dir: &Path) -> Result<Option<DeployKey>, KeyError> {
    let public = dir.join(PUBLIC_KEY);
    if !dir.join(PRIVATE_KEY).exists() {
        return Ok(None);
    }
    let public_key = fs::read_to_string(&public).map_err(file_error(&public))?;
    Ok(Some(DeployKey {
        public_key: public_key.trim().to_owned(),
        fingerprint: fingerprint(&public)?,
    }))
}

/// Genera la llave si no existe (idempotente) y la devuelve. `comment` la identifica en GitHub
/// (p. ej. `bbel-api@servidor`).
pub fn ensure(dir: &Path, comment: &str) -> Result<DeployKey, KeyError> {
    if let Some(key) = read(dir)? {
        return Ok(key);
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(file_error(dir))?;
    let private = dir.join(PRIVATE_KEY);
    // Restos de un intento anterior: ssh-keygen no sobrescribe sin preguntar.
    let _ = fs::remove_file(&private);
    let _ = fs::remove_file(dir.join(PUBLIC_KEY));
    let output = Command::new(SSH_KEYGEN)
        .env_clear()
        .args(["-q", "-t", "ed25519", "-N", "", "-C", comment, "-f"])
        .arg(&private)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| KeyError::Generate(error.to_string()))?;
    if !output.status.success() {
        return Err(KeyError::Generate(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    fs::set_permissions(&private, fs::Permissions::from_mode(0o600))
        .map_err(file_error(&private))?;
    read(dir)?.ok_or_else(|| KeyError::Generate("la llave no apareció tras generarla".into()))
}

/// Parte privada (para la credencial del despliegue).
pub fn private(dir: &Path) -> Result<Option<Vec<u8>>, KeyError> {
    Ok(super::files::read_private(&dir.join(PRIVATE_KEY))?)
}

/// `SHA256:…` de la llave pública (`ssh-keygen -l`).
fn fingerprint(public: &Path) -> Result<String, KeyError> {
    let output = Command::new(SSH_KEYGEN)
        .env_clear()
        .args(["-l", "-E", "sha256", "-f"])
        .arg(public)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| KeyError::Generate(error.to_string()))?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .find(|part| part.starts_with("SHA256:"))
        .map(str::to_owned)
        .ok_or_else(|| KeyError::Generate(format!("huella ilegible: {}", text.trim())))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use super::*;

    #[test]
    fn genera_una_vez_y_privada() {
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("deployments/api/key");
        assert!(read(&keys).unwrap().is_none());
        let key = ensure(&keys, "bbel-api@prueba").unwrap();
        assert!(key.public_key.starts_with("ssh-ed25519 "));
        assert!(key.public_key.ends_with(" bbel-api@prueba"));
        assert!(key.fingerprint.starts_with("SHA256:"));
        assert_eq!(
            fs::metadata(keys.join(PRIVATE_KEY)).unwrap().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::metadata(&keys).unwrap().mode() & 0o777, 0o700);
        assert!(
            String::from_utf8(private(&keys).unwrap().unwrap())
                .unwrap()
                .contains("PRIVATE KEY")
        );
        // Idempotente: la misma llave.
        assert_eq!(ensure(&keys, "otro").unwrap(), key);
    }
}
