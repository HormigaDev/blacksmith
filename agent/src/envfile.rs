//! Archivos de variables de entorno: `<dir>/global.env` (comunes a todos los servicios) y
//! `<dir>/services/<nombre>.env` (en producción, bajo `/etc/blacksmith/env`).
//!
//! Las plantillas `blacksmith@.service` y `blacksmith-deploy@.service` los cargan con `EnvironmentFile=`
//! (primero el global, así el del servicio gana). systemd los lee como root antes de arrancar,
//! así que pueden ser `0600` del usuario del agente: ni el propio servicio puede leer el archivo.
//!
//! Seguridad e integridad:
//! - Directorio `0700` y archivos `0600`; un archivo con permisos más abiertos o que no sea un
//!   archivo regular (p. ej. un enlace simbólico) se rechaza al leerlo.
//! - Escritura atómica: archivo temporal `0600` creado en exclusiva en el mismo directorio,
//!   `fsync`, `rename` y `fsync` del directorio. Un corte a mitad deja el archivo anterior intacto.
//! - Todo se valida antes de tocar el disco ([`validate`]): claves `[A-Za-z_][A-Za-z0-9_]*`,
//!   valores sin caracteres de control (ni saltos de línea), sin claves repetidas y con límites de
//!   tamaño.
//! - Los valores se escriben entre comillas dobles escapando `"`, `\`, `` ` `` y `$`, que es lo
//!   que systemd deshace al leerlos: el valor llega al servicio exactamente igual (verificado con
//!   systemd 255: espacios en los extremos, `#`, `;`, `=`, comillas, `$VAR`, UTF-8…).
//! - Los valores son secretos: nunca se registran.

use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

use shared::EnvVariable;

use crate::services::ServiceSlug;

pub const MAX_VARIABLES: usize = 256;
pub const MAX_KEY_BYTES: usize = 128;
pub const MAX_VALUE_BYTES: usize = 32 * 1024;
/// Muy por encima de lo que permiten los límites de arriba con claves y valores típicos.
const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Variables comunes a todos los servicios del servidor. No empieza por `blacksmith-`, así que nunca
/// coincide con el archivo de un servicio.
pub const GLOBAL_FILE: &str = "global.env";
/// Subdirectorio de los archivos de cada servicio: así ningún nombre de servicio choca con el
/// global.
const SERVICES_SUBDIR: &str = "services";
const HEADER: &str = "# Gestionado por Blacksmith: los cambios manuales se sobrescriben.\n";

/// Por qué no se aceptan unas variables. `index` es la posición en la lista recibida.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidEnv {
    #[error("hay más de {MAX_VARIABLES} variables")]
    TooMany,
    #[error("variable {index}: la clave está vacía")]
    EmptyKey { index: usize },
    #[error("variable {index}: la clave supera los {MAX_KEY_BYTES} bytes")]
    KeyTooLong { index: usize },
    #[error(
        "variable {index}: la clave «{key}» solo admite letras, dígitos y `_`, y no puede empezar por un dígito"
    )]
    InvalidKey { index: usize, key: String },
    #[error("variable {index}: la clave «{key}» está repetida")]
    DuplicateKey { index: usize, key: String },
    #[error("variable {index} («{key}»): el valor supera los {MAX_VALUE_BYTES} bytes")]
    ValueTooLong { index: usize, key: String },
    #[error(
        "variable {index} («{key}»): el valor contiene caracteres de control (p. ej. saltos de línea)"
    )]
    ControlCharacter { index: usize, key: String },
}

