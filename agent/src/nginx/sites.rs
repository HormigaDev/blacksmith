//! Sitio web de cada servicio: genera su configuración de nginx, la aplica de forma segura y
//! pide su certificado de Let's Encrypt.
//!
//! - **Aplicar**: el agente escribe `<sitios>/<servicio>.conf` (su directorio; nginx lo incluye
//!   desde `conf.d/blacksmith.conf`) y arranca `blacksmith-nginx.service`, que corre `nginx -t` y recarga. Si
//!   nginx lo rechaza, se restaura el archivo anterior y nginx sigue como estaba.
//! - **HTTPS**: el sitio se publica primero por HTTP (con el desafío ACME) y se pide el
//!   certificado en segundo plano con `blacksmith-certbot@<dominio>.service`; con el certificado, se
//!   vuelve a generar con HTTPS, redirección y HSTS. Si falla, el sitio sigue por HTTP y el motivo
//!   queda en `tls_detail`.
//! - Las dos unidades son del systemd del sistema (también en modo local) y polkit solo deja
//!   `start` de ellas (deploy/polkit/, deploy/nginx/polkit/).
//! - Un cambio a la vez: nginx tiene una sola configuración, así que se aplican en serie.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use shared::{Site, SiteConfig, SiteRoute, TlsStatus};
use tokio::sync::Mutex;

use super::template::{self, SiteVars, TemplateError};
use crate::{
    db::{Database, DbError, StoredSite},
    deploy::{
        Paths,
        files::{self, FileError},
    },
    error::DisplayChain,
    services::{Service, ServiceSlug},
    systemd::{Action, Journalctl, Systemctl, SystemctlError, UnitName},
};

/// Unidad que valida (`nginx -t`) y recarga nginx (deploy/nginx/systemd/blacksmith-nginx.service).
const APPLY_UNIT: &str = "blacksmith-nginx.service";
/// Algo más que su `TimeoutStartSec=` (60 s / 5 min): quien corta es systemd.
const APPLY_TIMEOUT: Duration = Duration::from_secs(70);
const CERTBOT_TIMEOUT: Duration = Duration::from_secs(6 * 60);
/// `blacksmith-certbot@<dominio>.service` ≤ 255 caracteres.
const MAX_PRIMARY_DOMAIN_LEN: usize = 255 - "blacksmith-certbot@.service".len();
/// Unidades de certbot que polkit deja arrancar al agente. Debe coincidir literalmente con las dos
/// reglas (servidor y modo local) y con [`MAX_PRIMARY_DOMAIN_LEN`]: lo comprueba un test.
pub const CERTBOT_UNIT_PATTERN: &str =
    r"^blacksmith-certbot@[a-z0-9](?:[a-z0-9.-]{0,226}[a-z0-9])?\.service$";
const MAX_DOMAINS: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum SiteError {
    #[error("no hay ningún servicio registrado como «{0}»")]
    NotRegistered(ServiceSlug),
    #[error("{0}")]
    Invalid(String),
    #[error("otro servicio ya usa el dominio «{0}»")]
    DomainInUse(String),
    #[error("nginx rechazó la configuración (se mantiene la anterior): {0}")]
    Rejected(String),
    #[error("no se pudo aplicar en nginx: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Template(#[from] TemplateError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Database(#[from] DbError),
}

#[derive(Debug, Clone)]
pub struct SiteManager {
    db: Database,
    /// Siempre el systemd del sistema: nginx y certbot lo son también en modo local.
    systemctl: Systemctl,
    /// Del sistema, como `systemctl`: el motivo de un fallo de nginx o certbot.
    journal: Journalctl,
    paths: Paths,
    /// Puerto del agente: ningún sitio puede publicarlo.
    agent_port: u16,
    lock: Arc<Mutex<()>>,
}

impl SiteManager {
    pub fn new(
        db: Database,
        systemctl: Systemctl,
        journal: Journalctl,
        paths: Paths,
        agent_port: u16,
    ) -> Self {
        Self {
            db,
            systemctl,
            journal,
            paths,
            agent_port,
            lock: Arc::new(Mutex::new(())),
        }
    }

