//! Base de datos local del agente (SQLite).
//!
//! - Un único archivo `0600` en un directorio `0700` (en producción, `StateDirectory=` de la
//!   unidad). No se siguen enlaces simbólicos al abrirlo.
//! - Una sola conexión protegida por un mutex: el agente es el único escritor y las operaciones
//!   son pocas y cortas. Cada una se ejecuta en `spawn_blocking` para no bloquear el runtime.
//! - Sin migraciones: el esquema se crea si la base de datos está vacía y, si tiene otra
//!   versión, el agente no arranca (ver `schema.sql`).

use std::{
    fmt, fs, io,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{
    Connection, ErrorCode, OpenFlags, OptionalExtension, Row, config::DbConfig, params,
};

use crate::services::{Service, ServiceId, ServiceName, ServiceSlug};

/// Versión del esquema de `schema.sql`. Se guarda en `PRAGMA user_version`.
pub const SCHEMA_VERSION: i64 = 1;
const SCHEMA: &str = include_str!("schema.sql");

/// Caché de páginas pequeña: el agente debe quedarse por debajo de 20 MB en reposo.
const CACHE_KIB: i64 = 512;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("no se pudo preparar {path}")]
    Prepare {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("{path} tiene permisos {mode:03o}; debe ser accesible solo por su dueño (0600)")]
    Permissions { path: PathBuf, mode: u32 },

    #[error(
        "la base de datos tiene el esquema v{found} y este agente usa v{expected}; \
         no hay migraciones: bórrela y se creará de nuevo"
    )]
    SchemaMismatch { found: i64, expected: i64 },

    #[error("la base de datos tiene tablas pero ninguna versión de esquema: no es del agente")]
    Foreign,

    #[error("ya existe un servicio con el identificador «{0}»")]
    DuplicateSlug(ServiceSlug),

    /// Un valor guardado no cumple las reglas de su tipo: la base de datos se modificó por fuera.
    #[error("dato inválido en la base de datos: {0}")]
    Corrupt(String),

    #[error("la conexión quedó inutilizable tras un pánico")]
    Poisoned,

    #[error("la tarea de base de datos se interrumpió")]
    Task(#[from] tokio::task::JoinError),

    #[error("error de SQLite")]
    Sqlite(#[from] rusqlite::Error),
}

impl DbError {
    /// Errores que no se arreglan reintentando (el agente debe salir con `EX_CONFIG`).
    pub fn is_configuration(&self) -> bool {
        matches!(
            self,
            Self::Permissions { .. } | Self::SchemaMismatch { .. } | Self::Foreign
        )
    }
}

/// Barato de clonar: comparte la misma conexión.
#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database").finish_non_exhaustive()
    }
}

impl Database {
    /// Abre (o crea) la base de datos. Es síncrono: se llama al arrancar, antes del runtime.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        prepare_file(path)?;
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let connection = Connection::open_with_flags(path, flags)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(connection)
    }

    /// Base de datos en memoria, para tests.
    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut connection: Connection) -> Result<Self, DbError> {
        // Endurecimiento: sin escrituras directas al esquema ni funciones peligrosas en él.
        connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
        connection.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "cache_size", -CACHE_KIB)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        ensure_schema(&mut connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    async fn call<T, F>(&self, operation: F) -> Result<T, DbError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, DbError> + Send + 'static,
    {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            let mut connection = connection.lock().map_err(|_| DbError::Poisoned)?;
            operation(&mut connection)
        })
        .await?
    }

    pub async fn insert_service(
        &self,
        slug: ServiceSlug,
        name: ServiceName,
    ) -> Result<Service, DbError> {
        self.call(move |connection| {
            let now = now();
            let inserted = connection.execute(
                "INSERT INTO services (slug, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
                params![slug.as_str(), name.as_str(), now],
            );
            match inserted {
                Ok(_) => Ok(Service {
                    id: ServiceId(connection.last_insert_rowid()),
                    slug,
                    name,
                    created_at: now,
                    updated_at: now,
                }),
                Err(error) if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) => {
                    Err(DbError::DuplicateSlug(slug))
                }
                Err(error) => Err(error.into()),
            }
        })
        .await
    }

    /// Todos los servicios registrados, en orden de alta.
    pub async fn services(&self) -> Result<Vec<Service>, DbError> {
        self.call(|connection| {
            let mut statement = connection.prepare(
                "SELECT id, slug, name, created_at, updated_at FROM services ORDER BY id",
            )?;
            let rows = statement.query_map([], service_row)?;
            rows.map(|row| row?).collect()
        })
        .await
    }

    pub async fn service(&self, id: ServiceId) -> Result<Option<Service>, DbError> {
        self.call(move |connection| {
            connection
                .query_row(
                    "SELECT id, slug, name, created_at, updated_at FROM services WHERE id = ?1",
                    [id.0],
                    service_row,
                )
                .optional()?
                .transpose()
        })
        .await
    }

    /// Devuelve si existía.
    pub async fn delete_service(&self, id: ServiceId) -> Result<bool, DbError> {
        self.call(move |connection| {
            Ok(connection.execute("DELETE FROM services WHERE id = ?1", [id.0])? > 0)
        })
        .await
    }
}