#[derive(Debug, thiserror::Error)]
pub enum EnvFileError {
    #[error(transparent)]
    Invalid(#[from] InvalidEnv),

    #[error("{path}: {reason}")]
    Insecure { path: PathBuf, reason: String },

    #[error("{path}, línea {line}: {reason}")]
    Malformed {
        path: PathBuf,
        line: usize,
        reason: &'static str,
    },

    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("la tarea de archivo se interrumpió")]
    Task(#[from] tokio::task::JoinError),
}

/// Valida las variables tal como llegan del cliente, antes de escribir nada.
pub fn validate(variables: &[EnvVariable]) -> Result<(), InvalidEnv> {
    if variables.len() > MAX_VARIABLES {
        return Err(InvalidEnv::TooMany);
    }
    let mut seen = HashSet::with_capacity(variables.len());
    for (index, EnvVariable { key, value }) in variables.iter().enumerate() {
        validate_key(key, index)?;
        if !seen.insert(key.as_str()) {
            return Err(InvalidEnv::DuplicateKey {
                index,
                key: key.clone(),
            });
        }
        if value.len() > MAX_VALUE_BYTES {
            return Err(InvalidEnv::ValueTooLong {
                index,
                key: key.clone(),
            });
        }
        if value.chars().any(char::is_control) {
            return Err(InvalidEnv::ControlCharacter {
                index,
                key: key.clone(),
            });
        }
    }
    Ok(())
}

fn validate_key(key: &str, index: usize) -> Result<(), InvalidEnv> {
    let Some(first) = key.bytes().next() else {
        return Err(InvalidEnv::EmptyKey { index });
    };
    if key.len() > MAX_KEY_BYTES {
        return Err(InvalidEnv::KeyTooLong { index });
    }
    let valid = (first.is_ascii_alphabetic() || first == b'_')
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(())
    } else {
        Err(InvalidEnv::InvalidKey {
            index,
            key: key.to_owned(),
        })
    }
}

/// Contenido del archivo. Requiere variables ya validadas.
pub fn serialize(variables: &[EnvVariable]) -> String {
    let mut content = String::from(HEADER);
    for EnvVariable { key, value } in variables {
        content.push_str(key);
        content.push_str("=\"");
        for character in value.chars() {
            if matches!(character, '"' | '\\' | '`' | '$') {
                content.push('\\');
            }
            content.push(character);
        }
        content.push_str("\"\n");
    }
    content
}

/// Lee lo que escribe [`serialize`], y también lo que suele escribirse a mano: comentarios (`#`,
/// `;`), líneas vacías, valores entre comillas simples o dobles y valores sin comillas simples
/// (sin espacios ni caracteres especiales). Cualquier otra cosa es un error con su línea, en lugar
/// de interpretarla de otra forma que systemd.
pub fn parse(content: &str) -> Result<Vec<EnvVariable>, (usize, &'static str)> {
    let mut variables = Vec::new();
    for (number, line) in content.lines().enumerate() {
        let line_number = number + 1;
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let (key, raw) = trimmed.split_once('=').ok_or((line_number, "falta `=`"))?;
        let key = key.trim_end();
        validate_key(key, 0).map_err(|_| (line_number, "clave inválida"))?;
        let value =
            parse_value(raw.trim()).ok_or((line_number, "valor con un formato no admitido"))?;
        variables.push(EnvVariable {
            key: key.to_owned(),
            value,
        });
    }
    Ok(variables)
}

fn parse_value(raw: &str) -> Option<String> {
    if let Some(inner) = raw.strip_prefix('"') {
        let inner = inner.strip_suffix('"')?;
        let mut value = String::with_capacity(inner.len());
        let mut characters = inner.chars();
        while let Some(character) = characters.next() {
            match character {
                '\\' => {
                    let escaped = characters.next()?;
                    // Como systemd: solo estos se desescapan; otra barra se conserva tal cual.
                    if !matches!(escaped, '"' | '\\' | '`' | '$') {
                        value.push('\\');
                    }
                    value.push(escaped);
                }
                // Una comilla sin escapar en medio cerraría el valor antes: no se admite.
                '"' => return None,
                other => value.push(other),
            }
        }
        Some(value)
    } else if let Some(inner) = raw.strip_prefix('\'') {
        let inner = inner.strip_suffix('\'')?;
        (!inner.contains('\'')).then(|| inner.to_owned())
    } else {
        raw.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./:@+,-".contains(&byte))
            .then(|| raw.to_owned())
    }
}

/// Directorio de los archivos de variables.
#[derive(Debug, Clone)]
pub struct EnvStore {
    dir: PathBuf,
}