    fn site_file(&self, slug: &ServiceSlug) -> PathBuf {
        self.paths.nginx_sites_dir.join(format!("{slug}.conf"))
    }

    async fn registered(&self, slug: &ServiceSlug) -> Result<Service, SiteError> {
        self.db
            .service_by_slug(slug.clone())
            .await?
            .ok_or_else(|| SiteError::NotRegistered(slug.clone()))
    }

    fn report(&self, slug: &ServiceSlug, stored: StoredSite) -> Site {
        Site {
            name: slug.to_string(),
            config: stored.config,
            tls: stored.tls,
            tls_detail: stored.tls_detail,
            config_path: self.site_file(slug).display().to_string(),
            updated_at: stored.updated_at,
        }
    }

    pub async fn get(&self, slug: &ServiceSlug) -> Result<Option<Site>, SiteError> {
        let service = self.registered(slug).await?;
        Ok(self
            .db
            .site(service.id)
            .await?
            .map(|stored| self.report(slug, stored)))
    }

    /// Guarda el sitio y lo aplica en nginx. Con HTTPS, si aún no hay certificado para esos
    /// dominios, lo publica por HTTP y lo pide en segundo plano (`tls: issuing`).
    pub async fn set(&self, slug: &ServiceSlug, config: SiteConfig) -> Result<Site, SiteError> {
        let service = self.registered(slug).await?;
        let publishes = self
            .db
            .deployment(service.id)
            .await?
            .is_some_and(|(definition, _)| definition.publish_dir.is_some());
        let config = validate(config, publishes, self.agent_port)?;
        if let Some(domain) = self
            .db
            .domain_in_use(service.id, config.domains.clone())
            .await?
        {
            return Err(SiteError::DomainInUse(domain));
        }
        let previous = self.db.site(service.id).await?;
        // El certificado sirve si ya estaba emitido para exactamente estos dominios.
        let certified = previous.as_ref().is_some_and(|site| {
            site.tls == TlsStatus::Active && site.config.domains == config.domains
        });
        let tls = match (config.https, certified) {
            (false, _) => TlsStatus::Off,
            (true, true) => TlsStatus::Active,
            (true, false) => TlsStatus::Issuing,
        };

        {
            let _guard = self.lock.lock().await;
            let undo = self
                .write_and_apply(&service, &config, tls == TlsStatus::Active)
                .await?;
            if let Err(error) = self.db.save_site(service.id, config.clone(), tls).await {
                undo.restore(self).await;
                return Err(match error {
                    DbError::DomainInUse(domain) => SiteError::DomainInUse(domain),
                    other => other.into(),
                });
            }
        }
        tracing::info!(name = %slug, template = ?config.template, ?tls, "sitio aplicado en nginx");
        if tls == TlsStatus::Issuing {
            tokio::spawn(self.clone().issue(service.clone(), config));
        }
        self.get(slug)
            .await?
            .ok_or_else(|| SiteError::NotRegistered(slug.clone()))
    }

    /// Vuelve a pedir el certificado (tras un fallo, p. ej. un DNS que aún no apuntaba aquí).
    pub async fn retry_certificate(&self, slug: &ServiceSlug) -> Result<Site, SiteError> {
        let service = self.registered(slug).await?;
        let stored = self
            .db
            .site(service.id)
            .await?
            .ok_or_else(|| SiteError::Invalid("el servicio no tiene sitio".into()))?;
        if !stored.config.https {
            return Err(SiteError::Invalid("el sitio no usa HTTPS".into()));
        }
        if stored.tls != TlsStatus::Issuing {
            self.db
                .set_site_tls(service.id, TlsStatus::Issuing, None)
                .await?;
            tokio::spawn(self.clone().issue(service, stored.config));
        }
        self.get(slug)
            .await?
            .ok_or_else(|| SiteError::NotRegistered(slug.clone()))
    }

