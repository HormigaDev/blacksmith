//! Plantillas de systemd de los servicios: `blacksmith@.service` (ejecuta la versión activa) y
//! `blacksmith-deploy@.service` (clona, prepara y activa una versión nueva).
//!
//! El agente nunca ejecuta código de los servicios: lo hacen estas unidades, como un usuario sin
//! privilegios (`blacksmith` en el servidor; el propio usuario en modo local) y con su propio sandbox.
//! El agente solo deja la especificación, el token de GitHub y, si el servicio la usa, la llave de
//! despliegue: llegan a la unidad de despliegue como credenciales (`LoadCredential=`, solo
//! legibles por ella) y sus copias se borran al terminar.
//!
//! Sandbox en el servidor: lo hace cumplir el kernel, no un análisis de los comandos (un `cd ..`
//! o una ruta absoluta dentro de un script lo burlarían). Cada unidad solo puede escribir en su
//! directorio (`%S/blacksmith/<nombre>`); el resto del sistema es de solo lectura, `/home` no existe,
//! y los directorios de los demás servicios, las variables (`%E/blacksmith`) y los datos del agente
//! (`%S/blacksmith`, donde vive el token) quedan invisibles. Tampoco ve procesos ajenos.
//!
//! En el servidor las instala root una vez (`deploy/systemd/`, generadas con [`render`]); en modo
//! local las instala el agente al arrancar ([`install_user_units`]).

use std::{fs, io, path::Path};

use crate::systemd::Scope;

pub const SERVICE_TEMPLATE: &str = "blacksmith@.service";
pub const DEPLOY_TEMPLATE: &str = "blacksmith-deploy@.service";
/// Binario del agente en el servidor (ver deploy/blacksmith.service).
pub const SYSTEM_BINARY: &str = "/opt/blacksmith/bin/blacksmith-agent";

/// Endurecimiento común en el servidor. En modo local no se aplica: casi todo exige un gestor
/// con privilegios (espacios de nombres), y allí el servicio ya corre como el propio usuario.
const HARDENING: &str = "\
# Solo su directorio: los demás servicios, las variables y los datos del agente no existen.
TemporaryFileSystem=%S/blacksmith:ro
BindPaths=%S/blacksmith/%i
InaccessiblePaths=-%E/blacksmith
InaccessiblePaths=-%S/blacksmith
ProtectProc=invisible
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
RestrictSUIDSGID=yes
RestrictRealtime=yes
LockPersonality=yes
RemoveIPC=yes
";

