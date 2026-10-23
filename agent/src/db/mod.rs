//! Base de datos local del agente (SQLite).
//!
//! - Un único archivo `0600` en un directorio `0700` (en producción, `StateDirectory=` de la
//!   unidad). No se siguen enlaces simbólicos al abrirlo.
//! - Una sola conexión protegida por un mutex: el agente es el único escritor y las operaciones
//!   son pocas y cortas. Cada una se ejecuta en `spawn_blocking` para no bloquear el runtime.
//! - El esquema se crea si la base de datos está vacía. Una versión anterior se actualiza con las
//!   migraciones de [`MIGRATIONS`] (solo añaden tablas); si no hay camino, el agente no arranca.

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

use shared::{
    DeployKind, DeployRun, DeployStatus, Priority, RepositoryAccess, RepositoryRef, ResourceLimits,
    SiteConfig, SiteTemplate, TlsStatus, UpdateDeployment,
};

use crate::services::{Service, ServiceDescription, ServiceId, ServiceName, ServiceSlug};

/// Versión del esquema de `schema.sql`. Se guarda en `PRAGMA user_version`.
pub const SCHEMA_VERSION: i64 = 11;
const SCHEMA: &str = include_str!("schema.sql");

/// Migraciones `(versión de origen, SQL)`, en orden: cada una lleva a la siguiente versión. Una
/// base de datos nueva las ejecuta todas tras `schema.sql`.
const MIGRATIONS: [(i64, &str); 1] = [(10, include_str!("limits.sql"))];
/// Versión de `schema.sql` sin migraciones.
const BASE_VERSION: i64 = 10;

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

    #[error("ya hay un despliegue en curso")]
    DeployInProgress,

    #[error("otro servicio ya usa el dominio «{0}»")]
    DomainInUse(String),

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
        description: ServiceDescription,
    ) -> Result<Service, DbError> {
        self.call(move |connection| {
            let now = now();
            let inserted = connection.execute(
                "INSERT INTO services (slug, name, description, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![slug.as_str(), name.as_str(), description.as_str(), now],
            );
            match inserted {
                Ok(_) => Ok(Service {
                    id: ServiceId(connection.last_insert_rowid()),
                    slug,
                    name,
                    description,
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
                "SELECT id, slug, name, description, created_at, updated_at FROM services ORDER BY id",
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
                    "SELECT id, slug, name, description, created_at, updated_at FROM services WHERE id = ?1",
                    [id.0],
                    service_row,
                )
                .optional()?
                .transpose()
        })
        .await
    }

    pub async fn service_by_slug(&self, slug: ServiceSlug) -> Result<Option<Service>, DbError> {
        self.call(move |connection| {
            connection
                .query_row(
                    "SELECT id, slug, name, description, created_at, updated_at FROM services WHERE slug = ?1",
                    [slug.as_str()],
                    service_row,
                )
                .optional()?
                .transpose()
        })
        .await
    }

    /// Definición del despliegue de un servicio y cuándo se guardó; `None` si nunca se definió.
    pub async fn deployment(
        &self,
        id: ServiceId,
    ) -> Result<Option<(UpdateDeployment, i64)>, DbError> {
        self.call(move |connection| {
            connection
                .query_row(
                    "SELECT repo_owner, repo_name, repo_branch, setup_commands, start_command, updated_at, access, \
                     publish_dir FROM deployments WHERE service_id = ?1",
                    [id.0],
                    |row| {
                        let owner: Option<String> = row.get(0)?;
                        let name: Option<String> = row.get(1)?;
                        let branch: Option<String> = row.get(2)?;
                        let commands: String = row.get(3)?;
                        Ok((
                            (owner, name, branch),
                            commands,
                            row.get::<_, String>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, Option<String>>(7)?,
                        ))
                    },
                )
                .optional()?
                .map(|((owner, name, branch), commands, start_command, updated_at, access, publish_dir)| {
                    let repository = match (owner, name, branch) {
                        (Some(owner), Some(name), Some(branch)) => Some(RepositoryRef { owner, name, branch }),
                        _ => None,
                    };
                    let setup_commands: Vec<String> = serde_json::from_str(&commands)
                        .map_err(|error| DbError::Corrupt(format!("comandos de preparación: {error}")))?;
                    let access = match access.as_str() {
                        "account" => RepositoryAccess::Account,
                        "deployKey" => RepositoryAccess::DeployKey,
                        other => return Err(DbError::Corrupt(format!("acceso «{other}»"))),
                    };
                    Ok((
                        UpdateDeployment {
                            repository,
                            access,
                            setup_commands,
                            start_command,
                            publish_dir,
                        },
                        updated_at,
                    ))
                })
                .transpose()
        })
        .await
    }

    /// Guarda (sustituye) la definición del despliegue, ya validada. Devuelve la fecha guardada.
    pub async fn set_deployment(
        &self,
        id: ServiceId,
        deployment: UpdateDeployment,
    ) -> Result<i64, DbError> {
        self.call(move |connection| {
            let now = now();
            let commands = serde_json::to_string(&deployment.setup_commands)
                .map_err(|error| DbError::Corrupt(format!("comandos de preparación: {error}")))?;
            let repository = deployment.repository.as_ref();
            connection.execute(
                "INSERT INTO deployments \
                     (service_id, repo_owner, repo_name, repo_branch, setup_commands, start_command, updated_at, access, \
                      publish_dir) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
                 ON CONFLICT (service_id) DO UPDATE SET \
                     repo_owner = excluded.repo_owner, repo_name = excluded.repo_name, \
                     repo_branch = excluded.repo_branch, setup_commands = excluded.setup_commands, \
                     start_command = excluded.start_command, updated_at = excluded.updated_at, \
                     access = excluded.access, publish_dir = excluded.publish_dir",
                params![
                    id.0,
                    repository.map(|r| r.owner.as_str()),
                    repository.map(|r| r.name.as_str()),
                    repository.map(|r| r.branch.as_str()),
                    commands,
                    deployment.start_command,
                    now,
                    match deployment.access {
                        RepositoryAccess::Account => "account",
                        RepositoryAccess::DeployKey => "deployKey",
                    },
                    deployment.publish_dir
                ],
            )?;
            Ok(now)
        })
        .await
    }

    /// Registra un despliegue en curso. Falla con `DeployInProgress` si ya hay uno.
    pub async fn start_deploy_run(
        &self,
        id: ServiceId,
        kind: DeployKind,
        release: String,
        branch: String,
    ) -> Result<DeployRun, DbError> {
        self.call(move |connection| {
            let transaction = connection.transaction()?;
            let running: i64 = transaction.query_row(
                "SELECT count(*) FROM deploy_runs WHERE service_id = ?1 AND status = 'running'",
                [id.0],
                |row| row.get(0),
            )?;
            if running > 0 {
                return Err(DbError::DeployInProgress);
            }
            let now = now();
            transaction.execute(
                "INSERT INTO deploy_runs (service_id, status, kind, release, branch, started_at) \
                 VALUES (?1, 'running', ?2, ?3, ?4, ?5)",
                params![id.0, kind_text(kind), release, branch, now],
            )?;
            let run = DeployRun {
                id: transaction.last_insert_rowid(),
                status: DeployStatus::Running,
                kind,
                release,
                branch,
                started_at: now,
                finished_at: None,
                detail: None,
            };
            transaction.commit()?;
            Ok(run)
        })
        .await
    }

    pub async fn finish_deploy_run(
        &self,
        run: i64,
        status: DeployStatus,
        detail: Option<String>,
    ) -> Result<(), DbError> {
        self.call(move |connection| {
            connection.execute(
                "UPDATE deploy_runs SET status = ?2, finished_at = ?3, detail = ?4 WHERE id = ?1",
                params![run, status_text(status), now(), detail],
            )?;
            Ok(())
        })
        .await
    }

    /// Los últimos despliegues del servicio, del más reciente al más antiguo.
    pub async fn deploy_runs(&self, id: ServiceId, limit: u32) -> Result<Vec<DeployRun>, DbError> {
        self.call(move |connection| {
            let mut statement = connection.prepare(
                "SELECT id, status, kind, release, branch, started_at, finished_at, detail \
                 FROM deploy_runs WHERE service_id = ?1 ORDER BY id DESC LIMIT ?2",
            )?;
            let rows = statement.query_map(params![id.0, limit], |row| {
                Ok((
                    (row.get::<_, i64>(0)?, row.get::<_, String>(1)?),
                    (row.get::<_, String>(2)?, row.get::<_, String>(3)?),
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            })?;
            rows.map(|row| {
                let ((id, status), (kind, release), branch, started_at, finished_at, detail) = row?;
                Ok(DeployRun {
                    id,
                    status: parse_status(&status)?,
                    kind: parse_kind(&kind)?,
                    release,
                    branch,
                    started_at,
                    finished_at,
                    detail,
                })
            })
            .collect()
        })
        .await
    }

    /// La versión `release` quedó activa (despliegue o rollback): la que lo era pasa a ser la
    /// anterior, la del siguiente rollback.
    pub async fn activate_release(&self, id: ServiceId, release: String) -> Result<(), DbError> {
        self.call(move |connection| {
            connection.execute(
                "UPDATE services SET previous_release = active_release, active_release = ?2 \
                 WHERE id = ?1 AND active_release IS NOT ?2",
                params![id.0, release],
            )?;
            Ok(())
        })
        .await
    }

    /// Versión activa y anterior del servicio.
    pub async fn releases(&self, id: ServiceId) -> Result<Releases, DbError> {
        self.call(move |connection| {
            Ok(connection.query_row(
                "SELECT active_release, previous_release FROM services WHERE id = ?1",
                [id.0],
                |row| {
                    Ok(Releases {
                        active: row.get(0)?,
                        previous: row.get(1)?,
                    })
                },
            )?)
        })
        .await
    }

    /// Rama con la que se construyó una versión.
    pub async fn release_branch(
        &self,
        id: ServiceId,
        release: String,
    ) -> Result<Option<String>, DbError> {
        self.call(move |connection| {
            Ok(connection
                .query_row(
                    "SELECT branch FROM deploy_runs WHERE service_id = ?1 AND release = ?2 \
                     AND kind = 'deploy' LIMIT 1",
                    params![id.0, release],
                    |row| row.get(0),
                )
                .optional()?)
        })
        .await
    }

    /// Al arrancar el agente: los despliegues que quedaron «en curso» ya no tienen quien los
    /// siga. Devuelve cuántos se marcaron.
    pub async fn interrupt_running_deploys(&self) -> Result<usize, DbError> {
        self.call(|connection| {
            Ok(connection.execute(
                "UPDATE deploy_runs SET status = 'failed', finished_at = ?1, \
                 detail = 'interrumpido: el agente se reinició' WHERE status = 'running'",
                [now()],
            )?)
        })
        .await
    }

    /// Sitio del servicio, si tiene.
    pub async fn site(&self, id: ServiceId) -> Result<Option<StoredSite>, DbError> {
        self.call(move |connection| {
            connection
                .query_row(
                    "SELECT template, domains, port, https, email, tls, tls_detail, updated_at, routes \
                     FROM sites WHERE service_id = ?1",
                    [id.0],
                    |row| {
                        Ok((
                            (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                            row.get::<_, Option<u16>>(2)?,
                            row.get::<_, bool>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            (row.get::<_, String>(5)?, row.get::<_, Option<String>>(6)?),
                            (row.get::<_, i64>(7)?, row.get::<_, String>(8)?),
                        ))
                    },
                )
                .optional()?
                .map(
                    |((template, domains), port, https, email, (tls, tls_detail), (updated_at, routes))| {
                        let template = SiteTemplate::ALL
                            .into_iter()
                            .find(|known| known.file_stem() == template)
                            .ok_or_else(|| DbError::Corrupt(format!("plantilla «{template}»")))?;
                        let domains: Vec<String> = serde_json::from_str(&domains)
                            .map_err(|error| DbError::Corrupt(format!("dominios: {error}")))?;
                        let routes = serde_json::from_str(&routes)
                            .map_err(|error| DbError::Corrupt(format!("rutas: {error}")))?;
                        Ok(StoredSite {
                            config: SiteConfig {
                                template,
                                domains,
                                port,
                                https,
                                email,
                                routes,
                            },
                            tls: parse_tls(&tls)?,
                            tls_detail,
                            updated_at,
                        })
                    },
                )
                .transpose()
        })
        .await
    }

    /// Primer dominio de `domains` que ya usa otro servicio.
    pub async fn domain_in_use(
        &self,
        id: ServiceId,
        domains: Vec<String>,
    ) -> Result<Option<String>, DbError> {
        self.call(move |connection| {
            let mut statement = connection
                .prepare("SELECT 1 FROM site_domains WHERE domain = ?1 AND service_id <> ?2")?;
            for domain in domains {
                if statement.exists(params![domain, id.0])? {
                    return Ok(Some(domain));
                }
            }
            Ok(None)
        })
        .await
    }

    /// Guarda (sustituye) el sitio y reserva sus dominios. Falla con `DomainInUse` si otro
    /// servicio ya tiene alguno.
    pub async fn save_site(
        &self,
        id: ServiceId,
        config: SiteConfig,
        tls: TlsStatus,
    ) -> Result<i64, DbError> {
        self.call(move |connection| {
            let now = now();
            let domains = serde_json::to_string(&config.domains)
                .map_err(|error| DbError::Corrupt(format!("dominios: {error}")))?;
            let routes = serde_json::to_string(&config.routes)
                .map_err(|error| DbError::Corrupt(format!("rutas: {error}")))?;
            let transaction = connection.transaction()?;
            transaction.execute("DELETE FROM site_domains WHERE service_id = ?1", [id.0])?;
            for domain in &config.domains {
                match transaction.execute(
                    "INSERT INTO site_domains (domain, service_id) VALUES (?1, ?2)",
                    params![domain, id.0],
                ) {
                    Ok(_) => {}
                    Err(rusqlite::Error::SqliteFailure(error, _))
                        if error.code == ErrorCode::ConstraintViolation =>
                    {
                        return Err(DbError::DomainInUse(domain.clone()));
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            transaction.execute(
                "INSERT INTO sites (service_id, template, domains, port, https, email, tls, tls_detail, updated_at, routes) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9) \
                 ON CONFLICT (service_id) DO UPDATE SET template = excluded.template, \
                     domains = excluded.domains, port = excluded.port, https = excluded.https, \
                     email = excluded.email, tls = excluded.tls, tls_detail = NULL, \
                     updated_at = excluded.updated_at, routes = excluded.routes",
                params![
                    id.0,
                    config.template.file_stem(),
                    domains,
                    config.port,
                    config.https,
                    config.email,
                    tls_text(tls),
                    now,
                    routes
                ],
            )?;
            transaction.commit()?;
            Ok(now)
        })
        .await
    }

    /// Estado del certificado del sitio.
    pub async fn set_site_tls(
        &self,
        id: ServiceId,
        tls: TlsStatus,
        detail: Option<String>,
    ) -> Result<(), DbError> {
        self.call(move |connection| {
            connection.execute(
                "UPDATE sites SET tls = ?2, tls_detail = ?3, updated_at = ?4 WHERE service_id = ?1",
                params![id.0, tls_text(tls), detail, now()],
            )?;
            Ok(())
        })
        .await
    }

    /// Borra el sitio y libera sus dominios. Devuelve si existía.
    pub async fn delete_site(&self, id: ServiceId) -> Result<bool, DbError> {
        self.call(move |connection| {
            let transaction = connection.transaction()?;
            transaction.execute("DELETE FROM site_domains WHERE service_id = ?1", [id.0])?;
            let deleted =
                transaction.execute("DELETE FROM sites WHERE service_id = ?1", [id.0])? > 0;
            transaction.commit()?;
            Ok(deleted)
        })
        .await
    }

    /// Límites guardados del servicio y cuándo se guardaron; `None` si no tiene.
    pub async fn limits(&self, id: ServiceId) -> Result<Option<(ResourceLimits, i64)>, DbError> {
        self.call(move |connection| {
            connection
                .query_row(
                    "SELECT cpu_percent, priority, memory_max, swap_max, tasks_max, updated_at \
                     FROM service_limits WHERE service_id = ?1",
                    [id.0],
                    |row| {
                        Ok((
                            row.get::<_, Option<u32>>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                            row.get::<_, Option<i64>>(3)?,
                            row.get::<_, Option<u32>>(4)?,
                            row.get::<_, i64>(5)?,
                        ))
                    },
                )
                .optional()?
                .map(
                    |(cpu_percent, priority, memory_max, swap_max, tasks_max, updated_at)| {
                        let priority = match priority.as_str() {
                            "low" => Priority::Low,
                            "normal" => Priority::Normal,
                            "high" => Priority::High,
                            other => return Err(DbError::Corrupt(format!("prioridad «{other}»"))),
                        };
                        let bytes = |value: Option<i64>| {
                            value
                                .map(u64::try_from)
                                .transpose()
                                .map_err(|_| DbError::Corrupt("límite de memoria negativo".into()))
                        };
                        let (memory_max, swap_max) = (bytes(memory_max)?, bytes(swap_max)?);
                        Ok((
                            ResourceLimits {
                                cpu_percent,
                                priority,
                                memory_max_bytes: memory_max,
                                swap_max_bytes: swap_max,
                                tasks_max,
                            },
                            updated_at,
                        ))
                    },
                )
                .transpose()
        })
        .await
    }

    /// Guarda los límites (ya validados) y devuelve cuándo.
    pub async fn set_limits(&self, id: ServiceId, limits: ResourceLimits) -> Result<i64, DbError> {
        self.call(move |connection| {
            let updated_at = now();
            let priority = match limits.priority {
                Priority::Low => "low",
                Priority::Normal => "normal",
                Priority::High => "high",
            };
            // SQLite guarda enteros con signo; los límites validados caben de sobra.
            let bytes = |value: Option<u64>| value.map(|value| i64::try_from(value).unwrap_or(i64::MAX));
            connection.execute(
                "INSERT INTO service_limits \
                     (service_id, cpu_percent, priority, memory_max, swap_max, tasks_max, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT (service_id) DO UPDATE SET cpu_percent = ?2, priority = ?3, \
                     memory_max = ?4, swap_max = ?5, tasks_max = ?6, updated_at = ?7",
                params![
                    id.0,
                    limits.cpu_percent,
                    priority,
                    bytes(limits.memory_max_bytes),
                    bytes(limits.swap_max_bytes),
                    limits.tasks_max,
                    updated_at
                ],
            )?;
            Ok(updated_at)
        })
        .await
    }

    /// Marca si el servicio debe estar en marcha (ver `autostart` en schema.sql).
    pub async fn set_autostart(&self, slug: ServiceSlug, enabled: bool) -> Result<(), DbError> {
        self.call(move |connection| {
            connection.execute(
                "UPDATE services SET autostart = ?1 WHERE slug = ?2",
                rusqlite::params![enabled, slug.as_str()],
            )?;
            Ok(())
        })
        .await
    }

    /// Servicios que deben estar en marcha.
    pub async fn autostart_services(&self) -> Result<Vec<ServiceSlug>, DbError> {
        self.call(|connection| {
            let mut statement =
                connection.prepare("SELECT slug FROM services WHERE autostart = 1 ORDER BY id")?;
            let slugs = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(slugs
                .into_iter()
                .filter_map(|slug| ServiceSlug::new(&slug).ok())
                .collect())
        })
        .await
    }

    /// Devuelve si existía.
    pub async fn delete_service_by_slug(&self, slug: ServiceSlug) -> Result<bool, DbError> {
        self.call(move |connection| {
            Ok(connection.execute("DELETE FROM services WHERE slug = ?1", [slug.as_str()])? > 0)
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
    let description: String = row.get(3)?;
    let service = (|| {
        Ok(Service {
            id: ServiceId(row.get(0)?),
            slug: ServiceSlug::new(&slug)
                .map_err(|error| DbError::Corrupt(format!("slug «{slug}»: {error}")))?,
            name: ServiceName::new(&name)
                .map_err(|error| DbError::Corrupt(format!("nombre «{name}»: {error}")))?,
            description: ServiceDescription::new(&description)
                .map_err(|error| DbError::Corrupt(format!("descripción: {error}")))?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })();
    Ok(service)
}

fn status_text(status: DeployStatus) -> &'static str {
    match status {
        DeployStatus::Running => "running",
        DeployStatus::Succeeded => "succeeded",
        DeployStatus::Failed => "failed",
    }
}

fn kind_text(kind: DeployKind) -> &'static str {
    match kind {
        DeployKind::Deploy => "deploy",
        DeployKind::Rollback => "rollback",
    }
}

fn parse_kind(raw: &str) -> Result<DeployKind, DbError> {
    match raw {
        "deploy" => Ok(DeployKind::Deploy),
        "rollback" => Ok(DeployKind::Rollback),
        other => Err(DbError::Corrupt(format!("tipo de despliegue «{other}»"))),
    }
}

/// Sitio guardado de un servicio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSite {
    pub config: SiteConfig,
    pub tls: TlsStatus,
    pub tls_detail: Option<String>,
    pub updated_at: i64,
}

fn tls_text(tls: TlsStatus) -> &'static str {
    match tls {
        TlsStatus::Off => "off",
        TlsStatus::Issuing => "issuing",
        TlsStatus::Active => "active",
        TlsStatus::Failed => "failed",
    }
}

fn parse_tls(raw: &str) -> Result<TlsStatus, DbError> {
    match raw {
        "off" => Ok(TlsStatus::Off),
        "issuing" => Ok(TlsStatus::Issuing),
        "active" => Ok(TlsStatus::Active),
        "failed" => Ok(TlsStatus::Failed),
        other => Err(DbError::Corrupt(format!("estado TLS «{other}»"))),
    }
}

/// Versión activa y anterior de un servicio (ver `active_release` en schema.sql).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Releases {
    pub active: Option<String>,
    pub previous: Option<String>,
}

fn parse_status(raw: &str) -> Result<DeployStatus, DbError> {
    match raw {
        "running" => Ok(DeployStatus::Running),
        "succeeded" => Ok(DeployStatus::Succeeded),
        "failed" => Ok(DeployStatus::Failed),
        other => Err(DbError::Corrupt(format!("estado de despliegue «{other}»"))),
    }
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
            for (_, migration) in MIGRATIONS {
                transaction.execute_batch(migration)?;
            }
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            transaction.commit()?;
            tracing::info!(
                version = SCHEMA_VERSION,
                "esquema de la base de datos creado"
            );
            Ok(())
        }
        found if (BASE_VERSION..SCHEMA_VERSION).contains(&found) => {
            let transaction = connection.transaction()?;
            for (_, migration) in MIGRATIONS.iter().filter(|(from, _)| *from >= found) {
                transaction.execute_batch(migration)?;
            }
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            transaction.commit()?;
            tracing::info!(
                from = found,
                to = SCHEMA_VERSION,
                "esquema de la base de datos actualizado"
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
    async fn sitios_y_dominios_unicos() {
        let db = Database::open_in_memory().unwrap();
        let web = db
            .insert_service(slug("web"), name("Web"), ServiceDescription::default())
            .await
            .unwrap();
        let api = db
            .insert_service(slug("api"), name("API"), ServiceDescription::default())
            .await
            .unwrap();
        let config = |domains: &[&str]| SiteConfig {
            template: SiteTemplate::Spa,
            domains: domains.iter().map(|domain| (*domain).to_owned()).collect(),
            port: None,
            https: true,
            email: Some("ops@example.com".into()),
            routes: vec![shared::SiteRoute {
                path: "/api/".into(),
                port: 3000,
                websocket: false,
            }],
        };
        assert_eq!(db.site(web.id).await.unwrap(), None);
        db.save_site(
            web.id,
            config(&["example.com", "www.example.com"]),
            TlsStatus::Issuing,
        )
        .await
        .unwrap();
        let site = db.site(web.id).await.unwrap().unwrap();
        assert_eq!(site.config, config(&["example.com", "www.example.com"]));
        assert_eq!(site.tls, TlsStatus::Issuing);

        // Otro servicio no puede quedarse un dominio en uso…
        assert_eq!(
            db.domain_in_use(
                api.id,
                vec!["api.example.com".into(), "www.example.com".into()]
            )
            .await
            .unwrap()
            .as_deref(),
            Some("www.example.com")
        );
        assert!(matches!(
            db.save_site(api.id, config(&["www.example.com"]), TlsStatus::Off).await,
            Err(DbError::DomainInUse(domain)) if domain == "www.example.com"
        ));
        // …pero sí cuando el primero lo libera.
        db.save_site(web.id, config(&["example.com"]), TlsStatus::Off)
            .await
            .unwrap();
        db.save_site(api.id, config(&["www.example.com"]), TlsStatus::Off)
            .await
            .unwrap();

        db.set_site_tls(web.id, TlsStatus::Failed, Some("dns".into()))
            .await
            .unwrap();
        let site = db.site(web.id).await.unwrap().unwrap();
        assert_eq!(
            (site.tls, site.tls_detail.as_deref()),
            (TlsStatus::Failed, Some("dns"))
        );
        assert!(db.delete_site(web.id).await.unwrap());
        assert_eq!(
            db.domain_in_use(api.id, vec!["example.com".into()])
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn arranque_automatico() {
        let db = Database::open_in_memory().unwrap();
        for service in ["api", "web", "bot"] {
            db.insert_service(slug(service), name(service), ServiceDescription::default())
                .await
                .unwrap();
        }
        assert!(db.autostart_services().await.unwrap().is_empty());
        db.set_autostart(slug("bot"), true).await.unwrap();
        db.set_autostart(slug("api"), true).await.unwrap();
        db.set_autostart(slug("web"), true).await.unwrap();
        db.set_autostart(slug("web"), false).await.unwrap();
        assert_eq!(
            db.autostart_services().await.unwrap(),
            [slug("api"), slug("bot")]
        );
    }

    #[tokio::test]
    async fn alta_consulta_y_baja() {
        let db = Database::open_in_memory().unwrap();
        let api = db
            .insert_service(slug("api"), name("API"), ServiceDescription::default())
            .await
            .unwrap();
        let web = db
            .insert_service(slug("web"), name("Web"), ServiceDescription::default())
            .await
            .unwrap();

        assert_eq!(db.services().await.unwrap(), [api.clone(), web.clone()]);
        assert_eq!(db.service(api.id).await.unwrap(), Some(api.clone()));
        assert_eq!(
            db.service_by_slug(slug("api")).await.unwrap(),
            Some(api.clone())
        );
        assert_eq!(db.service_by_slug(slug("otra")).await.unwrap(), None);
        assert!(db.delete_service(api.id).await.unwrap());
        assert!(!db.delete_service(api.id).await.unwrap());
        assert_eq!(db.service(api.id).await.unwrap(), None);
        assert_eq!(db.services().await.unwrap(), [web]);
    }

    #[tokio::test]
    async fn despliegue_se_guarda_y_se_borra_con_el_servicio() {
        let db = Database::open_in_memory().unwrap();
        let service = db
            .insert_service(slug("api"), name("API"), ServiceDescription::default())
            .await
            .unwrap();
        assert_eq!(db.deployment(service.id).await.unwrap(), None);
        let deployment = UpdateDeployment {
            repository: Some(RepositoryRef {
                owner: "o".into(),
                name: "r".into(),
                branch: "main".into(),
            }),
            access: RepositoryAccess::DeployKey,
            setup_commands: vec!["npm ci".into(), "npm run build".into()],
            start_command: "npm start".into(),
            publish_dir: Some("dist".into()),
        };
        db.set_deployment(service.id, deployment.clone())
            .await
            .unwrap();
        assert_eq!(
            db.deployment(service.id).await.unwrap().unwrap().0,
            deployment
        );
        // Sustituir, también quitando el repositorio.
        let without = UpdateDeployment {
            repository: None,
            ..deployment
        };
        db.set_deployment(service.id, without.clone())
            .await
            .unwrap();
        assert_eq!(db.deployment(service.id).await.unwrap().unwrap().0, without);

        assert!(db.delete_service(service.id).await.unwrap());
        assert_eq!(db.deployment(service.id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn historial_de_despliegues() {
        let db = Database::open_in_memory().unwrap();
        let service = db
            .insert_service(slug("api"), name("API"), ServiceDescription::default())
            .await
            .unwrap();
        let release = |n: u32| format!("r{n:010}-000000000");
        let first = db
            .start_deploy_run(service.id, DeployKind::Deploy, release(1), "main".into())
            .await
            .unwrap();
        assert!(matches!(
            db.start_deploy_run(service.id, DeployKind::Deploy, release(2), "main".into())
                .await,
            Err(DbError::DeployInProgress)
        ));
        db.finish_deploy_run(first.id, DeployStatus::Failed, Some("exit 1".into()))
            .await
            .unwrap();
        let second = db
            .start_deploy_run(service.id, DeployKind::Rollback, release(3), "dev".into())
            .await
            .unwrap();
        assert_eq!(db.interrupt_running_deploys().await.unwrap(), 1);
        let runs = db.deploy_runs(service.id, 10).await.unwrap();
        assert_eq!(
            runs.iter().map(|run| run.id).collect::<Vec<_>>(),
            [second.id, first.id]
        );
        assert_eq!(runs[0].status, DeployStatus::Failed);
        assert_eq!(runs[0].kind, DeployKind::Rollback);
        assert_eq!(runs[1].release, release(1));
        assert_eq!(runs[1].detail.as_deref(), Some("exit 1"));
        assert_eq!(
            db.release_branch(service.id, release(1))
                .await
                .unwrap()
                .as_deref(),
            Some("main")
        );
    }

    /// La versión activa pasa a ser la anterior al activar otra; repetir la activa no la pierde.
    #[tokio::test]
    async fn version_activa_y_anterior() {
        let db = Database::open_in_memory().unwrap();
        let service = db
            .insert_service(slug("api"), name("API"), ServiceDescription::default())
            .await
            .unwrap();
        let release = |n: u32| format!("r{n:010}-000000000");
        assert_eq!(db.releases(service.id).await.unwrap(), Releases::default());
        db.activate_release(service.id, release(1)).await.unwrap();
        db.activate_release(service.id, release(2)).await.unwrap();
        let releases = db.releases(service.id).await.unwrap();
        assert_eq!(releases.active, Some(release(2)));
        assert_eq!(releases.previous, Some(release(1)));
        // Rollback: vuelve a la 1 y la 2 queda como anterior.
        db.activate_release(service.id, release(1)).await.unwrap();
        db.activate_release(service.id, release(1)).await.unwrap();
        let releases = db.releases(service.id).await.unwrap();
        assert_eq!(releases.active, Some(release(1)));
        assert_eq!(releases.previous, Some(release(2)));
    }

    #[tokio::test]
    async fn los_ids_borrados_no_se_reutilizan() {
        let db = Database::open_in_memory().unwrap();
        let first = db
            .insert_service(slug("a"), name("A"), ServiceDescription::default())
            .await
            .unwrap();
        db.delete_service(first.id).await.unwrap();
        let second = db
            .insert_service(slug("a"), name("A"), ServiceDescription::default())
            .await
            .unwrap();
        assert!(second.id > first.id);
    }

    #[tokio::test]
    async fn slug_unico() {
        let db = Database::open_in_memory().unwrap();
        db.insert_service(slug("api"), name("API"), ServiceDescription::default())
            .await
            .unwrap();
        let error = db
            .insert_service(slug("api"), name("Otra"), ServiceDescription::default())
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
            for slug in [
                "",
                "API",
                "-a",
                "a-",
                "a_b",
                "a.b",
                "agent-permission-check",
                &"a".repeat(49),
            ] {
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
                .block_on(db.insert_service(
                    slug("api"),
                    name("API"),
                    ServiceDescription::default(),
                ))
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

    /// Una base de datos de la versión 10 (sin límites) se actualiza sin perder nada.
    #[tokio::test]
    async fn migra_desde_la_version_anterior() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.db");
        let db = Database::open(&path).unwrap();
        let service = db
            .insert_service(
                ServiceSlug::new("api").unwrap(),
                ServiceName::new("API").unwrap(),
                ServiceDescription::default(),
            )
            .await
            .unwrap();
        drop(db);
        // Como la dejaba la versión 10.
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("DROP TABLE service_limits")
            .unwrap();
        connection.pragma_update(None, "user_version", 10).unwrap();
        drop(connection);

        let db = Database::open(&path).unwrap();
        assert_eq!(db.services().await.unwrap().len(), 1);
        assert_eq!(db.limits(service.id).await.unwrap(), None);
        db.set_limits(service.id, ResourceLimits::default())
            .await
            .unwrap();
        let version: i64 = Connection::open(&path)
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn limites_del_servicio() {
        let db = Database::open_in_memory().unwrap();
        let service = db
            .insert_service(
                ServiceSlug::new("api").unwrap(),
                ServiceName::new("API").unwrap(),
                ServiceDescription::default(),
            )
            .await
            .unwrap();
        assert_eq!(db.limits(service.id).await.unwrap(), None);
        let limits = ResourceLimits {
            cpu_percent: Some(150),
            priority: Priority::High,
            memory_max_bytes: Some(512 * 1024 * 1024),
            swap_max_bytes: Some(0),
            tasks_max: None,
        };
        db.set_limits(service.id, limits).await.unwrap();
        assert_eq!(db.limits(service.id).await.unwrap().unwrap().0, limits);
        // Se sustituyen y se borran con el servicio.
        db.set_limits(service.id, ResourceLimits::default())
            .await
            .unwrap();
        assert_eq!(
            db.limits(service.id).await.unwrap().unwrap().0,
            ResourceLimits::default()
        );
        db.delete_service(service.id).await.unwrap();
        assert_eq!(db.limits(service.id).await.unwrap(), None);
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