impl EnvStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `<dir>/services/<nombre>.env` (el `%i` de las plantillas de systemd).
    pub fn path_for(&self, slug: &ServiceSlug) -> PathBuf {
        self.services_dir().join(format!("{slug}.env"))
    }

    fn services_dir(&self) -> PathBuf {
        self.dir.join(SERVICES_SUBDIR)
    }

    /// Archivo de las variables globales del servidor.
    pub fn global_path(&self) -> PathBuf {
        self.dir.join(GLOBAL_FILE)
    }

    pub async fn read_global(&self) -> Result<Vec<EnvVariable>, EnvFileError> {
        let path = self.global_path();
        tokio::task::spawn_blocking(move || read_file(&path)).await?
    }

    /// Valida y sustituye todas las variables globales de forma atómica.
    pub async fn write_global(&self, variables: Vec<EnvVariable>) -> Result<(), EnvFileError> {
        validate(&variables)?;
        let dir = self.dir.clone();
        let path = self.global_path();
        tokio::task::spawn_blocking(move || write_file(&dir, &path, &serialize(&variables))).await?
    }

    /// Variables guardadas; sin archivo, ninguna.
    pub async fn read(&self, slug: &ServiceSlug) -> Result<Vec<EnvVariable>, EnvFileError> {
        let path = self.path_for(slug);
        tokio::task::spawn_blocking(move || read_file(&path)).await?
    }

    /// Valida y sustituye todas las variables de forma atómica.
    pub async fn write(
        &self,
        slug: &ServiceSlug,
        variables: Vec<EnvVariable>,
    ) -> Result<(), EnvFileError> {
        validate(&variables)?;
        let dir = self.services_dir();
        let path = self.path_for(slug);
        tokio::task::spawn_blocking(move || write_file(&dir, &path, &serialize(&variables))).await?
    }

    /// Borra el archivo (al quitar el servicio: los secretos no deben quedarse). Devuelve si
    /// existía.
    pub async fn remove(&self, slug: &ServiceSlug) -> Result<bool, EnvFileError> {
        let path = self.path_for(slug);
        tokio::task::spawn_blocking(move || match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(EnvFileError::Io { path, source }),
        })
        .await?
    }
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> EnvFileError + '_ {
    move |source| EnvFileError::Io {
        path: path.to_owned(),
        source,
    }
}

fn nofollow() -> i32 {
    rustix::fs::OFlags::NOFOLLOW.bits() as i32
}

fn read_file(path: &Path) -> Result<Vec<EnvVariable>, EnvFileError> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(nofollow())
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) if error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) => {
            return Err(EnvFileError::Insecure {
                path: path.to_owned(),
                reason: "es un enlace simbólico".to_owned(),
            });
        }
        Err(source) => return Err(io_error(path)(source)),
    };
    // Metadatos del descriptor abierto, no de la ruta: sin carreras.
    let metadata = file.metadata().map_err(io_error(path))?;
    if !metadata.is_file() {
        return Err(EnvFileError::Insecure {
            path: path.to_owned(),
            reason: "no es un archivo regular".to_owned(),
        });
    }
    let mode = metadata.mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(EnvFileError::Insecure {
            path: path.to_owned(),
            reason: format!("permisos {mode:03o}; deben ser 0600"),
        });
    }
    if metadata.len() > MAX_FILE_BYTES {
        return Err(EnvFileError::Insecure {
            path: path.to_owned(),
            reason: format!("supera {MAX_FILE_BYTES} bytes"),
        });
    }
    let mut content = String::new();
    file.take(MAX_FILE_BYTES)
        .read_to_string(&mut content)
        .map_err(io_error(path))?;
    parse(&content).map_err(|(line, reason)| EnvFileError::Malformed {
        path: path.to_owned(),
        line,
        reason,
    })
}

