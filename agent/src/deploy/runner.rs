//! Lo que ejecutan las plantillas de systemd (ver `units.rs`), nunca como el agente ni con su
//! configuración:
//! - `run-fetch` (`blacksmith-fetch@`): clona la rama en `%S/blacksmith-fetch/<nombre>/source` con
//!   las credenciales. No ejecuta nada del repositorio.
//! - `run-deploy` (`blacksmith-deploy@`): copia ese clon a una versión nueva, ejecuta la
//!   preparación (sin credenciales), publica y activa. O, en un rollback, solo reactiva.
//! - `run-service` (`blacksmith@`): ejecuta el comando de arranque de la versión activa.
//!
//! Estructura del directorio del servicio (`$STATE_DIRECTORY`, `%S/blacksmith-apps/<nombre>`):
//!
//! ```text
//! releases/<id>/        una versión: clon de la rama + resultado de la preparación
//! releases/<id>.json    metadatos de esa versión (rama, commit, comando de arranque)
//! app -> releases/<id>  la versión activa (enlace que se sustituye de forma atómica)
//! public/<id>/          carpeta pública de esa versión, copiada para nginx (si la hay)
//! public/current        la publicada (cambia junto con `app`)
//! ```
//!
//! Permisos: el directorio del servicio es `0711` y las unidades usan `UMask=0077`, así que todo
//! lo que crea el servicio es privado. Solo `public/` es legible (`0755`/`0644`): nginx (otro
//! usuario) sirve esos archivos y no puede leer nada más del servicio.
//!
//! Acceso al repositorio:
//! - Con cuenta (HTTPS): si hay token de GitHub (credencial `github-token`), git lo obtiene con un
//!   *credential helper* que lee ese archivo: nunca pasa por argumentos, entorno ni `.git/config`.
//! - Con llave de despliegue (SSH): la llave llega como credencial `deploy-key` y las llaves de
//!   host de github.com van fijadas en la especificación.
//!
//! Un despliegue fallido no toca la versión activa: el servicio sigue con la anterior. Se
//! conservan las [`KEEP_RELEASES`] últimas versiones, y siempre la que estaba activa antes de la
//! nueva: a esa se vuelve con un rollback (tarea `activate`), sin clonar ni compilar nada.

use std::{
    ffi::OsString,
    fs,
    io::{self, Write},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
};

use serde::{Deserialize, Serialize};

use super::{
    spec::{BuildSpec, DeploySpec, DeployTask, is_release_id},
    units::SOURCE_ENV,
};

pub const KEEP_RELEASES: usize = 3;

/// Línea de progreso en stdout: con systemd va al journal de la unidad de despliegue, que el
/// cliente sigue en vivo. (No `tracing`: el runner no lleva la configuración del agente.)
macro_rules! step {
    ($($arg:tt)*) => {{
        let mut out = io::stdout().lock();
        let _ = writeln!(out, $($arg)*);
        let _ = out.flush();
    }};
}
const GIT: &str = "/usr/bin/git";
const SH: &str = "/bin/sh";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseInfo {
    branch: String,
    commit: String,
    start_command: String,
    created_at: i64,
}

/// `run-fetch`. Mensajes en stdout: van al journal de la unidad, que el cliente sigue en vivo.
pub fn run_fetch() -> ExitCode {
    report(
        fetch(
            &environment_dir("STATE_DIRECTORY"),
            &environment_dir("CREDENTIALS_DIRECTORY"),
        )
        .map(|commit| format!("==> Commit {commit} descargado")),
    )
}

/// `run-deploy`.
pub fn run_deploy() -> ExitCode {
    let source =
        std::env::var_os(SOURCE_ENV).map_or_else(|| PathBuf::from("source"), PathBuf::from);
    report(
        deploy(
            &environment_dir("STATE_DIRECTORY"),
            &environment_dir("CREDENTIALS_DIRECTORY"),
            &source,
        )
        .map(|release| format!("==> Versión {release} activa")),
    )
}

