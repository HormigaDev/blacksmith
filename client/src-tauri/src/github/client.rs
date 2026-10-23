//! Cliente HTTP de GitHub: Device Flow (autorización sin pegar nada y sin secretos en el cliente)
//! y la API REST para el usuario, sus repositorios y sus ramas.
//!
//! - Solo HTTPS en producción (`https_only`); las URL base se inyectan para los tests.
//! - Los errores se traducen a códigos estables; el token nunca aparece en un mensaje ni en un
//!   registro.

use std::time::Duration;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::{CommandError, CommandResult};

/// Las direcciones de GitHub que la app abre en el navegador.
const GITHUB_WEB_PREFIX: &str = "https://github.com/";
const API_VERSION: &str = "2022-11-28";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const PER_PAGE: usize = 100;
/// Más que suficiente para elegir un repositorio; evita paginar sin fin.
const MAX_REPOSITORIES: usize = 1000;
const MAX_BRANCHES: usize = 500;
/// Páginas de llaves de despliegue que se revisan (100 por página).
const MAX_KEY_PAGES: usize = 10;
const RELEASE_REPOSITORY: &str = "HormigaDev/blacksmith";

/// Respuesta de `POST /login/device/code`.
#[derive(Clone, Deserialize)]
pub struct DeviceCode {
    pub device_code: Zeroizing<String>,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Resultado de un sondeo de `POST /login/oauth/access_token`.
pub enum Poll {
    Pending,
    /// GitHub pide esperar más entre sondeos (segundos que añadir).
    SlowDown(u64),
    Authorized(Zeroizing<String>),
}

// A mano: `Zeroizing` muestra su contenido en `Debug`, y un `{:?}` no debe imprimir el código de
// dispositivo ni el token.
impl std::fmt::Debug for DeviceCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCode")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("expires_in", &self.expires_in)
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Poll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => f.write_str("Pending"),
            Self::SlowDown(seconds) => f.debug_tuple("SlowDown").field(seconds).finish(),
            Self::Authorized(_) => f.write_str("Authorized(..)"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubUser {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepository {
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub default_branch: String,
    pub html_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRelease {
    #[serde(rename = "tag_name")]
    pub tag_name: String,
    #[serde(rename = "html_url")]
    pub html_url: String,
    pub prerelease: bool,
    pub draft: bool,
}

pub struct GitHubClient {
    http: reqwest::Client,
    /// `https://github.com`
    web: String,
    /// `https://api.github.com`
    api: String,
}

impl GitHubClient {
    pub fn new() -> CommandResult<Self> {
        Self::with_urls("https://github.com", "https://api.github.com", true)
    }

    pub fn with_urls(web: &str, api: &str, https_only: bool) -> CommandResult<Self> {
        // Idempotente: reqwest se compila sin proveedor de criptografía propio.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .https_only(https_only)
            .user_agent(concat!("Blacksmith/", env!("CARGO_PKG_VERSION")))
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| CommandError::internal("creando el cliente de GitHub", error))?;
        Ok(Self {
            http,
            web: web.trim_end_matches('/').to_owned(),
            api: api.trim_end_matches('/').to_owned(),
        })
    }

    pub async fn device_code(&self, client_id: &str) -> CommandResult<DeviceCode> {
        let response = self
            .http
            .post(format!("{}/login/device/code", self.web))
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "client_id": client_id }))
            .send()
            .await
            .map_err(unreachable)?;
        let status = response.status();
        let body: serde_json::Value = response.json().await.map_err(unreachable)?;
        if let Some(error) = body.get("error").and_then(|error| error.as_str()) {
            return Err(oauth_error(error));
        }
        if !status.is_success() {
            return Err(CommandError::GitHub {
                status: status.as_u16(),
            });
        }
        let code: DeviceCode = serde_json::from_value(body).map_err(|_| CommandError::GitHub {
            status: status.as_u16(),
        })?;
        // La interfaz abre esta dirección en el navegador: solo una página de GitHub.
        if !code.verification_uri.starts_with(GITHUB_WEB_PREFIX) {
            return Err(CommandError::GitHub {
                status: status.as_u16(),
            });
        }
        Ok(code)
    }

