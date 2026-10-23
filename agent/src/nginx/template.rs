//! Configuración de nginx de un sitio: la plantilla del tipo de proyecto (el cuerpo del `server`,
//! editable en `%E/blacksmith/nginx/templates/`) dentro de un envoltorio fijo que genera el agente:
//! puertos, desafío ACME, redirección a HTTPS, certificado, TLS, cabeceras de seguridad y logs.
//!
//! Las variables (`{{root}}`, `{{upstream}}`…) solo reciben valores ya validados (dominios,
//! un puerto, rutas del agente): no hay forma de colar directivas de nginx desde la API.

use std::{fmt::Write as _, io, path::Path};

use shared::{SiteRoute, SiteTemplate};

/// Plantillas que trae el agente (las mismas que instala `deploy/nginx/install.sh`). Si el
/// administrador tiene una propia en el directorio de plantillas, se usa esa.
fn embedded(template: SiteTemplate) -> &'static str {
    match template {
        SiteTemplate::Static => include_str!("../../../deploy/nginx/templates/static.conf"),
        SiteTemplate::Spa => include_str!("../../../deploy/nginx/templates/spa.conf"),
        SiteTemplate::App => include_str!("../../../deploy/nginx/templates/app.conf"),
        SiteTemplate::Api => include_str!("../../../deploy/nginx/templates/api.conf"),
        SiteTemplate::Websocket => include_str!("../../../deploy/nginx/templates/websocket.conf"),
        SiteTemplate::Fullstack => include_str!("../../../deploy/nginx/templates/fullstack.conf"),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("no se pudo leer la plantilla {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("la plantilla «{template}» usa {{{{{name}}}}}, que no existe o no aplica a este sitio")]
    Variable {
        template: &'static str,
        name: String,
    },
    #[error("la plantilla «{template}» tiene un «{{{{» sin cerrar")]
    Unclosed { template: &'static str },
}

/// Texto de la plantilla: la del administrador si existe; si no, la que trae el agente.
pub fn load(templates_dir: &Path, template: SiteTemplate) -> Result<String, TemplateError> {
    let path = templates_dir.join(format!("{}.conf", template.file_stem()));
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(embedded(template).to_owned()),
        Err(source) => Err(TemplateError::Read {
            path: path.display().to_string(),
            source,
        }),
    }
}

/// Valores del sitio, ya validados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteVars {
    pub service: String,
    pub template: SiteTemplate,
    /// El primero es el principal (nombre del certificado).
    pub domains: Vec<String>,
    /// `127.0.0.1:<puerto>`, en las plantillas con proxy.
    pub upstream: Option<String>,
    /// Carpeta pública de la versión activa, en las plantillas que sirven archivos.
    pub root: Option<String>,
    /// HTTPS: certificado de Let's Encrypt ya emitido para `domains[0]`.
    pub tls: bool,
    /// Rutas hacia otros puertos (ya validadas): van antes del cuerpo de la plantilla.
    pub routes: Vec<SiteRoute>,
}

impl SiteVars {
    fn primary(&self) -> &str {
        self.domains.first().map_or("", String::as_str)
    }

    fn variable(&self, name: &str) -> Option<String> {
        match name {
            "service" => Some(self.service.clone()),
            "server_names" => Some(self.domains.join(" ")),
            "primary" => Some(self.primary().to_owned()),
            "upstream" => self.upstream.clone(),
            "root" => self.root.clone(),
            _ => None,
        }
    }
}

/// Sustituye las variables de `{{nombre}}` del cuerpo. Una variable desconocida o sin valor
/// (p. ej. `{{root}}` en un sitio sin carpeta pública) es un error: nunca se deja a medias. Las
/// líneas de comentario (`#`) se copian tal cual: documentan la plantilla.
pub fn render_body(text: &str, vars: &SiteVars) -> Result<String, TemplateError> {
    let template = vars.template.file_stem();
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        if line.trim_start().starts_with('#') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let mut rest = line;
        while let Some((before, after)) = rest.split_once("{{") {
            out.push_str(before);
            let (name, tail) = after
                .split_once("}}")
                .ok_or(TemplateError::Unclosed { template })?;
            let name = name.trim();
            let value = vars.variable(name).ok_or_else(|| TemplateError::Variable {
                template,
                name: name.to_owned(),
            })?;
            out.push_str(&value);
            rest = tail;
        }
        out.push_str(rest);
        out.push('\n');
    }
    Ok(out)
}

