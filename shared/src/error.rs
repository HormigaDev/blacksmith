use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// Código estable de error. El cliente decide qué hacer a partir de `code`, nunca de `message`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    BadRequest,
    InvalidJson,
    UnsupportedMediaType,
    PayloadTooLarge,
    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    Conflict,
    RequestTimeout,
    Overloaded,
    Unavailable,
    Internal,

    // --- Servicios gestionados ---
    /// El servicio no está registrado en el agente (aunque exista una unidad con ese nombre).
    ServiceNotFound,
    /// El servicio está registrado, pero su unidad no existe en systemd.
    UnitNotFound,
    UnitMasked,
    /// La unidad tiene `RefuseManualStart=`/`RefuseManualStop=`.
    UnitRefusesManualOperation,
    /// systemd ejecutó la acción y el servicio falló (p. ej. su proceso terminó con error).
    ServiceJobFailed,
    /// polkit no autoriza al agente: falta o está mal la regla de `deploy/polkit/`.
    SystemdPermissionDenied,
    /// `systemctl` no respondió a tiempo. En una acción, el trabajo puede seguir en curso.
    SystemdTimeout,
    /// Cualquier otro fallo al hablar con systemd o al medir el proceso.
    SystemdFailed,
    /// Se alcanzó el máximo de flujos de logs simultáneos (`BLACKSMITH_MAX_LOG_STREAMS`).
    LogStreamLimit,
    /// La ruta solo admite WebSocket y la petición no es un handshake válido.
    WebSocketRequired,
    /// Ya hay un servicio registrado con ese nombre.
    ServiceAlreadyExists,
    /// Alguna variable de entorno no es válida (el mensaje dice cuál y por qué).
    InvalidEnvironment,
    /// El archivo de variables del servidor no se puede usar (permisos, formato…).
    EnvironmentFileInvalid,
    /// La definición del despliegue no es válida (el mensaje dice qué campo y por qué).
    InvalidDeployment,
    /// El repositorio de un servicio no se puede cambiar una vez elegido (la rama sí).
    RepositoryLocked,
    /// Falta algo para desplegar (repositorio o comando de arranque).
    DeploymentIncomplete,
    /// El servicio usa llave de despliegue y aún no la tiene.
    DeployKeyMissing,
    /// Ya hay un despliegue en curso para ese servicio.
    DeployInProgress,
    /// El servicio no tiene versión activa: hay que desplegarlo antes de iniciarlo.
    NotDeployed,
    /// No hay versión anterior a la que volver (rollback).
    NothingToRollBack,
    /// El servicio no tiene comando de arranque (sitio solo estático): no hay nada que iniciar.
    NothingToRun,
    /// La configuración del sitio no es válida (el mensaje dice qué campo y por qué).
    InvalidSite,
    /// Otro servicio ya usa alguno de esos dominios.
    DomainInUse,
    /// nginx rechazó la configuración (`nginx -t`): se mantuvo la anterior. El mensaje trae su
    /// salida.
    NginxRejected,
    /// No se pudo aplicar en nginx: integración sin instalar (deploy/nginx/install.sh), sin
    /// permiso de polkit o nginx no disponible.
    NginxUnavailable,

    /// Código emitido por un agente más reciente que este cliente. El agente nunca lo envía.
    #[serde(other)]
    Unknown,
}

/// Cuerpo de toda respuesta de error del agente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: ErrorCode,
    /// Texto para registro o diagnóstico; no está pensado para mostrarse tal cual.
    pub message: Cow<'static, str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializa_codigos_estables() {
        let body = ApiErrorBody {
            code: ErrorCode::NotFound,
            message: "x".into(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"code":"NOT_FOUND","message":"x"}"#
        );
    }

    /// Todos los códigos que puede enviar el agente. El `match` exhaustivo obliga a actualizar
    /// esta lista (y con ella el cliente) al añadir una variante.
    fn sent_by_agent() -> Vec<ErrorCode> {
        use ErrorCode::*;
        let all = [
            BadRequest,
            InvalidJson,
            UnsupportedMediaType,
            PayloadTooLarge,
            Unauthorized,
            Forbidden,
            NotFound,
            MethodNotAllowed,
            Conflict,
            RequestTimeout,
            Overloaded,
            Unavailable,
            Internal,
            ServiceNotFound,
            UnitNotFound,
            UnitMasked,
            UnitRefusesManualOperation,
            ServiceJobFailed,
            SystemdPermissionDenied,
            SystemdTimeout,
            SystemdFailed,
            LogStreamLimit,
            WebSocketRequired,
            ServiceAlreadyExists,
            InvalidEnvironment,
            EnvironmentFileInvalid,
            InvalidDeployment,
            RepositoryLocked,
            DeploymentIncomplete,
            DeployKeyMissing,
            DeployInProgress,
            NotDeployed,
            NothingToRollBack,
            NothingToRun,
            InvalidSite,
            DomainInUse,
            NginxRejected,
            NginxUnavailable,
        ];
        for code in all {
            match code {
                BadRequest
                | InvalidJson
                | UnsupportedMediaType
                | PayloadTooLarge
                | Unauthorized
                | Forbidden
                | NotFound
                | MethodNotAllowed
                | Conflict
                | RequestTimeout
                | Overloaded
                | Unavailable
                | Internal
                | ServiceNotFound
                | UnitNotFound
                | UnitMasked
                | UnitRefusesManualOperation
                | ServiceJobFailed
                | SystemdPermissionDenied
                | SystemdTimeout
                | SystemdFailed
                | LogStreamLimit
                | WebSocketRequired
                | ServiceAlreadyExists
                | InvalidEnvironment
                | EnvironmentFileInvalid
                | InvalidDeployment
                | RepositoryLocked
                | DeploymentIncomplete
                | DeployKeyMissing
                | DeployInProgress
                | NotDeployed
                | NothingToRollBack
                | NothingToRun
                | InvalidSite
                | DomainInUse
                | NginxRejected
                | NginxUnavailable
                | Unknown => {}
            }
        }
        all.to_vec()
    }

    /// Regla de CLAUDE.md: cada código está en `API_ERROR_CODES` y tiene su `errors.agent.<CODE>`.
    #[test]
    fn el_cliente_conoce_todos_los_codigos() {
        let client = include_str!("../../client/src/lib/http/client.ts");
        let messages = include_str!("../../client/src/lib/i18n/messages/es.ts");
        for code in sent_by_agent() {
            let name = serde_json::to_value(code).unwrap();
            let name = name.as_str().unwrap();
            assert!(
                client.contains(&format!("'{name}',")),
                "falta {name} en API_ERROR_CODES"
            );
            assert!(
                messages.contains(&format!("            {name}:")),
                "falta errors.agent.{name} en es.ts"
            );
        }
    }

    #[test]
    fn tolera_codigos_desconocidos() {
        let body: ApiErrorBody =
            serde_json::from_str(r#"{"code":"FUTURE_CODE","message":"x"}"#).unwrap();
        assert_eq!(body.code, ErrorCode::Unknown);
    }
}