/// Fila → [`Service`], revalidando cada campo con las reglas de su tipo.
fn service_row(row: &Row<'_>) -> rusqlite::Result<Result<Service, DbError>> {
    let slug: String = row.get(1)?;
    let name: String = row.get(2)?;
    let service = (|| {
        Ok(Service {
            id: ServiceId(row.get(0)?),
            slug: ServiceSlug::new(&slug)
                .map_err(|error| DbError::Corrupt(format!("slug «{slug}»: {error}")))?,
            name: ServiceName::new(&name)
                .map_err(|error| DbError::Corrupt(format!("nombre «{name}»: {error}")))?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })();
    Ok(service)
}

fn ensure_schema(connection: &mut Connection) -> Result<(), DbError> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        SCHEMA_VERSION => Ok(()),
        0 => {
            let tables: i64 = connection.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type = 'table'",
                [],
                |row| row.get(0),
            )?;
            if tables > 0 {
                return Err(DbError::Foreign);
            }
            let transaction = connection.transaction()?;
            transaction.execute_batch(SCHEMA)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            transaction.commit()?;
            tracing::info!(
                version = SCHEMA_VERSION,
                "esquema de la base de datos creado"
            );
            Ok(())
        }
        found => Err(DbError::SchemaMismatch {
            found,
            expected: SCHEMA_VERSION,
        }),
    }
}

/// Crea el directorio (`0700`) y el archivo (`0600`) si faltan, y rechaza un archivo existente
/// que otros puedan leer. SQLite crea `-wal` y `-shm` con los mismos permisos que el archivo.
fn prepare_file(path: &Path) -> Result<(), DbError> {
    let prepare = |source| DbError::Prepare {
        path: path.to_owned(),
        source,
    };
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(prepare)?;
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        // Jamás truncar: el archivo puede ser la base de datos existente.
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .map_err(prepare)?;
    let metadata = file.metadata().map_err(prepare)?;
    let mode = metadata.mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(DbError::Permissions {
            path: path.to_owned(),
            mode,
        });
    }
    Ok(())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn slug(slug: &str) -> ServiceSlug {
        ServiceSlug::new(slug).unwrap()
    }

    fn name(name: &str) -> ServiceName {
        ServiceName::new(name).unwrap()
    }

    #[tokio::test]
    async fn alta_consulta_y_baja() {
        let db = Database::open_in_memory().unwrap();
        let api = db.insert_service(slug("api"), name("API")).await.unwrap();
        let web = db.insert_service(slug("web"), name("Web")).await.unwrap();

        assert_eq!(db.services().await.unwrap(), [api.clone(), web.clone()]);
        assert_eq!(db.service(api.id).await.unwrap(), Some(api.clone()));
        assert!(db.delete_service(api.id).await.unwrap());
        assert!(!db.delete_service(api.id).await.unwrap());
        assert_eq!(db.service(api.id).await.unwrap(), None);
        assert_eq!(db.services().await.unwrap(), [web]);
    }

    #[tokio::test]
    async fn los_ids_borrados_no_se_reutilizan() {
        let db = Database::open_in_memory().unwrap();
        let first = db.insert_service(slug("a"), name("A")).await.unwrap();
        db.delete_service(first.id).await.unwrap();
        let second = db.insert_service(slug("a"), name("A")).await.unwrap();
        assert!(second.id > first.id);
    }

    #[tokio::test]
    async fn slug_unico() {
        let db = Database::open_in_memory().unwrap();
        db.insert_service(slug("api"), name("API")).await.unwrap();
        let error = db
            .insert_service(slug("api"), name("Otra"))
            .await
            .unwrap_err();
        assert!(
            matches!(error, DbError::DuplicateSlug(ref s) if s.as_str() == "api"),
            "{error:?}"
        );
    }

    /// Las restricciones del esquema rechazan lo que la validación de Rust rechazaría, aunque se
    /// escriba con SQL directo.
    #[tokio::test]
    async fn el_esquema_aplica_las_mismas_reglas() {
        let db = Database::open_in_memory().unwrap();
        db.call(|connection| {
            for slug in ["", "API", "-a", "a-", "a_b", "a.b", &"a".repeat(49)] {
                let result = connection.execute(
                    "INSERT INTO services (slug, name, created_at, updated_at) VALUES (?1, 'x', 0, 0)",
                    [slug],
                );
                assert!(result.is_err(), "{slug:?} debería rechazarse");
            }
            let result = connection.execute(
                "INSERT INTO services (slug, name, created_at, updated_at) VALUES ('ok', '', 0, 0)",
                [],
            );
            assert!(result.is_err());
            // STRICT: tipos exactos.
            let result = connection.execute(
                "INSERT INTO services (slug, name, created_at, updated_at) VALUES ('ok', 'x', 'ayer', 0)",
                [],
            );
            assert!(result.is_err());
            Ok(())
        })
        .await
        .unwrap();
    }

    #[test]
    fn crea_archivo_privado_y_conserva_los_datos() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("datos/agent.db");
        {
            let db = Database::open(&path).unwrap();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            runtime
                .block_on(db.insert_service(slug("api"), name("API")))
                .unwrap();
        }
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);

        let db = Database::open(&path).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert_eq!(runtime.block_on(db.services()).unwrap().len(), 1);
    }

    #[test]
    fn rechaza_permisos_abiertos_y_enlaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.db");
        fs::write(&path, b"").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let error = Database::open(&path).unwrap_err();
        assert!(
            matches!(error, DbError::Permissions { mode: 0o644, .. }),
            "{error:?}"
        );
        assert!(error.is_configuration());

        let link = dir.path().join("enlace.db");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(matches!(
            Database::open(&link),
            Err(DbError::Prepare { .. })
        ));
    }

    #[test]
    fn otra_version_de_esquema_no_arranca() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.db");
        drop(Database::open(&path).unwrap());
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        let error = Database::open(&path).unwrap_err();
        assert!(matches!(error, DbError::SchemaMismatch { .. }), "{error:?}");
        assert!(error.is_configuration());
    }

    #[test]
    fn base_de_datos_ajena_no_arranca() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.db");
        fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE otra (x)")
            .unwrap();
        assert!(matches!(Database::open(&path), Err(DbError::Foreign)));
    }
}
