//! `POST /api/webhooks/github`: despliegues automáticos al hacer push.
//!
//! Es la única ruta que nginx expone a internet (`deploy/nginx/webhooks.conf`), así que no se
//! fía de nada que no venga firmado:
//! - La firma `X-Hub-Signature-256` (HMAC-SHA256 del cuerpo con el secreto del webhook,
//!   `BLACKSMITH_GITHUB_WEBHOOK_SECRET`) se comprueba en tiempo constante antes de leer el JSON.
//! - Sin secreto configurado, la ruta no existe (404).
//! - Un `push` despliega los servicios cuyo repositorio y rama coinciden con los del evento; el
//!   despliegue es el de siempre (clon con las credenciales del servidor, preparación en sandbox).

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use hmac::{Hmac, KeyInit, Mac};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{
    error::{AppError, AppResult},
    http::Json,
    state::AppState,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookReport {
    /// Servicios cuyo despliegue se lanzó.
    pub deployed: Vec<String>,
}

#[derive(Deserialize)]
struct PushEvent {
    #[serde(rename = "ref")]
    reference: String,
    #[serde(default)]
    deleted: bool,
    repository: PushRepository,
}

#[derive(Deserialize)]
struct PushRepository {
    full_name: String,
}

pub async fn github(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<(StatusCode, Json<WebhookReport>)> {
    let secret = state
        .config
        .github_webhook_secret
        .as_ref()
        .ok_or(AppError::NotFound)?;
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    if !valid_signature(secret.expose_secret().as_bytes(), &body, signature) {
        tracing::warn!("webhook con firma inválida");
        return Err(AppError::Unauthorized);
    }
    let accepted = |deployed| Ok((StatusCode::ACCEPTED, Json(WebhookReport { deployed })));
    let event = headers
        .get("x-github-event")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if event != "push" {
        // `ping` (al crear el webhook) y cualquier otro evento: aceptados, sin hacer nada.
        return accepted(Vec::new());
    }
    let push: PushEvent = serde_json::from_slice(&body)
        .map_err(|error| AppError::BadRequest(format!("evento push ilegible: {error}").into()))?;
    let Some(branch) = push.reference.strip_prefix("refs/heads/") else {
        return accepted(Vec::new()); // Etiquetas y otras referencias no despliegan.
    };
    if push.deleted {
        return accepted(Vec::new());
    }
    let Some((owner, name)) = push.repository.full_name.split_once('/') else {
        return Err(AppError::BadRequest("repositorio sin dueño".into()));
    };
    let deployed = state.services.deploy_on_push(owner, name, branch).await?;
    tracing::info!(repository = %push.repository.full_name, %branch, ?deployed, "push recibido");
    accepted(deployed)
}

/// `sha256=<hex>` igual al HMAC del cuerpo, comparado en tiempo constante.
fn valid_signature(secret: &[u8], body: &[u8], header: &str) -> bool {
    let Some(hex) = header.strip_prefix("sha256=") else {
        return false;
    };
    let Some(expected) = decode_hex(hex) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&expected).is_ok()
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if hex.len() != 64 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(hex.get(index..index + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firma_de_github() {
        // Ejemplo de la documentación de GitHub ("Validating webhook deliveries").
        let secret = b"It's a Secret to Everybody";
        let body = b"Hello, World!";
        let good = "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";
        assert!(valid_signature(secret, body, good));
        assert!(!valid_signature(secret, b"Hello, World?", good));
        assert!(!valid_signature(b"otro", body, good));
        assert!(!valid_signature(secret, body, "sha1=abc"));
        assert!(!valid_signature(secret, body, "sha256=zz"));
    }
}