    /// Quita el sitio de nginx (el servicio sigue registrado).
    pub async fn remove(&self, slug: &ServiceSlug) -> Result<(), SiteError> {
        let service = self.registered(slug).await?;
        let site = self.db.site(service.id).await?;
        self.unpublish(slug).await?;
        self.db.delete_site(service.id).await?;
        // Los datos de certbot de ese certificado ya no los pedirá nadie.
        if let Some(primary) = site.as_ref().and_then(|site| site.config.domains.first()) {
            let _ = remove(&self.paths.nginx_certs_dir.join(format!("{primary}.env"))).await;
        }
        tracing::info!(name = %slug, "sitio retirado de nginx");
        Ok(())
    }

    /// Tras dar de baja un servicio: retira su sitio de nginx si lo tenía. Los fallos solo se
    /// registran (la baja ya se hizo).
    pub async fn forget(&self, slug: &ServiceSlug) {
        if !self.site_file(slug).exists() {
            return;
        }
        if let Err(error) = self.unpublish(slug).await {
            tracing::warn!(name = %slug, error = %DisplayChain(&error), "no se pudo retirar el sitio de nginx");
        }
    }

    async fn unpublish(&self, slug: &ServiceSlug) -> Result<(), SiteError> {
        let _guard = self.lock.lock().await;
        let path = self.site_file(slug);
        let previous = read(&path).await?;
        remove(&path).await?;
        if let Err(error) = self.apply().await {
            if let Some(content) = previous {
                let _ = write(&path, content).await;
                let _ = self.apply().await;
            }
            return Err(error);
        }
        Ok(())
    }

    /// Escribe la configuración y la aplica. Si nginx la rechaza, deja la anterior (archivo y
    /// nginx) y devuelve el motivo. Devuelve cómo deshacerlo (si después falla otra cosa).
    async fn write_and_apply(
        &self,
        service: &Service,
        config: &SiteConfig,
        tls: bool,
    ) -> Result<Undo, SiteError> {
        let root = config
            .template
            .serves_files()
            .then(|| {
                self.paths
                    .app_dir(&service.slug)
                    .join("public/current")
                    .display()
                    .to_string()
            })
            .map(safe_root)
            .transpose()?;
        let vars = SiteVars {
            service: service.slug.to_string(),
            template: config.template,
            domains: config.domains.clone(),
            upstream: config.port.map(|port| format!("127.0.0.1:{port}")),
            root,
            tls,
            routes: config.routes.clone(),
        };
        let templates_dir = self.paths.nginx_templates_dir.clone();
        let template = config.template;
        let text = tokio::task::spawn_blocking(move || template::load(&templates_dir, template))
            .await
            .map_err(DbError::from)??;
        let body = template::render_body(&text, &vars)?;
        // Una ruta con el mismo prefijo que un `location` de la plantilla (la del administrador,
        // que puede haberla editado) sería un duplicado: nginx -t lo rechazaría sin más detalle.
        let taken = template::template_locations(&body);
        if let Some(route) = config
            .routes
            .iter()
            .find(|route| taken.contains(&route.path))
        {
            return Err(SiteError::Invalid(format!(
                "la plantilla «{}» ya define la ruta {}: quítala de las rutas",
                config.template.file_stem(),
                route.path
            )));
        }
        let site = template::render_site(&body, &vars);

        let path = self.site_file(&service.slug);
        let undo = Undo {
            path: path.clone(),
            previous: read(&path).await?,
        };
        write(&path, site.into_bytes()).await?;
        if let Err(error) = self.apply().await {
            undo.restore(self).await;
            return Err(error);
        }
        Ok(undo)
    }

    /// `nginx -t` y recarga, en la unidad privilegiada.
    async fn apply(&self) -> Result<(), SiteError> {
        let unit = UnitName::from_validated(APPLY_UNIT.to_owned());
        let since = crate::unix_now();
        match self
            .systemctl
            .run_with_timeout(Action::Start, &unit, APPLY_TIMEOUT)
            .await
        {
            Ok(_) => Ok(()),
            Err(SystemctlError::JobFailed { .. }) => {
                let detail = self.journal.tail(&unit, since).await;
                Err(SiteError::Rejected(if detail.is_empty() {
                    format!("nginx -t falló (journalctl -u {APPLY_UNIT})")
                } else {
                    detail
                }))
            }
            Err(
                error @ (SystemctlError::UnitNotFound { .. }
                | SystemctlError::PermissionDenied { .. }),
            ) => Err(SiteError::Unavailable(format!(
                "{error}. Instale la integración de nginx en el servidor: deploy/nginx/install.sh"
            ))),
            Err(error) => Err(SiteError::Unavailable(DisplayChain(&error).to_string())),
        }
    }

