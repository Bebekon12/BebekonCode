//! Sign in with ChatGPT (ChatGPT plan usage for open-source, locally hosted apps).
//!
//! Implements the documented flow at developers.openai.com/siwc/token-sharing-open-source:
//! dynamic client registration per account, PKCE with a one-shot 127.0.0.1 loopback callback,
//! ID-token validation against OpenAI's JWKS, refresh with the issued client id, and revocation
//! on sign-out. Tokens are stored per account with OS protection (see `secret_file`) and are only
//! ever handed to that account's Codex app-server process.

use crate::{error::CoreError, model::now, redaction::redact, secret_file, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Must match `clientInfo.name` sent to Codex app-server.
pub const AGENT_NAME: &str = "BebekonCode";
pub const USAGE_URL: &str = "https://chatgpt.com/settings/usage";
const AUTHORIZE: &str = "https://auth.openai.com/api/accounts/authorize";
const TOKEN: &str = "https://auth.openai.com/api/accounts/oauth/token";
const REVOKE: &str = "https://auth.openai.com/api/accounts/oauth/revoke";
const JWKS: &str = "https://auth.openai.com/.well-known/jwks.json";
const ISSUER: &str = "https://auth.openai.com";
pub const RESOURCE: &str = "https://api.openai.com/v1";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
pub const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
const DYNAMIC_CLIENT: &str = "dynamic_agent_client";
const CALLBACK_PATH: &str = "/auth/callback";
const PREFERRED_PORT: u16 = 1455;
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const CREDENTIALS_FILE: &str = "bebekon-siwc.bin";
const HOST_ID_FILE: &str = "agent-host-id";

/// One protected record per account registration. Never logged; no `Debug` on purpose.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Credentials {
    pub email: Option<String>,
    pub subject: Option<String>,
    pub client_id: Option<String>,
    pub id_token: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub earliest_refresh_at: Option<i64>,
    pub scopes: Vec<String>,
    pub plan: Option<String>,
    pub saved_at: i64,
}

impl Credentials {
    pub fn signed_in(&self) -> bool {
        self.refresh_token.is_some() || self.access_token_valid(0)
    }
    pub fn plan_enabled(&self) -> bool {
        self.scopes.iter().any(|scope| scope == PLAN_SCOPE)
    }
    pub fn access_token_valid(&self, margin: i64) -> bool {
        self.access_token.is_some() && self.expires_at.is_some_and(|at| at - margin > now())
    }
    fn clear_tokens(&mut self) {
        self.id_token = None;
        self.access_token = None;
        self.refresh_token = None;
        self.expires_at = None;
        self.earliest_refresh_at = None;
        self.scopes.clear();
    }
}

pub fn credentials_path(profile: &Path) -> PathBuf {
    profile.join(CREDENTIALS_FILE)
}

pub fn load(profile: &Path) -> Result<Credentials> {
    match secret_file::read(&credentials_path(profile))? {
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| CoreError::CredentialStore),
        None => Ok(Credentials::default()),
    }
}

pub fn save(profile: &Path, credentials: &Credentials) -> Result<()> {
    secret_file::write(
        &credentials_path(profile),
        &serde_json::to_vec(credentials)?,
    )
}

/// Stable, opaque identifier of this installation (`urn:uuid:` form), shared by all accounts.
pub fn host_id(provider_dir: &Path) -> Result<String> {
    let path = provider_dir.join(HOST_ID_FILE);
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim();
        if existing.starts_with("urn:uuid:") {
            return Ok(existing.to_string());
        }
    }
    std::fs::create_dir_all(provider_dir)?;
    let id = format!("urn:uuid:{}", uuid::Uuid::new_v4());
    std::fs::write(&path, &id)?;
    Ok(id)
}

/// An authorization attempt waiting for the browser to return to the loopback listener.
pub struct PendingLogin {
    listener: TcpListener,
    redirect_uri: String,
    state: String,
    nonce: String,
    verifier: String,
    /// Issued client id for reauthorization; `None` for a new registration.
    client_id: Option<String>,
    expected_subject: Option<String>,
}

