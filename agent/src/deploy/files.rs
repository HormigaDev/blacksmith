//! Archivos privados del agente: la especificación de cada despliegue y la credencial de GitHub.
//! Solo los lee el agente (`0600`, en su directorio) y la unidad de despliegue, que los recibe
//! como credenciales de systemd (`LoadCredential=`).

use std::{
    fs,
    io::{self, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
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

/// Escribe un archivo privado (`0600`, directorio `0700`) de forma atómica (temporal + `rename`).
pub fn write_private(path: &Path, content: &[u8]) -> Result<(), FileError> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(io_error(dir))?;
    let temporary = path.with_extension("tmp");
    let _ = fs::remove_file(&temporary);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(io_error(&temporary))?;
    file.write_all(content).map_err(io_error(&temporary))?;
    file.sync_all().map_err(io_error(&temporary))?;
    fs::rename(&temporary, path).map_err(io_error(path))
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