fn report(result: io::Result<String>) -> ExitCode {
    match result {
        Ok(message) => {
            step!("{message}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            step!("==> Error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `run-service`: ejecuta el comando de arranque de la versión activa en su lugar (`exec`), así
/// el proceso principal de la unidad es el del servicio.
pub fn run_service() -> ExitCode {
    let error = match start_command(Path::new(".")) {
        Ok(command) => Command::new(SH)
            .arg("-c")
            .arg(format!("exec {command}"))
            .exec(),
        Err(error) => error,
    };
    eprintln!("blacksmith-agent run-service: {error}");
    ExitCode::FAILURE
}

fn environment_dir(key: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        // Varias rutas separadas por `:` si la unidad declara más de una; se usa la primera.
        .and_then(|value| {
            value
                .to_str()
                .and_then(|v| v.split(':').next())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read_spec(credentials: &Path) -> io::Result<DeploySpec> {
    let spec: DeploySpec = serde_json::from_slice(&fs::read(credentials.join("spec"))?)
        .map_err(|error| io::Error::other(format!("especificación ilegible: {error}")))?;
    if !is_release_id(&spec.release) {
        return Err(io::Error::other("id de versión inválido"));
    }
    Ok(spec)
}

/// Clona la rama de la especificación en `<state>/source`, sustituyendo el clon anterior.
/// Devuelve el commit.
pub fn fetch(state: &Path, credentials: &Path) -> io::Result<String> {
    let DeployTask::Build(spec) = read_spec(credentials)?.task else {
        return Err(io::Error::other("un rollback no descarga nada"));
    };
    if spec.start_command.trim().is_empty() && spec.publish_dir.is_none() {
        return Err(io::Error::other(
            "no hay comando de arranque ni carpeta pública",
        ));
    }
    let source = state.join("source");
    if source.exists() {
        fs::remove_dir_all(&source)?;
    }
    let git = Git::new(credentials, state, spec.known_hosts.as_deref())?;
    step!("==> Comprobando el acceso a {}", spec.clone_url);
    check_access(&spec, &git)?;
    step!("==> Clonando {} ({})", spec.clone_url, spec.branch);
    let cloned = run(git
        .command()
        .args([
            "clone",
            "--quiet",
            "--depth",
            "1",
            "--single-branch",
            "--branch",
        ])
        .arg(&spec.branch)
        .arg("--")
        .arg(&spec.clone_url)
        .arg(&source))
    .and_then(|()| commit(&source));
    if cloned.is_err() {
        let _ = fs::remove_dir_all(&source);
    }
    cloned
}

/// Ejecuta la tarea de la especificación en `state`: construir una versión nueva a partir del
/// clon `source`, o reactivar una anterior. Devuelve el id de la versión activada.
pub fn deploy(state: &Path, credentials: &Path, source: &Path) -> io::Result<String> {
    let spec = read_spec(credentials)?;
    let releases = state.join("releases");
    fs::create_dir_all(&releases)?;
    let id = spec.release;
    let release = releases.join(&id);
    // La que estaba activa: se conserva al podar, para poder volver a ella.
    let previous = active_release(state);

    match spec.task {
        DeployTask::Activate => {
            if !release.is_dir() || !releases.join(format!("{id}.json")).is_file() {
                return Err(io::Error::other(format!(
                    "la versión {id} ya no está en el servidor"
                )));
            }
            step!("==> Volviendo a la versión {id}");
        }
        DeployTask::Build(build_spec) => {
            if release.exists() {
                return Err(io::Error::other(format!("la versión {id} ya existe")));
            }
            let public = state.join("public").join(&id);
            let built = build(&build_spec, source, &release).and_then(|commit| {
                if let Some(dir) = &build_spec.publish_dir {
                    publish(&release, dir, &public)?;
                }
                Ok(commit)
            });
            let commit = match built {
                Ok(commit) => commit,
                Err(error) => {
                    // La versión a medias no se queda: la activa sigue intacta.
                    let _ = fs::remove_dir_all(&release);
                    let _ = fs::remove_dir_all(&public);
                    return Err(error);
                }
            };
            let info = ReleaseInfo {
                branch: build_spec.branch.clone(),
                commit,
                start_command: build_spec.start_command.clone(),
                created_at: crate::unix_now(),
            };
            let info_json = serde_json::to_vec_pretty(&info).map_err(io::Error::other)?;
            fs::write(releases.join(format!("{id}.json")), info_json)?;
        }
    }

    activate(state, &id)?;
    prune(
        &releases,
        &[id.as_str(), previous.as_deref().unwrap_or_default()],
    );
    Ok(id)
}

/// Id de la versión a la que apunta `app`, si hay.
fn active_release(state: &Path) -> Option<String> {
    fs::read_link(state.join("app"))
        .ok()?
        .file_name()?
        .to_str()
        .map(str::to_owned)
}

/// Nombre de la credencial con el token de GitHub (vacía si el servidor no tiene uno).
pub const TOKEN_CREDENTIAL: &str = "github-token";
/// Nombre de la credencial con la llave de despliegue (vacía si el servicio no la usa).
pub const DEPLOY_KEY_CREDENTIAL: &str = "deploy-key";

/// `git` sin configuración ajena (ni la del sistema ni la del usuario) y, según el acceso, con
/// un *credential helper* que lee el token cuando GitHub lo pide o con SSH y la llave.
struct Git {
    token_file: Option<PathBuf>,
    ssh_command: Option<String>,
}

fn non_empty(path: &Path) -> Option<PathBuf> {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.len() > 0)
        .then(|| path.to_owned())
}

impl Git {
    fn new(credentials: &Path, state: &Path, known_hosts: Option<&str>) -> io::Result<Self> {
        let token_file = non_empty(&credentials.join(TOKEN_CREDENTIAL));
        let ssh_command = match (
            non_empty(&credentials.join(DEPLOY_KEY_CREDENTIAL)),
            known_hosts,
        ) {
            (Some(key), Some(hosts)) => {
                let ssh_dir = state.join(".ssh");
                fs::create_dir_all(&ssh_dir)?;
                let hosts_file = ssh_dir.join("known_hosts");
                fs::write(&hosts_file, hosts)?;
                // Comillas simples: GIT_SSH_COMMAND lo interpreta un shell. Las rutas las pone
                // systemd. `-F /dev/null`: sin configuración de SSH ajena.
                Some(format!(
                    "ssh -F /dev/null -i '{}' -o IdentitiesOnly=yes -o BatchMode=yes \
                     -o StrictHostKeyChecking=yes -o UserKnownHostsFile='{}'",
                    key.display(),
                    hosts_file.display()
                ))
            }
            (None, Some(_)) => {
                return Err(io::Error::other(
                    "el servicio usa llave de despliegue, pero no llegó la llave",
                ));
            }
            _ => None,
        };
        Ok(Self {
            token_file,
            ssh_command,
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new(GIT);
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            // Vacío primero: anula cualquier otro helper.
            .args(["-c", "credential.helper="]);
        if let Some(ssh) = &self.ssh_command {
            command.env("GIT_SSH_COMMAND", ssh);
        }
        if let Some(file) = &self.token_file {
            // La ruta va en el entorno del helper, no en el script: así no hay que escaparla.
            command.env("BLACKSMITH_GITHUB_TOKEN_FILE", file).args([
                "-c",
                "credential.helper=!f() { test \"$1\" = get || exit 0; \
                 echo username=x-access-token; \
                 printf 'password=%s\\n' \"$(cat \"$BLACKSMITH_GITHUB_TOKEN_FILE\")\"; }; f",
            ]);
        }
        command
    }
}

/// Copia el clon a `release` y ejecuta allí la preparación. Devuelve el commit.
fn build(spec: &BuildSpec, source: &Path, release: &Path) -> io::Result<String> {
    if !source.join(".git").is_dir() {
        return Err(io::Error::other(
            "no hay clon de la rama: falló la descarga",
        ));
    }
    copy_tree(source, release)?;
    let commit = commit(release)?;
    step!("==> Commit {commit}");
    for (index, command) in spec.setup_commands.iter().enumerate() {
        step!(
            "==> [{}/{}] {command}",
            index + 1,
            spec.setup_commands.len()
        );
        run(Command::new(SH).arg("-c").arg(command).current_dir(release))
            .map_err(|error| io::Error::other(format!("falló «{command}»: {error}")))?;
    }
    Ok(commit)
}

/// Commit de `HEAD` (40 hexadecimales), leído sin ejecutar git: el repositorio es de otro usuario
/// y git se negaría a abrirlo (`safe.directory`).
fn commit(repo: &Path) -> io::Result<String> {
    let invalid = || io::Error::other("el clon no tiene un commit válido");
    let git = repo.join(".git");
    let head = fs::read_to_string(git.join("HEAD"))?;
    let head = head.trim();
    let commit = match head.strip_prefix("ref: ") {
        Some(reference) if !reference.contains("..") => {
            match fs::read_to_string(git.join(reference)) {
                Ok(commit) => commit.trim().to_owned(),
                // Una rama recién clonada puede estar solo en `packed-refs`.
                Err(_) => fs::read_to_string(git.join("packed-refs"))?
                    .lines()
                    .find_map(|line| {
                        line.split_once(' ')
                            .filter(|(_, name)| *name == reference)
                            .map(|(commit, _)| commit.to_owned())
                    })
                    .ok_or_else(invalid)?,
            }
        }
        Some(_) => return Err(invalid()),
        None => head.to_owned(),
    };
    if commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(commit)
    } else {
        Err(invalid())
    }
}

/// Copia un árbol conservando los permisos de los archivos (el bit de ejecución) y los enlaces
/// simbólicos tal cual (dentro de la versión, que es privada del servicio).
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(entry.path())?, &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Comprueba, antes de clonar, que hay acceso y que la rama existe (`git ls-remote`), para
/// explicar los fallos comunes con un motivo claro.
fn check_access(spec: &BuildSpec, git: &Git) -> io::Result<()> {
    let output = git
        .command()
        .args(["ls-remote", "--heads", "--exit-code", "--"])
        .arg(&spec.clone_url)
        .arg(&spec.branch)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines().filter(|line| !line.trim().is_empty()) {
        step!("    {line}");
    }
    Err(access_error(
        spec,
        git.token_file.is_some(),
        output.status.code(),
        &stderr,
    ))
}

/// Traduce el fallo de `git ls-remote` a un motivo accionable.
fn access_error(spec: &BuildSpec, has_token: bool, code: Option<i32>, stderr: &str) -> io::Error {
    // `--exit-code`: 2 = el repositorio responde pero no tiene esa rama.
    if code == Some(2) {
        return io::Error::other(format!(
            "la rama «{}» no existe en el repositorio",
            spec.branch
        ));
    }
    if stderr.contains("Host key verification failed") {
        return io::Error::other(
            "la llave de host de github.com no coincide con la conocida: conexión rechazada",
        );
    }
    let repository = spec.github_repository().unwrap_or(spec.clone_url.as_str());
    if spec.known_hosts.is_some()
        && (stderr.contains("Permission denied (publickey)")
            || stderr.contains("Repository not found")
            || stderr.contains("Could not read from remote repository"))
    {
        return io::Error::other(format!(
            "GitHub rechazó la llave de despliegue: añádela en https://github.com/{repository}/settings/keys \
             (sin permiso de escritura)"
        ));
    }
    let denied = [
        "could not read Username",
        "Authentication failed",
        "Repository not found",
        "returned error: 403",
    ];
    if denied.iter().any(|hint| stderr.contains(hint)) {
        return io::Error::other(if has_token {
            format!(
                "GitHub no da acceso a {repository} con la cuenta conectada: dale acceso a la GitHub App \
                 en ese repositorio, o vuelve a conectar GitHub en el cliente"
            )
        } else {
            format!(
                "{repository} es privado o no existe, y el servidor no tiene acceso a GitHub: conecta \
                 GitHub en el cliente y vuelve a desplegar"
            )
        });
    }
    io::Error::other(format!(
        "no se pudo acceder al repositorio: {}",
        stderr.trim()
    ))
}

/// Ejecuta mostrando su salida (va al journal). Error si no termina bien.
fn run(command: &mut Command) -> io::Result<()> {
    let _ = io::stdout().flush();
    let status = command.stdin(Stdio::null()).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(match status.code() {
            Some(code) => format!("terminó con código {code}"),
            None => "terminó por una señal".to_owned(),
        }))
    }
}

/// Cambia `app` (y `public/current`, si la versión publica archivos) a la versión `id` de forma
/// atómica: enlace temporal + `rename`.
fn activate(state: &Path, id: &str) -> io::Result<()> {
    replace_link(&state.join("app"), &Path::new("releases").join(id))?;
    let public = state.join("public");
    if public.join(id).is_dir() {
        replace_link(&public.join("current"), Path::new(id))?;
    } else {
        // Esta versión no publica nada: nginx deja de servir la anterior.
        let _ = fs::remove_file(public.join("current"));
    }
    Ok(())
}

fn replace_link(link: &Path, target: &Path) -> io::Result<()> {
    let mut temporary = link.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let _ = fs::remove_file(&temporary);
    std::os::unix::fs::symlink(target, &temporary)?;
    fs::rename(&temporary, link)
}

/// Copia `<release>/<dir>` a `target` para que nginx lo sirva: directorios `0755`, archivos
/// `0644`. Solo archivos y directorios reales: un enlace simbólico podría apuntar fuera del
/// repositorio (p. ej. a `/etc` o a la configuración del servicio), así que se omite.
fn publish(release: &Path, dir: &str, target: &Path) -> io::Result<()> {
    let source = release.join(dir);
    let resolved = fs::canonicalize(&source).map_err(|error| {
        io::Error::other(format!(
            "la carpeta pública «{dir}» no existe tras la preparación: {error}"
        ))
    })?;
    if !resolved.starts_with(fs::canonicalize(release)?) || !resolved.is_dir() {
        return Err(io::Error::other(format!(
            "la carpeta pública «{dir}» no es un directorio del repositorio"
        )));
    }
    step!("==> Publicando {dir}");
    let parent = target.parent().unwrap_or(target);
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o755))?;
    let _ = fs::remove_dir_all(target);
    copy_public(&resolved, target)
}

fn copy_public(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    fs::set_permissions(to, fs::Permissions::from_mode(0o755))?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        // El historial de git nunca se publica, aunque la carpeta pública sea la raíz.
        if entry.file_name() == ".git" {
            continue;
        }
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_public(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target)?;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644))?;
        } else {
            step!(
                "    (omitido, no es un archivo: {})",
                entry.path().display()
            );
        }
    }
    Ok(())
}

