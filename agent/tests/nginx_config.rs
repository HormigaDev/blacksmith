//! La configuración que genera el agente la acepta un nginx real (`nginx -t`): todas las
//! plantillas, con y sin HTTPS, junto con los snippets y `conf.d` de `deploy/nginx/`. Se ejecuta
//! sin root, en un prefijo temporal; como `nginx -t` abre los puertos, 80 y 443 pasan a puertos
//! altos. Si nginx no está instalado, se omite.

use std::{fs, path::Path, process::Command};

use agent::nginx::template::{SiteVars, load, render_body, render_site};
use shared::{SiteRoute, SiteTemplate};

const NGINX: &str = "/usr/sbin/nginx";

/// 80/443 → 18080/18443: sin root no se pueden abrir puertos bajos.
fn high_ports(config: &str) -> String {
    config
        .replace("listen 80", "listen 18080")
        .replace("listen [::]:80", "listen [::]:18080")
        .replace("listen 443", "listen 18443")
        .replace("listen [::]:443", "listen [::]:18443")
}

#[test]
fn nginx_acepta_todas_las_plantillas() {
    if !Path::new(NGINX).exists() {
        eprintln!("nginx no está instalado: se omite");
        return;
    }
    let deploy = Path::new(env!("CARGO_MANIFEST_DIR")).join("../deploy/nginx");
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Snippets y ajustes comunes, como los instala install.sh.
    fs::create_dir_all(root.join("snippets/blacksmith")).unwrap();
    for entry in fs::read_dir(deploy.join("snippets/blacksmith")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(
            entry.path(),
            root.join("snippets/blacksmith").join(entry.file_name()),
        )
        .unwrap();
    }
    let sites = root.join("sites");
    fs::create_dir_all(&sites).unwrap();
    let common = fs::read_to_string(deploy.join("conf.d/blacksmith.conf"))
        .unwrap()
        .replace("@SITES_DIR@", sites.to_str().unwrap());
    fs::write(root.join("blacksmith.conf"), common).unwrap();
    fs::write(
        root.join("blacksmith-default.conf"),
        high_ports(&fs::read_to_string(deploy.join("conf.d/blacksmith-default.conf")).unwrap()),
    )
    .unwrap();

    // Certificado autofirmado en lugar del de Let's Encrypt.
    let certs = root.join("letsencrypt");
    fs::create_dir_all(&certs).unwrap();
    let status = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:prime256v1",
        ])
        .args([
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=blacksmith.test",
            "-keyout",
        ])
        .arg(certs.join("privkey.pem"))
        .arg("-out")
        .arg(certs.join("fullchain.pem"))
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );

    // Un sitio por plantilla y modo, con dominios distintos (como servicios distintos).
    let templates_dir = root.join("sin-plantillas-propias");
    for template in SiteTemplate::ALL {
        for tls in [false, true] {
            let service = format!(
                "{}-{}",
                template.file_stem(),
                if tls { "tls" } else { "http" }
            );
            let vars = SiteVars {
                service: service.clone(),
                template,
                domains: vec![
                    format!("{service}.blacksmith.test"),
                    format!("www.{service}.blacksmith.test"),
                ],
                upstream: template.proxies().then(|| "127.0.0.1:3000".into()),
                root: template
                    .serves_files()
                    .then(|| format!("/var/lib/blacksmith/{service}/public/current")),
                tls,
                // Rutas hacia otros puertos en todas: una normal y una de WebSocket.
                routes: vec![
                    SiteRoute {
                        path: "/graphql".into(),
                        port: 3002,
                        websocket: false,
                    },
                    SiteRoute {
                        path: "/socket.io/".into(),
                        port: 3001,
                        websocket: true,
                    },
                ],
            };
            let body = render_body(&load(&templates_dir, template).unwrap(), &vars).unwrap();
            let site = render_site(&body, &vars)
                // Rutas del sistema → prefijo temporal.
                .replace(
                    &format!("/etc/letsencrypt/live/{service}.blacksmith.test/"),
                    &format!("{}/", certs.display()),
                )
                .replace("/var/log/nginx/", &format!("{}/", root.display()));
            fs::write(sites.join(format!("{service}.conf")), high_ports(&site)).unwrap();
        }
    }

    // Los webhooks del agente (deploy/nginx/webhooks.conf), con su propio dominio.
    let webhooks = fs::read_to_string(deploy.join("webhooks.conf"))
        .unwrap()
        .replace(
            "/etc/letsencrypt/live/hooks.example.com/",
            &format!("{}/", certs.display()),
        );
    fs::write(sites.join("webhooks.conf"), high_ports(&webhooks)).unwrap();

    fs::write(
        root.join("nginx.conf"),
        format!(
            "pid {root}/nginx.pid;\nerror_log stderr;\nevents {{}}\nhttp {{\n    \
             include /etc/nginx/mime.types;\n    access_log {root}/access.log;\n    \
             include {root}/blacksmith.conf;\n    include {root}/blacksmith-default.conf;\n}}\n",
            root = root.display()
        ),
    )
    .unwrap();
    let output = Command::new(NGINX)
        .arg("-t")
        .arg("-p")
        .arg(root)
        .arg("-c")
        .arg(root.join("nginx.conf"))
        .args(["-e", "stderr"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "nginx -t falló:\n{stderr}");
    assert!(stderr.contains("test is successful"), "{stderr}");
    // Ni avisos: un aviso suele ser una configuración que no hace lo que parece.
    let warnings: Vec<&str> = stderr
        .lines()
        .filter(|line| {
            line.contains("[warn]") && !line.contains("\"http2\" directive is deprecated")
        })
        .collect();
    assert!(warnings.is_empty(), "nginx avisa:\n{}", warnings.join("\n"));
}
