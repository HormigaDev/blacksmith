//! Nombres de unidad `systemd`, validados antes de llegar a `systemctl`.

use std::{fmt, str::FromStr};

/// Longitud máxima de un nombre de unidad en systemd (`UNIT_NAME_MAX`), sufijo incluido.
const MAX_LEN: usize = 255;
const SUFFIX: &str = ".service";

/// Sufijos de otros tipos de unidad: se rechazan en lugar de convertirlos en `.service`, para que
/// `foo.socket` no acabe siendo `foo.socket.service` sin que nadie lo note.
const OTHER_SUFFIXES: [&str; 10] = [
    ".socket",
    ".target",
    ".timer",
    ".mount",
    ".automount",
    ".path",
    ".slice",
    ".scope",
    ".device",
    ".swap",
];

/// Nombre de una unidad `.service`, validado.
///
/// Más estricto que systemd a propósito:
/// - Solo `[A-Za-z0-9:_.-]` y como mucho una `@` (plantillas con instancia: `app@1.service`).
///   Sin `\` (escapes de systemd), ni espacios ni comodines: `systemctl stop 'app*'` actuaría
///   sobre todas las unidades que encajen.
/// - No empieza por `-` ni `.`: nunca puede confundirse con una opción (además se pasa tras `--`).
/// - Sin sufijo se asume `.service`; cualquier otro tipo de unidad se rechaza.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnitName(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidUnitName {
    #[error("el nombre de unidad está vacío")]
    Empty,
    #[error("el nombre de unidad supera los {MAX_LEN} caracteres")]
    TooLong,
    #[error("el nombre de unidad contiene caracteres no permitidos")]
    InvalidCharacter,
    #[error("el nombre de unidad no puede empezar por `-` ni `.`")]
    InvalidStart,
    #[error("solo se gestionan unidades `.service`")]
    NotAService,
    #[error("plantilla mal formada: se espera `nombre@instancia.service`")]
    InvalidTemplate,
}

impl UnitName {
    pub fn new(name: &str) -> Result<Self, InvalidUnitName> {
        let stem = match name.strip_suffix(SUFFIX) {
            Some(stem) => stem,
            None if OTHER_SUFFIXES.iter().any(|suffix| name.ends_with(suffix)) => {
                return Err(InvalidUnitName::NotAService);
            }
            None => name,
        };

        if stem.is_empty() {
            return Err(InvalidUnitName::Empty);
        }
        if stem.len() + SUFFIX.len() > MAX_LEN {
            return Err(InvalidUnitName::TooLong);
        }
        if !stem.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'.' | b'-' | b'@')
        }) {
            return Err(InvalidUnitName::InvalidCharacter);
        }
        if stem.starts_with(['-', '.']) {
            return Err(InvalidUnitName::InvalidStart);
        }
        if let Some((prefix, instance)) = stem.split_once('@') {
            // `app@.service` es la plantilla en sí: no se puede arrancar ni parar.
            if prefix.is_empty() || instance.is_empty() || instance.contains('@') {
                return Err(InvalidUnitName::InvalidTemplate);
            }
        }

        let mut unit = String::with_capacity(stem.len() + SUFFIX.len());
        unit.push_str(stem);
        unit.push_str(SUFFIX);
        Ok(Self(unit))
    }

    /// Para nombres que ya cumplen una regla más estricta que [`UnitName::new`] (p. ej. los de
    /// `ServiceSlug`). Sus tests comprueban que `new` los aceptaría igual.
    pub(crate) fn from_validated(name: String) -> Self {
        debug_assert_eq!(
            Self::new(&name).as_ref().map(Self::as_str),
            Ok(name.as_str())
        );
        Self(name)
    }

    /// Nombre completo, siempre con el sufijo `.service`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for UnitName {
    type Err = InvalidUnitName;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::new(name)
    }
}

impl AsRef<str> for UnitName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UnitName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acepta_y_normaliza_el_sufijo() {
        assert_eq!(UnitName::new("api").unwrap().as_str(), "api.service");
        assert_eq!(
            UnitName::new("api.service").unwrap().as_str(),
            "api.service"
        );
        assert_eq!(
            UnitName::new("mi-app_v2.web").unwrap().as_str(),
            "mi-app_v2.web.service"
        );
        assert_eq!(
            UnitName::new("worker@3.service").unwrap().as_str(),
            "worker@3.service"
        );
        assert_eq!(UnitName::new("a:b").unwrap().as_str(), "a:b.service");
    }

    #[test]
    fn rechaza_lo_que_podria_inyectar_opciones_o_comodines() {
        use InvalidUnitName::*;
        for (name, error) in [
            ("", Empty),
            (".service", Empty),
            ("--help", InvalidStart),
            ("-api", InvalidStart),
            (".oculto", InvalidStart),
            ("api*", InvalidCharacter),
            ("api?", InvalidCharacter),
            ("[ab]pi", InvalidCharacter),
            ("api; rm -rf /", InvalidCharacter),
            ("api\nstop", InvalidCharacter),
            ("api service", InvalidCharacter),
            ("dev-sda\\x2d1", InvalidCharacter),
            ("../../etc/passwd", InvalidCharacter),
            ("ápi", InvalidCharacter),
        ] {
            assert_eq!(UnitName::new(name), Err(error), "{name:?}");
        }
    }

    #[test]
    fn solo_servicios() {
        for name in [
            "ssh.socket",
            "multi-user.target",
            "backup.timer",
            "home.mount",
        ] {
            assert_eq!(
                UnitName::new(name),
                Err(InvalidUnitName::NotAService),
                "{name}"
            );
        }
    }

    #[test]
    fn plantillas_con_instancia() {
        for name in ["worker@.service", "@1", "a@b@c"] {
            assert_eq!(
                UnitName::new(name),
                Err(InvalidUnitName::InvalidTemplate),
                "{name}"
            );
        }
    }

    #[test]
    fn longitud_maxima_con_sufijo() {
        let max = "a".repeat(MAX_LEN - SUFFIX.len());
        assert!(UnitName::new(&max).is_ok());
        assert_eq!(
            UnitName::new(&format!("{max}a")),
            Err(InvalidUnitName::TooLong)
        );
    }
}