/// Un `location ^~` por ruta: gana a las expresiones regulares de la plantilla (p. ej. las de
/// archivos estáticos), así `/api/app.js` nunca se busca como archivo. `proxy_pass` sin URI: el
/// servicio recibe la ruta completa, con su prefijo.
fn render_routes(routes: &[SiteRoute]) -> String {
    let mut out = String::new();
    for route in routes {
        let (connections, websocket) = if route.websocket {
            (200, "    include snippets/blacksmith/websocket.conf;\n")
        } else {
            (100, "")
        };
        let _ = write!(
            out,
            "location ^~ {path} {{\n    limit_conn blacksmith_conn {connections};\n    \
             include snippets/blacksmith/proxy.conf;\n{websocket}    \
             proxy_pass http://127.0.0.1:{port};\n}}\n\n",
            path = route.path,
            port = route.port,
        );
    }
    out
}

/// Prefijos que la plantilla ya tiene como `location` (`location /api/`, `location ^~ /ws`,
/// `location = /index.html`): una ruta con el mismo prefijo daría un `location` duplicado.
pub fn template_locations(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| line.trim().strip_prefix("location "))
        .filter_map(|rest| {
            let rest = rest.trim_end_matches('{').trim();
            let path = rest
                .strip_prefix("^~ ")
                .or_else(|| rest.strip_prefix("= "))
                .unwrap_or(rest)
                .trim();
            // Las expresiones regulares (`~`, `~*`) no chocan: se dejan.
            path.starts_with('/').then(|| path.to_owned())
        })
        .collect()
}

/// Configuración completa del sitio: el envoltorio con el cuerpo ya renderizado.
pub fn render_site(body: &str, vars: &SiteVars) -> String {
    let service = &vars.service;
    let names = vars.domains.join(" ");
    let primary = vars.primary();
    let indented = indent(&format!("{}{body}", render_routes(&vars.routes)));
    let mut out = String::new();
    let _ = write!(
        out,
        "# Generado por Blacksmith: servicio «{service}», plantilla «{template}». No lo edite: se\n\
         # sustituye al guardar el sitio. Para cambiar el comportamiento edite la plantilla\n\
         # (/etc/blacksmith/nginx/templates/{template}.conf o la de su modo local).\n\n",
        template = vars.template.file_stem(),
    );
    let logs = format!(
        "    access_log /var/log/nginx/blacksmith-{service}.access.log;\n    \
         error_log /var/log/nginx/blacksmith-{service}.error.log warn;\n"
    );
    let _ = write!(
        out,
        "server {{\n    listen 80;\n    listen [::]:80;\n    server_name {names};\n{logs}    \
         include snippets/blacksmith/acme.conf;\n"
    );
    if vars.tls {
        let _ = write!(
            out,
            "\n    location / {{\n        return 301 https://$host$request_uri;\n    }}\n}}\n\n\
             server {{\n    listen 443 ssl http2;\n    listen [::]:443 ssl http2;\n    \
             server_name {names};\n{logs}\n    \
             ssl_certificate /etc/letsencrypt/live/{primary}/fullchain.pem;\n    \
             ssl_certificate_key /etc/letsencrypt/live/{primary}/privkey.pem;\n    \
             include snippets/blacksmith/tls.conf;\n    include snippets/blacksmith/security-headers.conf;\n\n\
             {indented}}}\n"
        );
    } else {
        let _ = write!(
            out,
            "    include snippets/blacksmith/security-headers.conf;\n\n{indented}}}\n"
        );
    }
    out
}

