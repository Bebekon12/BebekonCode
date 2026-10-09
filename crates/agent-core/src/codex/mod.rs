//! OpenAI Codex integration through the official `codex app-server` (JSON-RPC over stdio).
//!
//! Compliance boundaries (see docs/provider-compliance.md):
//! * Sign-in follows OpenAI's documented "Sign in with ChatGPT" flow for open-source, locally
//!   hosted apps (`siwc`): one dynamic client registration per account, tokens protected per
//!   account and passed only to that account's app-server as `ACCESS_TOKEN`, exactly as the
//!   "Codex app-server" page of that documentation describes. Users may explicitly choose
//!   official Codex ChatGPT login for quota access; CLI owns its per-home keyring credentials.
//! * Each account runs its own app-server with its own `CODEX_HOME`.
//! * Sandbox and approvals stay enabled by default; full access is an explicit chat choice.
//! * Usage limits are displayed as reported. Nothing switches accounts automatically.

mod mapping;
mod native_auth;
pub mod plugins;
mod rpc;
mod sandbox;
pub mod siwc;

use crate::{
    error::{CoreError, Result},
    model::*,
    process,
    provider::{AgentProvider, TurnRequest},
    redaction::redact,
};
use async_trait::async_trait;
use mapping::{clip, map_notification, str_at, summarize_paths, Mapped, TurnEnd, TurnState};
use rpc::{Incoming, RpcError, RpcPeer};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const PROVIDER_ID: &str = "openai";
/// Protocol version this adapter was verified against. Newer versions are allowed but flagged.
pub const TESTED_VERSION: &str = "0.160";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const INTERRUPT_GRACE: Duration = Duration::from_secs(15);

#[derive(Default, Clone)]
struct Detection {
    binary: Option<PathBuf>,
    managed_root: Option<PathBuf>,
    version: Option<String>,
}

struct AppServer {
    operation: tokio::sync::Mutex<()>,
    sandbox: Mutex<Option<SandboxStatus>>,
    peer: Arc<RpcPeer>,
    child: Mutex<Option<std::process::Child>>,
    /// Owns the whole process tree (Codex and helpers such as `git`); dropping it ends them.
    group: Mutex<Option<process::ProcessGroup>>,
    /// Threads with a turn running in this app; their server requests belong to that turn.
    active: Mutex<HashSet<String>>,
    /// Expiry of the access token this process was started with; renewal needs a restart.
    token_expires_at: Option<i64>,
    /// Threads already started or resumed on this process.
    loaded: Mutex<HashSet<String>>,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

impl AppServer {
    fn last_error(&self) -> Option<String> {
        let lines = self.stderr.lock().ok()?;
        let text = lines.iter().cloned().collect::<Vec<_>>().join("\n");
        (!text.trim().is_empty()).then(|| clip(&redact(&text), 600))
    }
    fn stop(&self) {
        self.peer.close();
        if let Some(group) = self.group.lock().ok().and_then(|mut group| group.take()) {
            group.terminate();
        }
        if let Some(mut child) = self.child.lock().ok().and_then(|mut child| child.take()) {
            let _ = child.kill();
            let _ = child.try_wait();
        }
    }
}

/// A JSON-RPC request Codex sent to the client.
struct ServerRequest {
    id: Value,
    method: String,
    params: Value,
}

struct PendingApproval {
    available_decisions: Option<Vec<ApprovalDecision>>,
    session_id: String,
    server: Arc<AppServer>,
    rpc_id: Value,
    events: mpsc::Sender<EventPayload>,
}

pub struct CodexProvider {
    detection: RwLock<Detection>,
    servers: tokio::sync::Mutex<HashMap<String, Arc<AppServer>>>,
    approvals: Arc<Mutex<HashMap<String, PendingApproval>>>,
    account_events: broadcast::Sender<AccountEvent>,
    /// Serializes token refresh per account so a rotating refresh token is never raced.
    auth_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    logins: Mutex<HashMap<String, tokio::task::AbortHandle>>,
}

impl CodexProvider {
    pub fn new(account_events: broadcast::Sender<AccountEvent>) -> Self {
        Self {
            detection: RwLock::new(Detection::default()),
            servers: tokio::sync::Mutex::new(HashMap::new()),
            approvals: Arc::default(),
            account_events,
            auth_locks: Mutex::default(),
            logins: Mutex::default(),
        }
    }

    fn profile(account: &AccountProfile) -> Result<PathBuf> {
        account
            .config_dir
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(|| CoreError::Invalid("У аккаунта нет папки профиля".into()))
    }

    fn auth_lock(&self, account: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.auth_locks
            .lock()
            .map(|mut locks| Arc::clone(locks.entry(account.to_string()).or_default()))
            .unwrap_or_default()
    }

    /// Returns usable credentials, refreshing the access token near expiry.
    async fn credentials(&self, account: &AccountProfile) -> Result<siwc::Credentials> {
        let profile = Self::profile(account)?;
        let lock = self.auth_lock(&account.id);
        let _guard = lock.lock().await;
        let mut credentials = siwc::load(&profile)?;
        if !credentials.signed_in() {
            return Err(siwc::signed_out());
        }
        if !credentials.plan_enabled() {
            return Err(CoreError::Provider {
                message: "Использование плана ChatGPT не разрешено для BebekonCode. Нажмите «Continue with ChatGPT» и разрешите доступ.".into(),
                kind: Some("auth".into()),
            });
        }
        let too_early = credentials.earliest_refresh_at.is_some_and(|at| at > now());
        if credentials.access_token_valid(300) || (too_early && credentials.access_token_valid(0)) {
            return Ok(credentials);
        }
        match siwc::refresh(&credentials).await {
            Ok(updated) => {
                siwc::save(&profile, &updated)?;
                Ok(updated)
            }
            Err(error) => {
                if siwc::is_terminal_refresh_error(&error) {
                    // Unusable token set: clear it and require a new sign-in with the saved
                    // client id. A temporary network failure keeps the credentials.
                    siwc::sign_out(&mut credentials).await;
                    siwc::save(&profile, &credentials)?;
                }
                Err(error)
            }
        }
    }

