//! Servidores configurados: el destino SSH de cada uno (`servers.json` en el directorio de datos).
//! No contiene secretos: la llave vive cifrada en la bóveda, indexada por el mismo id.

use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::{
    error::CommandError,
    ssh::{host_keys::write_private_file, target::SshTarget},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerProfile {
    /// `usuario@host:puerto`: el mismo id que la credencial y el túnel.
    pub id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
}

impl From<&SshTarget> for ServerProfile {
    fn from(target: &SshTarget) -> Self {
        Self {
            id: target.id(),
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

    /// Añade el servidor, o lo deja como está si ya existía.
    pub fn upsert(&self, target: &SshTarget) -> Result<ServerProfile, CommandError> {
        let _guard = self.guard()?;
        let mut store = self.load()?;
        let profile = ServerProfile::from(target);
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

    pub fn remove(&self, target: &SshTarget) -> Result<bool, CommandError> {
        let _guard = self.guard()?;
        let mut store = self.load()?;
        let before = store.servers.len();
        store.servers.retain(|server| server.id != target.id());
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

        store.upsert(&target("b.example.com")).unwrap();
        store.upsert(&target("a.example.com")).unwrap();
        store.upsert(&target("b.example.com")).unwrap();
        let ids: Vec<_> = store
            .list()
            .unwrap()
            .into_iter()
            .map(|server| server.id)
            .collect();
        assert_eq!(ids, ["deploy@b.example.com:22", "deploy@a.example.com:22"]);

        assert!(store.remove(&target("b.example.com")).unwrap());
        assert!(!store.remove(&target("b.example.com")).unwrap());
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn sobrevive_a_un_reinicio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("servers.json");
        ServerStore::new(path.clone())
            .upsert(&target("vps.example.com"))
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
