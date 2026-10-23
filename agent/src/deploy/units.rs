//! Plantillas de systemd de los servicios:
//! - `blacksmith@.service`: ejecuta la versión activa.
//! - `blacksmith-fetch@.service`: clona la rama, con el token de GitHub o la llave de despliegue.
//! - `blacksmith-deploy@.service`: prepara (comandos del usuario), publica y activa; sin credenciales.
//!
//! El agente nunca ejecuta código de los servicios, y en el servidor tampoco comparte usuario con
//! él: polkit autoriza al usuario del agente (`blacksmith`), así que un servicio que corriera como
//! él podría parar a los demás o cambiar sus límites por D-Bus.
//! - Los servicios y su preparación corren como `blacksmith-apps`.
//! - El clon, como `blacksmith-fetch`: es la única unidad con credenciales y el código del
//!   repositorio nunca se ejecuta con ese usuario. Un `postinstall` malicioso en la preparación no
//!   puede leer el token, ni siquiera durante el clon de otro servicio.
//!
//! Sandbox en el servidor: lo hace cumplir el kernel, no un análisis de los comandos (un `cd ..`
//! o una ruta absoluta dentro de un script lo burlarían). Cada unidad solo puede escribir en su
//! directorio; el resto del sistema es de solo lectura, `/home` no existe y los directorios de los
//! demás servicios, las variables (`%E/blacksmith`), los datos del agente (`%S/blacksmith`) y el
//! bus del sistema (D-Bus) quedan invisibles.
//!
//! En el servidor las instala root una vez (`deploy/systemd/`, generadas con [`render`]); en modo
//! local las instala el agente al arrancar ([`install_user_units`]). En modo local todo corre como
//! el propio usuario, sin sandbox: casi todo exige un gestor con privilegios.

use std::{fs, io, path::Path};

use crate::systemd::Scope;

pub const SERVICE_TEMPLATE: &str = "blacksmith@.service";
pub const FETCH_TEMPLATE: &str = "blacksmith-fetch@.service";
pub const DEPLOY_TEMPLATE: &str = "blacksmith-deploy@.service";
/// Binario del agente en el servidor (ver deploy/blacksmith.service).
pub const SYSTEM_BINARY: &str = "/opt/blacksmith/bin/blacksmith-agent";
/// Usuario de los servicios y de su preparación.
pub const SERVICE_USER: &str = "blacksmith-apps";
/// Usuario del clon: el único que recibe credenciales.
pub const FETCH_USER: &str = "blacksmith-fetch";
/// Variable con la ruta del clon que recibe `run-deploy`.
pub const SOURCE_ENV: &str = "BLACKSMITH_SOURCE";

/// Endurecimiento común en el servidor (las rutas visibles van aparte, por unidad).
const HARDENING: &str = "\
InaccessiblePaths=-%S/blacksmith
InaccessiblePaths=-%E/blacksmith
InaccessiblePaths=-/run/dbus
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

