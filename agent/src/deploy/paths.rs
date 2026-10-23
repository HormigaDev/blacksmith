//! Dónde vive cada cosa, según el gestor de systemd. Las plantillas usan los especificadores de
//! systemd (`%S`: raíz de estado, `%E`: raíz de configuración), así que estas rutas y las suyas
//! coinciden sin configurar nada:
//!
//! | | `system` (servidor) | `user` (modo local) |
//! |---|---|---|
//! | `%S` | `/var/lib` | `$XDG_STATE_HOME` (`~/.local/state`) |
//! | `%E` | `/etc` | `$XDG_CONFIG_HOME` (`~/.config`) |
//! | Variables | `%E/bbel/env` | |
//! | Versiones desplegadas | `%S/bbel/<nombre>` | |
//! | Especificación y token de cada despliegue | `%S/blacksmith/deployments/<nombre>` | |
//! | Acceso a GitHub (token) | `%S/blacksmith/github.json` | |
//! | Sitios de nginx generados | `%S/blacksmith/nginx/sites` | |
//! | Argumentos de certbot | `%S/blacksmith/nginx/certs` | |
//! | Plantillas de nginx (editables) | `%E/bbel/nginx/templates` | |

use std::path::{Path, PathBuf};

use crate::{services::ServiceSlug, systemd::Scope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Variables de entorno: `global.env` y `services/<nombre>.env`.
    pub env_dir: PathBuf,
    /// Directorios de trabajo: `<apps>/<nombre>` con `releases/` y el enlace `app`.
    pub apps_dir: PathBuf,
    /// Lo que el agente prepara para cada despliegue: `spec.json` y `github-token`.
    pub deployments_dir: PathBuf,
    /// Cuenta y token de GitHub del servidor (ver `crate::github`).
    pub github_file: PathBuf,
    /// Configuración de nginx de cada sitio (`<servicio>.conf`), que incluye `conf.d/bbel.conf`.
    pub nginx_sites_dir: PathBuf,
    /// Argumentos de certbot por certificado (`<dominio>.env`), para `bbel-certbot@.service`.
    pub nginx_certs_dir: PathBuf,
    /// Plantillas propias del administrador; si falta una, se usa la que trae el agente.
    pub nginx_templates_dir: PathBuf,
    /// Dónde instalar las plantillas (solo en modo `user`: en el servidor las instala root).
    pub units_dir: Option<PathBuf>,
}

impl Paths {
    pub fn for_scope(scope: Scope) -> Result<Self, String> {
        match scope {
            Scope::System => Ok(Self::rooted(Path::new("/var/lib"), Path::new("/etc"), None)),
            Scope::User => {
                let home = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|home| home.is_absolute())
                    .ok_or("HOME no está definida o no es una ruta absoluta")?;
                let xdg = |key: &str, fallback: &str| {
                    std::env::var_os(key)
                        .map(PathBuf::from)
                        .filter(|path| path.is_absolute())
                        .unwrap_or_else(|| home.join(fallback))
                };
                let config = xdg("XDG_CONFIG_HOME", ".config");
                let state = xdg("XDG_STATE_HOME", ".local/state");
                let units = config.join("systemd/user");
                Ok(Self::rooted(&state, &config, Some(units)))
            }
        }
    }

    /// Con raíces arbitrarias (tests).
    pub fn rooted(state: &Path, config: &Path, units_dir: Option<PathBuf>) -> Self {
        Self {
            env_dir: config.join("bbel/env"),
            apps_dir: state.join("bbel"),
            deployments_dir: state.join("blacksmith/deployments"),
            github_file: state.join("blacksmith/github.json"),
            nginx_sites_dir: state.join("blacksmith/nginx/sites"),
            nginx_certs_dir: state.join("blacksmith/nginx/certs"),
            nginx_templates_dir: config.join("bbel/nginx/templates"),
            units_dir,
        }
    }

    pub fn app_dir(&self, slug: &ServiceSlug) -> PathBuf {
        self.apps_dir.join(slug.as_str())
    }

    pub fn deployment_dir(&self, slug: &ServiceSlug) -> PathBuf {
        self.deployments_dir.join(slug.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas_del_sistema() {
        let paths = Paths::for_scope(Scope::System).unwrap();
        assert_eq!(paths.env_dir, Path::new("/etc/bbel/env"));
        assert_eq!(paths.apps_dir, Path::new("/var/lib/bbel"));
        assert_eq!(
            paths.deployments_dir,
            Path::new("/var/lib/blacksmith/deployments")
        );
        assert_eq!(
            paths.github_file,
            Path::new("/var/lib/blacksmith/github.json")
        );
        assert_eq!(
            paths.nginx_sites_dir,
            Path::new("/var/lib/blacksmith/nginx/sites")
        );
        assert_eq!(
            paths.nginx_templates_dir,
            Path::new("/etc/bbel/nginx/templates")
        );
        assert_eq!(paths.units_dir, None);
        let slug = ServiceSlug::new("api").unwrap();
        assert_eq!(paths.app_dir(&slug), Path::new("/var/lib/bbel/api"));
    }

    #[test]
    fn rutas_de_usuario() {
        let paths = Paths::rooted(
            Path::new("/home/ana/.local/state"),
            Path::new("/home/ana/.config"),
            Some(PathBuf::from("/home/ana/.config/systemd/user")),
        );
        assert_eq!(paths.env_dir, Path::new("/home/ana/.config/bbel/env"));
        assert_eq!(
            paths.deployments_dir,
            Path::new("/home/ana/.local/state/blacksmith/deployments")
        );
    }
}