    /// Pide el certificado y, con él, publica el sitio por HTTPS.
    async fn issue(self, service: Service, config: SiteConfig) {
        let slug = service.slug.clone();
        let outcome = self.certbot(&config).await;
        let _guard = self.lock.lock().await;
        // La configuración pudo cambiar mientras certbot trabajaba (otros dominios, o HTTPS
        // desactivado): entonces este resultado ya no es de nadie y no se aplica ni se guarda.
        let current = match self.db.site(service.id).await {
            Ok(Some(site)) if site.config.https && site.config.domains == config.domains => {
                site.config
            }
            _ => return,
        };
        let (tls, detail) = match outcome {
            Ok(()) => match self.write_and_apply(&service, &current, true).await {
                Ok(_) => (TlsStatus::Active, None),
                Err(error) => (TlsStatus::Failed, Some(DisplayChain(&error).to_string())),
            },
            Err(detail) => (TlsStatus::Failed, Some(detail)),
        };
        tracing::info!(name = %slug, ?tls, detail = detail.as_deref(), "certificado del sitio");
        if let Err(error) = self.db.set_site_tls(service.id, tls, detail).await {
            tracing::error!(name = %slug, %error, "no se pudo guardar el estado del certificado");
        }
    }

    async fn certbot(&self, config: &SiteConfig) -> Result<(), String> {
        let primary = config.domains.first().ok_or("sin dominios")?;
        // Solo datos: los argumentos de certbot los construye `deploy/nginx/certbot.sh`, que corre
        // como root y vuelve a validar cada valor (este archivo lo escribe un usuario sin
        // privilegios). Valores ya validados: sin espacios, comillas ni saltos de línea.
        let env = format!(
            "BLACKSMITH_DOMAINS={}\nBLACKSMITH_EMAIL={}\n",
            config.domains.join(","),
            config.email.as_deref().unwrap_or_default()
        );
        write(
            &self.paths.nginx_certs_dir.join(format!("{primary}.env")),
            env.into_bytes(),
        )
        .await
        .map_err(|error| DisplayChain(&error).to_string())?;
        let unit_name = format!("blacksmith-certbot@{primary}.service");
        let unit = UnitName::new(&unit_name).map_err(|error| error.to_string())?;
        let since = crate::unix_now();
        match self
            .systemctl
            .run_with_timeout(Action::Start, &unit, CERTBOT_TIMEOUT)
            .await
        {
            Ok(_) => Ok(()),
            Err(SystemctlError::JobFailed { .. }) => {
                let detail = self.journal.tail(&unit, since).await;
                Err(if detail.is_empty() {
                    format!("certbot falló (journalctl -u {unit_name})")
                } else {
                    detail
                })
            }
            Err(error) => Err(DisplayChain(&error).to_string()),
        }
    }
}

/// Cómo dejar el archivo del sitio como estaba (y nginx con él).
struct Undo {
    path: PathBuf,
    previous: Option<Vec<u8>>,
}

impl Undo {
    async fn restore(self, manager: &SiteManager) {
        let restored = match self.previous {
            Some(content) => write(&self.path, content).await,
            None => remove(&self.path).await,
        };
        if restored.is_ok() {
            let _ = manager.apply().await;
        }
    }
}

async fn read(path: &Path) -> Result<Option<Vec<u8>>, SiteError> {
    let path = path.to_owned();
    Ok(
        tokio::task::spawn_blocking(move || files::read_private(&path))
            .await
            .map_err(DbError::from)??,
    )
}

async fn write(path: &Path, content: Vec<u8>) -> Result<(), SiteError> {
    let path = path.to_owned();
    Ok(
        tokio::task::spawn_blocking(move || files::write_private(&path, &content))
            .await
            .map_err(DbError::from)??,
    )
}