/// Contenido de las tres plantillas (`(nombre, contenido)`).
pub fn render(scope: Scope, binary: &str) -> [(&'static str, String); 3] {
    let system = scope == Scope::System;
    let only_system = |text: String| if system { text } else { String::new() };
    let user = |name: &str| only_system(format!("User={name}\nGroup={name}\n"));
    let hardening = only_system(HARDENING.to_owned());
    // Contabilidad del cgroup explícita (consumo en la interfaz), sin depender de los valores
    // por defecto de cada distribución.
    // Directorio de estado y HOME propios: cachés de npm, cargo, pip… quedan dentro. `0711` +
    // `UMask=0077`: nadie más puede listarlo ni leer lo que crea el servicio; solo atravesarlo
    // hasta `public/`, que el runner publica legible para nginx.
    let app = format!(
        "{}StateDirectory=blacksmith-apps/%i\n\
         StateDirectoryMode=0711\n\
         UMask=0077\n\
         CPUAccounting=yes\n\
         MemoryAccounting=yes\n\
         TasksAccounting=yes\n\
         Environment=HOME=%S/blacksmith-apps/%i\n\
         EnvironmentFile=-%E/blacksmith/env/global.env\n\
         EnvironmentFile=-%E/blacksmith/env/services/%i.env\n",
        user(SERVICE_USER)
    );
    // Solo su directorio; los clones (con el código privado de otros servicios) no existen.
    let app_sandbox = only_system(
        "TemporaryFileSystem=%S/blacksmith-apps:ro\n\
         BindPaths=%S/blacksmith-apps/%i\n\
         InaccessiblePaths=-%S/blacksmith-fetch\n"
            .to_owned(),
    );
    let service = format!(
        "# Generada por Blacksmith (agent/src/deploy/units.rs): no la edite a mano.\n\
         # Servicio desplegado %i: ejecuta la versión activa (%S/blacksmith-apps/%i/app).\n\
         \n\
         [Unit]\n\
         Description=Blacksmith: %i\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         # Sin una versión desplegada no hay nada que arrancar.\n\
         ConditionPathIsSymbolicLink=%S/blacksmith-apps/%i/app\n\
         \n\
         [Service]\n\
         Type=exec\n\
         {app}\
         WorkingDirectory=%S/blacksmith-apps/%i/app\n\
         ExecStart={binary} run-service\n\
         Restart=on-failure\n\
         RestartSec=3\n\
         {app_sandbox}{hardening}{credentials}",
        // Un servicio no recibe credenciales ni ve las de otras unidades.
        credentials = only_system("InaccessiblePaths=-/run/credentials\n".to_owned()),
    );
    let fetch = format!(
        "# Generada por Blacksmith (agent/src/deploy/units.rs): no la edite a mano.\n\
         # Clon de %i: descarga la rama con las credenciales del despliegue. No ejecuta código del\n\
         # repositorio: eso lo hace blacksmith-deploy@%i, con otro usuario y sin credenciales.\n\
         \n\
         [Unit]\n\
         Description=Blacksmith: clon de %i\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         {}StateDirectory=blacksmith-fetch/%i\n\
         StateDirectoryMode=0711\n\
         # El clon queda legible: lo copia la preparación, que corre con otro usuario.\n\
         UMask=0022\n\
         Environment=HOME=%S/blacksmith-fetch/%i\n\
         WorkingDirectory=%S/blacksmith-fetch/%i\n\
         LoadCredential=spec:%S/blacksmith/deployments/%i/spec.json\n\
         LoadCredential=github-token:%S/blacksmith/deployments/%i/github-token\n\
         LoadCredential=deploy-key:%S/blacksmith/deployments/%i/deploy-key\n\
         ExecStart={binary} run-fetch\n\
         TimeoutStartSec=10min\n\
         {}{hardening}",
        user(FETCH_USER),
        only_system(
            "TemporaryFileSystem=%S/blacksmith-fetch:ro\n\
             BindPaths=%S/blacksmith-fetch/%i\n\
             InaccessiblePaths=-%S/blacksmith-apps\n"
                .to_owned()
        ),
    );
    let deploy = format!(
        "# Generada por Blacksmith (agent/src/deploy/units.rs): no la edite a mano.\n\
         # Despliegue de %i: copia el clon, ejecuta la preparación y activa la versión.\n\
         \n\
         [Unit]\n\
         Description=Blacksmith: despliegue de %i\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         {app}\
         WorkingDirectory=%S/blacksmith-apps/%i\n\
         # Solo la especificación, que no lleva secretos.\n\
         LoadCredential=spec:%S/blacksmith/deployments/%i/spec.json\n\
         Environment={SOURCE_ENV}=%S/blacksmith-fetch/%i/source\n\
         ExecStart={binary} run-deploy\n\
         TimeoutStartSec=30min\n\
         {}{hardening}",
        only_system(
            "TemporaryFileSystem=%S/blacksmith-apps:ro\n\
             BindPaths=%S/blacksmith-apps/%i\n\
             TemporaryFileSystem=%S/blacksmith-fetch:ro\n\
             BindReadOnlyPaths=-%S/blacksmith-fetch/%i\n"
                .to_owned()
        ),
    );
    [
        (SERVICE_TEMPLATE, service),
        (FETCH_TEMPLATE, fetch),
        (DEPLOY_TEMPLATE, deploy),
    ]
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
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../deploy/systemd");
        for (name, content) in render(Scope::System, SYSTEM_BINARY) {
            let installed = fs::read_to_string(dir.join(name)).unwrap_or_default();
            assert_eq!(
                installed, content,
                "deploy/systemd/{name} no coincide con agent/src/deploy/units.rs; regenérelo"
            );
        }
    }

    #[test]
    fn modo_local_sin_usuario_ni_sandbox() {
        let [(_, service), (_, fetch), (_, deploy)] =
            render(Scope::User, "/home/ana/bin/blacksmith-agent");
        for unit in [&service, &fetch, &deploy] {
            assert!(!unit.contains("User="));
            assert!(!unit.contains("ProtectSystem"));
            assert!(!unit.contains("TemporaryFileSystem"));
            assert!(unit.contains("/home/ana/bin/blacksmith-agent run-"));
        }
        assert!(service.contains("EnvironmentFile=-%E/blacksmith/env/services/%i.env"));
        assert!(
            fetch.contains("LoadCredential=github-token:%S/blacksmith/deployments/%i/github-token")
        );
    }

    #[test]
    fn en_el_servidor_ningun_servicio_corre_como_el_agente() {
        let [(_, service), (_, fetch), (_, deploy)] = render(Scope::System, SYSTEM_BINARY);
        assert!(service.contains("User=blacksmith-apps\n"));
        assert!(deploy.contains("User=blacksmith-apps\n"));
        assert!(fetch.contains("User=blacksmith-fetch\n"));
        for unit in [&service, &fetch, &deploy] {
            assert!(!unit.contains("User=blacksmith\n"));
            assert!(unit.contains("InaccessiblePaths=-%S/blacksmith\n"));
            assert!(unit.contains("InaccessiblePaths=-%E/blacksmith\n"));
            assert!(unit.contains("InaccessiblePaths=-/run/dbus\n"));
            assert!(unit.contains("ProtectSystem=strict\n"));
            assert!(unit.contains("ProtectHome=yes\n"));
            assert!(unit.contains("ProtectProc=invisible\n"));
        }
    }

    /// Las credenciales solo llegan al clon; quien ejecuta código del repositorio no las tiene.
    #[test]
    fn solo_el_clon_recibe_credenciales() {
        let [(_, service), (_, fetch), (_, deploy)] = render(Scope::System, SYSTEM_BINARY);
        assert!(fetch.contains("LoadCredential=github-token:"));
        assert!(fetch.contains("LoadCredential=deploy-key:"));
        for unit in [&service, &deploy] {
            assert!(!unit.contains("github-token"));
            assert!(!unit.contains("deploy-key"));
        }
        assert!(service.contains("InaccessiblePaths=-/run/credentials\n"));
        // El clon no ve los servicios, y los servicios no ven los clones.
        assert!(fetch.contains("InaccessiblePaths=-%S/blacksmith-apps\n"));
        assert!(service.contains("InaccessiblePaths=-%S/blacksmith-fetch\n"));
        assert!(deploy.contains("BindReadOnlyPaths=-%S/blacksmith-fetch/%i\n"));
    }

    #[test]
    fn instala_solo_si_cambia() {
        let dir = tempfile::tempdir().unwrap();
        let binary = Path::new("/opt/x/blacksmith-agent");
        assert!(install_user_units(dir.path(), binary).unwrap());
        assert!(!install_user_units(dir.path(), binary).unwrap());
        for name in [SERVICE_TEMPLATE, FETCH_TEMPLATE, DEPLOY_TEMPLATE] {
            assert!(dir.path().join(name).exists());
        }
    }
}
