//! Variables de entorno de un servicio (`/api/v1/services/{name}/env`).
//!
//! systemd las carga del archivo `EnvironmentFile=` de la unidad en cada arranque o reinicio: tras
//! un `PUT`, hay que reiniciar el servicio para que las use.

use serde::{Deserialize, Serialize};

/// Una variable. La clave sigue `[A-Za-z_][A-Za-z0-9_]*`; el valor es texto sin caracteres de
/// control (ni saltos de línea).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVariable {
    pub key: String,
    pub value: String,
}

/// Respuesta de `GET` y `PUT /api/v1/services/{name}/env`.
///
/// Las variables del servicio se suman a las globales del servidor ([`GlobalEnvironment`]); si
/// una clave está en ambas, gana la del servicio (systemd carga primero el archivo global).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEnvironment {
    pub name: String,
    pub unit: String,
    /// Ruta del archivo en el servidor.
    pub file: String,
    /// `true` si systemd cargará este archivo al arrancar la unidad (la directiva
    /// `EnvironmentFile=` está presente). `false` si falta el drop-in de despliegue o la unidad
    /// aún no existe: las variables se guardan, pero el servicio no las verá.
    pub loaded_by_unit: bool,
    /// Igual para el archivo global, que además debe cargarse antes que el del servicio.
    pub global_loaded_by_unit: bool,
    pub variables: Vec<EnvVariable>,
}

/// Respuesta de `GET` y `PUT /api/v1/env`: variables comunes a todos los servicios del servidor.
/// Se aplican a cada servicio en su siguiente arranque o reinicio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalEnvironment {
    pub file: String,
    pub variables: Vec<EnvVariable>,
}

/// Cuerpo de `PUT /api/v1/services/{name}/env`: sustituye todas las variables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateEnvironment {
    pub variables: Vec<EnvVariable>,
}