async fn remove(path: &Path) -> Result<(), SiteError> {
    let path = path.to_owned();
    Ok(tokio::task::spawn_blocking(move || files::remove(&path))
        .await
        .map_err(DbError::from)??)
}

/// Valida y normaliza la configuración. Todo lo que pasa de aquí acaba en la configuración de
/// nginx o en los argumentos de certbot, así que solo se aceptan valores con forma conocida.
pub fn validate(
    config: SiteConfig,
    publishes: bool,
    agent_port: u16,
) -> Result<SiteConfig, SiteError> {
    let invalid = |message: &str| SiteError::Invalid(message.to_owned());
    let mut domains: Vec<String> = Vec::with_capacity(config.domains.len());
    for domain in &config.domains {
        let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
        if !valid_domain(&domain) {
            return Err(SiteError::Invalid(format!(
                "dominio inválido «{domain}»: p. ej. ejemplo.com o app.ejemplo.com"
            )));
        }
        if domains.contains(&domain) {
            return Err(SiteError::Invalid(format!(
                "el dominio «{domain}» está repetido"
            )));
        }
        domains.push(domain);
    }
    if domains.is_empty() {
        return Err(invalid("indique al menos un dominio"));
    }
    if domains.len() > MAX_DOMAINS {
        return Err(SiteError::Invalid(format!(
            "como mucho {MAX_DOMAINS} dominios"
        )));
    }
    // El principal da nombre a la unidad de certbot, que no puede pasar de 255 caracteres.
    if domains
        .first()
        .is_some_and(|primary| primary.len() > MAX_PRIMARY_DOMAIN_LEN)
    {
        return Err(SiteError::Invalid(format!(
            "el dominio principal no puede pasar de {MAX_PRIMARY_DOMAIN_LEN} caracteres"
        )));
    }
    if config.template.serves_files() && !publishes {
        return Err(invalid(
            "esta plantilla sirve archivos: defina la carpeta pública del despliegue (p. ej. dist)",
        ));
    }
    let port = if config.template.proxies() {
        match config.port {
            Some(port) if port < 1024 => {
                return Err(invalid("el puerto del servicio debe ser 1024 o mayor"));
            }
            Some(port) if port == agent_port => {
                return Err(invalid("ese es el puerto del agente: use otro"));
            }
            Some(port) => Some(port),
            None => return Err(invalid("indique el puerto en el que escucha el servicio")),
        }
    } else {
        None
    };
    if config.https
        && let Some(local) = domains.iter().find(|domain| !publicly_resolvable(domain))
    {
        return Err(SiteError::Invalid(format!(
            "«{local}» no es un dominio público: Let's Encrypt no puede emitir su certificado"
        )));
    }
    let email = match config.email.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(email) if valid_email(email) => Some(email.to_owned()),
        Some(_) => return Err(invalid("correo electrónico inválido")),
    };
    let routes = validate_routes(config.routes, agent_port)?;
    Ok(SiteConfig {
        template: config.template,
        domains,
        port,
        https: config.https,
        email,
        routes,
    })
}

const MAX_ROUTES: usize = 20;
const MAX_ROUTE_LEN: usize = 100;

/// Rutas hacia otros puertos: prefijos que acaban en la configuración de nginx, así que solo con
/// caracteres de ruta seguros, sin `..` ni `//`, y nunca `/` (la raíz es de la plantilla) ni
/// `/.well-known` (es del desafío de Let's Encrypt).
fn validate_routes(routes: Vec<SiteRoute>, agent_port: u16) -> Result<Vec<SiteRoute>, SiteError> {
    if routes.len() > MAX_ROUTES {
        return Err(SiteError::Invalid(format!("como mucho {MAX_ROUTES} rutas")));
    }
    let mut seen: Vec<String> = Vec::with_capacity(routes.len());
    let mut valid = Vec::with_capacity(routes.len());
    for route in routes {
        let path = route.path.trim().to_owned();
        let well_formed = path.len() > 1
            && path.len() <= MAX_ROUTE_LEN
            && path.starts_with('/')
            && path.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b'~')
            })
            && !path.contains("//")
            && !path.split('/').any(|part| part == ".." || part == ".")
            && !path.starts_with("/.well-known");
        if !well_formed {
            return Err(SiteError::Invalid(format!(
                "ruta inválida «{path}»: un prefijo como /api/ o /socket.io/ (letras, dígitos, . _ - ~)"
            )));
        }
        if seen.contains(&path) {
            return Err(SiteError::Invalid(format!("la ruta {path} está repetida")));
        }
        if route.port < 1024 || route.port == agent_port {
            return Err(SiteError::Invalid(format!(
                "puerto inválido para {path}: 1024 o mayor, y distinto del del agente"
            )));
        }
        seen.push(path.clone());
        valid.push(SiteRoute {
            path,
            port: route.port,
            websocket: route.websocket,
        });
    }
    Ok(valid)
}

