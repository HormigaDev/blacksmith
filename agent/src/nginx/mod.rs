//! Sitios web de los servicios detrás de nginx: plantillas por tipo de proyecto, aplicación
//! segura de la configuración y certificados de Let's Encrypt (ver `deploy/nginx/`).

pub mod sites;
pub mod template;

pub use sites::{SiteError, SiteManager};