/// Borra las versiones más antiguas, conservando las [`KEEP_RELEASES`] últimas y las de `keep`
/// (la activa y la anterior).
fn prune(releases: &Path, keep: &[&str]) {
    let Ok(entries) = fs::read_dir(releases) else {
        return;
    };
    let mut ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    ids.sort();
    let excess = ids.len().saturating_sub(KEEP_RELEASES);
    let public = releases.with_file_name("public");
    for id in ids
        .iter()
        .take(excess)
        .filter(|id| !keep.contains(&id.as_str()))
    {
        let _ = fs::remove_dir_all(releases.join(id));
        let _ = fs::remove_file(releases.join(format!("{id}.json")));
        let _ = fs::remove_dir_all(public.join(id));
    }
}

/// Comando de arranque de la versión en `app` (el directorio de trabajo de la unidad).
fn start_command(app: &Path) -> io::Result<String> {
    let release = fs::canonicalize(app)?;
    let id = release
        .file_name()
        .ok_or_else(|| io::Error::other("versión activa sin nombre"))?;
    let mut info_name = OsString::from(id);
    info_name.push(".json");
    let info_path = release
        .parent()
        .ok_or_else(|| io::Error::other("versión activa fuera de releases/"))?
        .join(info_name);
    let info: ReleaseInfo = serde_json::from_slice(&fs::read(&info_path)?)
        .map_err(|error| io::Error::other(format!("{}: {error}", info_path.display())))?;
    Ok(info.start_command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::spec::{GITHUB_CLONE_BASE, new_release_id};

    /// Repositorio git local que hace de GitHub (`file://`).
    fn origin(dir: &Path, files: &[(&str, &str)]) -> String {
        let repo = dir.join("origen");
        fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            let status = Command::new(GIT)
                .arg("-C")
                .arg(&repo)
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .stdout(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?}");
        };
        git(&["init", "--quiet", "--initial-branch=main"]);
        for (name, content) in files {
            fs::write(repo.join(name), content).unwrap();
        }
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "inicial"]);
        format!("file://{}", repo.display())
    }

    fn credentials(dir: &Path, spec: &DeploySpec) -> PathBuf {
        let credentials = dir.join("credenciales");
        fs::create_dir_all(&credentials).unwrap();
        fs::write(credentials.join("spec"), serde_json::to_vec(spec).unwrap()).unwrap();
        // Como en el servidor sin cuenta de GitHub ni llave: credenciales vacías.
        fs::write(credentials.join(TOKEN_CREDENTIAL), "").unwrap();
        fs::write(credentials.join(DEPLOY_KEY_CREDENTIAL), "").unwrap();
        credentials
    }

    fn build_spec(url: String, setup: &[&str], start: &str) -> BuildSpec {
        BuildSpec {
            clone_url: url,
            branch: "main".into(),
            setup_commands: setup.iter().map(|command| (*command).to_owned()).collect(),
            start_command: start.into(),
            known_hosts: None,
            publish_dir: None,
        }
    }

    fn spec(url: String, setup: &[&str], start: &str) -> DeploySpec {
        DeploySpec {
            release: new_release_id(),
            task: DeployTask::Build(build_spec(url, setup, start)),
        }
    }

    fn activate_spec(release: &str) -> DeploySpec {
        DeploySpec {
            release: release.to_owned(),
            task: DeployTask::Activate,
        }
    }

    /// Clon (si la tarea construye) y despliegue, como hacen las dos unidades.
    fn deploy_from(state: &Path, credentials: &Path) -> io::Result<String> {
        let clone = state.with_file_name("clon");
        if matches!(read_spec(credentials)?.task, DeployTask::Build(_)) {
            fetch(&clone, credentials)?;
        }
        deploy(state, credentials, &clone.join("source"))
    }

    fn active(state: &Path) -> String {
        active_release(state).unwrap()
    }

    #[test]
    fn motivos_de_acceso_denegado() {
        let github =
            BuildSpec::github(GITHUB_CLONE_BASE, "acme", "api", "main", vec![], "x".into());
        let anonymous =
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled";
        assert!(
            access_error(&github, false, Some(128), anonymous)
                .to_string()
                .contains(
                    "acme/api es privado o no existe, y el servidor no tiene acceso a GitHub"
                )
        );
        let rejected = "remote: Repository not found.\nfatal: repository 'https://github.com/acme/api.git/' not found";
        assert!(
            access_error(&github, true, Some(128), rejected)
                .to_string()
                .contains("GitHub no da acceso a acme/api con la cuenta conectada")
        );
        assert!(
            access_error(&github, true, Some(2), "")
                .to_string()
                .contains("la rama «main» no existe")
        );
        assert!(
            access_error(
                &github,
                false,
                Some(128),
                "fatal: unable to access: Could not resolve host: github.com"
            )
            .to_string()
            .contains("Could not resolve host")
        );
    }

    /// El helper entrega el token solo cuando git lo pide, y tal cual está en el archivo.
    #[test]
    fn el_helper_entrega_el_token_del_archivo() {
        let dir = tempfile::tempdir().unwrap();
        let token = dir.path().join(TOKEN_CREDENTIAL);
        fs::write(&token, "ghu_secreto123").unwrap();
        let mut command = Git::new(dir.path(), dir.path(), None).unwrap().command();
        command
            .args(["credential", "fill"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"protocol=https\nhost=github.com\n\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("username=x-access-token\n"), "{text}");
        assert!(text.contains("password=ghu_secreto123\n"), "{text}");

        // Sin token (archivo vacío) no hay helper: git no tiene a quién preguntar.
        fs::write(&token, "").unwrap();
        assert!(
            Git::new(dir.path(), dir.path(), None)
                .unwrap()
                .token_file
                .is_none()
        );
    }

    /// Con llave de despliegue: SSH con la llave y las llaves de host fijadas; sin la llave no
    /// se intenta.
    #[test]
    fn ssh_con_la_llave_y_hosts_fijados() {
        let dir = tempfile::tempdir().unwrap();
        let hosts = "github.com ssh-ed25519 AAAA";
        assert!(Git::new(dir.path(), dir.path(), Some(hosts)).is_err());
        fs::write(dir.path().join(DEPLOY_KEY_CREDENTIAL), "-----BEGIN…").unwrap();
        let git = Git::new(dir.path(), dir.path(), Some(hosts)).unwrap();
        let ssh = git.ssh_command.unwrap();
        assert!(ssh.contains("-F /dev/null"), "{ssh}");
        assert!(ssh.contains("StrictHostKeyChecking=yes"), "{ssh}");
        assert!(ssh.contains(&format!(
            "-i '{}'",
            dir.path().join(DEPLOY_KEY_CREDENTIAL).display()
        )));
        assert_eq!(
            fs::read_to_string(dir.path().join(".ssh/known_hosts")).unwrap(),
            hosts
        );
        let spec = BuildSpec::github(
            crate::deploy::spec::GITHUB_SSH_BASE,
            "acme",
            "api",
            "main",
            vec![],
            "x".into(),
        )
        .with_ssh();
        assert!(
            access_error(
                &spec,
                false,
                Some(128),
                "git@github.com: Permission denied (publickey)."
            )
            .to_string()
            .contains("https://github.com/acme/api/settings/keys")
        );
    }

    #[test]
    fn despliega_prepara_y_activa() {
        let dir = tempfile::tempdir().unwrap();
        let url = origin(dir.path(), &[("app.sh", "echo hola")]);
        let state = dir.path().join("estado");
        let spec = spec(url, &["echo construido > build.txt"], "sh app.sh");
        let id = deploy_from(&state, &credentials(dir.path(), &spec)).unwrap();

        let app = state.join("app");
        assert!(fs::symlink_metadata(&app).unwrap().file_type().is_symlink());
        assert_eq!(
            fs::read_to_string(app.join("build.txt")).unwrap(),
            "construido\n"
        );
        assert_eq!(start_command(&app).unwrap(), "sh app.sh");
        let info: ReleaseInfo =
            serde_json::from_slice(&fs::read(state.join(format!("releases/{id}.json"))).unwrap())
                .unwrap();
        assert_eq!(info.commit.len(), 40);
    }

    #[test]
    fn un_fallo_no_toca_la_version_activa() {
        let dir = tempfile::tempdir().unwrap();
        let url = origin(dir.path(), &[("app.sh", "echo hola")]);
        let state = dir.path().join("estado");
        let good = deploy_from(
            &state,
            &credentials(dir.path(), &spec(url.clone(), &[], "sh app.sh")),
        )
        .unwrap();

        let error = deploy_from(
            &state,
            &credentials(dir.path(), &spec(url, &["exit 3"], "sh app.sh")),
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("exit 3") && error.contains("código 3"),
            "{error}"
        );
        assert_eq!(
            fs::read_link(state.join("app")).unwrap(),
            Path::new("releases").join(&good)
        );
        let releases: Vec<_> = fs::read_dir(state.join("releases"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .collect();
        assert_eq!(releases.len(), 1, "la versión fallida se borra");
    }

    #[test]
    fn rama_inexistente_o_sin_arranque() {
        let dir = tempfile::tempdir().unwrap();
        let url = origin(dir.path(), &[("a", "b")]);
        let state = dir.path().join("estado");
        let mut wrong = build_spec(url.clone(), &[], "sh a");
        wrong.branch = "no-existe".into();
        let wrong = DeploySpec {
            release: new_release_id(),
            task: DeployTask::Build(wrong),
        };
        assert!(deploy_from(&state, &credentials(dir.path(), &wrong)).is_err());
        assert!(deploy_from(&state, &credentials(dir.path(), &spec(url, &[], "  "))).is_err());
        assert!(!state.join("app").exists());
    }

    #[test]
    fn conserva_las_ultimas_versiones() {
        let dir = tempfile::tempdir().unwrap();
        let releases = dir.path();
        for id in ["r1", "r2", "r3", "r4", "r5"] {
            fs::create_dir(releases.join(id)).unwrap();
            fs::write(releases.join(format!("{id}.json")), "{}").unwrap();
        }
        prune(releases, &["r5", "r1"]);
        let mut left: Vec<_> = fs::read_dir(releases)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "r1", "r1.json", "r3", "r3.json", "r4", "r4.json", "r5", "r5.json"
            ]
        );
    }

    /// La carpeta pública se copia legible para nginx, sin enlaces que salgan del repositorio, y
    /// cambia con la versión activa (también en un rollback).
    #[test]
    fn publica_la_carpeta_publica() {
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        let url = origin(dir.path(), &[("app.sh", "echo hola")]);
        let state = dir.path().join("estado");
        let with_public = |setup: &[&str], publish: &str| DeploySpec {
            release: new_release_id(),
            task: DeployTask::Build(BuildSpec {
                publish_dir: Some(publish.into()),
                ..build_spec(url.clone(), setup, "")
            }),
        };
        let setup = [
            "mkdir -p dist/css && echo v1 > dist/index.html && echo x > dist/css/a.css",
            "ln -s /etc/hostname dist/fuera",
            "chmod 600 dist/index.html",
        ];
        let first = deploy_from(
            &state,
            &credentials(dir.path(), &with_public(&setup, "dist")),
        )
        .unwrap();
        let current = state.join("public/current");
        assert_eq!(
            fs::read_to_string(current.join("index.html")).unwrap(),
            "v1\n"
        );
        assert_eq!(
            fs::metadata(current.join("index.html")).unwrap().mode() & 0o777,
            0o644
        );
        assert_eq!(
            fs::metadata(current.join("css")).unwrap().mode() & 0o777,
            0o755
        );
        assert!(!current.join("fuera").exists(), "un enlace no se publica");

        let second = deploy_from(
            &state,
            &credentials(
                dir.path(),
                &with_public(&["mkdir dist && echo v2 > dist/index.html"], "dist"),
            ),
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(current.join("index.html")).unwrap(),
            "v2\n"
        );
        deploy_from(&state, &credentials(dir.path(), &activate_spec(&first))).unwrap();
        assert_eq!(
            fs::read_to_string(current.join("index.html")).unwrap(),
            "v1\n"
        );
        assert_ne!(first, second);

        // Una carpeta pública que es un enlace fuera del repositorio, o que no existe, falla.
        for (setup, publish) in [(&["ln -s /etc web"][..], "web"), (&[][..], "dist")] {
            let error = deploy_from(
                &state,
                &credentials(dir.path(), &with_public(setup, publish)),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("carpeta pública"), "{error}");
        }
        assert_eq!(
            fs::read_to_string(current.join("index.html")).unwrap(),
            "v1\n"
        );
    }

    /// Rollback: vuelve a la versión anterior sin clonar ni ejecutar nada, y se puede volver a
    /// la nueva. Desplegar varias veces nunca borra la versión a la que se volvería.
    #[test]
    fn vuelve_a_la_version_anterior() {
        let dir = tempfile::tempdir().unwrap();
        let url = origin(dir.path(), &[("app.sh", "echo hola")]);
        let state = dir.path().join("estado");
        let first = deploy_from(
            &state,
            &credentials(dir.path(), &spec(url.clone(), &[], "sh app.sh")),
        )
        .unwrap();
        let second = deploy_from(
            &state,
            &credentials(dir.path(), &spec(url.clone(), &[], "sh app.sh v2")),
        )
        .unwrap();
        assert_eq!(active(&state), second);

        deploy_from(&state, &credentials(dir.path(), &activate_spec(&first))).unwrap();
        assert_eq!(active(&state), first);
        assert_eq!(start_command(&state.join("app")).unwrap(), "sh app.sh");
        deploy_from(&state, &credentials(dir.path(), &activate_spec(&second))).unwrap();
        assert_eq!(active(&state), second);

        // Con la anterior activa, varios despliegues nuevos no la borran.
        deploy_from(&state, &credentials(dir.path(), &activate_spec(&first))).unwrap();
        for _ in 0..KEEP_RELEASES + 1 {
            deploy_from(
                &state,
                &credentials(dir.path(), &spec(url.clone(), &[], "sh app.sh")),
            )
            .unwrap();
        }
        let latest = active(&state);
        let previous_kept = deploy_from(&state, &credentials(dir.path(), &activate_spec(&latest)));
        assert!(previous_kept.is_ok());

        // Una versión que ya no existe, o un id que no es de versión, no se activa.
        assert!(
            deploy_from(
                &state,
                &credentials(dir.path(), &activate_spec("r0000000000-000000000"))
            )
            .unwrap_err()
            .to_string()
            .contains("ya no está en el servidor")
        );
        assert!(deploy_from(&state, &credentials(dir.path(), &activate_spec("../x"))).is_err());
        assert_eq!(active(&state), latest);
    }
}