    pub async fn poll_token(&self, client_id: &str, device_code: &str) -> CommandResult<Poll> {
        let response = self
            .http
            .post(format!("{}/login/oauth/access_token", self.web))
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "client_id": client_id,
                "device_code": device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            }))
            .send()
            .await
            .map_err(unreachable)?;
        let status = response.status();
        let body: serde_json::Value = response.json().await.map_err(unreachable)?;
        if let Some(token) = body.get("access_token").and_then(|token| token.as_str()) {
            return Ok(Poll::Authorized(Zeroizing::new(token.to_owned())));
        }
        match body.get("error").and_then(|error| error.as_str()) {
            Some("authorization_pending") => Ok(Poll::Pending),
            Some("slow_down") => Ok(Poll::SlowDown(
                body.get("interval")
                    .and_then(|interval| interval.as_u64())
                    .unwrap_or(5),
            )),
            Some(error) => Err(oauth_error(error)),
            None => Err(CommandError::GitHub {
                status: status.as_u16(),
            }),
        }
    }

    pub async fn user(&self, token: &str) -> CommandResult<GitHubUser> {
        self.get(token, "/user").await
    }

    pub async fn latest_release(&self) -> CommandResult<GitHubRelease> {
        let response = self
            .http
            .get(format!("{}/repos/{RELEASE_REPOSITORY}/releases/latest", self.api))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
            .send()
            .await
            .map_err(unreachable)?;
        let status = response.status();
        if !status.is_success() {
            return Err(CommandError::GitHub {
                status: status.as_u16(),
            });
        }
        response
            .json()
            .await
            .map_err(|_| CommandError::GitHub {
                status: status.as_u16(),
            })
    }

    /// Repositorios de todas las instalaciones de la App a las que el usuario tiene acceso
    /// (públicos y privados), ordenados por nombre completo.
    pub async fn repositories(&self, token: &str) -> CommandResult<Vec<GitHubRepository>> {
        #[derive(Deserialize)]
        struct Installations {
            installations: Vec<Installation>,
        }
        #[derive(Deserialize)]
        struct Installation {
            id: u64,
        }
        #[derive(Deserialize)]
        struct Repositories {
            repositories: Vec<RawRepository>,
        }
        #[derive(Deserialize)]
        struct RawRepository {
            name: String,
            full_name: String,
            private: bool,
            default_branch: Option<String>,
            html_url: String,
            owner: Owner,
        }
        #[derive(Deserialize)]
        struct Owner {
            login: String,
        }

        let mut installations = Vec::new();
        for page in 1.. {
            let batch: Installations = self
                .get(
                    token,
                    &format!("/user/installations?per_page={PER_PAGE}&page={page}"),
                )
                .await?;
            let done = batch.installations.len() < PER_PAGE;
            installations.extend(batch.installations);
            if done || installations.len() >= MAX_REPOSITORIES {
                break;
            }
        }

        let mut repositories = Vec::new();
        'installations: for installation in installations {
            for page in 1.. {
                let batch: Repositories = self
                    .get(
                        token,
                        &format!(
                            "/user/installations/{}/repositories?per_page={PER_PAGE}&page={page}",
                            installation.id
                        ),
                    )
                    .await?;
                let done = batch.repositories.len() < PER_PAGE;
                repositories.extend(batch.repositories.into_iter().map(|raw| GitHubRepository {
                    owner: raw.owner.login,
                    name: raw.name,
                    full_name: raw.full_name,
                    private: raw.private,
                    default_branch: raw.default_branch.unwrap_or_else(|| "main".to_owned()),
                    html_url: raw.html_url,
                }));
                if repositories.len() >= MAX_REPOSITORIES {
                    repositories.truncate(MAX_REPOSITORIES);
                    break 'installations;
                }
                if done {
                    break;
                }
            }
        }
        repositories.sort_by_key(|repository| repository.full_name.to_lowercase());
        repositories.dedup_by(|a, b| a.full_name == b.full_name);
        Ok(repositories)
    }

    pub async fn branches(
        &self,
        token: &str,
        owner: &str,
        name: &str,
    ) -> CommandResult<Vec<String>> {
        #[derive(Deserialize)]
        struct Branch {
            name: String,
        }
        validate_owner(owner)?;
        validate_repository(name)?;
        let mut branches = Vec::new();
        for page in 1.. {
            let batch: Vec<Branch> = self
                .get(
                    token,
                    &format!("/repos/{owner}/{name}/branches?per_page={PER_PAGE}&page={page}"),
                )
                .await?;
            let done = batch.len() < PER_PAGE;
            branches.extend(batch.into_iter().map(|branch| branch.name));
            if done || branches.len() >= MAX_BRANCHES {
                break;
            }
        }
        branches.truncate(MAX_BRANCHES);
        Ok(branches)
    }

    /// Añade `key` como *deploy key* de solo lectura del repositorio. Si ya estaba, no hace nada.
    /// Requiere una App con permiso *Administration: write* (solo la propia del usuario: la App
    /// de Blacksmith no lo pide); sin él falla con `GitHubPermissionMissing`.
    pub async fn add_deploy_key(
        &self,
        token: &str,
        owner: &str,
        name: &str,
        title: &str,
        key: &str,
    ) -> CommandResult<()> {
        #[derive(Deserialize)]
        struct Existing {
            key: String,
        }
        validate_owner(owner)?;
        validate_repository(name)?;
        if !key.starts_with("ssh-ed25519 ") || key.len() > 512 || key.contains(['\n', '\r']) {
            return Err(CommandError::InvalidTarget("key"));
        }
        let title: String = title
            .chars()
            .filter(|c| !c.is_control())
            .take(100)
            .collect();
        let response = self
            .http
            .post(format!("{}/repos/{owner}/{name}/keys", self.api))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
            .json(&serde_json::json!({ "title": title, "key": key, "read_only": true }))
            .send()
            .await
            .map_err(unreachable)?;
        match response.status().as_u16() {
            200..=299 => Ok(()),
            401 => Err(CommandError::GitHubNotConnected),
            // Sin el permiso Administration, o la App no está instalada en ese repositorio.
            403 | 404 => Err(CommandError::GitHubPermissionMissing),
            // 422 es «ya existe» (bien si es de este repositorio: se compara la llave) o una llave
            // que GitHub no acepta; lo dice el mensaje del error.
            422 => {
                let body: serde_json::Value = response.json().await.unwrap_or_default();
                let in_use = body
                    .get("errors")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|errors| {
                        errors.iter().any(|error| {
                            error
                                .get("message")
                                .and_then(serde_json::Value::as_str)
                                .is_some_and(|message| message.contains("already in use"))
                        })
                    });
                if !in_use {
                    return Err(CommandError::GitHub { status: 422 });
                }
                let ours = key.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
                for page in 1..=MAX_KEY_PAGES {
                    let existing: Vec<Existing> = self
                        .get(
                            token,
                            &format!("/repos/{owner}/{name}/keys?per_page={PER_PAGE}&page={page}"),
                        )
                        .await?;
                    if existing
                        .iter()
                        .any(|candidate| candidate.key.trim() == ours)
                    {
                        return Ok(());
                    }
                    if existing.len() < PER_PAGE {
                        break;
                    }
                }
                Err(CommandError::GitHubKeyInUse)
            }
            status => Err(CommandError::GitHub { status }),
        }
    }

    async fn get<T: DeserializeOwned>(&self, token: &str, path: &str) -> CommandResult<T> {
        let response = self
            .http
            .get(format!("{}{path}", self.api))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", API_VERSION)
            .send()
            .await
            .map_err(unreachable)?;
        let status = response.status();
        match status.as_u16() {
            200..=299 => response.json().await.map_err(|_| CommandError::GitHub {
                status: status.as_u16(),
            }),
            401 => Err(CommandError::GitHubNotConnected),
            403 | 429
                if response
                    .headers()
                    .get("x-ratelimit-remaining")
                    .is_some_and(|remaining| remaining == "0")
                    || status.as_u16() == 429 =>
            {
                Err(CommandError::GitHubRateLimited)
            }
            other => Err(CommandError::GitHub { status: other }),
        }
    }
}