/// Nombre de host: etiquetas de `a-z0-9-` (1-63, sin guion en los extremos), al menos dos, o
/// `localhost`. Sin comodines ni IPs.
fn valid_domain(domain: &str) -> bool {
    if domain == "localhost" {
        return true;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    domain.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|label| {
            (1..=63).contains(&label.len())
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
        // La última etiqueta (TLD) no es numérica: así una IP no pasa por dominio.
        && labels
            .last()
            .is_some_and(|tld| tld.bytes().any(|byte| byte.is_ascii_lowercase()))
}

/// Dominios con los que Let's Encrypt puede trabajar (no los reservados para uso local).
fn publicly_resolvable(domain: &str) -> bool {
    const LOCAL: [&str; 6] = [
        ".localhost",
        ".local",
        ".test",
        ".internal",
        ".lan",
        ".home.arpa",
    ];
    domain != "localhost" && !LOCAL.iter().any(|suffix| domain.ends_with(suffix))
}

/// Dirección sencilla y sin nada que el shell o certbot interpreten.
/// `root {{root}};` va sin comillas en las plantillas: en modo local la ruta sale de `$HOME`, y un
/// espacio o un `;` en ella rompería la configuración o le añadiría directivas.
fn safe_root(root: String) -> Result<String, SiteError> {
    if root
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        Ok(root)
    } else {
        Err(SiteError::Invalid(format!(
            "la carpeta pública ({root}) tiene caracteres que nginx no admite sin comillas: \
             use un directorio personal sin espacios ni símbolos"
        )))
    }
}

fn valid_email(email: &str) -> bool {
    let Some((user, host)) = email.split_once('@') else {
        return false;
    };
    (1..=64).contains(&user.len())
        // Con un guion delante, certbot lo leería como una opción.
        && !user.starts_with('-')
        && user.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
        })
        && valid_domain(&host.to_ascii_lowercase())
        && host != "localhost"
}

#[cfg(test)]
mod tests {
    use shared::SiteTemplate;

    use super::*;

    #[test]
    fn las_reglas_de_polkit_usan_el_mismo_patron_de_certbot() {
        for rule in [
            include_str!("../../../deploy/polkit/10-blacksmith.rules"),
            include_str!("../../../deploy/nginx/polkit/10-blacksmith-nginx-local.rules.in"),
        ] {
            assert!(rule.contains(&format!("/{CERTBOT_UNIT_PATTERN}/")));
        }
        // Un carácter, hasta 226 y otro: el largo máximo del dominio principal.
        assert_eq!(1 + 226 + 1, MAX_PRIMARY_DOMAIN_LEN);
    }

    #[test]
    fn carpeta_publica_sin_caracteres_peligrosos() {
        assert!(safe_root("/var/lib/blacksmith-apps/web/public/current".into()).is_ok());
        for root in [
            "/home/ana maria/.local/state/blacksmith-apps/web/public/current",
            "/home/x;include /etc/shadow/public/current",
        ] {
            assert!(safe_root(root.into()).is_err(), "{root}");
        }
    }

    fn config(
        template: SiteTemplate,
        domains: &[&str],
        port: Option<u16>,
        https: bool,
    ) -> SiteConfig {
        SiteConfig {
            template,
            domains: domains.iter().map(|domain| (*domain).to_owned()).collect(),
            port,
            https,
            email: None,
            routes: Vec::new(),
        }
    }

