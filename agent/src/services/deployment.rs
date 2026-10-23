//! Validación de la definición del despliegue de un servicio. Estos valores acaban en comandos,
//! en rutas y en una unidad de systemd, así que se rechaza cualquier cosa que pueda corromperlos
//! (caracteres de control, saltos de línea, nombres de rama que git no acepta, rutas que salgan
//! del repositorio…).

use shared::{RepositoryRef, UpdateDeployment};

pub const MAX_SETUP_COMMANDS: usize = 20;
pub const MAX_COMMAND_CHARS: usize = 2048;
pub const MAX_START_COMMAND_CHARS: usize = 1024;
pub const MAX_PUBLISH_DIR_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidDeployment {
    #[error("propietario del repositorio inválido: 1-39 letras, dígitos o guiones")]
    Owner,
    #[error("nombre del repositorio inválido: 1-100 letras, dígitos, `.`, `-` o `_`")]
    Repository,
    #[error("rama inválida: {0}")]
    Branch(&'static str),
    #[error("hay más de {MAX_SETUP_COMMANDS} comandos de preparación")]
    TooManyCommands,
    #[error("el comando de preparación {index} está vacío")]
    EmptyCommand { index: usize },
    #[error("el comando de preparación {index} supera los {MAX_COMMAND_CHARS} caracteres")]
    CommandTooLong { index: usize },
    #[error(
        "el comando de preparación {index} contiene caracteres de control (p. ej. saltos de línea)"
    )]
    CommandControl { index: usize },
    #[error("el comando de arranque supera los {MAX_START_COMMAND_CHARS} caracteres")]
    StartTooLong,
    #[error("el comando de arranque contiene caracteres de control (p. ej. saltos de línea)")]
    StartControl,
    #[error(
        "carpeta pública inválida: una ruta relativa dentro del repositorio (p. ej. `dist` o \
         `web/build`), con letras, dígitos, `.`, `_`, `-` y `/`, sin `..`"
    )]
    PublishDir,
}

/// Carpeta pública: relativa al repositorio y sin poder salir de él. `.` (la raíz) se acepta.
fn normalize_publish_dir(dir: &str) -> Result<Option<String>, InvalidDeployment> {
    let dir = dir.trim().trim_end_matches('/');
    if dir.is_empty() {
        return Ok(None);
    }
    let valid = dir.chars().count() <= MAX_PUBLISH_DIR_CHARS
        && !dir.starts_with('/')
        && dir
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
        && dir
            .split('/')
            .all(|part| !part.is_empty() && part != ".." && (part != "." || dir == "."));
    valid
        .then(|| Some(dir.to_owned()))
        .ok_or(InvalidDeployment::PublishDir)
}

/// Quita espacios en los extremos de cada campo y lo valida.
pub fn normalize(update: UpdateDeployment) -> Result<UpdateDeployment, InvalidDeployment> {
    let repository = update
        .repository
        .map(|repository| {
            let repository = RepositoryRef {
                owner: repository.owner.trim().to_owned(),
                name: repository.name.trim().to_owned(),
                branch: repository.branch.trim().to_owned(),
            };
            validate_repository(&repository).map(|()| repository)
        })
        .transpose()?;

    if update.setup_commands.len() > MAX_SETUP_COMMANDS {
        return Err(InvalidDeployment::TooManyCommands);
    }
    let mut setup_commands = Vec::with_capacity(update.setup_commands.len());
    for (index, command) in update.setup_commands.iter().enumerate() {
        let command = command.trim();
        if command.is_empty() {
            return Err(InvalidDeployment::EmptyCommand { index });
        }
        if command.chars().count() > MAX_COMMAND_CHARS {
            return Err(InvalidDeployment::CommandTooLong { index });
        }
        if command.chars().any(char::is_control) {
            return Err(InvalidDeployment::CommandControl { index });
        }
        setup_commands.push(command.to_owned());
    }

    let start_command = update.start_command.trim();
    if start_command.chars().count() > MAX_START_COMMAND_CHARS {
        return Err(InvalidDeployment::StartTooLong);
    }
    if start_command.chars().any(char::is_control) {
        return Err(InvalidDeployment::StartControl);
    }

    Ok(UpdateDeployment {
        repository,
        access: update.access,
        setup_commands,
        start_command: start_command.to_owned(),
        publish_dir: update
            .publish_dir
            .as_deref()
            .map(normalize_publish_dir)
            .transpose()?
            .flatten(),
    })
}

fn validate_repository(repository: &RepositoryRef) -> Result<(), InvalidDeployment> {
    let owner = &repository.owner;
    let owner_ok = (1..=39).contains(&owner.len())
        && owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-');
    if !owner_ok {
        return Err(InvalidDeployment::Owner);
    }
    let name = &repository.name;
    let name_ok = (1..=100).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        && name != "."
        && name != "..";
    if !name_ok {
        return Err(InvalidDeployment::Repository);
    }
    validate_branch(&repository.branch)
}