/// Usuario u organización de GitHub: letras, dígitos y guiones, sin guion en los extremos.
pub fn validate_owner(owner: &str) -> CommandResult<()> {
    let valid = (1..=39).contains(&owner.len())
        && owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-');
    valid
        .then_some(())
        .ok_or(CommandError::InvalidTarget("owner"))
}

pub fn validate_repository(name: &str) -> CommandResult<()> {
    let valid = (1..=100).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        && name != "."
        && name != "..";
    valid
        .then_some(())
        .ok_or(CommandError::InvalidTarget("repository"))
}

fn unreachable(_: reqwest::Error) -> CommandError {
    CommandError::GitHubUnreachable
}

/// Errores del Device Flow (RFC 8628 y los propios de GitHub).
fn oauth_error(error: &str) -> CommandError {
    match error {
        "access_denied" => CommandError::GitHubAccessDenied,
        "expired_token" | "token_expired" => CommandError::GitHubCodeExpired,
        "device_flow_disabled" => CommandError::GitHubDeviceFlowDisabled,
        "incorrect_client_credentials" | "unauthorized_client" => CommandError::GitHubNotConfigured,
        _ => CommandError::GitHub { status: 400 },
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{collections::VecDeque, net::SocketAddr, sync::Arc};

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };

    use super::*;

    /// GitHub falso: responde en orden y guarda la primera línea de cada petición.
    pub struct FakeGitHub {
        pub addr: SocketAddr,
        pub requests: Arc<Mutex<Vec<String>>>,
    }

    pub async fn fake(responses: Vec<(u16, String)>) -> FakeGitHub {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let queue = Arc::new(Mutex::new(VecDeque::from(responses)));
        let log = Arc::clone(&requests);
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buffer = vec![0u8; 16 * 1024];
                let read = socket.read(&mut buffer).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
                log.lock().await.push(request);
                let (status, body) = queue.lock().await.pop_front().unwrap_or((500, "{}".into()));
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
            }
        });
        FakeGitHub { addr, requests }
    }

    pub fn client(fake: &FakeGitHub) -> GitHubClient {
        let base = format!("http://{}", fake.addr);
        GitHubClient::with_urls(&base, &base, false).unwrap()
    }

    #[tokio::test]
    async fn device_flow() {
        let github = fake(vec![
            (
                200,
                r#"{"device_code":"dc","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#.into(),
            ),
            (200, r#"{"error":"authorization_pending"}"#.into()),
            (200, r#"{"error":"slow_down","interval":10}"#.into()),
            (200, r#"{"access_token":"gho_secreto","token_type":"bearer","scope":""}"#.into()),
            (200, r#"{"error":"access_denied"}"#.into()),
            (200, r#"{"error":"expired_token"}"#.into()),
            (200, r#"{"error":"device_flow_disabled"}"#.into()),
            (
                200,
                r#"{"device_code":"dc","user_code":"X","verification_uri":"https://evil.example/login","expires_in":900,"interval":5}"#.into(),
            ),
        ])
        .await;
        let client = client(&github);
        let code = client.device_code("Iv23liABC").await.unwrap();
        assert_eq!(code.user_code, "ABCD-1234");
        assert!(matches!(
            client.poll_token("Iv23liABC", "dc").await,
            Ok(Poll::Pending)
        ));
        assert!(matches!(
            client.poll_token("Iv23liABC", "dc").await,
            Ok(Poll::SlowDown(10))
        ));
        match client.poll_token("Iv23liABC", "dc").await {
            Ok(Poll::Authorized(token)) => assert_eq!(token.as_str(), "gho_secreto"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            client.poll_token("x", "dc").await,
            Err(CommandError::GitHubAccessDenied)
        ));
        assert!(matches!(
            client.poll_token("x", "dc").await,
            Err(CommandError::GitHubCodeExpired)
        ));
        assert!(matches!(
            client.poll_token("x", "dc").await,
            Err(CommandError::GitHubDeviceFlowDisabled)
        ));
        // Una dirección que no es de GitHub no llega a abrirse en el navegador.
        assert!(client.device_code("Iv23liABC").await.is_err());
        let requests = github.requests.lock().await;
        assert!(requests[0].starts_with("POST /login/device/code "));
        assert!(requests[1].contains("urn:ietf:params:oauth:grant-type:device_code"));
    }

    #[tokio::test]
    async fn repositorios_de_todas_las_instalaciones_paginados() {
        let page = |from: usize, count: usize| {
            let repos: Vec<String> = (from..from + count)
                .map(|i| {
                    format!(
                        r#"{{"name":"r{i:03}","full_name":"o/r{i:03}","private":{},"default_branch":"main","html_url":"https://github.com/o/r{i:03}","owner":{{"login":"o"}}}}"#,
                        i % 2 == 0
                    )
                })
                .collect();
            format!(r#"{{"repositories":[{}]}}"#, repos.join(","))
        };
        let github = fake(vec![
            (200, r#"{"installations":[{"id":1},{"id":2}]}"#.into()),
            (200, page(0, PER_PAGE)),
            (200, page(PER_PAGE, 3)),
            (
                200,
                r#"{"repositories":[{"name":"Zeta","full_name":"org/Zeta","private":true,"default_branch":null,"html_url":"https://github.com/org/Zeta","owner":{"login":"org"}}]}"#.into(),
            ),
        ])
        .await;
        let repositories = client(&github).repositories("token").await.unwrap();
        assert_eq!(repositories.len(), PER_PAGE + 4);
        assert_eq!(repositories[0].full_name, "o/r000");
        let zeta = repositories.last().unwrap();
        assert_eq!(
            (zeta.owner.as_str(), zeta.default_branch.as_str()),
            ("org", "main")
        );
        let requests = github.requests.lock().await;
        assert!(requests[0].starts_with("GET /user/installations?per_page=100&page=1 "));
        assert!(
            requests[2].starts_with("GET /user/installations/1/repositories?per_page=100&page=2 ")
        );
        assert!(
            requests[3].starts_with("GET /user/installations/2/repositories?per_page=100&page=1 ")
        );
        assert!(requests[0].contains("authorization: Bearer token"));
        assert!(requests[0].contains("x-github-api-version: 2022-11-28"));
    }

    #[tokio::test]
    async fn registra_la_llave_de_despliegue_de_solo_lectura() {
        let key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOk blacksmith-api@vps";
        let github = fake(vec![
            (201, r#"{"id":1}"#.into()),
            (
                403,
                r#"{"message":"Resource not accessible by integration"}"#.into(),
            ),
            (422, r#"{"message":"Validation Failed","errors":[{"resource":"PublicKey","code":"custom","field":"key","message":"key is already in use"}]}"#.into()),
            (
                200,
                r#"[{"key":"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOk"}]"#.into(),
            ),
            (422, r#"{"message":"Validation Failed","errors":[{"resource":"PublicKey","code":"custom","field":"key","message":"key is already in use"}]}"#.into()),
            (200, r#"[]"#.into()),
            (
                422,
                r#"{"message":"Validation Failed","errors":[{"resource":"PublicKey","code":"custom","field":"key","message":"key is invalid. You must supply a key in OpenSSH public key format"}]}"#.into(),
            ),
        ])
        .await;
        let client = client(&github);
        client
            .add_deploy_key("t", "o", "r", "blacksmith-api", key)
            .await
            .unwrap();
        // La App de Blacksmith (sin Administration) no puede: la interfaz pasa al registro manual.
        assert!(matches!(
            client.add_deploy_key("t", "o", "r", "x", key).await,
            Err(CommandError::GitHubPermissionMissing)
        ));
        // Ya registrada en este repositorio: correcto.
        client
            .add_deploy_key("t", "o", "r", "x", key)
            .await
            .unwrap();
        // Registrada en otro repositorio (GitHub no permite repetirla).
        assert!(matches!(
            client.add_deploy_key("t", "o", "r", "x", key).await,
            Err(CommandError::GitHubKeyInUse)
        ));
        // Otro 422 (llave rechazada) no es «ya en uso».
        assert!(matches!(
            client.add_deploy_key("t", "o", "r", "x", key).await,
            Err(CommandError::GitHub { status: 422 })
        ));
        assert!(matches!(
            client
                .add_deploy_key("t", "o", "r", "x", "ssh-rsa AAAA")
                .await,
            Err(CommandError::InvalidTarget("key"))
        ));
        let requests = github.requests.lock().await;
        assert!(requests[0].starts_with("POST /repos/o/r/keys "));
        assert!(requests[0].contains(r#""read_only":true"#));
    }

    #[tokio::test]
    async fn errores_de_la_api() {
        let github = fake(vec![
            (401, r#"{"message":"Bad credentials"}"#.into()),
            (429, r#"{"message":"slow"}"#.into()),
            (404, r#"{"message":"Not Found"}"#.into()),
        ])
        .await;
        let client = client(&github);
        assert!(matches!(
            client.user("t").await,
            Err(CommandError::GitHubNotConnected)
        ));
        assert!(matches!(
            client.user("t").await,
            Err(CommandError::GitHubRateLimited)
        ));
        assert!(matches!(
            client.branches("t", "o", "r").await,
            Err(CommandError::GitHub { status: 404 })
        ));
        // Nunca se compone una ruta con un nombre inválido.
        assert!(matches!(
            client.branches("t", "../admin", "r").await,
            Err(CommandError::InvalidTarget("owner"))
        ));
        assert!(matches!(
            client.branches("t", "o", "r/../../x").await,
            Err(CommandError::InvalidTarget("repository"))
        ));
    }

    #[test]
    fn solo_https_en_produccion() {
        let client = GitHubClient::new().unwrap();
        assert!(client.web.starts_with("https://") && client.api.starts_with("https://"));
    }
}
