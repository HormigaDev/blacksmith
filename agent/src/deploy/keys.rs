//! Llave de despliegue de un servicio (opcional, acceso `deployKey`): ed25519, generada en el
//! servidor con `ssh-keygen` y de solo lectura en GitHub. La privada nunca sale del servidor
//! (`0600`, en el directorio del agente) y solo la recibe `blacksmith-deploy@<nombre>.service`, como
//! credencial y durante el despliegue; la pública se muestra para añadirla al repositorio
//! (*Settings → Deploy keys*).

use std::{
    fs,
    os::unix::fs::PermissionsExt,
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
/// (p. ej. `blacksmith-api@servidor`).
pub fn ensure(dir: &Path, comment: &str) -> Result<DeployKey, KeyError> {
    if let Some(key) = read(dir)? {
        return Ok(key);
    }
    // Se genera aparte y se coloca con un `rename` atómico: si dos peticiones llegan a la vez,
    // gana la primera y la otra devuelve esa misma llave (la pública siempre corresponde a la
    // privada guardada).
    let staging = super::files::temporary_path(dir);
    super::files::private_dir(&staging)?;
    let generated = generate(&staging, comment).and_then(|()| {
        // Sustituye un directorio vacío (restos de un intento anterior) pero no uno con llave.
        match fs::rename(&staging, dir) {
            Ok(()) => Ok(()),
            Err(_) if read(dir)?.is_some() => Ok(()),
            Err(error) => Err(file_error(dir)(error)),
        }
    });
    let _ = fs::remove_dir_all(&staging);
    generated?;
    read(dir)?.ok_or_else(|| KeyError::Generate("la llave no apareció tras generarla".into()))
}

fn generate(dir: &Path, comment: &str) -> Result<(), KeyError> {
    let private = dir.join(PRIVATE_KEY);
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
    fs::set_permissions(&private, fs::Permissions::from_mode(0o600)).map_err(file_error(&private))
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

    /// Dos peticiones a la vez: las dos devuelven la misma llave, la que quedó guardada.
    #[test]
    fn generaciones_simultaneas() {
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("deployments/api/key");
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let keys = keys.clone();
                std::thread::spawn(move || ensure(&keys, "blacksmith-api@test").unwrap())
            })
            .collect();
        let results: Vec<DeployKey> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let stored = read(&keys).unwrap().unwrap();
        for key in results {
            assert_eq!(key.public_key, stored.public_key);
        }
        // Sin temporales sueltos.
        assert_eq!(fs::read_dir(keys.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn genera_una_vez_y_privada() {
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("deployments/api/key");
        assert!(read(&keys).unwrap().is_none());
        let key = ensure(&keys, "blacksmith-api@prueba").unwrap();
        assert!(key.public_key.starts_with("ssh-ed25519 "));
        assert!(key.public_key.ends_with(" blacksmith-api@prueba"));
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