/// Prepares the browser URL. The listener is bound before the URL exists, as required.
pub async fn begin(credentials: &Credentials, host: &str) -> Result<(String, PendingLogin)> {
    let listener = match TcpListener::bind(("127.0.0.1", PREFERRED_PORT)).await {
        Ok(listener) => listener,
        Err(_) => TcpListener::bind(("127.0.0.1", 0)).await?,
    };
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
    let state = random_token(32)?;
    let nonce = random_token(32)?;
    let verifier = random_token(48)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));

    let mut url = reqwest::Url::parse(AUTHORIZE).map_err(|_| CoreError::CredentialStore)?;
    {
        let mut query = url.query_pairs_mut();
        match &credentials.client_id {
            Some(client) => {
                query.append_pair("client_id", client);
                if let Some(hint) = &credentials.id_token {
                    query.append_pair("id_token_hint", hint);
                }
                if let Some(email) = &credentials.email {
                    query.append_pair("login_hint", email);
                }
            }
            None => {
                query.append_pair("client_id", DYNAMIC_CLIENT);
                query.append_pair("agent_name_hint", AGENT_NAME);
            }
        }
        query
            .append_pair("ext_agent_host_id", host)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("scope", SCOPES)
            .append_pair("resource", RESOURCE)
            .append_pair("state", &state)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &challenge);
    }
    Ok((
        url.into(),
        PendingLogin {
            listener,
            redirect_uri,
            state,
            nonce,
            verifier,
            client_id: credentials.client_id.clone(),
            expected_subject: credentials.subject.clone(),
        },
    ))
}

/// Waits for the callback, exchanges the code and validates the result. The previous record is
/// only replaced after the new identity is verified.
pub async fn complete(pending: PendingLogin, previous: Credentials) -> Result<Credentials> {
    let callback = tokio::time::timeout(LOGIN_TIMEOUT, wait_for_callback(&pending))
        .await
        .map_err(|_| failure("Время ожидания входа истекло. Начните вход заново."))??;
    drop(pending.listener);

    if callback.state.as_deref() != Some(pending.state.as_str()) {
        return Err(failure("Ответ входа не прошёл проверку state"));
    }
    if let Some(error) = callback.error {
        return Err(if error == "access_denied" {
            failure("Вход отменён в браузере")
        } else {
            failure(&format!("OpenAI отклонил вход: {}", redact(&error)))
        });
    }
    let client_id = match (&pending.client_id, callback.client_id) {
        (Some(saved), Some(returned)) if *saved != returned => {
            return Err(failure("OpenAI вернул другой client_id; вход отклонён"))
        }
        (Some(saved), _) => saved.clone(),
        (None, Some(issued)) if issued != DYNAMIC_CLIENT => issued,
        (None, _) => {
            return Err(failure(
                "Регистрация не завершена: OpenAI не выдал client_id",
            ))
        }
    };
    let code = callback
        .code
        .ok_or_else(|| failure("OpenAI не вернул код авторизации"))?;

    let client = http()?;
    let tokens = token_request(
        &client,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &client_id),
            ("code", &code),
            ("code_verifier", &pending.verifier),
            ("redirect_uri", &pending.redirect_uri),
            ("resource", RESOURCE),
        ],
    )
    .await?;
    let id_token = tokens
        .id_token
        .clone()
        .ok_or_else(|| failure("OpenAI не вернул ID-токен"))?;
    let claims = validate_id_token(&client, &id_token, &client_id, &pending.nonce).await?;
    if let Some(expected) = &pending.expected_subject {
        if *expected != claims.sub {
            return Err(failure(
                "Выполнен вход в другой аккаунт ChatGPT. Для другого аккаунта добавьте новую запись.",
            ));
        }
    }
    let mut credentials = previous;
    credentials.client_id = Some(client_id);
    credentials.subject = Some(claims.sub);
    credentials.email = claims.email.or(credentials.email);
    credentials.plan = claims.plan.or(credentials.plan);
    credentials.id_token = Some(id_token);
    apply_tokens(&mut credentials, tokens);
    Ok(credentials)
}