    fn route(path: &str, port: u16) -> SiteRoute {
        SiteRoute {
            path: path.into(),
            port,
            websocket: false,
        }
    }

    #[test]
    fn rutas_hacia_otros_puertos() {
        let with = |routes: Vec<SiteRoute>| {
            validate(
                SiteConfig {
                    routes,
                    ..config(SiteTemplate::Spa, &["example.com"], None, false)
                },
                true,
                4040,
            )
        };
        let ok = with(vec![route(" /api/ ", 3000), route("/socket.io/", 3001)]).unwrap();
        assert_eq!(ok.routes[0].path, "/api/");
        for bad in [
            vec![route("/", 3000)],
            vec![route("api/", 3000)],
            vec![route("/a b", 3000)],
            vec![route("/../etc", 3000)],
            vec![route("/a//b", 3000)],
            vec![route("/api;", 3000)],
            vec![route("/api/ { return 200; }", 3000)],
            vec![route("/.well-known/x", 3000)],
            vec![route("/api/", 80)],
            vec![route("/api/", 4040)],
            vec![route("/api/", 3000), route("/api/", 3001)],
        ] {
            assert!(
                matches!(with(bad.clone()), Err(SiteError::Invalid(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn normaliza_y_acepta() {
        let normalized = validate(
            SiteConfig {
                email: Some(" ops+blacksmith@Example.com ".into()),
                ..config(
                    SiteTemplate::App,
                    &[" Example.COM. ", "www.example.com"],
                    Some(3000),
                    true,
                )
            },
            false,
            4040,
        )
        .unwrap();
        assert_eq!(normalized.domains, ["example.com", "www.example.com"]);
        assert_eq!(normalized.port, Some(3000));
        assert_eq!(
            normalized.email.as_deref(),
            Some("ops+blacksmith@Example.com")
        );
        // Sin proxy, el puerto no se guarda.
        let spa = validate(
            config(SiteTemplate::Spa, &["app.localhost"], Some(3000), false),
            true,
            4040,
        )
        .unwrap();
        assert_eq!(spa.port, None);
    }

    #[test]
    fn rechaza_lo_que_no_es_seguro_o_no_tiene_sentido() {
        let cases = [
            (config(SiteTemplate::App, &[], Some(3000), false), true),
            (
                config(SiteTemplate::App, &["a.com", "A.com"], Some(3000), false),
                true,
            ),
            (
                config(SiteTemplate::App, &["*.example.com"], Some(3000), false),
                true,
            ),
            (
                config(SiteTemplate::App, &["example.com;"], Some(3000), false),
                true,
            ),
            (
                config(SiteTemplate::App, &["exa mple.com"], Some(3000), false),
                true,
            ),
            (
                config(SiteTemplate::App, &["192.168.1.10"], Some(3000), false),
                true,
            ),
            (
                config(SiteTemplate::App, &["-a.com"], Some(3000), false),
                true,
            ),
            (
                config(SiteTemplate::App, &["example.com"], None, false),
                true,
            ),
            (
                config(SiteTemplate::App, &["example.com"], Some(80), false),
                true,
            ),
            (
                config(SiteTemplate::App, &["example.com"], Some(4040), false),
                true,
            ),
            (
                config(SiteTemplate::Static, &["example.com"], None, false),
                false,
            ),
            (
                config(SiteTemplate::App, &["app.localhost"], Some(3000), true),
                true,
            ),
        ];
        for (case, publishes) in cases {
            assert!(
                matches!(
                    validate(case.clone(), publishes, 4040),
                    Err(SiteError::Invalid(_))
                ),
                "{case:?}"
            );
        }
        for email in [
            "sin-arroba",
            "a b@example.com",
            "a@example.com --x",
            "$(id)@example.com",
        ] {
            let case = SiteConfig {
                email: Some(email.into()),
                ..config(SiteTemplate::App, &["example.com"], Some(3000), true)
            };
            assert!(validate(case, false, 4040).is_err(), "{email}");
        }
    }
}
