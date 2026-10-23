//! Definición del despliegue de un servicio (`/api/v1/services/{name}/deployment`): de qué
//! repositorio sale, cómo se accede a él, qué comandos lo preparan y cómo se arranca.

use serde::{Deserialize, Serialize};

/// Repositorio de GitHub y rama que se despliega.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryRef {
    pub owner: String,
    pub name: String,
    pub branch: String,
}

/// Cómo descarga el servidor el repositorio.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryAccess {
    /// Por HTTPS: sin credenciales si es público; con la cuenta de GitHub que el cliente dejó en
    /// el servidor (`GET /api/v1/github`) si es privado.
    #[default]
    Account,
    /// Por SSH, con una llave de solo lectura de este repositorio, generada en el servidor
    /// (`/api/v1/services/{name}/deploy-key`). No depende de ninguna cuenta.
    DeployKey,
}

/// Respuesta de `GET` y `PUT /api/v1/services/{name}/deployment`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDeployment {
    pub name: String,
    /// `None` hasta que se elija uno.
    pub repository: Option<RepositoryRef>,
    pub access: RepositoryAccess,
    /// Directorio aislado del servicio en el servidor. Lo asigna el agente: no es configurable.
    pub working_directory: String,
    /// Se ejecutan en orden, en `working_directory`, antes de arrancar (p. ej. `npm ci`).
    pub setup_commands: Vec<String>,
    /// Cómo se arranca, relativo a `working_directory` (p. ej. `./bin/api` o `npm start`).
    /// Vacío mientras no se defina, o en un sitio solo estático (con `publish_dir`).
    pub start_command: String,
    /// Carpeta del repositorio, tras la preparación, que se publica para nginx (p. ej. `dist`).
    /// `None` si el servicio no sirve archivos estáticos.
    pub publish_dir: Option<String>,
    /// Segundos Unix de la última modificación; `None` si nunca se guardó.
    pub updated_at: Option<i64>,
}

/// Cuerpo de `PUT /api/v1/services/{name}/deployment`: sustituye la definición entera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeployment {
    pub repository: Option<RepositoryRef>,
    #[serde(default)]
    pub access: RepositoryAccess,
    pub setup_commands: Vec<String>,
    pub start_command: String,
    #[serde(default)]
    pub publish_dir: Option<String>,
}

/// Llave de despliegue del servicio (solo la parte pública): se añade al repositorio como
/// *deploy key* de solo lectura. La privada nunca sale del servidor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployKey {
    pub public_key: String,
    /// `SHA256:…`
    pub fingerprint: String,
}

/// Cuenta de GitHub con la que el servidor descarga los repositorios privados
/// (`GET /api/v1/github`). El token nunca sale del servidor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubAccess {
    pub login: String,
    /// Segundos Unix.
    pub saved_at: i64,
}

/// Cuerpo de `PUT /api/v1/github`: el cliente deja en el servidor el token de su GitHub App
/// (solo lectura de contenido). `Debug` no muestra el token.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetGitHubAccess {
    pub login: String,
    pub token: String,
}

impl std::fmt::Debug for SetGitHubAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SetGitHubAccess")
            .field("login", &self.login)
            .field("token", &"<oculto>")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeployKind {
    /// Versión nueva: clona, prepara y activa.
    Deploy,
    /// Vuelta a la versión anterior, ya construida.
    Rollback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeployStatus {
    Running,
    Succeeded,
    Failed,
}

/// Un despliegue (`POST /api/v1/services/{name}/deploy`, `GET …/deploys`). Su salida está en los
/// logs de `bbel-deploy@<nombre>.service` (`…/logs/ws?source=deploy`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployRun {
    pub id: i64,
    pub status: DeployStatus,
    pub kind: DeployKind,
    /// Versión que construye o a la que vuelve (`r<segundos>-<nanos>`).
    pub release: String,
    pub branch: String,
    /// Segundos Unix.
    pub started_at: i64,
    pub finished_at: Option<i64>,
    /// Motivo del fallo, si falló.
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_token_no_aparece_en_debug() {
        let access = SetGitHubAccess {
            login: "octocat".into(),
            token: "ghu_secreto".into(),
        };
        let shown = format!("{access:?}");
        assert!(shown.contains("octocat"));
        assert!(!shown.contains("ghu_secreto"));
    }
}