/// Contenido de las dos plantillas (`(nombre, contenido)`).
pub fn render(scope: Scope, binary: &str) -> [(&'static str, String); 2] {
    let system = scope == Scope::System;
    let user = if system {
        "User=blacksmith\nGroup=blacksmith\n"
    } else {
        ""
    };
    let hardening = if system { HARDENING } else { "" };
    // El servicio no recibe credenciales: tampoco ve las de otras unidades (el token).
    let credentials = if system {
        "InaccessiblePaths=-/run/credentials\n"
    } else {
        ""
    };
    // Contabilidad del cgroup explícita (consumo en la interfaz), sin depender de los valores
    // por defecto de cada distribución.
    // Directorio de estado y HOME propios: cachés de npm, cargo, pip… quedan dentro. `0711` +
    // `UMask=0077`: nadie más puede listarlo ni leer lo que crea el servicio; solo atravesarlo
    // hasta `public/`, que el runner publica legible para nginx.
    let common = format!(
        "{user}StateDirectory=blacksmith/%i\n\
         StateDirectoryMode=0711\n\
         UMask=0077\n\
         CPUAccounting=yes\n\
         MemoryAccounting=yes\n\
         TasksAccounting=yes\n\
         Environment=HOME=%S/blacksmith/%i\n\
         EnvironmentFile=-%E/blacksmith/env/global.env\n\
         EnvironmentFile=-%E/blacksmith/env/services/%i.env\n"
    );
    let service = format!(
        "# Generada por Blacksmith (agent/src/deploy/units.rs): no la edite a mano.\n\
         # Servicio desplegado %i: ejecuta la versión activa (%S/blacksmith/%i/app).\n\
         \n\
         [Unit]\n\
         Description=Blacksmith: %i\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         # Sin una versión desplegada no hay nada que arrancar.\n\
         ConditionPathIsSymbolicLink=%S/blacksmith/%i/app\n\
         \n\
         [Service]\n\
         Type=exec\n\
         {common}\
         WorkingDirectory=%S/blacksmith/%i/app\n\
         ExecStart={binary} run-service\n\
         Restart=on-failure\n\
         RestartSec=3\n\
         {hardening}{credentials}"
    );
    let deploy = format!(
        "# Generada por Blacksmith (agent/src/deploy/units.rs): no la edite a mano.\n\
         # Despliegue de %i: clona la rama, ejecuta la preparación y activa la versión.\n\
         \n\
         [Unit]\n\
         Description=Blacksmith: despliegue de %i\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         {common}\
         WorkingDirectory=%S/blacksmith/%i\n\
         LoadCredential=spec:%S/blacksmith/deployments/%i/spec.json\n\
         LoadCredential=github-token:%S/blacksmith/deployments/%i/github-token\n\
         LoadCredential=deploy-key:%S/blacksmith/deployments/%i/deploy-key\n\
         ExecStart={binary} run-deploy\n\
         TimeoutStartSec=30min\n\
         {hardening}"
    );
    [(SERVICE_TEMPLATE, service), (DEPLOY_TEMPLATE, deploy)]
}

/// Modo local: instala (o actualiza) las plantillas en el systemd del usuario. Devuelve si cambió
/// alguna, para hacer `daemon-reload` solo entonces.
pub fn install_user_units(units_dir: &Path, binary: &Path) -> io::Result<bool> {
    fs::create_dir_all(units_dir)?;
    let binary = binary.to_string_lossy();
    let mut changed = false;
    for (name, content) in render(Scope::User, &binary) {
        let path = units_dir.join(name);
        if fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()) {
            fs::write(&path, content)?;
            changed = true;
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que root instala en el servidor es exactamente lo que genera el código.
    #[test]
    fn las_plantillas_del_servidor_estan_al_dia() {
        let [(service_name, service), (deploy_name, deploy)] = render(Scope::System, SYSTEM_BINARY);
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../deploy/systemd");
        for (name, content) in [(service_name, service), (deploy_name, deploy)] {
            let installed = fs::read_to_string(dir.join(name)).unwrap_or_default();
            assert_eq!(
                installed, content,
                "deploy/systemd/{name} no coincide con agent/src/deploy/units.rs; regenérelo"
            );
        }
    }

    #[test]
    fn modo_local_sin_usuario_ni_sandbox() {
        let [(_, service), (_, deploy)] = render(Scope::User, "/home/ana/bin/blacksmith-agent");
        for unit in [&service, &deploy] {
            assert!(!unit.contains("User="));
            assert!(!unit.contains("ProtectSystem"));
            assert!(unit.contains("/home/ana/bin/blacksmith-agent run-"));
            assert!(unit.contains("EnvironmentFile=-%E/blacksmith/env/services/%i.env"));
        }
        assert!(
            deploy
                .contains("LoadCredential=github-token:%S/blacksmith/deployments/%i/github-token")
        );
    }

    #[test]
    fn en_el_servidor_cada_servicio_solo_ve_su_directorio() {
        let [(_, service), (_, deploy)] = render(Scope::System, SYSTEM_BINARY);
        for unit in [&service, &deploy] {
            assert!(unit.contains("User=blacksmith\n"));
            assert!(
                unit.contains("TemporaryFileSystem=%S/blacksmith:ro\nBindPaths=%S/blacksmith/%i\n")
            );
            assert!(unit.contains("InaccessiblePaths=-%S/blacksmith\n"));
            assert!(unit.contains("InaccessiblePaths=-%E/blacksmith\n"));
            assert!(unit.contains("ProtectSystem=strict\n"));
            assert!(unit.contains("ProtectHome=yes\n"));
            assert!(unit.contains("ProtectProc=invisible\n"));
        }
        assert!(service.contains("InaccessiblePaths=-/run/credentials\n"));
        assert!(!deploy.contains("InaccessiblePaths=-/run/credentials"));
    }

    #[test]
    fn instala_solo_si_cambia() {
        let dir = tempfile::tempdir().unwrap();
        let binary = Path::new("/opt/x/blacksmith-agent");
        assert!(install_user_units(dir.path(), binary).unwrap());
        assert!(!install_user_units(dir.path(), binary).unwrap());
        assert!(dir.path().join(SERVICE_TEMPLATE).exists());
    }
}
