//! Acceso del servidor a GitHub: la cuenta y el token que el cliente deja aquí para que los
//! despliegues descarguen repositorios privados sin que el portátil intervenga después.
//!
//! - El token es el de la GitHub App del cliente: solo lectura de contenido, de los repositorios
//!   donde esté instalada.
//! - Vive en `%S/blacksmith/github.json` (`0600`, directorio del agente): los servicios no lo ven
//!   (su sandbox oculta `%S/blacksmith`). A cada despliegue llega como credencial de systemd.
//! - Nunca se devuelve por la API ni se escribe en logs.
//! - El agente no habla con GitHub (no tiene salida a internet): quien usa el token es `git`
//!   dentro de la unidad de despliegue.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use shared::{GitHubAccess, SetGitHubAccess};

use crate::deploy::files::{self, FileError};

/// Login de GitHub: letras, dígitos y guiones, hasta 39 caracteres, sin guion en los extremos.
const LOGIN_MAX_LEN: usize = 39;
/// Los tokens de GitHub (`ghu_…`, `gho_…`, `github_pat_…`) son alfanuméricos con `_`.
const TOKEN_MAX_LEN: usize = 255;
const TOKEN_MIN_LEN: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum GitHubAccessError {
    #[error("login de GitHub inválido")]
    InvalidLogin,
    #[error("token de GitHub inválido")]
    InvalidToken,
    #[error(transparent)]
    File(#[from] FileError),
    #[error("credencial de GitHub ilegible: {0}")]
    Corrupt(String),
    #[error("tarea interrumpida: {0}")]
    Join(#[from] tokio::task::JoinError),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    login: String,
    token: String,
    saved_at: i64,
}

#[derive(Debug, Clone)]
pub struct GitHubStore {
    path: PathBuf,
}

impl GitHubStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Cuenta guardada, sin el token.
    pub async fn access(&self) -> Result<Option<GitHubAccess>, GitHubAccessError> {
        Ok(self.load().await?.map(|stored| GitHubAccess {
            login: stored.login,
            saved_at: stored.saved_at,
        }))
    }

    /// Token guardado (para preparar un despliegue).
    pub async fn token(&self) -> Result<Option<String>, GitHubAccessError> {
        Ok(self.load().await?.map(|stored| stored.token))
    }

    /// Guarda (o sustituye) la cuenta y su token.
    pub async fn save(&self, access: SetGitHubAccess) -> Result<GitHubAccess, GitHubAccessError> {
        validate_login(&access.login)?;
        validate_token(&access.token)?;
        let stored = Stored {
            login: access.login,
            token: access.token,
            saved_at: crate::unix_now(),
        };
        let json = serde_json::to_vec(&stored)
            .map_err(|error| GitHubAccessError::Corrupt(error.to_string()))?;
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || files::write_private(&path, &json)).await??;
        tracing::info!(login = %stored.login, "acceso a GitHub guardado");
        Ok(GitHubAccess {
            login: stored.login,
            saved_at: stored.saved_at,
        })
    }

    /// Olvida la cuenta: los repositorios privados dejan de poder desplegarse.
    pub async fn remove(&self) -> Result<(), GitHubAccessError> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || files::remove(&path)).await??;
        tracing::info!("acceso a GitHub eliminado");
        Ok(())
    }

    async fn load(&self) -> Result<Option<Stored>, GitHubAccessError> {
        let path = self.path.clone();
        let Some(bytes) = tokio::task::spawn_blocking(move || files::read_private(&path)).await??
        else {
            return Ok(None);
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| GitHubAccessError::Corrupt(error.to_string()))
    }
}

fn validate_login(login: &str) -> Result<(), GitHubAccessError> {
    valid_login(login)
        .then_some(())
        .ok_or(GitHubAccessError::InvalidLogin)
}

/// Usuario u organización de GitHub: letras, dígitos y guiones, sin guion en los extremos, hasta
/// 39 caracteres (la regla de GitHub).
pub fn valid_login(login: &str) -> bool {
    (1..=LOGIN_MAX_LEN).contains(&login.len())
        && login
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !login.starts_with('-')
        && !login.ends_with('-')
}

/// Solo caracteres de token: lo que llega al helper de git no puede llevar saltos de línea ni
/// nada que el shell interprete.
fn validate_token(token: &str) -> Result<(), GitHubAccessError> {
    let valid = (TOKEN_MIN_LEN..=TOKEN_MAX_LEN).contains(&token.len())
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    valid.then_some(()).ok_or(GitHubAccessError::InvalidToken)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use super::*;

    fn access(login: &str, token: &str) -> SetGitHubAccess {
        SetGitHubAccess {
            login: login.into(),
            token: token.into(),
        }
    }

    #[tokio::test]
    async fn guarda_privado_y_nunca_devuelve_el_token() {
        let dir = tempfile::tempdir().unwrap();
        let store = GitHubStore::new(dir.path().join("blacksmith/github.json"));
        assert_eq!(store.access().await.unwrap(), None);

        let saved = store
            .save(access("octocat", "ghu_0123456789abcdefghijKLMNOP"))
            .await
            .unwrap();
        assert_eq!(saved.login, "octocat");
        assert_eq!(store.access().await.unwrap(), Some(saved));
        assert_eq!(
            store.token().await.unwrap().as_deref(),
            Some("ghu_0123456789abcdefghijKLMNOP")
        );
        let mode = std::fs::metadata(dir.path().join("blacksmith/github.json"))
            .unwrap()
            .mode();
        assert_eq!(mode & 0o777, 0o600);

        store.remove().await.unwrap();
        assert_eq!(store.token().await.unwrap(), None);
    }

    #[tokio::test]
    async fn rechaza_valores_que_no_son_de_github() {
        let dir = tempfile::tempdir().unwrap();
        let store = GitHubStore::new(dir.path().join("github.json"));
        for (login, token) in [
            ("octocat", "corto"),
            ("octocat", "ghu_0123456789abcdef\nexit"),
            ("octocat", "ghu_0123456789abcdef$(id)"),
            ("-octo", "ghu_0123456789abcdefghij"),
            ("octo cat", "ghu_0123456789abcdefghij"),
        ] {
            assert!(
                store.save(access(login, token)).await.is_err(),
                "{login} {token:?}"
            );
        }
        assert_eq!(store.access().await.unwrap(), None);
    }
}
