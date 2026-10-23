//! Servicios gestionados por el agente.
//!
//! Dos reglas que no dependen de la buena voluntad del código que llame:
//!
//! 1. **Para el cliente solo existe lo registrado.** El agente nunca lista unidades de systemd:
//!    los servicios salen de la base de datos y las acciones se piden por `id` de registro. Una
//!    unidad desplegada a mano fuera del agente no aparece ni se puede nombrar desde el cliente.
//! 2. **Solo se gestionan unidades con el prefijo reservado.** La unidad de un servicio se deriva
//!    de su [`ServiceSlug`] (`bbel-<slug>.service`); nunca se acepta un nombre de unidad del
//!    cliente. La regla de polkit (`deploy/polkit/10-blacksmith.rules`) aplica el mismo patrón
//!    en el sistema, de modo que ni un agente comprometido puede tocar `ssh.service` y similares.

use std::{fmt, str::FromStr};

use rustix::process::Pid;

use shared::{ServiceActionReport, ServiceOutcome, ServiceStatusReport};

use crate::{
    db::{Database, DbError},
    metrics::{MetricsError, ProcessSampler},
    systemd::{Action, Outcome, Systemctl, SystemctlError, UnitName},
};

/// Prefijo reservado a las unidades del agente. No lo comparte su propia unidad
/// (`blacksmith.service`), así que el agente no puede pararse a sí mismo.
pub const MANAGED_UNIT_PREFIX: &str = "bbel-";

/// Patrón de las unidades gestionadas. Debe coincidir literalmente con el de la regla de polkit
/// (lo comprueba un test).
pub const MANAGED_UNIT_PATTERN: &str = r"^bbel-[a-z0-9](?:[a-z0-9-]{0,46}[a-z0-9])?\.service$";

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
/// principio ni al final. Define su unidad: `bbel-<slug>.service`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

    /// Unidad de systemd del servicio.
    pub fn unit(&self) -> UnitName {
        UnitName::from_validated(format!("{MANAGED_UNIT_PREFIX}{}.service", self.0))
    }
}

impl FromStr for ServiceSlug {
    type Err = InvalidSlug;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        Self::new(slug)
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

/// Un servicio registrado. Las fechas son segundos Unix (UTC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub id: ServiceId,
    pub slug: ServiceSlug,
    pub name: ServiceName,
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

    #[error("no se pudo medir el proceso")]
    Metrics(#[from] MetricsError),

    #[error(transparent)]
    Database(#[from] DbError),
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

/// Servicios registrados: resuelve el nombre en el registro y solo entonces actúa sobre su
/// unidad o consulta su estado.
#[derive(Debug, Clone)]
pub struct ServiceManager {
    db: Database,
    systemctl: Systemctl,
    sampler: ProcessSampler,
}

impl ServiceManager {
    pub fn new(db: Database, systemctl: Systemctl) -> Self {
        Self {
            db,
            systemctl,
            sampler: ProcessSampler::default(),
        }
    }

    pub async fn start(&self, slug: &ServiceSlug) -> Result<ServiceActionReport, ServiceError> {
        self.run(Action::Start, slug).await
    }

    pub async fn stop(&self, slug: &ServiceSlug) -> Result<ServiceActionReport, ServiceError> {
        self.run(Action::Stop, slug).await
    }

    pub async fn restart(&self, slug: &ServiceSlug) -> Result<ServiceActionReport, ServiceError> {
        self.run(Action::Restart, slug).await
    }

    pub async fn run(
        &self,
        action: Action,
        slug: &ServiceSlug,
    ) -> Result<ServiceActionReport, ServiceError> {
        let unit = self.registered(slug).await?.slug.unit();
        let outcome = match self.systemctl.run(action, &unit).await? {
            Outcome::Started => ServiceOutcome::Started,
            Outcome::Stopped => ServiceOutcome::Stopped,
            Outcome::Restarted => ServiceOutcome::Restarted,
        };
        Ok(ServiceActionReport {
            name: slug.to_string(),
            unit: unit.to_string(),
            outcome,
        })
    }

    /// PID del proceso principal, o `None` si el servicio no tiene uno (ver
    /// [`Systemctl::main_pid`]).
    pub async fn main_pid(&self, slug: &ServiceSlug) -> Result<Option<Pid>, ServiceError> {
        let unit = self.registered(slug).await?.slug.unit();
        Ok(self.systemctl.main_pid(&unit).await?)
    }

    /// Estado de systemd y, si hay proceso principal, su consumo.
    pub async fn status(&self, slug: &ServiceSlug) -> Result<ServiceStatusReport, ServiceError> {
        let unit = self.registered(slug).await?.slug.unit();
        let status = self.systemctl.status(&unit).await?;
        let metrics = match status.main_pid {
            Some(pid) => self.sampler.sample(pid).await?,
            None => None,
        };
        Ok(ServiceStatusReport {
            name: slug.to_string(),
            unit: unit.to_string(),
            load_state: status.load_state,
            active_state: status.active_state,
            sub_state: status.sub_state,
            pid: status
                .main_pid
                .map(|pid| pid.as_raw_nonzero().get().unsigned_abs()),
            metrics,
        })
    }

    /// Comprueba de verdad, sin efectos, si systemd dejará actuar al agente: pide `stop` de la
    /// unidad reservada `bbel-agent-permission-check.service`, que no existe. polkit decide
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
            assert_eq!(unit.as_str(), format!("bbel-{slug}.service"));
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
    fn la_propia_unidad_del_agente_no_es_gestionable() {
        assert!(!"blacksmith.service".starts_with(MANAGED_UNIT_PREFIX));
    }
}
