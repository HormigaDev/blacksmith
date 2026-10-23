//! Servicios gestionados por el agente.
//!
//! Dos reglas que no dependen de la buena voluntad del código que llame:
//!
//! 1. **Para el cliente solo existe lo registrado.** El agente nunca lista unidades de systemd:
//!    los servicios salen de la base de datos y las acciones se piden por `id` de registro. Una
//!    unidad desplegada a mano fuera del agente no aparece ni se puede nombrar desde el cliente.
//! 2. **Solo se gestionan unidades con el prefijo reservado.** La unidad de un servicio se deriva
//!    de su [`ServiceSlug`] (`blacksmith@<slug>.service`); nunca se acepta un nombre de unidad del
//!    cliente. La regla de polkit (`deploy/polkit/10-blacksmith.rules`) aplica el mismo patrón
//!    en el sistema, de modo que ni un agente comprometido puede tocar `ssh.service` y similares.

pub mod deployment;

use std::{fmt, path::PathBuf, sync::Arc, time::Duration};

use shared::{
    ActiveState, DeployKey, DeployKind, DeployRun, DeployStatus, EnvVariable, GitHubAccess,
    GlobalEnvironment, RegisterService, RepositoryAccess, ResourceLimits, ServiceActionReport,
    ServiceDeployment, ServiceEnvironment, ServiceLimits, ServiceStatusReport, ServiceSummary,
    SetGitHubAccess, UpdateDeployment,
};

use self::deployment::InvalidDeployment;

use crate::{
    db::{Database, DbError},
    deploy::{
        Paths,
        files::{self, FileError},
        keys::{self, KeyError},
        runner::{DEPLOY_KEY_CREDENTIAL, TOKEN_CREDENTIAL},
        spec::{
            BuildSpec, DeploySpec, DeployTask, GITHUB_CLONE_BASE, GITHUB_SSH_BASE, new_release_id,
        },
    },
    envfile::{EnvFileError, EnvStore},
    error::DisplayChain,
    github::{GitHubAccessError, GitHubStore},
    limits::{self, InvalidLimits},
    resources::{CgroupCpu, host_capacity},
    systemd::{Action, Systemctl, SystemctlError, UnitName},
};

/// Plantilla de los servicios desplegados: `blacksmith@<nombre>.service` (deploy/systemd/blacksmith@.service).
/// Su propia unidad (`blacksmith.service`) no encaja, así que el agente no puede pararse a sí mismo.
pub const MANAGED_UNIT_PREFIX: &str = "blacksmith@";

/// Plantilla del clon: `blacksmith-fetch@<nombre>.service` descarga la rama con las credenciales.
pub const FETCH_UNIT_PREFIX: &str = "blacksmith-fetch@";

/// Plantilla del despliegue: `blacksmith-deploy@<nombre>.service` prepara y activa una versión.
pub const DEPLOY_UNIT_PREFIX: &str = "blacksmith-deploy@";

/// Patrón de las unidades gestionadas (servicios, clones y despliegues). Debe coincidir
/// literalmente con el de la regla de polkit (lo comprueba un test).
pub const MANAGED_UNIT_PATTERN: &str =
    r"^blacksmith(-deploy|-fetch)?@[a-z0-9](?:[a-z0-9-]{0,46}[a-z0-9])?\.service$";

const SLUG_MAX_LEN: usize = 48;

/// Nombre reservado para la comprobación de permisos al arrancar
/// ([`ServiceManager::check_permissions`]): no se puede registrar, así que esa comprobación
/// nunca actúa sobre un servicio real.
pub const PERMISSION_PROBE_SLUG: &str = "agent-permission-check";

/// Identificador de un servicio en el registro. Nunca se reutiliza (`AUTOINCREMENT`): un `id`
/// guardado por el cliente no puede acabar apuntando a otro servicio tras un borrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ServiceId(pub i64);

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Nombre corto y estable de un servicio: `[a-z0-9-]`, de 1 a 48 caracteres, sin guion al
/// principio ni al final. Define su unidad: `blacksmith@<slug>.service`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct ServiceSlug(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidSlug {
    #[error("el identificador está vacío")]
    Empty,
    #[error("el identificador supera los {SLUG_MAX_LEN} caracteres")]
    TooLong,
    #[error("el identificador solo admite minúsculas, dígitos y guiones")]
    InvalidCharacter,
    #[error("el identificador no puede empezar ni terminar con guion")]
    EdgeHyphen,
    #[error("el identificador «{PERMISSION_PROBE_SLUG}» está reservado")]
    Reserved,
}