    fn detection(&self) -> Detection {
        self.detection
            .read()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    async fn server(&self, account: &AccountProfile) -> Result<Arc<AppServer>> {
        let mut servers = self.servers.lock().await;
        let native = native_auth::selected(account)?;
        let credentials = if native {
            None
        } else {
            Some(self.credentials(account).await?)
        };
        if let Some(server) = servers.get(&account.id) {
            let fresh = server
                .token_expires_at
                .map_or(native, |at| at - 300 > now());
            let busy = server
                .active
                .lock()
                .map(|active| !active.is_empty())
                .unwrap_or(true)
                || server.operation.try_lock().is_err();
            if !server.peer.is_closed() && (fresh || busy) {
                return Ok(Arc::clone(server));
            }
            // Documented renewal: restart with the new token, then resume threads by id.
            server.stop();
        }
        let server = self.spawn(account, credentials.as_ref()).await?;
        servers.insert(account.id.clone(), Arc::clone(&server));
        Ok(server)
    }

    async fn spawn(
        &self,
        account: &AccountProfile,
        credentials: Option<&siwc::Credentials>,
    ) -> Result<Arc<AppServer>> {
        let detection = self.detection();
        let binary = detection.binary.ok_or_else(|| CoreError::Provider {
            message: "Codex CLI не найден. Установите его и нажмите «Обновить» в настройках."
                .into(),
            kind: Some("unavailable".into()),
        })?;
        let home = account
            .config_dir
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(|| CoreError::Invalid("У аккаунта нет папки профиля".into()))?;
        std::fs::create_dir_all(&home)?;

        let mut extra = vec![("CODEX_HOME", OsString::from(&home))];
        if let Some(credentials) = credentials {
            let token = credentials
                .access_token
                .clone()
                .ok_or_else(siwc::signed_out)?;
            extra.push(("ACCESS_TOKEN", OsString::from(token)));
        }
        if let Some(root) = &detection.managed_root {
            // Mirrors the official npm launcher, which we bypass only to avoid extra processes.
            extra.push(("CODEX_MANAGED_BY_NPM", OsString::from("1")));
            extra.push(("CODEX_MANAGED_PACKAGE_ROOT", OsString::from(root)));
        }
        let mut command = std::process::Command::new(&binary);
        command.args(["app-server", "--listen", "stdio://"]);
        // Desktop-only plugins must never load their skills, helpers or MCP servers here.
        for entry in plugins::installed(self, account)
            .await?
            .entries
            .into_iter()
            .filter(|entry| entry.installed && plugins::desktop_only(&entry.name))
        {
            command.args(["-c", &format!("plugins.{:?}.enabled=false", entry.id)]);
        }
        if credentials.is_some() {
            // Provider configuration from the SIWC "Codex app-server" documentation.
            command.args(
                [
                    "model_provider=\"openai_chatgpt_plan\"",
                    "model_providers.openai_chatgpt_plan.name=\"ChatGPT plan\"",
                    "model_providers.openai_chatgpt_plan.base_url=\"https://api.openai.com/v1\"",
                    "model_providers.openai_chatgpt_plan.env_key=\"ACCESS_TOKEN\"",
                    "model_providers.openai_chatgpt_plan.wire_api=\"responses\"",
                    "model_providers.openai_chatgpt_plan.requires_openai_auth=false",
                    "model_providers.openai_chatgpt_plan.supports_websockets=false",
                ]
                .into_iter()
                .flat_map(|value| ["-c", value]),
            );
        } else {
            command.args(
                [
                    "model_provider=\"openai\"",
                    "forced_login_method=\"chatgpt\"",
                    "cli_auth_credentials_store=\"keyring\"",
                    "analytics.enabled=false",
                    "feedback.enabled=false",
                ]
                .into_iter()
                .flat_map(|value| ["-c", value]),
            );
        }
        command
            .args(
                [
                    "shell_environment_policy.inherit=\"core\"",
                    "shell_environment_policy.ignore_default_excludes=false",
                    "sandbox_mode=\"workspace-write\"",
                    "approval_policy=\"on-request\"",
                    "features.hooks=false",
                ]
                .into_iter()
                .flat_map(|value| ["-c", value]),
            )
            .current_dir(&home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        process::harden(&mut command, process::child_env(&extra));
        let mut child = command.spawn().map_err(|_| CoreError::Provider {
            message: "Не удалось запустить Codex app-server".into(),
            kind: Some("unavailable".into()),
        })?;
        let group = process::ProcessGroup::adopt(&child);
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            return Err(CoreError::Provider {
                message: "Не удалось подключиться к Codex app-server".into(),
                kind: None,
            });
        };

        // Keep only a short, redacted tail of stderr for error messages; never log it.
        let tail: Arc<Mutex<VecDeque<String>>> = Arc::default();
        let tail_writer = Arc::clone(&tail);
        let _ = std::thread::Builder::new()
            .name("codex-stderr".into())
            .spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stderr).lines() {
                    let Ok(line) = line else { break };
                    if let Ok(mut tail) = tail_writer.lock() {
                        tail.push_back(line.chars().take(300).collect());
                        while tail.len() > 6 {
                            tail.pop_front();
                        }
                    }
                }
            });

        let peer = match RpcPeer::from_process(stdout, stdin) {
            Ok(peer) => peer,
            Err(_) => {
                drop(group);
                let _ = child.kill();
                return Err(CoreError::Provider {
                    message: "Не удалось подключиться к Codex app-server".into(),
                    kind: None,
                });
            }
        };
        let server = Arc::new(AppServer {
            operation: tokio::sync::Mutex::new(()),
            sandbox: Mutex::default(),
            peer: Arc::clone(&peer),
            child: Mutex::new(Some(child)),
            group: Mutex::new(group),
            active: Mutex::default(),
            token_expires_at: credentials.and_then(|c| c.expires_at),
            loaded: Mutex::default(),
            stderr: tail,
        });
        self.route(account.id.clone(), Arc::clone(&server));

        let initialized = peer
            .request(
                "initialize",
                json!({
                    "clientInfo": {
                        // Must match the SIWC `agent_name_hint` for attribution.
                        "name": siwc::AGENT_NAME,
                        "title": "BebekonCode",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "capabilities": { "experimentalApi": false },
                }),
                REQUEST_TIMEOUT,
            )
            .await;
        if let Err(error) = initialized {
            let detail = server.last_error();
            server.stop();
            return Err(provider_error(error, detail));
        }
        let _ = peer.notify("initialized", None).await;
        Ok(server)
    }

