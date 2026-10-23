//! Archivos privados del agente: la especificación de cada despliegue, sus credenciales (token de
//! GitHub y llave de despliegue) y la cuenta de GitHub. Solo los lee el agente (`0600`, en su
//! directorio) y las unidades del despliegue, que los reciben como credenciales de systemd
//! (`LoadCredential=`).

use std::{
    ffi::OsString,
    fs,
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug, thiserror::Error)]
#[error("{path}: {source}")]
pub struct FileError {
    pub path: PathBuf,
    #[source]
    pub source: io::Error,
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> FileError + '_ {
    move |source| FileError {
        path: path.to_owned(),
        source,
    }
}

/// Ruta temporal junto a `path`, propia de cada llamada: dos escrituras a la vez del mismo archivo
/// no comparten temporal (una borraría o renombraría el de la otra).
pub fn temporary_path(path: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut name = OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    path.with_file_name(name)
}

/// Crea el directorio (y los que falten) solo accesible para el agente.
pub fn private_dir(dir: &Path) -> Result<(), FileError> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(io_error(dir))
}

/// Escribe un archivo privado (`0600`, directorio `0700`) de forma atómica (temporal + `rename`).
pub fn write_private(path: &Path, content: &[u8]) -> Result<(), FileError> {
    private_dir(path.parent().unwrap_or(Path::new(".")))?;
    let temporary = temporary_path(path);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(io_error(&temporary))?;
    let written = file
        .write_all(content)
        .and_then(|()| file.sync_all())
        .map_err(io_error(&temporary))
        .and_then(|()| fs::rename(&temporary, path).map_err(io_error(path)));
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}

/// Lee un archivo privado; `None` si no existe.
pub fn read_private(path: &Path) -> Result<Option<Vec<u8>>, FileError> {
    match fs::read(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(FileError {
            path: path.to_owned(),
            source,
        }),
    }
}

/// Borra un archivo o un directorio entero; no existir no es un error.
pub fn remove(path: &Path) -> Result<(), FileError> {
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(FileError {
            path: path.to_owned(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use super::*;

    #[test]
    fn escritura_privada_y_borrado() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x/spec.json");
        write_private(&path, b"{}").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(path.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        write_private(&path, b"[]").unwrap();
        assert_eq!(read_private(&path).unwrap().unwrap(), b"[]");
        remove(&path).unwrap();
        assert_eq!(read_private(&path).unwrap(), None);
        remove(&dir.path().join("x")).unwrap();
        remove(&dir.path().join("x")).unwrap();
        assert!(!dir.path().join("x").exists());
    }
}