/// Standard refresh with the issued client id. Callers serialize refreshes per account.
pub async fn refresh(credentials: &Credentials) -> Result<Credentials> {
    let (Some(client_id), Some(refresh_token)) =
        (&credentials.client_id, &credentials.refresh_token)
    else {
        return Err(signed_out());
    };
    let tokens = token_request(
        &http()?,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
            ("resource", RESOURCE),
        ],
    )
    .await?;
    let mut updated = credentials.clone();
    if let Some(id_token) = tokens.id_token.clone() {
        updated.id_token = Some(id_token);
    }
    apply_tokens(&mut updated, tokens);
    Ok(updated)
}

/// Revokes the renewable session and clears local tokens. Returns `false` when remote
/// revocation could not be confirmed (tokens are cleared locally either way).
pub async fn sign_out(credentials: &mut Credentials) -> bool {
    let confirmed = match (&credentials.client_id, &credentials.refresh_token) {
        (Some(client_id), Some(token)) => revoke(client_id, token).await,
        _ => true,
    };
    credentials.clear_tokens();
    confirmed
}

async fn revoke(client_id: &str, token: &str) -> bool {
    let Ok(client) = http() else { return false };
    for attempt in 0..3u64 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(attempt * 2)).await;
        }
        let response = client
            .post(REVOKE)
            .form(&[
                ("token", token),
                ("token_type_hint", "refresh_token"),
                ("client_id", client_id),
            ])
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => return true,
            Ok(response) if !response.status().is_server_error() => return false,
            _ => continue,
        }
    }
    false
}

/// Codes that mean the token set can no longer be renewed and a new sign-in is required.
pub fn is_terminal_refresh_error(error: &CoreError) -> bool {
    matches!(error, CoreError::Provider { kind: Some(kind), .. } if kind == "auth")
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
    earliest_refresh_at: Option<Value>,
}

fn apply_tokens(credentials: &mut Credentials, tokens: TokenResponse) {
    let saved_at = now();
    credentials.access_token = tokens.access_token;
    if tokens.refresh_token.is_some() {
        credentials.refresh_token = tokens.refresh_token;
    }
    credentials.expires_at = tokens.expires_in.map(|seconds| saved_at + seconds);
    credentials.earliest_refresh_at = tokens.earliest_refresh_at.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    });
    if let Some(scope) = tokens.scope {
        credentials.scopes = scope.split_whitespace().map(str::to_string).collect();
    }
    credentials.saved_at = saved_at;
}

async fn token_request(client: &reqwest::Client, form: &[(&str, &str)]) -> Result<TokenResponse> {
    let response = client
        .post(TOKEN)
        .form(form)
        .send()
        .await
        .map_err(|_| network())?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return serde_json::from_value(body)
            .map_err(|_| failure("OpenAI вернул неожиданный ответ"));
    }
    let code = body
        .get("error")
        .and_then(|error| error.as_str().or_else(|| error.get("code")?.as_str()))
        .unwrap_or_default()
        .to_string();
    let terminal = matches!(
        code.as_str(),
        "invalid_grant"
            | "invalid_refresh_token"
            | "token_expired"
            | "refresh_token_expired"
            | "refresh_token_invalidated"
            | "refresh_token_reused"
    );
    if terminal {
        return Err(signed_out());
    }
    if status.is_server_error() {
        return Err(network());
    }
    Err(failure(&format!(
        "OpenAI отклонил запрос токена ({}{})",
        status.as_u16(),
        if code.is_empty() {
            String::new()
        } else {
            format!(", {}", redact(&code))
        }
    )))
}

struct Claims {
    sub: String,
    email: Option<String>,
    plan: Option<String>,
}