    /// Background routing for traffic that no running turn owns: account notifications and
    /// server requests for idle threads, which are declined so Codex never waits forever.
    fn route(&self, account_id: String, server: Arc<AppServer>) {
        let mut incoming = server.peer.subscribe();
        let events = self.account_events.clone();
        let closed = server.peer.closed();
        tokio::spawn(async move {
            loop {
                let message = tokio::select! {
                    _ = closed.cancelled() => break,
                    message = incoming.recv() => message,
                };
                let message = match message {
                    Ok(message) => message,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                match message {
                    Incoming::Notification { method, params } => {
                        if let Some(event) = account_notification(&account_id, &method, &params) {
                            let _ = events.send(event);
                        }
                    }
                    Incoming::Request { id, method, params } => {
                        let thread = str_at(&params, "threadId").map(str::to_string);
                        let owned = thread.as_ref().is_some_and(|thread| {
                            server
                                .active
                                .lock()
                                .map(|active| active.contains(thread))
                                .unwrap_or(false)
                        });
                        if owned {
                            continue;
                        }
                        let _ = decline_unhandled(&server.peer, id, &method).await;
                    }
                }
            }
        });
    }

    async fn open_thread(
        &self,
        server: &AppServer,
        request: &TurnRequest,
        events: &mpsc::Sender<EventPayload>,
    ) -> Result<String> {
        let session = &request.session;
        let read_only = session.permission_profile == "read_only";
        let full_access = session.permission_profile == "full_access";
        let policy: crate::model::ToolPolicy = serde_json::from_str(&session.tool_policy)?;
        let extensions = if policy.mcp_servers.is_some() || policy.skills.is_some() {
            Some(self.extensions(&request.account).await?)
        } else {
            None
        };
        let mut config = serde_json::Map::new();
        if let Some(extensions) = &extensions {
            if !extensions.errors.is_empty() {
                return Err(CoreError::Invalid(
                    "Не удалось применить инструменты чата".into(),
                ));
            }
            if let Some(selected) = &policy.mcp_servers {
                for item in &extensions.mcp_servers {
                    config.insert(
                        format!("mcp_servers.{:?}.enabled", item.id),
                        json!(selected.contains(&item.id) && item.enabled),
                    );
                }
            }
            if let Some(selected) = &policy.skills {
                config.insert("skills.config".into(), json!(extensions.skills.iter().map(|item| json!({"path": std::path::Path::new(&item.id).parent().unwrap_or(std::path::Path::new(&item.id)).to_string_lossy(), "enabled": selected.contains(&item.id) && item.enabled})).collect::<Vec<_>>()));
            }
        }
        let common = json!({
            "cwd": plain_path(&session.working_directory),
            "model": session.model,
            // Full access is chosen explicitly per chat; resume re-applies the current mode.
            "approvalPolicy": if full_access { "never" } else { "on-request" },
            "sandbox": if read_only {
                "read-only"
            } else if full_access {
                "danger-full-access"
            } else {
                "workspace-write"
            },
            "config": config,
        });
        if let Some(thread) = &session.provider_session_id {
            // Resume re-applies thread-local extension overrides, including returning to defaults.
            let mut params = common.clone();
            params["threadId"] = json!(thread);
            match server
                .peer
                .request("thread/resume", params, REQUEST_TIMEOUT)
                .await
            {
                Ok(_) => {
                    if let Ok(mut loaded) = server.loaded.lock() {
                        loaded.insert(thread.clone());
                    }
                    return Ok(thread.clone());
                }
                // Do not discard history on an invalid override or an unavailable saved thread.
                // The user can explicitly hand off into a new conversation with a summary.
                Err(error) => return Err(provider_error(error, server.last_error())),
            }
        }
        let started = server
            .peer
            .request("thread/start", common, REQUEST_TIMEOUT)
            .await
            .map_err(|error| provider_error(error, server.last_error()))?;
        let thread = started
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::Provider {
                message: "Codex не вернул идентификатор потока".into(),
                kind: None,
            })?
            .to_string();
        if let Ok(mut loaded) = server.loaded.lock() {
            loaded.insert(thread.clone());
        }
        let _ = events
            .send(EventPayload::ProviderSession { id: thread.clone() })
            .await;
        Ok(thread)
    }

    async fn handle_request(
        &self,
        server: &Arc<AppServer>,
        state: &TurnState,
        session_id: &str,
        read_only: bool,
        events: &mpsc::Sender<EventPayload>,
        request: ServerRequest,
    ) {
        let ServerRequest { id, method, params } = request;
        let params = &params;
        let (kind, title, detail) = match method.as_str() {
            "item/commandExecution/requestApproval" => {
                if let Some(host) = params
                    .pointer("/networkApprovalContext/host")
                    .and_then(Value::as_str)
                {
                    let protocol = params
                        .pointer("/networkApprovalContext/protocol")
                        .and_then(Value::as_str)
                        .unwrap_or("сеть");
                    (
                        "network",
                        "Codex запрашивает сетевой доступ",
                        format!("{protocol}: {host}"),
                    )
                } else {
                    let command = str_at(params, "command")
                        .map(str::to_string)
                        .or_else(|| {
                            params
                                .pointer("/networkApprovalContext/host")
                                .and_then(Value::as_str)
                                .map(|host| format!("Сетевой доступ к {host}"))
                        })
                        .unwrap_or_else(|| "Команда без описания".into());
                    ("command", "Codex хочет выполнить команду", command)
                }
            }
            "item/fileChange/requestApproval" => {
                let detail = str_at(params, "grantRoot")
                    .map(|root| format!("Разрешить запись в папку {root}"))
                    .or_else(|| {
                        str_at(params, "itemId")
                            .and_then(|item| state.file_change_paths(item))
                            .map(summarize_paths)
                    })
                    .unwrap_or_else(|| "Изменение файлов проекта".into());
                ("file_change", "Codex хочет изменить файлы", detail)
            }
            _ => {
                let _ = decline_unhandled(&server.peer, id, &method).await;
                let _ = events
                    .send(EventPayload::ToolActivity {
                        label: "Запрос Codex отклонён".into(),
                        detail: format!("BebekonCode пока не поддерживает «{method}»"),
                    })
                    .await;
                return;
            }
        };
        if read_only {
            let _ = decline_unhandled(&server.peer, id, &method).await;
            let _ = events.send(EventPayload::ToolActivity {
                label: "Дополнительный доступ отклонён".into(),
                detail: "В режиме «Только чтение» выход из песочницы и разрешение записи недоступны. Измените доступ агента перед следующим запуском задачи.".into(),
            }).await;
            return;
        }
        let approval = Uuid::new_v4().to_string();
        let available_decisions = sandbox::approval_decisions(params);
        if let Ok(mut approvals) = self.approvals.lock() {
            approvals.insert(
                approval.clone(),
                PendingApproval {
                    available_decisions: available_decisions.clone(),
                    session_id: session_id.to_string(),
                    server: Arc::clone(server),
                    rpc_id: id,
                    events: events.clone(),
                },
            );
        }
        let _ = events
            .send(EventPayload::ApprovalRequested {
                available_decisions,
                id: approval,
                kind: kind.into(),
                title: title.into(),
                detail: clip(&redact(&detail), 2000),
                cwd: str_at(params, "cwd").map(plain_path),
                reason: str_at(params, "reason").map(|reason| clip(&redact(reason), 600)),
            })
            .await;
    }

