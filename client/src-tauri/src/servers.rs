//! Servidores configurados (`servers.json` en el directorio de datos). No contiene secretos: la
//! llave de un servidor SSH vive cifrada en la bóveda, indexada por el mismo id.
//!
//! Dos tipos:
//! - `ssh`: un VPS; se llega al agente por un túnel SSH.
//! - `local`: este mismo equipo (`localhost`); no hay túnel ni llave, se habla con el agente
//!   directamente en `127.0.0.1:<puerto>`. Sirve para probar el cliente con un agente local.

use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::{error::CommandError, files::write_private_file, ssh::target::SshTarget};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerKind {
    /// Los registros anteriores a los servidores locales no tienen `kind`: eran todos SSH.
    #[default]
    Ssh,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerProfile {
    /// SSH: `usuario@host:puerto` (el mismo id que la credencial y el túnel). Local: `local:<puerto>`.
    pub id: String,
    #[serde(default)]
    pub kind: ServerKind,
    pub host: String,
    /// SSH: puerto de SSH. Local: puerto del agente.
    pub port: u16,
    /// Vacío en un servidor local.
    #[serde(default)]
    pub username: String,
}

impl ServerProfile {
    pub fn local(agent_port: u16) -> Self {
        Self {
            id: format!("local:{agent_port}"),
            kind: ServerKind::Local,
            host: "localhost".to_owned(),
            port: agent_port,
            username: String::new(),
        }
    }
}

impl From<&SshTarget> for ServerProfile {
    fn from(target: &SshTarget) -> Self {
        Self {
            id: target.id(),
            kind: ServerKind::Ssh,
            host: target.host.clone(),
            port: target.port,
            username: target.username.clone(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoreFile {
    version: u32,
    servers: Vec<ServerProfile>,
}

pub struct ServerStore {
    path: PathBuf,
    /// Serializa lectura-modificación-escritura: dos altas a la vez no se pisan.
    lock: Mutex<()>,
}

impl ServerStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    /// En orden de alta.
    pub fn list(&self) -> Result<Vec<ServerProfile>, CommandError> {
        let _guard = self.guard()?;
        Ok(self.load()?.servers)
    }

    pub fn get(&self, id: &str) -> Result<Option<ServerProfile>, CommandError> {
        Ok(self.list()?.into_iter().find(|server| server.id == id))
    }

    /// Añade el servidor, o lo deja como está si ya existía.
    pub fn upsert(&self, profile: ServerProfile) -> Result<ServerProfile, CommandError> {
        let _guard = self.guard()?;
        let mut store = self.load()?;
        match store
            .servers
            .iter_mut()
            .find(|server| server.id == profile.id)
        {
            Some(existing) => *existing = profile.clone(),
            None => store.servers.push(profile.clone()),
        }
        self.save(&store)?;
        Ok(profile)
    }

    /// Nuevo orden de la lista. `ids` debe ser exactamente una permutación de los servidores
    /// guardados: ni falta ni sobra ninguno, ni hay repetidos.
    pub fn reorder(&self, ids: &[String]) -> Result<Vec<ServerProfile>, CommandError> {
        let _guard = self.guard()?;
        let mut store = self.load()?;
        let mut current: Vec<&str> = store
            .servers
            .iter()
            .map(|server| server.id.as_str())
            .collect();
        let mut requested: Vec<&str> = ids.iter().map(String::as_str).collect();
        current.sort_unstable();
        requested.sort_unstable();
        if current != requested {
            return Err(CommandError::InvalidTarget("order"));
        }
        store
            .servers
            .sort_by_key(|server| ids.iter().position(|id| *id == server.id));
        self.save(&store)?;
        Ok(store.servers)
    }

    pub fn remove(&self, id: &str) -> Result<bool, CommandError> {
        let _guard = self.guard()?;
        let mut store = self.load()?;
        let before = store.servers.len();
        store.servers.retain(|server| server.id != id);
        let removed = store.servers.len() != before;
        if removed {
            self.save(&store)?;
        }
        Ok(removed)
    }

    fn guard(&self) -> Result<std::sync::MutexGuard<'_, ()>, CommandError> {
        self.lock
            .lock()
            .map_err(|_| CommandError::internal("registro de servidores", "mutex envenenado"))
    }

    fn load(&self) -> Result<StoreFile, CommandError> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                CommandError::internal("leyendo el registro de servidores", error)
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(StoreFile {
                version: 1,
                ..Default::default()
            }),
            Err(error) => Err(CommandError::internal(
                "leyendo el registro de servidores",
                error,
            )),
        }
    }

    fn save(&self, store: &StoreFile) -> Result<(), CommandError> {
        let bytes = serde_json::to_vec_pretty(store).map_err(|error| {
            CommandError::internal("serializando el registro de servidores", error)
        })?;
        write_private_file(&self.path, &bytes)
            .map_err(|error| CommandError::internal("guardando el registro de servidores", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(host: &str) -> SshTarget {
        SshTarget::new(host, 22, "deploy").unwrap()
    }

    #[test]
    fn alta_sin_duplicados_en_orden_y_baja() {
        let dir = tempfile::tempdir().unwrap();
        let store = ServerStore::new(dir.path().join("servers.json"));
        assert!(store.list().unwrap().is_empty());

        store
            .upsert(ServerProfile::from(&target("b.example.com")))
            .unwrap();
        store
            .upsert(ServerProfile::from(&target("a.example.com")))
            .unwrap();
        store
            .upsert(ServerProfile::from(&target("b.example.com")))
            .unwrap();
        let ids: Vec<_> = store
            .list()
            .unwrap()
            .into_iter()
            .map(|server| server.id)
            .collect();
        assert_eq!(ids, ["deploy@b.example.com:22", "deploy@a.example.com:22"]);

        assert!(store.remove("deploy@b.example.com:22").unwrap());
        assert!(!store.remove("deploy@b.example.com:22").unwrap());
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn reordena_solo_con_una_permutacion_exacta() {
        let dir = tempfile::tempdir().unwrap();
        let store = ServerStore::new(dir.path().join("servers.json"));
        for host in ["a.example.com", "b.example.com", "c.example.com"] {
            store.upsert(ServerProfile::from(&target(host))).unwrap();
        }
        let id = |host: &str| format!("deploy@{host}:22");
        let order = [
            id("c.example.com"),
            id("a.example.com"),
            id("b.example.com"),
        ];
        let reordered: Vec<_> = store
            .reorder(&order)
            .unwrap()
            .into_iter()
            .map(|server| server.id)
            .collect();
        assert_eq!(reordered, order);
        assert_eq!(store.list().unwrap()[0].id, id("c.example.com"));

        for invalid in [
            vec![id("a.example.com"), id("b.example.com")],
            vec![
                id("a.example.com"),
                id("b.example.com"),
                id("b.example.com"),
            ],
            vec![
                id("a.example.com"),
                id("b.example.com"),
                id("x.example.com"),
            ],
            vec![
                id("a.example.com"),
                id("b.example.com"),
                id("c.example.com"),
                id("d.example.com"),
            ],
        ] {
            assert!(
                matches!(
                    store.reorder(&invalid),
                    Err(CommandError::InvalidTarget("order"))
                ),
                "{invalid:?}"
            );
        }
        // Un intento inválido no altera nada.
        assert_eq!(store.list().unwrap()[0].id, id("c.example.com"));
    }

    #[test]
    fn servidores_locales_y_registros_antiguos() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("servers.json");
        // Registro anterior a los servidores locales: sin `kind`.
        fs::write(
            &path,
            r#"{"version":1,"servers":[{"id":"deploy@vps:22","host":"vps","port":22,"username":"deploy"}]}"#,
        )
        .unwrap();
        let store = ServerStore::new(path);
        assert_eq!(store.list().unwrap()[0].kind, ServerKind::Ssh);

        store.upsert(ServerProfile::local(4040)).unwrap();
        let local = store.get("local:4040").unwrap().unwrap();
        assert_eq!(
            (local.kind, local.host.as_str(), local.port),
            (ServerKind::Local, "localhost", 4040)
        );
        assert!(store.remove("local:4040").unwrap());
    }

    #[test]
    fn sobrevive_a_un_reinicio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("servers.json");
        ServerStore::new(path.clone())
            .upsert(ServerProfile::from(&target("vps.example.com")))
            .unwrap();
        let profile = &ServerStore::new(path).list().unwrap()[0];
        assert_eq!(
            (
                profile.host.as_str(),
                profile.port,
                profile.username.as_str()
            ),
            ("vps.example.com", 22, "deploy")
        );
    }
}