async fn validate_id_token(
    client: &reqwest::Client,
    id_token: &str,
    client_id: &str,
    nonce: &str,
) -> Result<Claims> {
    use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
    let invalid = || failure("ID-токен OpenAI не прошёл проверку подписи");
    let header = decode_header(id_token).map_err(|_| invalid())?;
    let kid = header.kid.ok_or_else(invalid)?;
    let keys: JwkSet = client
        .get(JWKS)
        .send()
        .await
        .map_err(|_| network())?
        .json()
        .await
        .map_err(|_| network())?;
    let key = keys.find(&kid).ok_or_else(invalid)?;
    let key = DecodingKey::from_jwk(key).map_err(|_| invalid())?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[client_id]);
    validation.leeway = 60;
    let data = decode::<Value>(id_token, &key, &validation).map_err(|_| invalid())?;
    let claims = data.claims;
    if claims.get("nonce").and_then(Value::as_str) != Some(nonce) {
        return Err(failure("ID-токен OpenAI не прошёл проверку nonce"));
    }
    let sub = claims
        .get("sub")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?
        .to_string();
    Ok(Claims {
        sub,
        email: claims
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_string),
        plan: claims
            .pointer("/https:~1~1api.openai.com~1auth/chatgpt_plan_type")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

struct Callback {
    code: Option<String>,
    state: Option<String>,
    client_id: Option<String>,
    error: Option<String>,
}

async fn wait_for_callback(pending: &PendingLogin) -> Result<Callback> {
    loop {
        let (mut stream, _) = pending.listener.accept().await?;
        let mut buffer = vec![0u8; 8192];
        let mut length = 0;
        // Read just the request head; the callback is a single small GET.
        while length < buffer.len() {
            let read =
                tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer[length..]))
                    .await
                    .map_err(|_| failure("Браузер не прислал ответ входа"))??;
            if read == 0 {
                break;
            }
            length += read;
            if buffer[..length]
                .windows(4)
                .any(|window| window == b"\r\n\r\n")
            {
                break;
            }
        }
        let head = String::from_utf8_lossy(&buffer[..length]);
        let target = head
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("GET "))
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or_default()
            .to_string();
        let Ok(url) = reqwest::Url::parse(&format!("http://127.0.0.1{target}")) else {
            respond(&mut stream, 400, "Некорректный запрос").await;
            continue;
        };
        if url.path() != CALLBACK_PATH {
            respond(&mut stream, 404, "Страница не найдена").await;
            continue;
        }
        let value = |key: &str| {
            url.query_pairs()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.into_owned())
        };
        let callback = Callback {
            code: value("code"),
            state: value("state"),
            client_id: value("client_id"),
            error: value("error"),
        };
        let message = if callback.error.is_some() {
            "Вход не выполнен. Вернитесь в BebekonCode."
        } else {
            "Вход выполнен. Вкладку можно закрыть и вернуться в BebekonCode."
        };
        respond(&mut stream, 200, message).await;
        return Ok(callback);
    }
}

async fn respond(stream: &mut tokio::net::TcpStream, status: u16, message: &str) {
    let body = format!(
        "<!doctype html><html lang=\"ru\"><meta charset=\"utf-8\"><title>BebekonCode</title>\
         <body style=\"font-family:Segoe UI,sans-serif;background:#0e131c;color:#e8ecff;\
         display:grid;place-items:center;height:100vh;margin:0\"><p>{message}</p></body></html>"
    );
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Bad Request",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn http() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("BebekonCode/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| network())
}

fn random_token(bytes: usize) -> Result<String> {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer).map_err(|_| CoreError::CredentialStore)?;
    Ok(URL_SAFE_NO_PAD.encode(buffer))
}