    /// Answers every approval still open for a session when its turn ends.
    async fn expire_approvals(&self, session_id: &str) {
        let expired: Vec<(String, PendingApproval)> = self
            .approvals
            .lock()
            .map(|mut approvals| {
                let ids: Vec<String> = approvals
                    .iter()
                    .filter(|(_, pending)| pending.session_id == session_id)
                    .map(|(id, _)| id.clone())
                    .collect();
                ids.into_iter()
                    .filter_map(|id| approvals.remove(&id).map(|pending| (id, pending)))
                    .collect()
            })
            .unwrap_or_default();
        for (id, pending) in expired {
            let _ = pending
                .server
                .peer
                .respond(pending.rpc_id, json!({ "decision": "cancel" }))
                .await;
            let _ = pending
                .events
                .send(EventPayload::ApprovalResolved {
                    id,
                    decision: "expired".into(),
                })
                .await;
        }
    }
}

#[async_trait]
impl AgentProvider for CodexProvider {
    fn info(&self) -> ProviderInfo {
        let detection = self.detection();
        let detail = match (&detection.binary, &detection.version) {
            (None, _) => {
                "Codex CLI не найден. Установите официальный CLI: npm install -g @openai/codex"
                    .to_string()
            }
            (Some(_), Some(version)) if !version.starts_with(TESTED_VERSION) => format!(
                "Codex CLI {version}. Проверено с {TESTED_VERSION}.x — протокол app-server экспериментальный, возможны расхождения."
            ),
            (Some(_), Some(version)) => format!("Codex CLI {version} · официальный app-server"),
            (Some(_), None) => "Codex CLI найден, версию определить не удалось".to_string(),
        };
        ProviderInfo {
            id: PROVIDER_ID.into(),
            name: "OpenAI / Codex".into(),
            available: detection.binary.is_some(),
            detected_path: detection
                .binary
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            detail,
            models: vec![],
        }
    }

    async fn refresh(&self) {
        let detection = tokio::task::spawn_blocking(detect)
            .await
            .unwrap_or_default();
        if let Ok(mut current) = self.detection.write() {
            *current = detection;
        }
    }

    fn manages_accounts(&self) -> bool {
        true
    }