impl ServiceSlug {
    pub fn new(slug: &str) -> Result<Self, InvalidSlug> {
        if slug.is_empty() {
            return Err(InvalidSlug::Empty);
        }
        if slug.len() > SLUG_MAX_LEN {
            return Err(InvalidSlug::TooLong);
        }
        if !slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(InvalidSlug::InvalidCharacter);
        }
        if slug.starts_with('-') || slug.ends_with('-') {
            return Err(InvalidSlug::EdgeHyphen);
        }
        if slug == PERMISSION_PROBE_SLUG {
            return Err(InvalidSlug::Reserved);
        }
        Ok(Self(slug.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn unit(&self) -> UnitName {
        UnitName::from_validated(format!("{MANAGED_UNIT_PREFIX}{}.service", self.0))
    }

    pub fn fetch_unit(&self) -> UnitName {
        UnitName::from_validated(format!("{FETCH_UNIT_PREFIX}{}.service", self.0))
    }

    pub fn deploy_unit(&self) -> UnitName {
        UnitName::from_validated(format!("{DEPLOY_UNIT_PREFIX}{}.service", self.0))
    }
}

// Para recibirlo ya validado en las rutas (`Path<ServiceSlug>`).
impl TryFrom<String> for ServiceSlug {
    type Error = InvalidSlug;

    fn try_from(slug: String) -> Result<Self, Self::Error> {
        Self::new(&slug)
    }
}

impl fmt::Display for ServiceSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

const NAME_MAX_CHARS: usize = 100;

/// Nombre visible de un servicio: texto libre de 1 a 100 caracteres, sin espacios en los
/// extremos ni caracteres de control.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceName(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidServiceName {
    #[error("el nombre está vacío")]
    Empty,
    #[error("el nombre supera los {NAME_MAX_CHARS} caracteres")]
    TooLong,
    #[error("el nombre tiene espacios en los extremos o caracteres de control")]
    InvalidCharacter,
}

impl ServiceName {
    pub fn new(name: &str) -> Result<Self, InvalidServiceName> {
        if name.is_empty() {
            return Err(InvalidServiceName::Empty);
        }
        if name.chars().count() > NAME_MAX_CHARS {
            return Err(InvalidServiceName::TooLong);
        }
        if name.trim() != name || name.chars().any(char::is_control) {
            return Err(InvalidServiceName::InvalidCharacter);
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServiceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

const DESCRIPTION_MAX_CHARS: usize = 500;

/// Descripción libre de un servicio: hasta 500 caracteres; admite saltos de línea y tabuladores,
/// pero ningún otro carácter de control.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ServiceDescription(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidDescription {
    #[error("la descripción supera los {DESCRIPTION_MAX_CHARS} caracteres")]
    TooLong,
    #[error("la descripción contiene caracteres de control")]
    InvalidCharacter,
}

impl ServiceDescription {
    pub fn new(description: &str) -> Result<Self, InvalidDescription> {
        if description.chars().count() > DESCRIPTION_MAX_CHARS {
            return Err(InvalidDescription::TooLong);
        }
        if description
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
        {
            return Err(InvalidDescription::InvalidCharacter);
        }
        Ok(Self(description.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Un servicio registrado. Las fechas son segundos Unix (UTC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub id: ServiceId,
    pub slug: ServiceSlug,
    pub name: ServiceName,
    pub description: ServiceDescription,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// También cuando la unidad existe en systemd pero no en el registro: para el cliente no
    /// existe.
    #[error("el servicio «{0}» no está registrado en el agente")]
    NotRegistered(ServiceSlug),

    #[error(transparent)]
    Systemctl(#[from] SystemctlError),

    #[error(transparent)]
    Database(#[from] DbError),

    /// Datos de alta no válidos (el mensaje dice cuál).
    #[error("{0}")]
    Invalid(String),

    #[error("ya hay un servicio registrado como «{0}»")]
    AlreadyExists(ServiceSlug),

    #[error(transparent)]
    Env(#[from] EnvFileError),

    #[error(transparent)]
    InvalidDeployment(#[from] InvalidDeployment),

    #[error("el repositorio ya está elegido y no se puede cambiar (solo la rama)")]
    RepositoryLocked,

    #[error("el despliegue no está completo: {0}")]
    DeploymentIncomplete(&'static str),

    #[error("el servicio usa llave de despliegue y aún no la tiene")]
    DeployKeyMissing,

    #[error("ya hay un despliegue en curso")]
    DeployInProgress,

    #[error("el servicio no tiene versión activa: despliégalo antes de iniciarlo")]
    NotDeployed,

    #[error("no hay una versión anterior a la que volver")]
    NothingToRollBack,

    #[error("el servicio no tiene comando de arranque (sitio estático): no hay nada que iniciar")]
    NothingToRun,

    #[error(transparent)]
    File(#[from] FileError),

    #[error(transparent)]
    DeployKey(#[from] KeyError),

    #[error(transparent)]
    GitHub(#[from] GitHubAccessError),

    #[error(transparent)]
    InvalidLimits(#[from] InvalidLimits),

    #[error("la tarea de archivos se interrumpió")]
    Task(#[from] tokio::task::JoinError),
}

/// Trabajo de archivos en un hilo aparte, para no bloquear el runtime.
async fn blocking<T, E>(
    task: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, ServiceError>
where
    T: Send + 'static,
    E: Send + 'static,
    ServiceError: From<E>,
{
    Ok(tokio::task::spawn_blocking(task).await??)
}

/// Credenciales de un despliegue (vacías si no hacen falta).
#[derive(Default)]
struct Credentials {
    token: String,
    deploy_key: Vec<u8>,
}

/// Resultado de [`ServiceManager::check_permissions`].
#[derive(Debug)]
pub enum PermissionCheck {
    /// polkit autoriza al agente a gestionar sus unidades.
    Authorized,
    /// polkit lo deniega: toda acción fallará con `SYSTEMD_PERMISSION_DENIED`.
    Denied,
    /// No se pudo saber (p. ej. `systemctl` bloqueado por el endurecimiento de la unidad).
    Inconclusive(SystemctlError),
}

fn summary(service: &Service) -> ServiceSummary {
    ServiceSummary {
        name: service.slug.to_string(),
        display_name: service.name.to_string(),
        description: service.description.as_str().to_owned(),
        unit: service.slug.unit().to_string(),
        created_at: service.created_at,
    }
}

/// Servicios registrados: resuelve el nombre en el registro y solo entonces actúa sobre su
/// unidad o consulta su estado.
#[derive(Debug, Clone)]
pub struct ServiceManager {
    db: Database,
    systemctl: Systemctl,
    env: EnvStore,
    paths: Paths,
    /// Base de las URL de clon (`https://github.com/`; otra en tests o GitHub Enterprise).
    clone_base: String,
    /// Lo mismo por SSH, para la llave de despliegue (`git@github.com:`).
    ssh_clone_base: String,
    github: GitHubStore,
    cgroup_cpu: CgroupCpu,
    /// Guardar la definición comprueba que el repositorio no cambie y luego escribe: dos guardados
    /// a la vez podrían fijar repositorios distintos.
    deployment_lock: Arc<tokio::sync::Mutex<()>>,
}

impl ServiceManager {
    pub fn new(db: Database, systemctl: Systemctl, paths: Paths) -> Self {
        Self {
            db,
            systemctl,
            env: EnvStore::new(paths.env_dir.clone()),
            github: GitHubStore::new(paths.github_file.clone()),
            cgroup_cpu: CgroupCpu::default(),
            paths,
            clone_base: GITHUB_CLONE_BASE.to_owned(),
            ssh_clone_base: GITHUB_SSH_BASE.to_owned(),
            deployment_lock: Arc::default(),
        }
    }

    /// Otra base para las URL de clon (`<base><dueño>/<repo>.git`), por HTTPS y por SSH.
    /// Solo para tests (un repositorio local en lugar de GitHub).
    #[doc(hidden)]
    pub fn with_clone_base(mut self, base: impl Into<String>) -> Self {
        self.clone_base = base.into();
        self.ssh_clone_base = self.clone_base.clone();
        self
    }

    fn key_dir(&self, slug: &ServiceSlug) -> PathBuf {
        self.paths.deployment_dir(slug).join("key")
    }

    /// Llave de despliegue del servicio, si ya se generó.
    pub async fn deploy_key(&self, slug: &ServiceSlug) -> Result<Option<DeployKey>, ServiceError> {
        self.registered(slug).await?;
        let dir = self.key_dir(slug);
        blocking(move || keys::read(&dir)).await
    }

    /// Genera la llave de despliegue (si no existe) y devuelve su parte pública.
    pub async fn create_deploy_key(&self, slug: &ServiceSlug) -> Result<DeployKey, ServiceError> {
        self.registered(slug).await?;
        let dir = self.key_dir(slug);
        let host = rustix::system::uname()
            .nodename()
            .to_string_lossy()
            .into_owned();
        let comment = format!("blacksmith-{slug}@{host}");
        let key = blocking(move || keys::ensure(&dir, &comment)).await?;
        tracing::info!(name = %slug, fingerprint = %key.fingerprint, "llave de despliegue lista");
        Ok(key)
    }

    /// Cuenta de GitHub del servidor (sin el token).
    pub async fn github_access(&self) -> Result<Option<GitHubAccess>, ServiceError> {
        Ok(self.github.access().await?)
    }

    /// Guarda la cuenta y el token con los que el servidor descarga repositorios privados.
    pub async fn set_github_access(
        &self,
        access: SetGitHubAccess,
    ) -> Result<GitHubAccess, ServiceError> {
        Ok(self.github.save(access).await?)
    }

    pub async fn remove_github_access(&self) -> Result<(), ServiceError> {
        Ok(self.github.remove().await?)
    }

    /// Últimos despliegues del servicio.
    pub async fn deploy_runs(&self, slug: &ServiceSlug) -> Result<Vec<DeployRun>, ServiceError> {
        let service = self.registered(slug).await?;
        Ok(self.db.deploy_runs(service.id, 20).await?)
    }

    /// Lanza un despliegue: deja la especificación para `blacksmith-deploy@<nombre>.service`, encola la
    /// unidad y lo sigue en segundo plano. Si termina bien, reinicia el servicio con la versión
    /// nueva. La salida está en los logs de la unidad de despliegue.
    pub async fn deploy(&self, slug: &ServiceSlug) -> Result<DeployRun, ServiceError> {
        let service = self.registered(slug).await?;
        let (definition, _) = self
            .db
            .deployment(service.id)
            .await?
            .ok_or(ServiceError::DeploymentIncomplete("elige un repositorio"))?;
        let repository = definition
            .repository
            .ok_or(ServiceError::DeploymentIncomplete("elige un repositorio"))?;
        // Un sitio solo estático (con carpeta pública) no necesita proceso.
        if definition.start_command.is_empty() && definition.publish_dir.is_none() {
            return Err(ServiceError::DeploymentIncomplete(
                "define el comando de arranque o la carpeta pública",
            ));
        }
        let credentials = match definition.access {
            // Sin cuenta de GitHub el token va vacío: los repositorios públicos no lo necesitan.
            RepositoryAccess::Account => Credentials {
                token: self.github.token().await?.unwrap_or_default(),
                deploy_key: Vec::new(),
            },
            RepositoryAccess::DeployKey => {
                let dir = self.key_dir(slug);
                let key = blocking(move || keys::private(&dir))
                    .await?
                    .ok_or(ServiceError::DeployKeyMissing)?;
                Credentials {
                    token: String::new(),
                    deploy_key: key,
                }
            }
        };
        let (base, ssh) = match definition.access {
            RepositoryAccess::Account => (&self.clone_base, false),
            RepositoryAccess::DeployKey => (&self.ssh_clone_base, true),
        };
        let mut build = BuildSpec::github(
            base,
            &repository.owner,
            &repository.name,
            &repository.branch,
            definition.setup_commands,
            definition.start_command,
        );
        build.publish_dir = definition.publish_dir;
        let build = if ssh { build.with_ssh() } else { build };
        self.launch(
            &service,
            DeployKind::Deploy,
            new_release_id(),
            repository.branch,
            DeployTask::Build(build),
            credentials,
        )
        .await
    }

    /// Push a una rama (webhook): despliega cada servicio de ese repositorio y esa rama con el
    /// despliegue automático activado. Uno que
    /// ya está desplegando o al que le falta algo se salta (queda en el log), sin parar al resto.
    pub async fn deploy_on_push(
        &self,
        owner: &str,
        name: &str,
        branch: &str,
    ) -> Result<Vec<String>, ServiceError> {
        let mut deployed = Vec::new();
        for service in self.db.services().await? {
            let Some((definition, _)) = self.db.deployment(service.id).await? else {
                continue;
            };
            let matches = definition.auto_deploy
                && definition.repository.is_some_and(|repository| {
                    repository.owner.eq_ignore_ascii_case(owner)
                        && repository.name.eq_ignore_ascii_case(name)
                        && repository.branch == branch
                });
            if !matches {
                continue;
            }
            match self.deploy(&service.slug).await {
                Ok(_) => deployed.push(service.slug.to_string()),
                Err(error) => {
                    tracing::warn!(name = %service.slug, error = %DisplayChain(&error), "push sin desplegar");
                }
            }
        }
        Ok(deployed)
    }

    /// Vuelve a la versión anterior (ya construida en el servidor): no clona ni ejecuta la
    /// preparación, solo la activa y reinicia el servicio. Repetirlo vuelve a la otra.
    pub async fn rollback(&self, slug: &ServiceSlug) -> Result<DeployRun, ServiceError> {
        let service = self.registered(slug).await?;
        let release = self
            .db
            .releases(service.id)
            .await?
            .previous
            .ok_or(ServiceError::NothingToRollBack)?;
        let branch = self
            .db
            .release_branch(service.id, release.clone())
            .await?
            .unwrap_or_default();
        self.launch(
            &service,
            DeployKind::Rollback,
            release,
            branch,
            DeployTask::Activate,
            Credentials::default(),
        )
        .await
    }

    /// Deja la especificación y las credenciales para `blacksmith-deploy@<nombre>.service`, registra el
    /// despliegue y lo sigue en segundo plano.
    async fn launch(
        &self,
        service: &Service,
        kind: DeployKind,
        release: String,
        branch: String,
        task: DeployTask,
        credentials: Credentials,
    ) -> Result<DeployRun, ServiceError> {
        let slug = &service.slug;
        // Un despliegue que el agente ya no registra (p. ej. tras perder la base de datos) puede
        // seguir en marcha: lanzar otro encima pisaría su especificación.
        if self.unit_active(&slug.fetch_unit()).await || self.unit_active(&slug.deploy_unit()).await
        {
            return Err(ServiceError::DeployInProgress);
        }
        let dir = self.paths.deployment_dir(slug);
        let spec = DeploySpec {
            release: release.clone(),
            task,
        };
        let spec = serde_json::to_vec_pretty(&spec)
            .map_err(|error| DbError::Corrupt(format!("especificación: {error}")))?;
        let run = match self
            .db
            .start_deploy_run(service.id, kind, release, branch)
            .await
        {
            Ok(run) => run,
            Err(DbError::DeployInProgress) => return Err(ServiceError::DeployInProgress),
            Err(error) => return Err(error.into()),
        };
        let prepared = async {
            let dir = dir.clone();
            tokio::task::spawn_blocking(move || {
                files::write_private(&dir.join("spec.json"), &spec)?;
                // La unidad exige las credenciales aunque vayan vacías (`LoadCredential=`).
                files::write_private(&dir.join(TOKEN_CREDENTIAL), credentials.token.as_bytes())?;
                files::write_private(&dir.join(DEPLOY_KEY_CREDENTIAL), &credentials.deploy_key)
            })
            .await??;
            Ok::<_, ServiceError>(())
        };
        if let Err(error) = prepared.await {
            let detail = DisplayChain(&error).to_string();
            self.db
                .finish_deploy_run(run.id, DeployStatus::Failed, Some(detail))
                .await?;
            return Err(error);
        }
        tracing::info!(name = %slug, run = run.id, ?kind, release = %run.release, "despliegue iniciado");
        let fetch = run.kind == DeployKind::Deploy;
        tokio::spawn(
            self.clone()
                .follow_deploy(service.clone(), run.clone(), fetch),
        );
        Ok(run)
    }

    /// Ejecuta el clon (si la tarea construye) y después la unidad de despliegue, esperando a que
    /// terminen (su resultado es el del trabajo). Si se activó la versión, la registra como activa
    /// (aunque luego el servicio no arranque: así se puede volver a la anterior) y reinicia el
    /// servicio con ella.
    async fn follow_deploy(self, service: Service, run: DeployRun, fetch: bool) {
        let slug = service.slug.clone();
        let outcome = self.run_deploy_units(&service, &run, fetch).await;
        if let Ok(started) = outcome
            && let Err(error) = self.db.set_autostart(service.id, started).await
        {
            tracing::warn!(name = %slug, %error, "no se pudo marcar el arranque automático");
        }
        let (status, detail) = match outcome {
            Ok(_) => (DeployStatus::Succeeded, None),
            Err(detail) => (DeployStatus::Failed, Some(detail)),
        };
        tracing::info!(name = %slug, run = run.id, ?status, detail = detail.as_deref(), "despliegue terminado");
        if let Err(error) = self.db.finish_deploy_run(run.id, status, detail).await {
            tracing::error!(name = %slug, run = run.id, %error, "no se pudo guardar el resultado del despliegue");
        }
    }

    /// Devuelve si el servicio quedó en marcha, o el motivo del fallo.
    async fn run_deploy_units(
        &self,
        service: &Service,
        run: &DeployRun,
        fetch: bool,
    ) -> Result<bool, String> {
        // Algo más que el `TimeoutStartSec=` de las plantillas: quien corta es systemd.
        const FETCH_LIMIT: Duration = Duration::from_secs(11 * 60);
        const DEPLOY_LIMIT: Duration = Duration::from_secs(31 * 60);
        let slug = &service.slug;
        let failed = |unit: &'static str| {
            move |error: SystemctlError| match error {
                SystemctlError::JobFailed { .. } => format!("{unit}: revisa sus logs"),
                other => DisplayChain(&other).to_string(),
            }
        };
        if fetch {
            let fetched = self
                .systemctl
                .run_with_timeout(Action::Start, &slug.fetch_unit(), FETCH_LIMIT)
                .await
                .map_err(failed("no se pudo descargar la rama"));
            // Las copias del token y de la llave solo hacen falta durante el clon.
            self.remove_credential_copies(slug).await;
            fetched?;
        }
        self.systemctl
            .run_with_timeout(Action::Start, &slug.deploy_unit(), DEPLOY_LIMIT)
            .await
            .map_err(failed(match run.kind {
                DeployKind::Deploy => "el despliegue falló",
                DeployKind::Rollback => "no se pudo volver a la versión anterior",
            }))?;
        self.db
            .activate_release(service.id, run.release.clone())
            .await
            .map_err(|error| {
                format!("versión activa en el servidor, pero no se pudo registrar: {error}")
            })?;
        // Un sitio solo estático no tiene proceso: nginx ya sirve la versión nueva.
        if !self
            .runnable(service)
            .await
            .map_err(|error| error.to_string())?
        {
            return Ok(false);
        }
        self.systemctl
            .run(Action::Restart, &slug.unit())
            .await
            .map(|_| true)
            .map_err(|error| format!("versión activa, pero el servicio no arrancó: {error}"))
    }

    async fn remove_credential_copies(&self, slug: &ServiceSlug) {
        let dir = self.paths.deployment_dir(slug);
        let removed = tokio::task::spawn_blocking(move || {
            files::remove(&dir.join(TOKEN_CREDENTIAL))?;
            files::remove(&dir.join(DEPLOY_KEY_CREDENTIAL))
        })
        .await;
        if !matches!(removed, Ok(Ok(()))) {
            tracing::warn!(name = %slug, "no se pudieron borrar las copias de las credenciales");
        }
    }

    /// Al arrancar el agente: retoma los despliegues que dejó a medias. Sus unidades siguen en
    /// systemd aunque el agente se reinicie; `start` sobre una que ya arranca se une a su trabajo y
    /// devuelve su resultado. Si ninguna sigue activa, no hay forma de saber cómo terminó: se da
    /// por interrumpido. Las copias de credenciales que quedaran se borran (el clon en curso ya
    /// las cargó al arrancar).
    pub async fn recover_deploys(self) {
        let dir = self.paths.deployments_dir.clone();
        let cleaned = tokio::task::spawn_blocking(move || {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                return Ok(());
            };
            for entry in entries.flatten() {
                files::remove(&entry.path().join(TOKEN_CREDENTIAL))?;
                files::remove(&entry.path().join(DEPLOY_KEY_CREDENTIAL))?;
            }
            Ok::<_, FileError>(())
        })
        .await;
        if !matches!(cleaned, Ok(Ok(()))) {
            tracing::warn!("no se pudieron borrar las copias de credenciales pendientes");
        }
        let ids = match self.db.services_deploying().await {
            Ok(ids) => ids,
            Err(error) => {
                tracing::error!(%error, "no se pudieron leer los despliegues pendientes");
                return;
            }
        };
        for id in ids {
            let (Ok(Some(service)), Ok(runs)) =
                (self.db.service(id).await, self.db.deploy_runs(id, 1).await)
            else {
                continue;
            };
            let Some(run) = runs.into_iter().next() else {
                continue;
            };
            let slug = service.slug.clone();
            if self.unit_active(&slug.deploy_unit()).await {
                tracing::info!(name = %slug, run = run.id, "retomando el despliegue en curso");
                tokio::spawn(self.clone().follow_deploy(service, run, false));
            } else if run.kind == DeployKind::Deploy && self.unit_active(&slug.fetch_unit()).await {
                tracing::info!(name = %slug, run = run.id, "retomando el clon en curso");
                tokio::spawn(self.clone().follow_deploy(service, run, true));
            } else if let Err(error) = self
                .db
                .finish_deploy_run(
                    run.id,
                    DeployStatus::Failed,
                    Some("interrumpido: el agente se reinició".to_owned()),
                )
                .await
            {
                tracing::error!(name = %slug, %error, "no se pudo cerrar el despliegue interrumpido");
            }
        }
    }

    /// La unidad está en marcha o arrancando. Ante la duda (systemd no responde), se considera
    /// activa: es más seguro esperar que lanzar otro despliegue encima.
    async fn unit_active(&self, unit: &UnitName) -> bool {
        self.systemctl.status(unit).await.ok().is_none_or(|status| {
            matches!(
                status.active_state,
                ActiveState::Active
                    | ActiveState::Activating
                    | ActiveState::Reloading
                    | ActiveState::Deactivating
            )
        })
    }

    /// Al arrancar el agente (p. ej. tras reiniciar el servidor): pone en marcha los servicios que
    /// estaban en marcha. Arrancar uno que ya corre no hace nada. Los fallos solo se registran.
    pub async fn resume(self) {
        let slugs = match self.db.autostart_services().await {
            Ok(slugs) => slugs,
            Err(error) => {
                tracing::error!(%error, "no se pudieron leer los servicios a reanudar");
                return;
            }
        };
        for slug in slugs {
            match self.registered(&slug).await {
                Ok(service)
                    if self.deployed(&service).await.unwrap_or(false)
                        && self.runnable(&service).await.unwrap_or(false) => {}
                _ => continue,
            }
            match self.systemctl.run(Action::Start, &slug.unit()).await {
                Ok(_) => tracing::info!(name = %slug, "servicio reanudado"),
                Err(error) => {
                    tracing::warn!(name = %slug, error = %DisplayChain(&error), "no se pudo reanudar el servicio");
                }
            }
        }
    }

    /// Variables comunes a todos los servicios del servidor.
    pub async fn global_environment(&self) -> Result<GlobalEnvironment, ServiceError> {
        Ok(GlobalEnvironment {
            file: self.env.global_path().display().to_string(),
            variables: self.env.read_global().await?,
        })
    }

    /// Sustituye las variables globales. Cada servicio las usa desde su siguiente arranque o
    /// reinicio; las de un servicio con la misma clave siguen ganando.
    pub async fn set_global_environment(
        &self,
        variables: Vec<EnvVariable>,
    ) -> Result<GlobalEnvironment, ServiceError> {
        let keys = variables.len();
        self.env.write_global(variables.clone()).await?;
        tracing::info!(
            variables = keys,
            "variables de entorno globales actualizadas"
        );
        Ok(GlobalEnvironment {
            file: self.env.global_path().display().to_string(),
            variables,
        })
    }

    /// Definición del despliegue; vacía si nunca se guardó.
    pub async fn deployment(&self, slug: &ServiceSlug) -> Result<ServiceDeployment, ServiceError> {
        let service = self.registered(slug).await?;
        let stored = self.db.deployment(service.id).await?;
        let (definition, updated_at) = match stored {
            Some((definition, updated_at)) => (definition, Some(updated_at)),
            None => (UpdateDeployment::default(), None),
        };
        Ok(self.deployment_report(slug, definition, updated_at))
    }

    /// Valida y sustituye la definición del despliegue. Solo se guarda: no se ejecuta nada.
    pub async fn set_deployment(
        &self,
        slug: &ServiceSlug,
        update: UpdateDeployment,
    ) -> Result<ServiceDeployment, ServiceError> {
        let service = self.registered(slug).await?;
        let definition = deployment::normalize(update)?;
        let _guard = self.deployment_lock.lock().await;
        // El repositorio no cambia una vez elegido (sus versiones, llave y datos dependen de él);
        // la rama, sí.
        if let Some((stored, _)) = self.db.deployment(service.id).await?
            && let Some(current) = stored.repository
        {
            let same = definition.repository.as_ref().is_some_and(|next| {
                next.owner.eq_ignore_ascii_case(&current.owner)
                    && next.name.eq_ignore_ascii_case(&current.name)
            });
            if !same {
                return Err(ServiceError::RepositoryLocked);
            }
        }
        let updated_at = self
            .db
            .set_deployment(service.id, definition.clone())
            .await?;
        tracing::info!(name = %slug, "definición del despliegue actualizada");
        Ok(self.deployment_report(slug, definition, Some(updated_at)))
    }

    fn deployment_report(
        &self,
        slug: &ServiceSlug,
        definition: UpdateDeployment,
        updated_at: Option<i64>,
    ) -> ServiceDeployment {
        ServiceDeployment {
            name: slug.to_string(),
            repository: definition.repository,
            access: definition.access,
            working_directory: self.paths.app_dir(slug).join("app").display().to_string(),
            setup_commands: definition.setup_commands,
            start_command: definition.start_command,
            publish_dir: definition.publish_dir,
            auto_deploy: definition.auto_deploy,
            updated_at,
        }
    }

    /// Límites de recursos guardados; sin límites si nunca se configuraron.
    pub async fn limits(&self, slug: &ServiceSlug) -> Result<ServiceLimits, ServiceError> {
        let service = self.registered(slug).await?;
        let (limits, updated_at) = match self.db.limits(service.id).await? {
            Some((limits, updated_at)) => (limits, Some(updated_at)),
            None => (ResourceLimits::default(), None),
        };
        Ok(ServiceLimits {
            limits,
            host: host_capacity(),
            updated_at,
        })
    }

    /// Valida los límites, los aplica a la unidad (al momento, sin reiniciar) y, solo si systemd
    /// los aceptó, los guarda.
    pub async fn set_limits(
        &self,
        slug: &ServiceSlug,
        limits: ResourceLimits,
    ) -> Result<ServiceLimits, ServiceError> {
        let service = self.registered(slug).await?;
        let host = host_capacity();
        let limits = limits::validate(limits, host)?;
        // Primero el registro: así nunca queda en systemd un límite que el agente no conoce (y
        // que la baja no quitaría). Si systemd lo rechaza, se vuelve a lo anterior.
        let previous = self.db.limits(service.id).await?;
        let updated_at = self.db.set_limits(service.id, limits).await?;
        if let Err(error) = self
            .systemctl
            .set_properties(&service.slug.unit(), &limits::properties(&limits))
            .await
        {
            let restored = match previous {
                Some((previous, _)) => self.db.set_limits(service.id, previous).await.map(drop),
                None => self.db.delete_limits(service.id).await,
            };
            if let Err(restore) = restored {
                tracing::error!(name = %slug, error = %restore, "no se pudieron restaurar los límites guardados");
            }
            return Err(error.into());
        }
        Ok(ServiceLimits {
            limits,
            host,
            updated_at: Some(updated_at),
        })
    }

    /// Servicios registrados, en el orden almacenado.
    pub async fn list(&self) -> Result<Vec<ServiceSummary>, ServiceError> {
        Ok(self.db.services().await?.iter().map(summary).collect())
    }

    /// Registra un servicio. Solo guarda sus datos: no crea ni toca nada en systemd.
    pub async fn register(&self, request: RegisterService) -> Result<ServiceSummary, ServiceError> {
        let invalid = |error: &dyn std::fmt::Display| ServiceError::Invalid(error.to_string());
        let slug = ServiceSlug::new(request.name.trim()).map_err(|error| invalid(&error))?;
        let name =
            ServiceName::new(request.display_name.trim()).map_err(|error| invalid(&error))?;
        let description =
            ServiceDescription::new(request.description.trim()).map_err(|error| invalid(&error))?;
        match self.db.insert_service(slug, name, description).await {
            Ok(service) => {
                tracing::info!(name = %service.slug, "servicio registrado");
                Ok(summary(&service))
            }
            Err(DbError::DuplicateSlug(slug)) => Err(ServiceError::AlreadyExists(slug)),
            Err(error) => Err(error.into()),
        }
    }

    /// Quita el servicio del registro, lo para y borra sus variables y la especificación de sus
    /// despliegues (son secretos: no deben quedarse en el servidor). No se puede durante un
    /// despliegue: su unidad seguiría escribiendo en un servicio que ya no existe.
    pub async fn unregister(&self, slug: &ServiceSlug) -> Result<(), ServiceError> {
        let service = self.registered(slug).await?;
        let deploying = self
            .db
            .deploy_runs(service.id, 1)
            .await?
            .first()
            .is_some_and(|run| run.status == DeployStatus::Running);
        if deploying {
            return Err(ServiceError::DeployInProgress);
        }
        let had_limits = self.db.limits(service.id).await?.is_some();
        // Primero el registro: si algo de lo siguiente falla, el servicio ya no existe para el
        // cliente y lo que quede son restos, no un servicio a medias.
        if !self.db.delete_service_by_slug(slug.clone()).await? {
            return Err(ServiceError::NotRegistered(slug.clone()));
        }
        let unit = service.slug.unit();
        // Un servicio que nadie ve no debe seguir en marcha. Sin bloquear la baja.
        if let Err(error) = self.systemctl.run(Action::Stop, &unit).await {
            tracing::warn!(name = %slug, %error, "no se pudo parar el servicio dado de baja");
        }
        // Los límites viven en systemd (drop-ins), no en el registro: se quitan para que un
        // servicio que se registre luego con el mismo nombre no los herede.
        if had_limits
            && let Err(error) = self
                .systemctl
                .set_properties(&unit, &limits::properties(&ResourceLimits::default()))
                .await
        {
            tracing::warn!(name = %slug, %error, "no se pudieron quitar los límites de la unidad");
        }
        self.env.remove(slug).await?;
        let dir = self.paths.deployment_dir(slug);
        blocking(move || files::remove(&dir)).await?;
        tracing::info!(name = %slug, "servicio quitado del registro");
        Ok(())
    }

    /// Variables guardadas y si systemd las cargará al arrancar la unidad.
    pub async fn environment(
        &self,
        slug: &ServiceSlug,
    ) -> Result<ServiceEnvironment, ServiceError> {
        let unit = self.unit_of(slug).await?;
        let variables = self.env.read(slug).await?;
        self.environment_report(slug, &unit, variables).await
    }

    /// Sustituye todas las variables. El servicio las usa a partir de su siguiente arranque o
    /// reinicio: systemd lee `EnvironmentFile=` cada vez, sin `daemon-reload`.
    pub async fn set_environment(
        &self,
        slug: &ServiceSlug,
        variables: Vec<EnvVariable>,
    ) -> Result<ServiceEnvironment, ServiceError> {
        let unit = self.unit_of(slug).await?;
        let keys = variables.len();
        self.env.write(slug, variables.clone()).await?;
        // Nunca los valores: son secretos.
        tracing::info!(%unit, variables = keys, "variables de entorno actualizadas");
        self.environment_report(slug, &unit, variables).await
    }

    async fn environment_report(
        &self,
        slug: &ServiceSlug,
        unit: &UnitName,
        variables: Vec<EnvVariable>,
    ) -> Result<ServiceEnvironment, ServiceError> {
        let file = self.env.path_for(slug);
        let global = self.env.global_path();
        let files = self.systemctl.environment_files(unit).await?;
        let position = |path: &PathBuf| files.iter().position(|candidate| candidate == path);
        let loaded_by_unit = position(&file).is_some();
        // El global debe cargarse antes que el del servicio para que este gane.
        let global_loaded_by_unit = match (position(&global), position(&file)) {
            (Some(global), Some(service)) => global < service,
            (Some(_), None) => true,
            (None, _) => false,
        };
        Ok(ServiceEnvironment {
            name: slug.to_string(),
            unit: unit.to_string(),
            file: file.display().to_string(),
            loaded_by_unit,
            global_loaded_by_unit,
            variables,
        })
    }

    pub async fn run(
        &self,
        action: Action,
        slug: &ServiceSlug,
    ) -> Result<ServiceActionReport, ServiceError> {
        let service = self.registered(slug).await?;
        let unit = service.slug.unit();
        // Sin versión activa systemd «omitiría» el arranque (ConditionPathIsSymbolicLink) y la
        // acción parecería correcta sin hacer nada: se rechaza con un motivo claro.
        if action != Action::Stop && !self.deployed(&service).await? {
            return Err(ServiceError::NotDeployed);
        }
        if action != Action::Stop && !self.runnable(&service).await? {
            return Err(ServiceError::NothingToRun);
        }
        let outcome = self.systemctl.run(action, &unit).await?;
        // Lo que el usuario pidió por última vez es lo que se restablece al reiniciar el servidor.
        self.db
            .set_autostart(service.id, action != Action::Stop)
            .await?;
        Ok(ServiceActionReport {
            name: slug.to_string(),
            unit: unit.to_string(),
            outcome,
        })
    }

    /// Estado de systemd y el consumo del servicio entero (su cgroup).
    pub async fn status(&self, slug: &ServiceSlug) -> Result<ServiceStatusReport, ServiceError> {
        let service = self.registered(slug).await?;
        let unit = service.slug.unit();
        let status = self.systemctl.status(&unit).await?;
        let read_at = std::time::Instant::now();
        let releases = self.db.releases(service.id).await?;
        let running = matches!(
            status.active_state,
            ActiveState::Active | ActiveState::Reloading | ActiveState::Deactivating
        );
        let resources = self
            .cgroup_cpu
            .resources(
                &self.systemctl,
                &unit,
                status.resources,
                read_at,
                status.control_group.as_deref(),
                running,
            )
            .await;
        Ok(ServiceStatusReport {
            name: slug.to_string(),
            unit: unit.to_string(),
            load_state: status.load_state,
            active_state: status.active_state,
            sub_state: status.sub_state,
            pid: status
                .main_pid
                .map(|pid| pid.as_raw_nonzero().get().unsigned_abs()),
            deployed: releases.active.is_some(),
            release: releases.active,
            previous_release: releases.previous,
            resources,
            host: host_capacity(),
        })
    }

    /// Tiene comando de arranque (un sitio solo estático no tiene proceso que ejecutar).
    async fn runnable(&self, service: &Service) -> Result<bool, ServiceError> {
        Ok(self
            .db
            .deployment(service.id)
            .await?
            .is_none_or(|(definition, _)| !definition.start_command.is_empty()))
    }

    /// Hay versión activa (la registra el agente al terminar cada despliegue: no necesita leer el
    /// directorio del servicio, que es del usuario de los servicios).
    async fn deployed(&self, service: &Service) -> Result<bool, ServiceError> {
        Ok(self.db.releases(service.id).await?.active.is_some())
    }

    /// Comprueba de verdad, sin efectos, si systemd dejará actuar al agente: pide `stop` de la
    /// unidad reservada `blacksmith-agent-permission-check.service`, que no existe. polkit decide
    /// antes de que systemd busque la unidad, así que la respuesta dice cuál de los dos habló:
    /// «no cargada» es que polkit lo autorizó; «acceso denegado», que no. Recorre el mismo
    /// camino que una acción real (endurecimiento de la unidad, D-Bus, polkit y systemd).
    pub async fn check_permissions(&self) -> PermissionCheck {
        let unit = UnitName::from_validated(format!(
            "{MANAGED_UNIT_PREFIX}{PERMISSION_PROBE_SLUG}.service"
        ));
        match self.systemctl.run_unlogged(Action::Stop, &unit).await {
            // Ok: alguien creó esa unidad a mano; aun así, polkit autorizó.
            Ok(_) | Err(SystemctlError::UnitNotFound { .. }) => PermissionCheck::Authorized,
            Err(SystemctlError::PermissionDenied { .. }) => PermissionCheck::Denied,
            Err(error) => PermissionCheck::Inconclusive(error),
        }
    }

    /// Unidad de un servicio registrado.
    pub async fn unit_of(&self, slug: &ServiceSlug) -> Result<UnitName, ServiceError> {
        Ok(self.registered(slug).await?.slug.unit())
    }

    async fn registered(&self, slug: &ServiceSlug) -> Result<Service, ServiceError> {
        self.db
            .service_by_slug(slug.clone())
            .await?
            .ok_or_else(|| ServiceError::NotRegistered(slug.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_validos_y_su_unidad() {
        for slug in ["api", "a", "mi-app-2", "0", &"a".repeat(SLUG_MAX_LEN)] {
            let slug = ServiceSlug::new(slug).unwrap();
            let unit = slug.unit();
            assert_eq!(unit.as_str(), format!("blacksmith@{slug}.service"));
            assert_eq!(
                slug.deploy_unit().as_str(),
                format!("blacksmith-deploy@{slug}.service")
            );
            // Una unidad gestionada siempre es un `UnitName` válido.
            assert_eq!(UnitName::new(unit.as_str()).unwrap(), unit);
        }
    }

    #[test]
    fn slugs_invalidos() {
        use InvalidSlug::*;
        for (slug, error) in [
            ("", Empty),
            ("API", InvalidCharacter),
            ("mi_app", InvalidCharacter),
            ("mi.app", InvalidCharacter),
            ("app@1", InvalidCharacter),
            ("../etc", InvalidCharacter),
            ("app*", InvalidCharacter),
            ("ñandú", InvalidCharacter),
            ("-app", EdgeHyphen),
            ("app-", EdgeHyphen),
            (PERMISSION_PROBE_SLUG, Reserved),
        ] {
            assert_eq!(ServiceSlug::new(slug), Err(error), "{slug:?}");
        }
        assert_eq!(
            ServiceSlug::new(&"a".repeat(SLUG_MAX_LEN + 1)),
            Err(TooLong)
        );
    }

    /// El patrón de polkit (JavaScript) y la validación de Rust describen lo mismo: se comprueba
    /// que la regla contiene literalmente el patrón y que este encaja con la validación.
    #[test]
    fn la_regla_de_polkit_usa_el_mismo_patron() {
        let rule = include_str!("../../../deploy/polkit/10-blacksmith.rules");
        assert!(
            rule.contains(&format!("/{MANAGED_UNIT_PATTERN}/")),
            "la regla de polkit no contiene el patrón {MANAGED_UNIT_PATTERN}"
        );
        // Longitud máxima del patrón: 1 + 46 + 1 = SLUG_MAX_LEN.
        assert_eq!(SLUG_MAX_LEN, 48);
    }

    #[test]
    fn nombres_visibles() {
        assert!(ServiceName::new("API de pagos (producción)").is_ok());
        assert_eq!(
            ServiceName::new(&"ñ".repeat(NAME_MAX_CHARS))
                .unwrap()
                .as_str()
                .chars()
                .count(),
            100
        );
        assert_eq!(ServiceName::new(""), Err(InvalidServiceName::Empty));
        assert_eq!(
            ServiceName::new(&"a".repeat(101)),
            Err(InvalidServiceName::TooLong)
        );
        for name in [" api", "api ", "a\nb", "a\u{7}b"] {
            assert_eq!(
                ServiceName::new(name),
                Err(InvalidServiceName::InvalidCharacter),
                "{name:?}"
            );
        }
    }

    #[test]
    fn descripciones() {
        assert!(ServiceDescription::new("").is_ok());
        assert!(ServiceDescription::new("API de pagos.\nDos líneas\tcon tabulador.").is_ok());
        assert_eq!(
            ServiceDescription::new(&"ñ".repeat(501)),
            Err(InvalidDescription::TooLong)
        );
        assert_eq!(
            ServiceDescription::new("a\u{1b}[31m"),
            Err(InvalidDescription::InvalidCharacter)
        );
        assert_eq!(
            ServiceDescription::new("a\rb"),
            Err(InvalidDescription::InvalidCharacter)
        );
    }

    #[test]
    fn la_propia_unidad_del_agente_no_es_gestionable() {
        assert!(!"blacksmith.service".starts_with(MANAGED_UNIT_PREFIX));
    }
}