/// Reglas de `git check-ref-format` para una rama, más un límite de longitud.
fn validate_branch(branch: &str) -> Result<(), InvalidDeployment> {
    use InvalidDeployment::Branch;
    if branch.is_empty() {
        return Err(Branch("está vacía"));
    }
    if branch.len() > 255 {
        return Err(Branch("supera los 255 bytes"));
    }
    if branch.chars().any(|c| {
        c.is_control() || c == ' ' || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
    }) {
        return Err(Branch(
            "contiene espacios o caracteres no permitidos (~ ^ : ? * [ \\)",
        ));
    }
    if branch.starts_with('-')
        || branch.starts_with('/')
        || branch.ends_with('/')
        || branch.ends_with('.')
    {
        return Err(Branch(
            "no puede empezar por `-` o `/` ni terminar en `/` o `.`",
        ));
    }
    if branch.contains("..") || branch.contains("//") || branch.contains("@{") || branch == "@" {
        return Err(Branch("no puede contener `..`, `//` ni `@{`"));
    }
    if branch
        .split('/')
        .any(|part| part.starts_with('.') || part.ends_with(".lock"))
    {
        return Err(Branch(
            "ningún tramo puede empezar por `.` ni terminar en `.lock`",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(
        repository: Option<(&str, &str, &str)>,
        setup: &[&str],
        start: &str,
    ) -> UpdateDeployment {
        UpdateDeployment {
            repository: repository.map(|(owner, name, branch)| RepositoryRef {
                owner: owner.into(),
                name: name.into(),
                branch: branch.into(),
            }),
            access: shared::RepositoryAccess::Account,
            setup_commands: setup.iter().map(|command| (*command).to_owned()).collect(),
            start_command: start.into(),
            publish_dir: None,
        }
    }

    #[test]
    fn carpeta_publica() {
        let with = |dir: &str| {
            normalize(UpdateDeployment {
                publish_dir: Some(dir.into()),
                ..update(None, &[], "")
            })
            .map(|update| update.publish_dir)
        };
        assert_eq!(with(" dist/ "), Ok(Some("dist".into())));
        assert_eq!(with("web/build"), Ok(Some("web/build".into())));
        assert_eq!(with("."), Ok(Some(".".into())));
        assert_eq!(with("  "), Ok(None));
        for bad in [
            "/etc",
            "../x",
            "a/../../b",
            "a//b",
            "a b",
            "a/./b",
            "x;rm",
            "a\nb",
        ] {
            assert_eq!(with(bad), Err(InvalidDeployment::PublishDir), "{bad:?}");
        }
    }

    #[test]
    fn normaliza_y_acepta() {
        let normalized = normalize(update(
            Some((" HormigaDev ", "bbel-blacksmith", " feature/v2.1 ")),
            &[
                "  npm ci  ",
                "npm run build -- --mode=production && echo 'listo'",
            ],
            "  ./bin/api --port 8080 ",
        ))
        .unwrap();
        let repository = normalized.repository.unwrap();
        assert_eq!(
            (repository.owner.as_str(), repository.branch.as_str()),
            ("HormigaDev", "feature/v2.1")
        );
        assert_eq!(normalized.setup_commands[0], "npm ci");
        assert_eq!(normalized.start_command, "./bin/api --port 8080");
        // Todo vacío es válido: aún no se definió.
        assert!(normalize(update(None, &[], "")).is_ok());
    }

    #[test]
    fn repositorios_invalidos() {
        for (owner, name, branch, expected) in [
            ("", "r", "main", InvalidDeployment::Owner),
            ("-a", "r", "main", InvalidDeployment::Owner),
            ("a b", "r", "main", InvalidDeployment::Owner),
            (&*"a".repeat(40), "r", "main", InvalidDeployment::Owner),
            ("a", "", "main", InvalidDeployment::Repository),
            ("a", "..", "main", InvalidDeployment::Repository),
            ("a", "r/x", "main", InvalidDeployment::Repository),
        ] {
            assert_eq!(
                normalize(update(Some((owner, name, branch)), &[], "")),
                Err(expected),
                "{owner}/{name}"
            );
        }
    }

    #[test]
    fn ramas_que_git_no_acepta() {
        for branch in [
            "",
            "-main",
            "/main",
            "main/",
            "main.",
            "a..b",
            "a//b",
            "a@{1}",
            "@",
            "a b",
            "a~1",
            "a^",
            "a:b",
            "a?",
            "a*",
            "a[",
            "a\\b",
            ".oculta",
            "x/.oculta",
            "rama.lock",
            "a\nb",
        ] {
            assert!(
                matches!(
                    normalize(update(Some(("o", "r", branch)), &[], "")),
                    Err(InvalidDeployment::Branch(_))
                ),
                "{branch:?}"
            );
        }
        for branch in ["main", "release/2026.10", "feature/añadir-ñ", "v1.0.0"] {
            assert!(
                normalize(update(Some(("o", "r", branch)), &[], "")).is_ok(),
                "{branch}"
            );
        }
    }

    #[test]
    fn comandos() {
        assert_eq!(
            normalize(update(None, &["npm ci", "   "], "")),
            Err(InvalidDeployment::EmptyCommand { index: 1 })
        );
        assert_eq!(
            normalize(update(None, &["npm ci\nrm -rf /"], "")),
            Err(InvalidDeployment::CommandControl { index: 0 })
        );
        assert_eq!(
            normalize(update(None, &[&"x".repeat(MAX_COMMAND_CHARS + 1)], "")),
            Err(InvalidDeployment::CommandTooLong { index: 0 })
        );
        let many: Vec<&str> = std::iter::repeat_n("true", MAX_SETUP_COMMANDS + 1).collect();
        assert_eq!(
            normalize(update(None, &many, "")),
            Err(InvalidDeployment::TooManyCommands)
        );
        assert_eq!(
            normalize(update(None, &[], "./api\rrm -rf /")),
            Err(InvalidDeployment::StartControl)
        );
        assert_eq!(
            normalize(update(None, &[], &"x".repeat(MAX_START_COMMAND_CHARS + 1))),
            Err(InvalidDeployment::StartTooLong)
        );
    }
}