    async fn models(&self, account: &AccountProfile) -> Result<Vec<ModelInfo>> {
        let server = self.server(account).await?;
        let mut models = Vec::new();
        let mut cursor = Value::Null;
        for _ in 0..5 {
            let page = server
                .peer
                .request("model/list", json!({ "cursor": cursor }), REQUEST_TIMEOUT)
                .await
                .map_err(|error| provider_error(error, server.last_error()))?;
            for model in page
                .get("data")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if model.get("hidden").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                let Some(id) = str_at(model, "model").or_else(|| str_at(model, "id")) else {
                    continue;
                };
                models.push(ModelInfo {
                    id: id.into(),
                    name: str_at(model, "displayName").unwrap_or(id).into(),
                    description: str_at(model, "description").unwrap_or_default().into(),
                    is_default: model.get("isDefault").and_then(Value::as_bool) == Some(true),
                    reasoning_efforts: model
                        .get("supportedReasoningEfforts")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|value| str_at(value, "reasoningEffort").map(str::to_string))
                        .collect(),
                    default_reasoning_effort: str_at(model, "defaultReasoningEffort")
                        .map(str::to_string),
                });
            }
            cursor = page.get("nextCursor").cloned().unwrap_or(Value::Null);
            if cursor.is_null() {
                break;
            }
        }
        Ok(models)
    }

    async fn account_status(&self, account: &AccountProfile) -> Result<AccountStatus> {
        let mut status = AccountStatus {
            account_id: account.id.clone(),
            checked_at: now(),
            manage_usage_url: Some(siwc::USAGE_URL.into()),
            ..AccountStatus::default()
        };
        if self.detection().binary.is_none() {
            status.state = "unavailable".into();
            status.message = Some("Codex CLI не найден".into());
            return Ok(status);
        }
        if native_auth::selected(account)? {
            status.auth_mode = Some("codex".into());
            let server = self.server(account).await?;
            native_auth::read(&server.peer, &mut status).await?;
            if status.state == "signed_in" {
                read_rate_limits(&server.peer, &mut status).await;
                status.sandbox = Some(sandbox::readiness(&server).await);
            }
            return Ok(status);
        }
        status.auth_mode = Some("siwc".into());
        let pending = self
            .logins
            .lock()
            .map(|logins| {
                logins
                    .get(&account.id)
                    .is_some_and(|task| !task.is_finished())
            })
            .unwrap_or(false);
        let credentials = siwc::load(&Self::profile(account)?)?;
        status.email = credentials.email.clone();
        status.plan = credentials.plan.clone();
        if !credentials.signed_in() {
            status.state = "signed_out".into();
            if pending {
                status.message = Some("Ожидание входа в браузере".into());
            }
            return Ok(status);
        }
        status.state = "signed_in".into();
        status.plan_usage_enabled = Some(credentials.plan_enabled());
        status.message = (!credentials.plan_enabled())
            .then(|| "Вход выполнен, но использование плана ChatGPT не разрешено.".into());
        if credentials.plan_enabled() {
            match self.server(account).await {
                Ok(server) => {
                    read_rate_limits(&server.peer, &mut status).await;
                    status.sandbox = Some(sandbox::readiness(&server).await);
                }
                Err(_) => {
                    status.usage_error =
                        Some("Вход выполнен. CLI пока недоступен для проверки лимитов.".into())
                }
            }
        }
        Ok(status)
    }

    async fn login(&self, account: &AccountProfile) -> Result<LoginStart> {
        if native_auth::selected(account)? {
            return native_auth::login(&self.server(account).await?.peer).await;
        }
        let profile = Self::profile(account)?;
        std::fs::create_dir_all(&profile)?;
        let provider_dir = profile
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| CoreError::Invalid("Некорректная папка профиля".into()))?;
        let host = siwc::host_id(provider_dir)?;
        let previous = siwc::load(&profile)?;
        let (url, pending) = siwc::begin(&previous, &host).await?;
        let lock = self.auth_lock(&account.id);
        let events = self.account_events.clone();
        let account_id = account.id.clone();
        let task = tokio::spawn(async move {
            let result = siwc::complete(pending, previous).await;
            let event = match result {
                Ok(credentials) => {
                    let _guard = lock.lock().await;
                    match siwc::save(&profile, &credentials) {
                        Ok(()) if credentials.plan_enabled() => ("login_completed", None),
                        Ok(()) => (
                            "login_completed",
                            Some("Вход выполнен, но разрешение на использование плана ChatGPT не выдано".to_string()),
                        ),
                        Err(error) => ("login_failed", Some(error.to_string())),
                    }
                }
                Err(error) => ("login_failed", Some(redact(&error.to_string()))),
            };
            let _ = events.send(AccountEvent {
                account_id,
                kind: event.0.into(),
                status: None,
                message: event.1,
            });
        });
        if let Ok(mut logins) = self.logins.lock() {
            if let Some(previous) = logins.insert(account.id.clone(), task.abort_handle()) {
                previous.abort();
            }
        }
        Ok(LoginStart { url })
    }

    async fn logout(&self, account: &AccountProfile) -> Result<()> {
        if native_auth::selected(account)? {
            let server = self.server(account).await?;
            server
                .peer
                .request("account/logout", Value::Null, REQUEST_TIMEOUT)
                .await
                .map_err(|e| provider_error(e, None))?;
            self.release_account(account).await;
        }
        self.release_account(account).await;
        let profile = Self::profile(account)?;
        let lock = self.auth_lock(&account.id);
        let _guard = lock.lock().await;
        let mut credentials = siwc::load(&profile)?;
        let confirmed = siwc::sign_out(&mut credentials).await;
        siwc::save(&profile, &credentials)?;
        if !confirmed {
            let _ = self.account_events.send(AccountEvent {
                account_id: account.id.clone(),
                kind: "notice".into(),
                status: None,
                message: Some(
                    "Вы вышли локально, но отзыв доступа на сервере OpenAI не подтверждён. Отключите BebekonCode в настройках ChatGPT."
                        .into(),
                ),
            });
        }
        Ok(())
    }

    async fn login_for_usage(&self, account: &AccountProfile) -> Result<LoginStart> {
        let mut servers = self.servers.lock().await;
        if let Some(server) = servers.get(&account.id) {
            if server.operation.try_lock().is_err()
                || server.active.lock().map(|a| !a.is_empty()).unwrap_or(true)
            {
                return Err(CoreError::Invalid(
                    "Остановите задачи аккаунта перед сменой способа входа".into(),
                ));
            }
        }
        if let Some(server) = servers.remove(&account.id) {
            server.stop();
        }
        if let Some(login) = self
            .logins
            .lock()
            .map_err(|_| CoreError::Busy)?
            .remove(&account.id)
        {
            login.abort();
        }
        native_auth::select(account)?;
        drop(servers);
        native_auth::login(&self.server(account).await?.peer).await
    }

    async fn plugins(&self, account: &AccountProfile) -> Result<plugins::PluginInventory> {
        plugins::list(self, account).await
    }

    async fn change_plugin(&self, account: &AccountProfile, id: &str, install: bool) -> Result<()> {
        self.plugin_operation(account, Some((id, install)), None)
            .await
    }

    async fn add_plugin_source(&self, account: &AccountProfile, source: &str) -> Result<()> {
        self.plugin_operation(account, None, Some(source)).await
    }

    async fn extensions(&self, account: &AccountProfile) -> Result<Extensions> {
        let server = self.server(account).await?;
        let mut extensions = Extensions::default();
        let request = |method: &'static str, params: Value| {
            let peer = Arc::clone(&server.peer);
            async move { peer.request(method, params, REQUEST_TIMEOUT).await }
        };
        let (mcp, skills) = tokio::join!(
            request("mcpServerStatus/list", json!({})),
            request("skills/list", json!({})),
        );
        // Use the documented CLI, never the production-forbidden app-server plugin/list.
        match plugins::installed(self, account).await {
            Ok(inventory) => {
                extensions.plugins = inventory
                    .entries
                    .into_iter()
                    .filter(|entry| entry.installed)
                    .map(|entry| ExtensionItem {
                        id: entry.id,
                        name: entry.name,
                        enabled: entry.enabled,
                        detail: entry.unavailable_reason.clone().or(entry.version),
                        status: entry.unavailable_reason.map(|_| "unsupported".into()),
                    })
                    .collect()
            }
            Err(error) => extensions.errors.push(error.to_string()),
        }
        match mcp {
            Ok(mut value) => {
                let mut pages = 0;
                loop {
                    for server in value
                        .get("data")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        let tools = server
                            .get("tools")
                            .and_then(Value::as_object)
                            .map(|tools| tools.len())
                            .unwrap_or(0);
                        extensions.mcp_servers.push(ExtensionItem {
                            id: str_at(server, "name").unwrap_or_default().into(),
                            name: str_at(server, "name").unwrap_or("MCP").into(),
                            detail: Some(format!("инструментов: {tools}")),
                            enabled: server.get("toolsError").is_none_or(Value::is_null),
                            status: server
                                .get("authStatus")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        });
                    }
                    let cursor = value.get("nextCursor").cloned().unwrap_or(Value::Null);
                    if cursor.is_null() {
                        break;
                    }
                    pages += 1;
                    if pages >= 10 {
                        extensions.errors.push(
                            "Слишком большой список MCP-серверов; выбор инструментов не применён"
                                .into(),
                        );
                        break;
                    }
                    match request("mcpServerStatus/list", json!({"cursor":cursor,"limit":100}))
                        .await
                    {
                        Ok(next) => value = next,
                        Err(_) => {
                            extensions
                                .errors
                                .push("Не удалось получить полный список MCP-серверов".into());
                            break;
                        }
                    }
                }
            }
            Err(_) => extensions
                .errors
                .push("Не удалось получить список MCP-серверов".into()),
        }
        match skills {
            Ok(value) => {
                for skill in value
                    .get("data")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .flat_map(|entry| {
                        entry
                            .get("skills")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                    })
                {
                    extensions.skills.push(ExtensionItem {
                        id: str_at(skill, "path").unwrap_or_default().into(),
                        name: str_at(skill, "name").unwrap_or("Навык").into(),
                        detail: str_at(skill, "shortDescription")
                            .or_else(|| str_at(skill, "description"))
                            .map(|text| clip(text, 160)),
                        enabled: skill.get("enabled").and_then(Value::as_bool) != Some(false),
                        status: str_at(skill, "scope").map(str::to_string),
                    });
                }
            }
            Err(_) => extensions
                .errors
                .push("Не удалось получить список навыков".into()),
        }
        Ok(extensions)
    }

    async fn run(
        &self,
        request: TurnRequest,
        events: mpsc::Sender<EventPayload>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if request.account.provider != PROVIDER_ID {
            return Err(CoreError::Invalid(
                "Аккаунт не относится к провайдеру".into(),
            ));
        }
        let server = self.server(&request.account).await?;
        let operation = tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            operation = server.operation.lock() => operation,
        };
        // Full access runs without the Windows sandbox, so its setup is not required.
        if request.session.permission_profile != "full_access" {
            sandbox::ensure_ready(&server).await?;
        }
        let thread = self.open_thread(&server, &request, &events).await?;
        let mut incoming = server.peer.subscribe();
        if let Ok(mut active) = server.active.lock() {
            if !active.insert(thread.clone()) {
                return Err(CoreError::Busy);
            }
        }
        drop(operation);
        let result = self
            .drive_turn(&server, &request, &thread, &events, &mut incoming, &cancel)
            .await;
        if let Ok(mut active) = server.active.lock() {
            active.remove(&thread);
        }
        self.expire_approvals(&request.session.id).await;
        result
    }

    async fn resolve_approval(
        &self,
        session_id: &str,
        approval_id: &str,
        decision: ApprovalDecision,
    ) -> Result<()> {
        let pending = {
            let mut approvals = self.approvals.lock().map_err(|_| CoreError::Busy)?;
            if approvals.get(approval_id).is_some_and(|pending| {
                pending.session_id == session_id
                    && pending
                        .available_decisions
                        .as_ref()
                        .is_some_and(|choices| !choices.contains(&decision))
            }) {
                return Err(CoreError::Invalid(
                    "Codex не предлагает это решение для данного запроса".into(),
                ));
            }
            match approvals.get(approval_id) {
                Some(pending) if pending.session_id == session_id => approvals.remove(approval_id),
                _ => None,
            }
        }
        .ok_or(CoreError::NotFound)?;
        let answer = match decision {
            ApprovalDecision::AllowOnce => "accept",
            ApprovalDecision::AllowSession => "acceptForSession",
            ApprovalDecision::Deny => "decline",
        };
        pending
            .server
            .peer
            .respond(pending.rpc_id, json!({ "decision": answer }))
            .await
            .map_err(|error| provider_error(error, None))?;
        let decision = serde_json::to_value(decision)?
            .as_str()
            .unwrap_or("deny")
            .to_string();
        let _ = pending
            .events
            .send(EventPayload::ApprovalResolved {
                id: approval_id.into(),
                decision,
            })
            .await;
        Ok(())
    }

    async fn setup_sandbox(&self, account: &AccountProfile) -> Result<SandboxStatus> {
        let server = self.server(account).await?;
        let result = sandbox::setup(&server).await;
        let _ = self.account_events.send(AccountEvent {
            account_id: account.id.clone(),
            kind: "updated".into(),
            status: None,
            message: None,
        });
        result
    }

    async fn release_account(&self, account: &AccountProfile) {
        if let Some(login) = self
            .logins
            .lock()
            .ok()
            .and_then(|mut logins| logins.remove(&account.id))
        {
            login.abort();
        }
        let server = self.servers.lock().await.remove(&account.id);
        if let Some(server) = server {
            server.stop();
        }
    }

    async fn shutdown(&self) {
        let servers: Vec<_> = self.servers.lock().await.drain().collect();
        for (_, server) in servers {
            server.stop();
        }
    }
}

