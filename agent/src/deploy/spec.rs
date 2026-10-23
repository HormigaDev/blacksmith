//! Especificación de un despliegue: lo que el agente deja para `bbel-deploy@<nombre>.service`
//! (`spec.json`, que la unidad recibe como credencial). Es el contrato entre el agente y
//! `run-deploy`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploySpec {
    /// Id de la versión (ver [`new_release_id`]). Lo asigna el agente, que así sabe qué versión
    /// está activa y cuál era la anterior sin leer el directorio del servicio.
    pub release: String,
    pub task: DeployTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeployTask {
    /// Clona, prepara y activa una versión nueva.
    Build(BuildSpec),
    /// Vuelve a activar una versión ya construida (rollback): no clona ni ejecuta nada.
    Activate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildSpec {
    /// URL de `git clone`: `https://github.com/<dueño>/<repo>.git` (acceso con cuenta) o
    /// `git@github.com:<dueño>/<repo>.git` (llave de despliegue).
    pub clone_url: String,
    pub branch: String,
    pub setup_commands: Vec<String>,
    pub start_command: String,
    /// `known_hosts` para clonar por SSH (llave de despliegue): las llaves de github.com.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_hosts: Option<String>,
    /// Carpeta (relativa al repositorio, ya validada) que se publica para nginx.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_dir: Option<String>,
}

/// Base de las URL de clon por SSH (acceso con llave de despliegue).
pub const GITHUB_SSH_BASE: &str = "git@github.com:";

/// Llaves de host de github.com publicadas por GitHub (`GET https://api.github.com/meta`,
/// `ssh_keys`). El clon por SSH las exige (`StrictHostKeyChecking=yes`): un GitHub suplantado no
/// pasa.
pub const GITHUB_KNOWN_HOSTS: &str = "\
github.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl
github.com ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBEmKSENjQEezOmxkZMy7opKgwFB9nkt5YRrYMjNuG5N87uRgg6CLrbo5wAdT/y6v0mKV0U2w0WZ2YB/++Tpockg=
github.com ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQCj7ndNxQowgcQnjshcLrqPEiiphnt+VTTvDP6mHBL9j1aNUkY4Ue1gvwnGLVlOhGeYrnZaMgRK6+PKCUXaDbC7qtbW8gIkhL7aGCsOr/C56SJMy/BCZfxd1nWzAOxSDPgVsmerOBYfNqltV9/hWCqBywINIR+5dIg6JTJ72pcEpEjcYgXkE2YEFXV1JHnsKgbLWNlhScqb2UmyRkQyytRLtL+38TGxkxCflmO+5Z8CSSNY7GidjMIZ7Q4zMjA2n1nGrlTDkzwDCsw+wqFPGQA179cnfGWOWRVruj16z6XyvxvjJwbz0wQZ75XK5tKSb7FNyeIEs4TT4jk+S4dhPeAUC5y+bDYirYgM4GC7uEnztnZyaVWQ7B381AK4Qdrwt51ZqExKbQpTUNn+EjqoTwvqNj4kqx5QUCI0ThS/YkOxJCXmPUWZbhjpCg56i+2aB6CmK2JGhn57K5mj0MNdBXA4/WnwH6XoPWJzK5Nyu2zB3nAZp+S5hpQs+p1vN1/wsjk=
";

/// Base de las URL de clon: `<base><dueño>/<repo>.git`. Por HTTPS: un repositorio público no
/// necesita credenciales, y uno privado usa el token de GitHub que el cliente dejó en el
/// servidor (credencial `github-token` de la unidad).
pub const GITHUB_CLONE_BASE: &str = "https://github.com/";

impl BuildSpec {
    pub fn github(
        clone_base: &str,
        owner: &str,
        name: &str,
        branch: &str,
        setup: Vec<String>,
        start: String,
    ) -> Self {
        Self {
            clone_url: format!("{clone_base}{owner}/{name}.git"),
            branch: branch.to_owned(),
            setup_commands: setup,
            start_command: start,
            known_hosts: None,
            publish_dir: None,
        }
    }

    /// Por SSH con la llave de despliegue: exige las llaves de host de github.com.
    pub fn with_ssh(mut self) -> Self {
        self.known_hosts = Some(GITHUB_KNOWN_HOSTS.to_owned());
        self
    }

    /// `dueño/repo` si la URL es de GitHub.
    pub fn github_repository(&self) -> Option<&str> {
        self.clone_url
            .strip_prefix(GITHUB_CLONE_BASE)
            .or_else(|| self.clone_url.strip_prefix(GITHUB_SSH_BASE))
            .and_then(|path| path.strip_suffix(".git"))
    }
}

/// Único y ordenable por fecha: `r<segundos Unix, 10 dígitos>-<nanosegundos, 9 dígitos>`. Dos
/// despliegues en el mismo segundo no comparten directorio.
pub fn new_release_id() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("r{:010}-{:09}", elapsed.as_secs(), elapsed.subsec_nanos())
}

/// Un id con la forma de [`new_release_id`]: nombra un directorio, así que no puede llevar nada
/// más (ni `/` ni `..`).
pub fn is_release_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() == 21
        && bytes.first() == Some(&b'r')
        && bytes.get(11) == Some(&b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 0 | 11) || byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_de_version() {
        let id = new_release_id();
        assert!(is_release_id(&id), "{id}");
        assert!(new_release_id() > id || new_release_id() >= id);
        for bad in [
            "",
            "r1",
            "../r0000000000-0",
            "r0000000000-00000000/",
            "x0000000000-000000000",
        ] {
            assert!(!is_release_id(bad), "{bad}");
        }
    }

    #[test]
    fn json_de_la_tarea() {
        let spec = DeploySpec {
            release: "r0000000001-000000002".into(),
            task: DeployTask::Activate,
        };
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(
            json,
            r#"{"release":"r0000000001-000000002","task":"activate"}"#
        );
        assert_eq!(serde_json::from_str::<DeploySpec>(&json).unwrap(), spec);
    }
}