fn indent(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + body.lines().count() * 4);
    for line in body.trim_end().lines() {
        if !line.is_empty() {
            out.push_str("    ");
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(template: SiteTemplate, tls: bool) -> SiteVars {
        SiteVars {
            service: "web".into(),
            template,
            domains: vec!["example.com".into(), "www.example.com".into()],
            upstream: template.proxies().then(|| "127.0.0.1:3000".into()),
            root: template
                .serves_files()
                .then(|| "/var/lib/blacksmith/web/public/current".into()),
            tls,
            routes: Vec::new(),
        }
    }

    #[test]
    fn todas_las_plantillas_renderizan_con_y_sin_tls() {
        for template in SiteTemplate::ALL {
            for tls in [false, true] {
                let vars = vars(template, tls);
                let body = render_body(embedded(template), &vars).unwrap();
                assert!(
                    body.lines()
                        .filter(|line| !line.trim_start().starts_with('#'))
                        .all(|line| !line.contains("{{")),
                    "{template:?}: {body}"
                );
                let site = render_site(&body, &vars);
                assert!(site.contains("server_name example.com www.example.com;"));
                assert!(site.contains("include snippets/blacksmith/acme.conf;"));
                assert!(site.contains("include snippets/blacksmith/security-headers.conf;"));
                assert_eq!(
                    site.contains(
                        "ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;"
                    ),
                    tls
                );
                assert_eq!(site.contains("return 301 https://$host$request_uri;"), tls);
                assert_eq!(
                    site.matches('{').count(),
                    site.matches('}').count(),
                    "{site}"
                );
            }
        }
    }

    #[test]
    fn rutas_antes_de_la_plantilla() {
        let mut site = vars(SiteTemplate::Spa, true);
        site.routes = vec![
            SiteRoute {
                path: "/api/".into(),
                port: 3000,
                websocket: false,
            },
            SiteRoute {
                path: "/socket.io/".into(),
                port: 3001,
                websocket: true,
            },
        ];
        let body = render_body(embedded(SiteTemplate::Spa), &site).unwrap();
        let full = render_site(&body, &site);
        // Con HTTPS, el servidor HTTP solo redirige: las rutas van en el de HTTPS.
        let rendered = &full[full.find("listen 443").unwrap()..];
        assert!(!full[..full.find("listen 443").unwrap()].contains("/api/"));
        let api = rendered.find("location ^~ /api/ {").unwrap();
        let socket = rendered.find("location ^~ /socket.io/ {").unwrap();
        let spa = rendered.find("location / {").unwrap();
        assert!(api < socket && socket < spa, "{rendered}");
        assert!(rendered.contains("proxy_pass http://127.0.0.1:3000;"));
        assert!(rendered.contains("proxy_pass http://127.0.0.1:3001;"));
        // Solo la de WebSocket amplía los tiempos.
        assert_eq!(
            rendered
                .matches("include snippets/blacksmith/websocket.conf;")
                .count(),
            1
        );
    }

    #[test]
    fn locations_de_la_plantilla() {
        let body = "location ^~ /api/ {\n}\nlocation /ws {\n}\nlocation = /index.html {\n}\n\
                    location ~* \\.(?:css|js)$ {\n}\nlocation / {\n}\n";
        assert_eq!(
            template_locations(body),
            ["/api/", "/ws", "/index.html", "/"]
        );
    }

    #[test]
    fn variables_desconocidas_o_sin_valor() {
        let static_site = vars(SiteTemplate::Static, false);
        assert!(matches!(
            render_body("proxy_pass http://{{upstream}};", &static_site),
            Err(TemplateError::Variable { name, .. }) if name == "upstream"
        ));
        assert!(matches!(
            render_body("root {{ruta}};", &static_site),
            Err(TemplateError::Variable { name, .. }) if name == "ruta"
        ));
        assert!(matches!(
            render_body("root {{root;", &static_site),
            Err(TemplateError::Unclosed { .. })
        ));
        // Los comentarios se copian tal cual.
        assert_eq!(
            render_body(
                "# usa {{upstream}} o {{root}}\nroot {{root}};",
                &static_site
            )
            .unwrap(),
            "# usa {{upstream}} o {{root}}\nroot /var/lib/blacksmith/web/public/current;\n"
        );
    }

    #[test]
    fn la_plantilla_del_administrador_tiene_prioridad() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load(dir.path(), SiteTemplate::Api).unwrap(),
            embedded(SiteTemplate::Api)
        );
        std::fs::write(dir.path().join("api.conf"), "location / { return 204; }\n").unwrap();
        assert_eq!(
            load(dir.path(), SiteTemplate::Api).unwrap(),
            "location / { return 204; }\n"
        );
    }
}