impl CodexProvider {
    async fn plugin_operation(
        &self,
        account: &AccountProfile,
        change: Option<(&str, bool)>,
        source: Option<&str>,
    ) -> Result<()> {
        // Serializes mutations against app-server creation and account operations.
        let mut servers = self.servers.lock().await;
        let server = servers.get(&account.id).cloned();
        let _operation = if let Some(server) = &server {
            if server
                .active
                .lock()
                .map(|active| !active.is_empty())
                .unwrap_or(true)
            {
                return Err(CoreError::Busy);
            }
            Some(server.operation.try_lock().map_err(|_| CoreError::Busy)?)
        } else {
            None
        };
        let result = if let Some((id, install)) = change {
            plugins::change(self, account, id, install).await
        } else if let Some(source) = source {
            plugins::add_source(self, account, source).await
        } else {
            Err(CoreError::Invalid("Неизвестная операция".into()))
        };
        // CLI owns configuration writes. Next app-server reads the new configuration.
        if let Some(server) = servers.remove(&account.id) {
            server.stop();
        }
        result
    }

    async fn drive_turn(
        &self,
        server: &Arc<AppServer>,
        request: &TurnRequest,
        thread: &str,
        events: &mpsc::Sender<EventPayload>,
        incoming: &mut broadcast::Receiver<Incoming>,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let mut state = TurnState::new(thread);
        let effort = if let Some(effort) = &request.session.reasoning_effort {
            Some(effort.clone())
        } else {
            self.models(&request.account)
                .await?
                .into_iter()
                .find(|m| m.id == request.session.model)
                .and_then(|m| m.default_reasoning_effort)
        };
        let policy: crate::model::ToolPolicy = serde_json::from_str(&request.session.tool_policy)?;
        let disabled_plugins = if let Some(selected) = &policy.plugins {
            let extensions = self.extensions(&request.account).await?;
            if !extensions.errors.is_empty() {
                return Err(CoreError::Invalid(
                    "Не удалось применить выбранные плагины чата".into(),
                ));
            }
            Some(
                extensions
                    .plugins
                    .into_iter()
                    .filter(|item| !selected.contains(&item.id))
                    .map(|item| item.id)
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        let full_access = request.session.permission_profile == "full_access";
        let sandbox = if request.session.permission_profile == "read_only" {
            json!({"type":"readOnly", "networkAccess":false})
        } else if full_access {
            json!({"type":"dangerFullAccess"})
        } else {
            json!({"type":"workspaceWrite", "writableRoots":[plain_path(&request.session.working_directory)], "networkAccess":false})
        };
        let started = server
            .peer
            .request(
                "turn/start",
                json!({
                    "threadId": thread,
                    "input": std::iter::once(json!({"type":"text", "text":request.prompt})).chain(request.attachments.iter().filter(|file| file.mime.starts_with("image/")).map(|file| json!({"type":"localImage", "path":file.path}))).collect::<Vec<_>>(),
                    "model": request.session.model,
                    "effort": effort,
                    "approvalPolicy": if full_access { "never" } else { "on-request" },
                    "sandboxPolicy": sandbox,
                    "disabledPluginIds": disabled_plugins,
                    "outputSchema": request.output_schema,
                }),
                REQUEST_TIMEOUT,
            )
            .await
            .map_err(|error| provider_error(error, server.last_error()))?;
        state.turn_id = started
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .map(str::to_string);

        let closed = server.peer.closed();
        let mut interrupt_deadline: Option<tokio::time::Instant> = None;
        let mut structured_result: Option<String> = None;
        loop {
            let deadline = interrupt_deadline;
            let message = tokio::select! {
                _ = cancel.cancelled(), if interrupt_deadline.is_none() => {
                    if let Some(turn) = &state.turn_id {
                        let _ = server.peer.request(
                            "turn/interrupt",
                            json!({ "threadId": thread, "turnId": turn }),
                            REQUEST_TIMEOUT,
                        ).await;
                    }
                    interrupt_deadline = Some(tokio::time::Instant::now() + INTERRUPT_GRACE);
                    continue;
                }
                _ = async { tokio::time::sleep_until(deadline.unwrap_or_else(tokio::time::Instant::now)).await }, if deadline.is_some() => {
                    // Codex did not confirm the interrupt in time; the turn is treated as stopped.
                    return Ok(());
                }
                _ = closed.cancelled() => {
                    return Err(CoreError::Provider {
                        message: match server.last_error() {
                            Some(detail) => format!("Процесс Codex неожиданно завершился. {detail}"),
                            None => "Процесс Codex неожиданно завершился".into(),
                        },
                        kind: None,
                    });
                }
                message = incoming.recv() => message,
            };
            match message {
                Ok(Incoming::Notification { method, params }) => {
                    if request.output_schema.is_some()
                        && state.owns(&params)
                        && method == "item/completed"
                    {
                        if let Some(item) = params.get("item") {
                            if str_at(item, "type") == Some("agentMessage")
                                && str_at(item, "phase") != Some("commentary")
                            {
                                structured_result = str_at(item, "text").map(str::to_string);
                            }
                        }
                    }
                    for mapped in map_notification(&method, &params, &mut state) {
                        match mapped {
                            Mapped::Event(event) => {
                                if request.output_schema.is_some()
                                    && matches!(event, EventPayload::AssistantTextDelta { .. })
                                {
                                    continue;
                                }
                                if events.send(event).await.is_err() {
                                    return Ok(());
                                }
                            }
                            Mapped::Finished(TurnEnd::Completed) => {
                                if request.output_schema.is_some() {
                                    let text = structured_result.take().ok_or_else(|| {
                                        CoreError::Invalid(
                                            "Codex не вернул итоговый структурированный ответ"
                                                .into(),
                                        )
                                    })?;
                                    if events
                                        .send(EventPayload::AssistantTextDelta { text })
                                        .await
                                        .is_err()
                                    {
                                        return Ok(());
                                    }
                                }
                                return Ok(());
                            }
                            Mapped::Finished(TurnEnd::Interrupted) => return Ok(()),
                            Mapped::Finished(TurnEnd::Failed { message, kind }) => {
                                return Err(CoreError::Provider { message, kind })
                            }
                        }
                    }
                }
                Ok(Incoming::Request { id, method, params }) => {
                    if state.owns(&params) {
                        self.handle_request(
                            server,
                            &state,
                            &request.session.id,
                            request.session.permission_profile == "read_only",
                            events,
                            ServerRequest { id, method, params },
                        )
                        .await;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let _ = events
                        .send(EventPayload::ToolActivity {
                            label: "Часть событий пропущена".into(),
                            detail:
                                "Codex прислал слишком много событий подряд; итог хода сохранится"
                                    .into(),
                        })
                        .await;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(CoreError::Provider {
                        message: "Соединение с Codex закрыто".into(),
                        kind: None,
                    })
                }
            }
        }
    }
}

async fn decline_unhandled(
    peer: &RpcPeer,
    id: Value,
    method: &str,
) -> std::result::Result<(), RpcError> {
    match method {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            peer.respond(id, json!({ "decision": "decline" })).await
        }
        _ => {
            peer.respond_error(id, -32601, "Не поддерживается клиентом BebekonCode")
                .await
        }
    }
}

fn account_notification(account_id: &str, method: &str, params: &Value) -> Option<AccountEvent> {
    let event = |kind: &str, status: Option<AccountStatus>, message: Option<String>| AccountEvent {
        account_id: account_id.into(),
        kind: kind.into(),
        status,
        message,
    };
    match method {
        "account/login/completed" => {
            if params.get("success").and_then(Value::as_bool) == Some(true) {
                Some(event("login_completed", None, None))
            } else {
                Some(event(
                    "login_failed",
                    None,
                    str_at(params, "error").map(|error| clip(&redact(error), 300)),
                ))
            }
        }
        "account/updated" => Some(event("updated", None, None)),
        "account/rateLimits/updated" => {
            let mut status = AccountStatus {
                account_id: account_id.into(),
                state: "signed_in".into(),
                checked_at: now(),
                ..AccountStatus::default()
            };
            apply_rate_limits(params, &mut status);
            Some(event("usage", Some(status), None))
        }
        _ => None,
    }
}

async fn read_rate_limits(peer: &RpcPeer, status: &mut AccountStatus) {
    match peer
        .request("account/rateLimits/read", Value::Null, REQUEST_TIMEOUT)
        .await
    {
        Ok(value) => {
            apply_rate_limits(&value, status);
            if status.usage.is_empty() {
                status.usage_error = Some(
                    "Codex App Server не передал проценты лимитов для этого подключения.".into(),
                );
            }
        }
        Err(error) => {
            let detail = provider_error(error, None).to_string();
            status.usage_error = Some(format!(
                "Лимиты Codex недоступны для текущей авторизации. {detail}"
            ));
        }
    }
}

/// Reads the Codex rate-limit snapshot. Prefers the `codex` bucket when several are reported.
fn apply_rate_limits(value: &Value, status: &mut AccountStatus) {
    let snapshot = value
        .pointer("/rateLimitsByLimitId/codex")
        .filter(|snapshot| !snapshot.is_null())
        .or_else(|| value.get("rateLimits"))
        .unwrap_or(&Value::Null);
    for key in ["primary", "secondary"] {
        let Some(window) = snapshot.get(key).filter(|window| !window.is_null()) else {
            continue;
        };
        let Some(used) = window.get("usedPercent").and_then(Value::as_f64) else {
            continue;
        };
        if !(0.0..=100.0).contains(&used) {
            continue;
        }
        status.usage.push(UsageWindow {
            label: None,
            window_minutes: window.get("windowDurationMins").and_then(Value::as_i64),
            used_percent: used,
            resets_at: window.get("resetsAt").and_then(Value::as_i64),
            source: "codex".into(),
        });
    }
    if let Some(plan) = str_at(snapshot, "planType") {
        status.plan = Some(plan.to_string());
    }
    status.limit_reached = str_at(snapshot, "rateLimitReachedType").map(str::to_string);
    if let Some(credits) = snapshot.get("credits").filter(|value| !value.is_null()) {
        status.credits = if credits.get("unlimited").and_then(Value::as_bool) == Some(true) {
            Some("без ограничений".into())
        } else {
            str_at(credits, "balance").map(str::to_string)
        };
    }
}

fn provider_error(error: RpcError, detail: Option<String>) -> CoreError {
    let message = match error {
        RpcError::Server { message, .. } => format!("Codex: {}", redact(&message)),
        RpcError::Timeout => "Codex не ответил вовремя".into(),
        RpcError::Closed => match detail {
            Some(detail) => format!("Процесс Codex завершился. {detail}"),
            None => "Процесс Codex завершился".into(),
        },
    };
    CoreError::Provider {
        message,
        kind: None,
    }
}

/// Codex expects a normal Windows path, not the `\\?\` form used for boundary checks.
fn plain_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(path).to_string()
}

/// Finds the official Codex binary. For npm installs the native executable is used directly
/// with the same environment the npm launcher sets, avoiding an extra shell and Node process.
fn detect() -> Detection {
    let Ok(found) = which::which("codex") else {
        return Detection::default();
    };
    let mut detection = Detection {
        binary: Some(found.clone()),
        ..Detection::default()
    };
    #[cfg(windows)]
    if let Some(directory) = found.parent() {
        let root = directory.join("node_modules").join("@openai").join("codex");
        let triple = Path::new("vendor")
            .join("x86_64-pc-windows-msvc")
            .join("bin")
            .join("codex.exe");
        let candidates = [
            root.join("node_modules")
                .join("@openai")
                .join("codex-win32-x64")
                .join(&triple),
            directory
                .join("node_modules")
                .join("@openai")
                .join("codex-win32-x64")
                .join(&triple),
            root.join(&triple),
        ];
        if let Some(native) = candidates.into_iter().find(|path| path.is_file()) {
            detection.binary = Some(native);
            detection.managed_root = Some(root);
        }
    }
    detection.version = detection.binary.as_deref().and_then(version);
    detection
}

fn version(binary: &Path) -> Option<String> {
    let mut command = std::process::Command::new(binary);
    command
        .arg("--version")
        .env_clear()
        .envs(process::child_env(&[]));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command.output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .last()
        .filter(|value| value.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_paths_are_plain_for_codex() {
        assert_eq!(plain_path(r"\\?\C:\Work\app"), r"C:\Work\app");
        assert_eq!(plain_path(r"\\?\UNC\srv\share"), r"\\srv\share");
        assert_eq!(plain_path(r"D:\Work"), r"D:\Work");
    }

    #[test]
    fn rate_limits_prefer_the_codex_bucket_and_keep_reported_values() {
        let mut status = AccountStatus::default();
        apply_rate_limits(
            &json!({
                "rateLimits": {"primary": {"usedPercent": 99}},
                "rateLimitsByLimitId": {"codex": {
                    "planType": "plus",
                    "primary": {"usedPercent": 25, "windowDurationMins": 300, "resetsAt": 1800000000},
                    "secondary": {"usedPercent": 58, "windowDurationMins": 10080, "resetsAt": null},
                    "rateLimitReachedType": null,
                    "credits": {"hasCredits": false, "unlimited": false, "balance": null}
                }}
            }),
            &mut status,
        );
        assert_eq!(status.plan.as_deref(), Some("plus"));
        assert_eq!(status.usage.len(), 2);
        assert_eq!(status.usage[0].used_percent, 25.0);
        assert_eq!(status.usage[0].window_minutes, Some(300));
        assert_eq!(status.usage[1].window_minutes, Some(10080));
        assert_eq!(status.limit_reached, None);
    }

    #[test]
    fn missing_limits_are_not_invented() {
        let mut status = AccountStatus::default();
        apply_rate_limits(&json!({"rateLimits": {"primary": null}}), &mut status);
        assert!(status.usage.is_empty());
        assert!(status.plan.is_none());
    }

    #[tokio::test]
    async fn quota_refresh_requests_only_the_official_read_method_and_surfaces_errors() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        for fails in [false, true] {
            let (client, server) = tokio::io::duplex(8192);
            let (read, write) = tokio::io::split(client);
            let peer = RpcPeer::start(read, write);
            let job = tokio::spawn(async move {
                let (read, mut write) = tokio::io::split(server);
                let mut lines = BufReader::new(read).lines();
                let request: Value =
                    serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
                assert_eq!(request["method"], "account/rateLimits/read");
                let response = if fails {
                    json!({"id":request["id"],"error":{"code":-32600,"message":"ChatGPT authentication required"}})
                } else {
                    json!({"id":request["id"],"result":{"rateLimits":{"primary":{"usedPercent":27,"resetsAt":1800000000,"windowDurationMins":300}}}})
                };
                write
                    .write_all(format!("{response}\n").as_bytes())
                    .await
                    .unwrap();
            });
            let mut status = AccountStatus::default();
            read_rate_limits(&peer, &mut status).await;
            if fails {
                assert!(status.usage.is_empty());
                assert!(status
                    .usage_error
                    .unwrap()
                    .contains("authentication required"));
            } else {
                assert_eq!(status.usage[0].used_percent, 27.0);
                assert!(status.usage_error.is_none());
            }
            job.await.unwrap();
        }
    }

    #[test]
    fn login_failure_is_reported_as_an_account_event() {
        let event = account_notification(
            "a",
            "account/login/completed",
            &json!({"success": false, "error": "access_token=secret"}),
        )
        .expect("event");
        assert_eq!(event.kind, "login_failed");
        assert!(!event.message.unwrap_or_default().contains("secret"));
    }
}