fn write_file(dir: &Path, path: &Path, content: &str) -> Result<(), EnvFileError> {
    ensure_private_dir(dir)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("env");
    let temporary = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    // Por si quedó uno de un fallo anterior de este mismo proceso.
    let _ = fs::remove_file(&temporary);

    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(nofollow())
            .open(&temporary)
            .map_err(io_error(&temporary))?;
        file.write_all(content.as_bytes())
            .map_err(io_error(&temporary))?;
        file.sync_all().map_err(io_error(&temporary))?;
        // `rename` sustituye la entrada del directorio sin seguir un enlace en el destino.
        fs::rename(&temporary, path).map_err(io_error(path))?;
        fs::File::open(dir)
            .and_then(|dir| dir.sync_all())
            .map_err(io_error(dir))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Crea el directorio (`0700`, con sus padres) si falta y rechaza uno que otros puedan leer o
/// escribir. En producción, `/etc/blacksmith/env` lo crea systemd (`ConfigurationDirectory=`).
fn ensure_private_dir(dir: &Path) -> Result<(), EnvFileError> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(io_error(dir))?;
    let metadata = fs::symlink_metadata(dir).map_err(io_error(dir))?;
    if !metadata.is_dir() {
        return Err(EnvFileError::Insecure {
            path: dir.to_owned(),
            reason: "no es un directorio".to_owned(),
        });
    }
    let mode = metadata.mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(EnvFileError::Insecure {
            path: dir.to_owned(),
            reason: format!("permisos {mode:03o}; deben ser 0700"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn var(key: &str, value: &str) -> EnvVariable {
        EnvVariable {
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }

    fn unit() -> ServiceSlug {
        ServiceSlug::new("api").unwrap()
    }

    /// Los valores que se comprobaron contra systemd 255 (vuelven al servicio idénticos).
    fn tricky() -> Vec<EnvVariable> {
        vec![
            var("SIMPLE", "hola"),
            var("ESPACIOS", "  con espacios  "),
            var("COMILLAS", r#"dice "hola" y 'adios'"#),
            var("BARRA", r"C:\ruta\n\t literal"),
            var("DOLAR", "$HOME ${PATH} $(id) `id`"),
            var("ALMOHADILLA", "a # no es comentario ; tampoco"),
            var("IGUAL", "a=b=c"),
            var("UTF8", "ñandú €"),
            var("VACIA", ""),
        ]
    }

    #[test]
    fn ida_y_vuelta() {
        let variables = tricky();
        validate(&variables).unwrap();
        let content = serialize(&variables);
        assert!(content.starts_with('#'));
        assert!(
            content.contains(r#"DOLAR="\$HOME \${PATH} \$(id) \`id\`""#),
            "{content}"
        );
        assert_eq!(parse(&content).unwrap(), variables);
    }

    #[test]
    fn claves_invalidas() {
        for (variables, expected) in [
            (vec![var("", "x")], InvalidEnv::EmptyKey { index: 0 }),
            (
                vec![var("OK", "x"), var("1A", "x")],
                InvalidEnv::InvalidKey {
                    index: 1,
                    key: "1A".into(),
                },
            ),
            (
                vec![var("A-B", "x")],
                InvalidEnv::InvalidKey {
                    index: 0,
                    key: "A-B".into(),
                },
            ),
            (
                vec![var("A B", "x")],
                InvalidEnv::InvalidKey {
                    index: 0,
                    key: "A B".into(),
                },
            ),
            (
                vec![var("Ñ", "x")],
                InvalidEnv::InvalidKey {
                    index: 0,
                    key: "Ñ".into(),
                },
            ),
            (
                vec![var("A", "1"), var("A", "2")],
                InvalidEnv::DuplicateKey {
                    index: 1,
                    key: "A".into(),
                },
            ),
            (
                vec![var(&"K".repeat(MAX_KEY_BYTES + 1), "x")],
                InvalidEnv::KeyTooLong { index: 0 },
            ),
        ] {
            assert_eq!(validate(&variables), Err(expected));
        }
    }

    #[test]
    fn valores_que_corromperian_el_archivo() {
        for value in ["a\nB=inyectada", "a\rb", "a\0b", "a\u{1b}[31m", "a\tb"] {
            assert!(
                matches!(
                    validate(&[var("A", value)]),
                    Err(InvalidEnv::ControlCharacter { .. })
                ),
                "{value:?}"
            );
        }
        assert!(matches!(
            validate(&[var("A", &"x".repeat(MAX_VALUE_BYTES + 1))]),
            Err(InvalidEnv::ValueTooLong { .. })
        ));
        let many: Vec<_> = (0..=MAX_VARIABLES)
            .map(|i| var(&format!("V{i}"), "x"))
            .collect();
        assert_eq!(validate(&many), Err(InvalidEnv::TooMany));
    }

    #[test]
    fn lee_formatos_escritos_a_mano() {
        let content = "\n# comentario\n; otro\n  A=simple-1.2/x:y@z+w,v\nB='entre simples $NO'\nC = \"dobles \\\" escapada\"\n";
        assert_eq!(
            parse(content).unwrap(),
            [
                var("A", "simple-1.2/x:y@z+w,v"),
                var("B", "entre simples $NO"),
                var("C", "dobles \" escapada"),
            ]
        );
        for (content, line) in [
            ("A=uno\nsin igual\n", 2),
            ("A=con espacio\n", 1),
            ("A=\"sin cerrar\n", 1),
            ("A=\"a\"b\"\n", 1),
            ("1A=x\n", 1),
            ("A=$VAR\n", 1),
        ] {
            assert_eq!(parse(content).unwrap_err().0, line, "{content:?}");
        }
    }

    #[tokio::test]
    async fn escritura_privada_y_atomica() {
        let dir = tempfile::tempdir().unwrap();
        let store = EnvStore::new(dir.path().join("env"));
        assert_eq!(store.read(&unit()).await.unwrap(), []);

        store.write(&unit(), tricky()).await.unwrap();
        let path = store.path_for(&unit());
        assert_eq!(path, dir.path().join("env/services/api.env"));
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(&dir.path().join("env/services")), 0o700);
        assert_eq!(store.read(&unit()).await.unwrap(), tricky());

        // Sustituir no deja temporales.
        store.write(&unit(), vec![var("A", "1")]).await.unwrap();
        let entries: Vec<_> = fs::read_dir(dir.path().join("env/services"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, ["api.env"]);
        assert_eq!(store.read(&unit()).await.unwrap(), [var("A", "1")]);

        assert!(store.remove(&unit()).await.unwrap());
        assert!(!store.remove(&unit()).await.unwrap());
    }

    #[tokio::test]
    async fn alcance_global_separado() {
        let dir = tempfile::tempdir().unwrap();
        let store = EnvStore::new(dir.path().join("env"));
        assert_eq!(store.read_global().await.unwrap(), []);
        store
            .write_global(vec![var("COMUN", "global")])
            .await
            .unwrap();
        store
            .write(&unit(), vec![var("COMUN", "servicio")])
            .await
            .unwrap();
        assert_eq!(store.global_path(), dir.path().join("env/global.env"));
        assert_eq!(store.read_global().await.unwrap(), [var("COMUN", "global")]);
        assert_eq!(
            store.read(&unit()).await.unwrap(),
            [var("COMUN", "servicio")]
        );
        assert!(matches!(
            store.write_global(vec![var("A", "x\ny")]).await,
            Err(EnvFileError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn no_escribe_nada_si_no_valida() {
        let dir = tempfile::tempdir().unwrap();
        let store = EnvStore::new(dir.path().join("env"));
        store.write(&unit(), vec![var("A", "1")]).await.unwrap();
        let error = store
            .write(&unit(), vec![var("B", "2"), var("C", "x\nD=y")])
            .await
            .unwrap_err();
        assert!(matches!(error, EnvFileError::Invalid(_)));
        assert_eq!(store.read(&unit()).await.unwrap(), [var("A", "1")]);
    }

    #[tokio::test]
    async fn rechaza_archivos_inseguros() {
        let dir = tempfile::tempdir().unwrap();
        let store = EnvStore::new(dir.path().join("env"));
        store.write(&unit(), vec![var("A", "1")]).await.unwrap();
        let path = store.path_for(&unit());

        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            store.read(&unit()).await,
            Err(EnvFileError::Insecure { .. })
        ));

        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", &path).unwrap();
        assert!(matches!(
            store.read(&unit()).await,
            Err(EnvFileError::Insecure { .. })
        ));
        fs::remove_file(&path).unwrap();

        fs::write(&path, "A=con espacio\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            store.read(&unit()).await,
            Err(EnvFileError::Malformed { line: 1, .. })
        ));

        // Directorio abierto a otros: no se escribe ahí.
        fs::set_permissions(
            dir.path().join("env/services"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        assert!(matches!(
            store.write(&unit(), vec![var("A", "1")]).await,
            Err(EnvFileError::Insecure { .. })
        ));
    }
}
