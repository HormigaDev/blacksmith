//! Sitio web de cada servicio: genera su configuración de nginx, la aplica de forma segura y
//! pide su certificado de Let's Encrypt.
//!
//! - **Aplicar**: el agente escribe `<sitios>/<servicio>.conf` (su directorio; nginx lo incluye
//!   desde `conf.d/bbel.conf`) y arranca `bbel-nginx.service`, que corre `nginx -t` y recarga. Si
//!   nginx lo rechaza, se restaura el archivo anterior y nginx sigue como estaba.
//! - **HTTPS**: el sitio se publica primero por HTTP (con el desafío ACME) y se pide el
//!   certificado en segundo plano con `bbel-certbot@<dominio>.service`; con el certificado, se
//!   vuelve a generar con HTTPS, redirección y HSTS. Si falla, el sitio sigue por HTTP y el motivo
//!   queda en `tls_detail`.
//! - Las dos unidades son del systemd del sistema (también en modo local) y polkit solo deja
//!   `start` de ellas (deploy/polkit/, deploy/nginx/polkit/).
//! - Un cambio a la vez: nginx tiene una sola configuración, así que se aplican en serie.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use shared::{Site, SiteConfig, TlsStatus};
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
    systemd::{Action, Systemctl, SystemctlError, UnitName},
};

/// Unidad que valida (`nginx -t`) y recarga nginx (deploy/nginx/systemd/bbel-nginx.service).
const APPLY_UNIT: &str = "bbel-nginx.service";
/// Algo más que su `TimeoutStartSec=` (60 s / 5 min): quien corta es systemd.
const APPLY_TIMEOUT: Duration = Duration::from_secs(70);
const CERTBOT_TIMEOUT: Duration = Duration::from_secs(6 * 60);
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
    journalctl: PathBuf,
    paths: Paths,
    /// Puerto del agente: ningún sitio puede publicarlo.
    agent_port: u16,
    lock: Arc<Mutex<()>>,
}

impl SiteManager {
    pub fn new(
        db: Database,
        systemctl: Systemctl,
        journalctl: impl Into<PathBuf>,
        paths: Paths,
        agent_port: u16,
    ) -> Self {
        Self {
            db,
            systemctl,
            journalctl: journalctl.into(),
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
        self.unpublish(slug).await?;
        self.db.delete_site(service.id).await?;
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
        let vars = SiteVars {
            service: service.slug.to_string(),
            template: config.template,
            domains: config.domains.clone(),
            upstream: config.port.map(|port| format!("127.0.0.1:{port}")),
            root: config.template.serves_files().then(|| {
                self.paths
                    .app_dir(&service.slug)
                    .join("public/current")
                    .display()
                    .to_string()
            }),
            tls,
        };
        let templates_dir = self.paths.nginx_templates_dir.clone();
        let template = config.template;
        let text = tokio::task::spawn_blocking(move || template::load(&templates_dir, template))
            .await
            .map_err(DbError::from)??;
        let site = template::render_site(&template::render_body(&text, &vars)?, &vars);

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
        let since = now();
        match self
            .systemctl
            .run_with_timeout(Action::Start, &unit, APPLY_TIMEOUT)
            .await
        {
            Ok(_) => Ok(()),
            Err(SystemctlError::JobFailed { .. }) => {
                let detail = self.journal_tail(APPLY_UNIT, since).await;
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
        let (tls, detail) = match outcome {
            Ok(()) => {
                let _guard = self.lock.lock().await;
                // La configuración pudo cambiar mientras tanto: se usa la guardada.
                let current = match self.db.site(service.id).await {
                    Ok(Some(site)) if site.config.domains == config.domains => site.config,
                    _ => return,
                };
                match self.write_and_apply(&service, &current, true).await {
                    Ok(_) => (TlsStatus::Active, None),
                    Err(error) => (TlsStatus::Failed, Some(DisplayChain(&error).to_string())),
                }
            }
            Err(detail) => (TlsStatus::Failed, Some(detail)),
        };
        tracing::info!(name = %slug, ?tls, detail = detail.as_deref(), "certificado del sitio");
        if let Err(error) = self.db.set_site_tls(service.id, tls, detail).await {
            tracing::error!(name = %slug, %error, "no se pudo guardar el estado del certificado");
        }
    }

    async fn certbot(&self, config: &SiteConfig) -> Result<(), String> {
        let primary = config.domains.first().ok_or("sin dominios")?;
        let mut args: Vec<String> = config
            .domains
            .iter()
            .map(|domain| format!("-d {domain}"))
            .collect();
        args.push(match &config.email {
            Some(email) => format!("--email {email}"),
            None => "--register-unsafely-without-email".to_owned(),
        });
        // Valores ya validados: solo dominios y un correo, sin espacios ni comillas.
        let env = format!("BBEL_CERTBOT_ARGS={}\n", args.join(" "));
        write(
            &self.paths.nginx_certs_dir.join(format!("{primary}.env")),
            env.into_bytes(),
        )
        .await
        .map_err(|error| DisplayChain(&error).to_string())?;
        let unit_name = format!("bbel-certbot@{primary}.service");
        let unit = UnitName::new(&unit_name).map_err(|error| error.to_string())?;
        let since = now();
        match self
            .systemctl
            .run_with_timeout(Action::Start, &unit, CERTBOT_TIMEOUT)
            .await
        {
            Ok(_) => Ok(()),
            Err(SystemctlError::JobFailed { .. }) => {
                let detail = self.journal_tail(&unit_name, since).await;
                Err(if detail.is_empty() {
                    format!("certbot falló (journalctl -u {unit_name})")
                } else {
                    detail
                })
            }
            Err(error) => Err(DisplayChain(&error).to_string()),
        }
    }

    /// Últimas líneas del journal de una unidad desde `since`: el motivo de un fallo.
    async fn journal_tail(&self, unit: &str, since: i64) -> String {
        let output = tokio::process::Command::new(&self.journalctl)
            .env_clear()
            .env("LC_ALL", "C")
            .args([
                "--system",
                "--no-pager",
                "--quiet",
                "--output=cat",
                "--lines=40",
            ])
            .arg(format!("--since=@{since}"))
            .arg(format!("--unit={unit}"))
            .kill_on_drop(true)
            .output();
        let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(5), output).await else {
            return String::new();
        };
        let text = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        let tail = lines
            .get(lines.len().saturating_sub(12)..)
            .unwrap_or_default()
            .join("\n");
        tail.chars().take(2000).collect()
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

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
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
    Ok(SiteConfig {
        template: config.template,
        domains,
        port,
        https: config.https,
        email,
    })
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
fn valid_email(email: &str) -> bool {
    let Some((user, host)) = email.split_once('@') else {
        return false;
    };
    (1..=64).contains(&user.len())
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
        }
    }

    #[test]
    fn normaliza_y_acepta() {
        let normalized = validate(
            SiteConfig {
                email: Some(" ops+bbel@Example.com ".into()),
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
        assert_eq!(normalized.email.as_deref(), Some("ops+bbel@Example.com"));
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