fn failure(message: &str) -> CoreError {
    CoreError::Provider {
        message: message.into(),
        kind: None,
    }
}
fn network() -> CoreError {
    CoreError::Provider {
        message: "Нет связи с OpenAI. Проверьте подключение и повторите.".into(),
        kind: Some("network".into()),
    }
}
pub fn signed_out() -> CoreError {
    CoreError::Provider {
        message: "Вход в ChatGPT истёк или был отозван. Войдите снова.".into(),
        kind: Some("auth".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_registration_url_follows_the_documented_parameters() {
        let (url, pending) = begin(&Credentials::default(), "urn:uuid:test-host")
            .await
            .expect("begin");
        let url = reqwest::Url::parse(&url).expect("url");
        assert_eq!(url.host_str(), Some("auth.openai.com"));
        assert_eq!(url.path(), "/api/accounts/authorize");
        let get = |key: &str| {
            url.query_pairs()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.into_owned())
        };
        assert_eq!(get("client_id").as_deref(), Some(DYNAMIC_CLIENT));
        assert_eq!(get("agent_name_hint").as_deref(), Some(AGENT_NAME));
        assert_eq!(
            get("ext_agent_host_id").as_deref(),
            Some("urn:uuid:test-host")
        );
        assert_eq!(get("response_type").as_deref(), Some("code"));
        assert_eq!(get("resource").as_deref(), Some(RESOURCE));
        assert_eq!(get("code_challenge_method").as_deref(), Some("S256"));
        assert!(get("scope").unwrap_or_default().contains(PLAN_SCOPE));
        let redirect = get("redirect_uri").unwrap_or_default();
        assert!(redirect.starts_with("http://127.0.0.1:"), "{redirect}");
        assert!(redirect.ends_with("/auth/callback"));
        assert_eq!(get("state").as_deref(), Some(pending.state.as_str()));
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(pending.verifier.as_bytes()));
        assert_eq!(get("code_challenge"), Some(challenge));
    }

    #[tokio::test]
    async fn reauthorization_reuses_the_issued_client_and_hints() {
        let saved = Credentials {
            client_id: Some("oaiapp_123".into()),
            email: Some("user@example.com".into()),
            id_token: Some("previous.id.token".into()),
            ..Credentials::default()
        };
        let (url, _pending) = begin(&saved, "urn:uuid:test-host").await.expect("begin");
        assert!(url.contains("client_id=oaiapp_123"));
        assert!(url.contains("id_token_hint=previous.id.token"));
        assert!(url.contains("login_hint=user%40example.com"));
        assert!(!url.contains("agent_name_hint"));
    }

    #[tokio::test]
    async fn callback_with_wrong_state_is_rejected_without_exchanging() {
        let (_, pending) = begin(&Credentials::default(), "urn:uuid:h")
            .await
            .expect("begin");
        let port = pending.listener.local_addr().expect("addr").port();
        let browser = tokio::spawn(async move {
            let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .expect("connect");
            stream
                .write_all(
                    b"GET /auth/callback?code=c&state=forged&client_id=oaiapp_x HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                )
                .await
                .expect("write");
            let mut reply = String::new();
            let _ = stream.read_to_string(&mut reply).await;
            reply
        });
        let error = complete(pending, Credentials::default())
            .await
            .err()
            .expect("must fail");
        assert!(error.to_string().contains("state"));
        assert!(browser.await.expect("browser").starts_with("HTTP/1.1 200"));
    }

    #[test]
    fn host_id_is_stable_and_opaque() {
        let directory = std::env::temp_dir().join(format!("siwc-host-{}", uuid::Uuid::new_v4()));
        let first = host_id(&directory).expect("host");
        assert!(first.starts_with("urn:uuid:"));
        assert_eq!(host_id(&directory).expect("again"), first);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn plan_usage_requires_the_documented_scope() {
        let mut credentials = Credentials {
            scopes: vec!["openid".into(), "email".into()],
            ..Credentials::default()
        };
        assert!(!credentials.plan_enabled());
        credentials.scopes.push(PLAN_SCOPE.into());
        assert!(credentials.plan_enabled());
    }
}
