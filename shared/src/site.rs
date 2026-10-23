//! Sitio web de un servicio detrás de nginx (`/api/v1/services/{name}/site`): plantilla, dominios,
//! puerto del servicio y HTTPS con Let's Encrypt. El agente genera la configuración solo con estos
//! valores (validados): nunca acepta texto de configuración de nginx.

use serde::{Deserialize, Serialize};

/// Tipo de proyecto: elige la plantilla (`/etc/bbel/nginx/templates/<plantilla>.conf`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SiteTemplate {
    /// Archivos de la carpeta pública (HTML, CSS, JS).
    Static,
    /// Aplicación de una sola página: las rutas del cliente devuelven `index.html`.
    Spa,
    /// Aplicación web servida por el propio servicio (SSR, MVC), con WebSocket.
    App,
    /// API HTTP con límite de peticiones por IP.
    Api,
    /// Servidor de WebSocket o eventos (conexiones largas, sin búfer).
    Websocket,
    /// Frontend estático + `/api/` y `/ws` hacia el servicio.
    Fullstack,
}

impl SiteTemplate {
    pub const ALL: [Self; 6] = [
        Self::Static,
        Self::Spa,
        Self::App,
        Self::Api,
        Self::Websocket,
        Self::Fullstack,
    ];

    /// Nombre del archivo de la plantilla, sin `.conf`.
    pub fn file_stem(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Spa => "spa",
            Self::App => "app",
            Self::Api => "api",
            Self::Websocket => "websocket",
            Self::Fullstack => "fullstack",
        }
    }

    /// Necesita la carpeta pública del despliegue.
    pub fn serves_files(self) -> bool {
        matches!(self, Self::Static | Self::Spa | Self::Fullstack)
    }

    /// Reenvía peticiones al servicio (necesita su puerto).
    pub fn proxies(self) -> bool {
        matches!(
            self,
            Self::App | Self::Api | Self::Websocket | Self::Fullstack
        )
    }
}

/// Cuerpo de `PUT /api/v1/services/{name}/site`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteConfig {
    pub template: SiteTemplate,
    /// El primero es el principal (nombre del certificado); el resto, alias (p. ej. `www.`).
    pub domains: Vec<String>,
    /// Puerto en el que escucha el servicio en 127.0.0.1 (plantillas con proxy).
    pub port: Option<u16>,
    /// HTTPS con un certificado de Let's Encrypt (redirige HTTP → HTTPS).
    pub https: bool,
    /// Para los avisos de caducidad de Let's Encrypt (opcional).
    pub email: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TlsStatus {
    /// Sin HTTPS.
    Off,
    /// Pidiendo el certificado (el sitio ya responde por HTTP).
    Issuing,
    /// Certificado emitido: el sitio va por HTTPS.
    Active,
    /// No se pudo emitir: el sitio sigue por HTTP (ver `tls_detail`).
    Failed,
}

/// Respuesta de `GET` y `PUT /api/v1/services/{name}/site`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    pub name: String,
    #[serde(flatten)]
    pub config: SiteConfig,
    pub tls: TlsStatus,
    /// Motivo del último fallo del certificado.
    pub tls_detail: Option<String>,
    /// Configuración generada en el servidor (para revisarla o depurar).
    pub config_path: String,
    /// Segundos Unix de la última vez que se aplicó.
    pub updated_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_del_sitio() {
        let site = Site {
            name: "web".into(),
            config: SiteConfig {
                template: SiteTemplate::Fullstack,
                domains: vec!["example.com".into(), "www.example.com".into()],
                port: Some(3000),
                https: true,
                email: None,
            },
            tls: TlsStatus::Issuing,
            tls_detail: None,
            config_path: "/var/lib/blacksmith/nginx/sites/web.conf".into(),
            updated_at: 1,
        };
        let json = serde_json::to_string(&site).unwrap();
        assert!(json.contains(r#""template":"fullstack""#), "{json}");
        assert!(json.contains(r#""tls":"issuing""#), "{json}");
        assert_eq!(serde_json::from_str::<Site>(&json).unwrap(), site);
    }
}
