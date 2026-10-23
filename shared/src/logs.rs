//! Logs en tiempo real de un servicio (`GET /api/v1/services/{name}/logs/ws`).
//!
//! Cada mensaje de texto del WebSocket es un [`LogEvent`] en JSON, etiquetado por `type`.

use serde::{Deserialize, Serialize};

/// Líneas anteriores que se envían al conectar, antes de seguir en tiempo real.
pub const LOG_BACKLOG_LINES: u16 = 100;

/// Una entrada del journal de la unidad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// Microsegundos desde la época Unix (`__REALTIME_TIMESTAMP`).
    pub timestamp_us: u64,
    /// Prioridad syslog: 0 (emerg) … 7 (debug). `None` si la entrada no la trae.
    pub priority: Option<u8>,
    pub pid: Option<u32>,
    /// `SYSLOG_IDENTIFIER`: normalmente el nombre del ejecutable.
    pub identifier: Option<String>,
    /// Texto del mensaje. Los bytes que no son UTF-8 se sustituyen por `�`; los códigos de
    /// color ANSI se conservan tal cual.
    pub message: String,
}

/// Por qué termina el flujo. Tras un `end`, el agente cierra el WebSocket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogEndReason {
    /// `journalctl` terminó o dejó de poder leerse (detalle en `detail`).
    JournalClosed,
    /// El agente se está deteniendo.
    AgentShutdown,
    /// El cliente no leía lo bastante rápido y se cortó para no acumular memoria.
    ClientTooSlow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LogEvent {
    Entry(LogEntry),
    End {
        reason: LogEndReason,
        /// Diagnóstico (p. ej. el error de `journalctl`); no está pensado para mostrarse tal cual.
        detail: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_de_los_eventos() {
        let entry = LogEvent::Entry(LogEntry {
            timestamp_us: 1_792_676_641_122_411,
            priority: Some(6),
            pid: Some(1058),
            identifier: Some("api".into()),
            message: "listo".into(),
        });
        assert_eq!(
            serde_json::to_string(&entry).unwrap(),
            r#"{"type":"entry","timestampUs":1792676641122411,"priority":6,"pid":1058,"identifier":"api","message":"listo"}"#
        );
        let end = LogEvent::End {
            reason: LogEndReason::AgentShutdown,
            detail: None,
        };
        assert_eq!(
            serde_json::to_string(&end).unwrap(),
            r#"{"type":"end","reason":"agentShutdown","detail":null}"#
        );
        let back: LogEvent = serde_json::from_str(&serde_json::to_string(&entry).unwrap()).unwrap();
        assert_eq!(back, entry);
    }
}
