//! Unmodified Claude Code CLI. Authentication is owned by the CLI, never imported.
mod access;
use crate::provider_diagnostics as diagnostics;
mod guard;
mod mapping;
pub mod rules;
#[cfg(test)]
mod tests;
mod tools;
mod usage;

use crate::{
    error::CoreError,
    model::*,
    process,
    provider::{AgentProvider, TurnRequest},
    Result,
};
use async_trait::async_trait;
pub use guard::hook_entry;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MIN_VERSION: &str = "2.1.293";
// Replayed user messages can contain 6 MiB of images: 8 MiB base64 plus text/envelope.
const MAX_LINE: u64 = 9 * 1024 * 1024;
pub const USAGE_URL: &str = "https://claude.ai/settings/usage";

#[derive(Default, Clone)]
struct Detection {
    binary: Option<PathBuf>,
    version: Option<String>,
    #[cfg(test)]
    prefix: Vec<String>,
}

pub(super) struct Pending {
    session: String,
    allow_session: bool,
    answer: oneshot::Sender<ApprovalDecision>,
}

pub struct ClaudeProvider {
    detection: RwLock<Detection>,
    notifications: broadcast::Sender<AccountEvent>,
    pending: Arc<Mutex<HashMap<String, Pending>>>,
    grants: guard::Grants,
    running: Mutex<HashMap<String, (String, CancellationToken)>>,
    logins: Mutex<HashMap<String, CancellationToken>>,
}

impl ClaudeProvider {
    pub fn new(notifications: broadcast::Sender<AccountEvent>) -> Self {
        Self {
            detection: RwLock::default(),
            notifications,
            pending: Arc::default(),
            grants: Arc::default(),
            running: Mutex::default(),
            logins: Mutex::default(),
        }
    }
    fn detection(&self) -> Detection {
        self.detection.read().map(|d| d.clone()).unwrap_or_default()
    }
    fn ready(&self) -> Result<Detection> {
        let d = self.detection();
        if d.binary.is_none() || !version_supported(d.version.as_deref()) || !cfg!(windows) {
            return Err(CoreError::Provider { message: format!("Нужен официальный нативный Claude Code {MIN_VERSION}+ для Windows. Установите CLI или выполните claude update и обновите список провайдеров."), kind: None });
        }
        Ok(d)
    }
    fn command(&self, account: Option<&AccountProfile>) -> Result<Command> {
        let detection = self.detection();
        let binary = detection.binary.ok_or(CoreError::ProviderUnavailable)?;
        let mut command = Command::new(binary);
        #[cfg(test)]
        command.args(detection.prefix);
        let mut env = vec![
            ("DISABLE_TELEMETRY", OsString::from("1")),
            ("DISABLE_ERROR_REPORTING", OsString::from("1")),
            (
                "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
                OsString::from("1"),
            ),
        ];
        if let Some(account) = account {
            if account.provider != "anthropic" {
                return Err(CoreError::Invalid("Аккаунт не относится к Claude".into()));
            }
            let profile = account
                .config_dir
                .as_deref()
                .ok_or_else(|| CoreError::Invalid("Нет отдельного профиля Claude".into()))?;
            env.push(("CLAUDE_CONFIG_DIR", OsString::from(profile)));
            command.current_dir(profile);
        }
        process::harden(&mut command, process::child_env(&env));
        Ok(command)
    }
    async fn auth_status(&self, account: &AccountProfile) -> Result<AccountStatus> {
        self.ready()?;
        let mut command = self.command(Some(account))?;
        command.args(["auth", "status"]);
        let (success, bytes) = capture(command, Duration::from_secs(20)).await?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| failure("Claude CLI вернул неизвестный формат статуса", None))?;
        if let Some(directory) = value.get("configDirectory").and_then(Value::as_str) {
            let expected = account
                .config_dir
                .as_deref()
                .ok_or(CoreError::ProviderUnavailable)?;
            if Path::new(directory).canonicalize()? != Path::new(expected).canonicalize()? {
                return Err(failure(
                    "Claude CLI использует другой профиль; вход заблокирован",
                    None,
                ));
            }
        }
        // Only documented public status fields; never inspect credential files.
        let signed_in = success && value.get("loggedIn").and_then(Value::as_bool) == Some(true);
        Ok(AccountStatus {
            account_id: account.id.clone(),
            state: if signed_in { "signed_in" } else { "signed_out" }.into(),
            email: value
                .get("email")
                .and_then(Value::as_str)
                .map(str::to_string),
            plan: value
                .get("subscriptionType")
                .and_then(Value::as_str)
                .map(str::to_string),
            manage_usage_url: Some(USAGE_URL.into()),
            message: None,
            checked_at: now(),
            ..AccountStatus::default()
        })
    }
}

fn supports_fast_mode(model: &str) -> bool {
    matches!(
        model,
        "claude-opus-5-5" | "claude-opus-5" | "claude-opus-4-8"
    )
}

fn version_supported(version: Option<&str>) -> bool {
    version
        .and_then(|v| semver::Version::parse(v).ok())
        .is_some_and(|v| v >= semver::Version::parse(MIN_VERSION).expect("static version"))
}

pub(super) fn failure(message: &str, kind: Option<&str>) -> CoreError {
    CoreError::Provider {
        message: crate::redaction::redact(message)
            .chars()
            .take(1500)
            .collect(),
        kind: kind.map(str::to_string),
    }
}

struct OwnedChild {
    child: std::process::Child,
    group: Option<process::ProcessGroup>,
}
impl OwnedChild {
    fn spawn(command: &mut Command) -> Result<Self> {
        let mut child = command.spawn()?;
        let group = process::ProcessGroup::adopt(&child);
        if cfg!(windows) && group.is_none() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(failure(
                "Не удалось изолировать дерево процессов Claude",
                None,
            ));
        }
        Ok(Self { child, group })
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(group) = self.group.take() {
            group.terminate();
        }
        let _ = self.child.kill();
        let _ = self.child.try_wait();
    }
}

async fn capture(mut command: Command, deadline: Duration) -> Result<(bool, Vec<u8>)> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut owned = OwnedChild::spawn(&mut command)?;
    let stdout = owned.child.stdout.take().ok_or(CoreError::Busy)?;
    let (tx, rx) = oneshot::channel();
    std::thread::Builder::new()
        .name("claude-status".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout
                .take(MAX_LINE + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = tx.send(result);
        })?;
    let bytes = tokio::time::timeout(deadline, rx)
        .await
        .map_err(|_| failure("Claude CLI не ответил вовремя", None))?
        .map_err(|_| CoreError::Busy)??;
    if bytes.len() as u64 > MAX_LINE {
        return Err(failure("Слишком большой ответ CLI", None));
    }
    let end = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = owned.child.try_wait()? {
            return Ok((status.success(), bytes));
        }
        if tokio::time::Instant::now() > end {
            return Err(failure("Claude CLI не завершился", None));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[async_trait]
impl AgentProvider for ClaudeProvider {
    async fn fork_session(
        &self,
        account: &AccountProfile,
        session: &Session,
    ) -> Result<Option<String>> {
        let Some(id) = &session.provider_session_id else {
            return Ok(None);
        };
        if session.provider != "anthropic" || account.id != session.account_profile_id {
            return Err(CoreError::ProviderUnavailable);
        }
        Uuid::parse_str(id).map_err(|_| CoreError::NotFound)?;
        let node = which::which("node")
            .map_err(|_| failure("Для ветки Claude требуется Node.js 18+", None))?;
        let directory = std::env::temp_dir().join(format!("bebekon-session-{}", Uuid::new_v4()));
        std::fs::create_dir(&directory)?;
        let file = directory.join("session.mjs");
        std::fs::write(
            &file,
            include_str!(concat!(env!("OUT_DIR"), "/claude-session.mjs")),
        )?;
        let profile = account
            .config_dir
            .as_deref()
            .ok_or(CoreError::ProviderUnavailable)?;
        let mut command = Command::new(node);
        command
            .arg(&file)
            .arg(id)
            .arg(&session.working_directory)
            .current_dir(&session.working_directory);
        process::harden(
            &mut command,
            process::child_env(&[
                ("CLAUDE_CONFIG_DIR", OsString::from(profile)),
                ("DISABLE_TELEMETRY", OsString::from("1")),
                ("DISABLE_ERROR_REPORTING", OsString::from("1")),
            ]),
        );
        let result = capture(command, Duration::from_secs(20)).await;
        let _ = std::fs::remove_file(file);
        let _ = std::fs::remove_dir(directory);
        let (ok, bytes) = result?;
        if !ok {
            return Err(failure(
                "Официальный SDK не создал ветку сессии Claude",
                None,
            ));
        }
        let value: Value = serde_json::from_slice(&bytes)?;
        let fork = value["session_id"].as_str().ok_or(CoreError::NotFound)?;
        Uuid::parse_str(fork).map_err(|_| CoreError::NotFound)?;
        if fork == id {
            return Err(failure("Ветка должна иметь отдельный ID", None));
        }
        Ok(Some(fork.into()))
    }
    fn info(&self) -> ProviderInfo {
        let d = self.detection();
        let available =
            cfg!(windows) && d.binary.is_some() && version_supported(d.version.as_deref());
        ProviderInfo {
            id: "anthropic".into(),
            name: "Claude · официальный Claude Code CLI".into(),
            available,
            detected_path: d.binary.map(|p| p.to_string_lossy().into_owned()),
            detail: if available {
                format!("Claude Code {}", d.version.unwrap_or_default())
            } else {
                format!("Нужен Claude Code {MIN_VERSION}+: выполните claude update")
            },
            models: vec!["sonnet".into(), "opus".into(), "haiku".into()],
        }
    }
    async fn refresh(&self) {
        #[cfg(test)]
        if !self.detection().prefix.is_empty() {
            return;
        }
        let binary = which::which("claude").ok().filter(|p| {
            if cfg!(windows) {
                p.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            } else {
                true
            }
        });
        if let Ok(mut detection) = self.detection.write() {
            *detection = Detection {
                binary,
                ..Detection::default()
            };
        }
        let version = if let Ok(mut command) = self.command(None) {
            command.arg("--version");
            capture(command, Duration::from_secs(10))
                .await
                .ok()
                .filter(|(ok, _)| *ok)
                .and_then(|(_, bytes)| String::from_utf8(bytes).ok())
                .and_then(|text| text.split_whitespace().next().map(str::to_string))
        } else {
            None
        };
        if let Ok(mut detection) = self.detection.write() {
            detection.version = version;
        }
    }
    fn manages_accounts(&self) -> bool {
        true
    }
    async fn account_status(&self, account: &AccountProfile) -> Result<AccountStatus> {
        if self.ready().is_err() {
            return Ok(AccountStatus {
                account_id: account.id.clone(),
                state: "unavailable".into(),
                message: Some(self.info().detail),
                checked_at: now(),
                ..AccountStatus::default()
            });
        }
        let mut status = self.auth_status(account).await?;
        if status.state == "signed_in" {
            if let Err(error) = usage::read(self, account, &mut status).await {
                status.usage_error = Some(error.to_string());
            }
        }
        Ok(status)
    }
    async fn models(&self, _account: &AccountProfile) -> Result<Vec<ModelInfo>> {
        self.ready()?;
        Ok([
            ("sonnet", "Claude Sonnet · авто"),
            ("opus", "Claude Opus · авто"),
            ("haiku", "Claude Haiku · авто"),
            ("claude-fable-5-1[1m]", "Claude Fable 5.1"),
            ("claude-opus-5-5", "Claude Opus 5.5"),
            ("claude-sonnet-5-5", "Claude Sonnet 5.5"),
            ("claude-haiku-5-5", "Claude Haiku 5.5"),
            ("claude-opus-5", "Claude Opus 5"),
            ("claude-sonnet-5", "Claude Sonnet 5"),
            ("claude-opus-4-8", "Claude Opus 4.8"),
            ("claude-opus-4-7", "Claude Opus 4.7"),
            ("claude-opus-4-6", "Claude Opus 4.6"),
            ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
            ("claude-haiku-4-5", "Claude Haiku 4.5"),
        ]
        .into_iter()
        .map(|(id, name)| ModelInfo {
            id: id.into(),
            name: name.into(),
            description: if ["sonnet", "opus", "haiku"].contains(&id) {
                "Последняя версия"
            } else {
                ""
            }
            .into(),
            is_default: id == "sonnet",
            reasoning_efforts: if id == "claude-haiku-4-5" {
                vec![]
            } else if ["claude-opus-4-6", "claude-sonnet-4-6"].contains(&id) {
                ["low", "medium", "high", "max"]
                    .map(str::to_string)
                    .to_vec()
            } else {
                ["low", "medium", "high", "xhigh", "max"]
                    .map(str::to_string)
                    .to_vec()
            },
            default_reasoning_effort: None,
            fast_mode_available: supports_fast_mode(id),
        })
        .collect())
    }
    async fn login(&self, account: &AccountProfile) -> Result<LoginStart> {
        self.ready()?;
        let mut logins = self.logins.lock().map_err(|_| CoreError::Busy)?;
        if logins
            .get(&account.id)
            .is_some_and(|token| !token.is_cancelled())
        {
            return Err(CoreError::Busy);
        }
        let cancel = CancellationToken::new();
        let mut command = self.command(Some(account))?;
        command
            .args(["auth", "login"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut owned = OwnedChild::spawn(&mut command)?;
        logins.insert(account.id.clone(), cancel.clone());
        let notifications = self.notifications.clone();
        let account_id = account.id.clone();
        tokio::spawn(async move {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
            let success = loop {
                if cancel.is_cancelled() || tokio::time::Instant::now() > deadline {
                    break false;
                }
                match owned.child.try_wait() {
                    Ok(Some(status)) => break status.success(),
                    Err(_) => break false,
                    _ => {}
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            };
            cancel.cancel();
            let _ = notifications.send(AccountEvent { account_id, kind: if success { "login_completed" } else { "login_failed" }.into(), status: None, message: (!success).then(|| "Вход Claude не завершён. Повторите вход или войдите через claude auth login в папке профиля аккаунта.".into()) });
        });
        // The unmodified CLI opens its own official browser flow. No URL/token is intercepted.
        Ok(LoginStart { url: String::new() })
    }
    async fn logout(&self, account: &AccountProfile) -> Result<()> {
        self.release_account(account).await;
        let mut command = self.command(Some(account))?;
        command.args(["auth", "logout"]);
        let (success, _) = capture(command, Duration::from_secs(20)).await?;
        if !success {
            return Err(failure("Claude CLI не подтвердил выход", None));
        }
        Ok(())
    }
    async fn extensions(&self, account: &AccountProfile) -> Result<Extensions> {
        access::inventory(account)
    }
    async fn run(
        &self,
        request: TurnRequest,
        events: mpsc::Sender<EventPayload>,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.ready()?;
        if request.account.provider != "anthropic"
            || request.session.provider != "anthropic"
            || request.session.account_profile_id != request.account.id
        {
            return Err(CoreError::Invalid(
                "Нарушена привязка аккаунта Claude".into(),
            ));
        }
        access::Access::for_request(&request)?;
        // Automatic and bypass modes are explicit per-chat choices; defaults remain guarded.
        crate::permissions::validate_profile("anthropic", &request.session.permission_profile)?;
        if request.session.permission_profile == "claude_auto"
            && request.session.model == "claude-haiku-4-5"
        {
            return Err(failure("Auto недоступен для Claude Haiku 4.5", None));
        }
        if !self.models(&request.account).await?.iter().any(|model| {
            model.id == request.session.model
                && request
                    .session
                    .reasoning_effort
                    .as_ref()
                    .is_none_or(|effort| model.reasoning_efforts.contains(effort))
        }) {
            return Err(CoreError::Invalid(
                "Неизвестная модель или уровень рассуждения Claude".into(),
            ));
        }
        let key = Uuid::new_v4().to_string();
        self.running
            .lock()
            .map_err(|_| CoreError::Busy)?
            .insert(key.clone(), (request.account.id.clone(), cancel.clone()));
        let (observed_tx, observed_rx) = mpsc::channel(64);
        let operation = async {
            let result = async {
                let status = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return Ok(()),
                    status = self.auth_status(&request.account) => status?,
                };
                if status.state != "signed_in" {
                    return Err(failure(
                        "Войдите в аккаунт Claude в настройках",
                        Some("auth"),
                    ));
                }
                self.drive(&request, observed_tx.clone(), cancel.clone())
                    .await
            }
            .await;
            guard::expire(&self.pending, &request.session.id, &observed_tx).await;
            drop(observed_tx);
            result
        };
        let started = std::time::Instant::now();
        let (result, mut report) = tokio::join!(
            operation,
            diagnostics::forward(&request, &key, observed_rx, &events, &cancel)
        );
        report.elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        report.status = if cancel.is_cancelled() {
            "stopped"
        } else if result.is_ok() {
            "completed"
        } else {
            "failed"
        }
        .into();
        if let Err(CoreError::Provider { kind, .. }) = &result {
            report.error_kind = kind.clone();
        }
        diagnostics::finish(&events, report, &cancel).await;
        self.running
            .lock()
            .map_err(|_| CoreError::Busy)?
            .remove(&key);
        result
    }
    async fn resolve_approval(
        &self,
        session_id: &str,
        id: &str,
        decision: ApprovalDecision,
    ) -> Result<()> {
        let pending = {
            let mut pending = self.pending.lock().map_err(|_| CoreError::Busy)?;
            if pending.get(id).is_some_and(|p| p.session == session_id) {
                if decision == ApprovalDecision::AllowSession
                    && pending.get(id).is_some_and(|p| !p.allow_session)
                {
                    return Err(CoreError::Invalid(
                        "Этот инструмент требует подтверждения каждого вызова".into(),
                    ));
                }
                pending.remove(id)
            } else {
                None
            }
        }
        .ok_or(CoreError::NotFound)?;
        pending
            .answer
            .send(decision)
            .map_err(|_| CoreError::NotFound)
    }
    async fn release_account(&self, account: &AccountProfile) {
        if let Ok(logins) = self.logins.lock() {
            if let Some(cancel) = logins.get(&account.id) {
                cancel.cancel();
            }
        }
        if let Ok(running) = self.running.lock() {
            for (id, cancel) in running.values() {
                if id == &account.id {
                    cancel.cancel();
                }
            }
        }
        // Signing out one account must not revoke unrelated accounts' file grants.
        if let Ok(mut grants) = self.grants.lock() {
            grants.retain(|(account_id, _, _)| account_id != &account.id);
        }
    }
    async fn shutdown(&self) {
        if let Ok(logins) = self.logins.lock() {
            for cancel in logins.values() {
                cancel.cancel();
            }
        }
        if let Ok(running) = self.running.lock() {
            for (_, cancel) in running.values() {
                cancel.cancel();
            }
        }
    }
}

impl ClaudeProvider {
    async fn drive(
        &self,
        request: &TurnRequest,
        events: mpsc::Sender<EventPayload>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let root = Path::new(&request.session.working_directory).canonicalize()?;
        let access = access::Access::for_request(request)?;
        let bridge = guard::Bridge::start(
            request,
            &access,
            events.clone(),
            cancel.child_token(),
            self.pending.clone(),
            self.grants.clone(),
        )
        .await?;
        if request.session.fast_mode && !supports_fast_mode(&request.session.model) {
            return Err(failure(
                "Скоростной режим недоступен для выбранной модели Claude",
                None,
            ));
        }
        // Native subagents (Explore etc.) keep the main context small, as in Claude Desktop.
        // Team members stay single-agent: the lead already coordinates them.
        let subagents =
            request.session.parent_session_id.is_none() && request.session.chat_mode == "single";
        let mut settings = guard::settings(
            &bridge.pipe,
            &std::env::current_exe()?,
            &request.session.permission_profile,
            subagents,
        );
        // Explicit per-session setting: never inherit an account's paid Fast preference.
        settings["fastMode"] = json!(request.session.fast_mode);
        let conversation = crate::conversation::ConversationSettings::read(&request.session)?;
        if let Some(tokens) = conversation.compact_tokens {
            settings["env"]["CLAUDE_CODE_AUTO_COMPACT_WINDOW"] = json!(tokens.to_string());
        }
        if let Some(percent) = conversation.claude_compact_percent {
            settings["env"]["CLAUDE_AUTOCOMPACT_PCT_OVERRIDE"] = json!(percent.to_string());
        }
        let rules = rules::read(&request.account, &request.session)?;
        if !rules.is_empty() && request.session.permission_profile == "standard" {
            settings["permissions"]["allow"] = json!(rules
                .iter()
                .map(|r| format!("{}(./{})", r.tool, r.path))
                .collect::<Vec<_>>());
            // The hook still asks on every file not explicitly approved by the user.
            settings["permissions"]["ask"] = json!(["Bash", "PowerShell", "mcp__*"]);
        }
        let mut command = self.command(Some(&request.account))?;
        command.current_dir(&root).args([
            "--print",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--input-format",
            "stream-json",
        ]);
        let live = request.live_input.is_some()
            && request.prompt != "/compact"
            && request.output_schema.is_none();
        if live {
            command.arg("--replay-user-messages");
        }
        let mode = guard::permission_mode(&request.session.permission_profile);
        if mode != "bypassPermissions" {
            command.arg("--restricted");
        }
        command.args([
            "--permission-mode",
            mode,
            "--setting-sources",
            "",
            "--strict-mcp-config",
            "--mcp-config",
            &access.mcp_config().to_string(),
            "--tools",
        ]);
        command.arg(if request.prompt == "/compact" {
            ""
        } else if matches!(
            request.session.permission_profile.as_str(),
            "read_only" | "plan"
        ) {
            "Read,Glob,Grep,TodoWrite,TaskCreate,TaskUpdate,TaskList,TaskGet"
        } else {
            "Read,Glob,Grep,Edit,Write,Bash,PowerShell,TodoWrite,TaskCreate,TaskUpdate,TaskList,TaskGet,TaskStop,TaskOutput,ToolSearch,Agent,Monitor,WebSearch,WebFetch"
        });
        if request.prompt != "/compact" {
            command.arg("--disable-slash-commands");
        }
        if access.mcp.is_empty() {
            command.args(["--disallowedTools", "mcp__*"]);
        }
        match access.skills_root() {
            Some(root) => {
                command.arg("--add-dir").arg(root);
            }
            None => {
                for directory in access.skills.values() {
                    command.arg("--add-dir").arg(directory);
                }
            }
        }
        if !access.skills.is_empty() {
            command.args(["--append-system-prompt", &access.guidance()]);
        }
        // CLI scrub forces default mode in supported native builds; child_env already
        // strips credentials. Do not silently override a requested automatic mode.
        if mode == "default" {
            command.env("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "1");
        }
        command.args([
            "--model",
            &request.session.model,
            "--settings",
            &settings.to_string(),
        ]);
        if let Some(effort) = &request.session.reasoning_effort {
            command.args(["--effort", effort]);
        }
        if let Some(id) = &request.session.provider_session_id {
            Uuid::parse_str(id).map_err(|_| {
                failure(
                    "Некорректный ID диалога Claude; автоматический новый чат не создаётся",
                    None,
                )
            })?;
            command.args(["--resume", id]);
        }
        if let Some(schema) = &request.output_schema {
            command.args(["--json-schema", &schema.to_string()]);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut owned = OwnedChild::spawn(&mut command)?;
        let stdout = owned.child.stdout.take().ok_or(CoreError::Busy)?;
        let stdin = owned.child.stdin.take().ok_or(CoreError::Busy)?;
        let prompt = input_line(
            &request.session,
            &request.prompt,
            &request.attachments,
            None,
        )?;
        let (tx, mut lines) = mpsc::channel::<Result<Value>>(64);
        let (input_sender, mut input_receiver) = mpsc::channel::<String>(2);
        let write_errors = tx.clone();
        std::thread::Builder::new()
            .name("claude-prompt".into())
            .spawn(move || {
                let mut stdin = stdin;
                while let Some(line) = input_receiver.blocking_recv() {
                    if stdin
                        .write_all(line.as_bytes())
                        .and_then(|_| stdin.flush())
                        .is_err()
                    {
                        let _ = write_errors.blocking_send(Err(failure(
                            "Не удалось передать сообщение Claude CLI; автоматического повтора нет",
                            None,
                        )));
                        break;
                    }
                }
            })?;
        input_sender
            .send(prompt)
            .await
            .map_err(|_| CoreError::Busy)?;
        let mut input_sender = if live {
            Some(input_sender)
        } else {
            drop(input_sender);
            None
        };
        let mut followups = if live {
            Some(
                request
                    .live_input
                    .as_ref()
                    .ok_or(CoreError::Busy)?
                    .lock()
                    .await,
            )
        } else {
            None
        };
        let mut pending: HashMap<String, oneshot::Sender<Result<()>>> = HashMap::new();
        let mut seen_results = HashSet::new();
        let mut last_result = false;
        std::thread::Builder::new()
            .name("claude-stream".into())
            .spawn(move || {
                let mut reader = std::io::BufReader::new(stdout);
                loop {
                    let mut line = Vec::new();
                    match reader
                        .by_ref()
                        .take(MAX_LINE + 1)
                        .read_until(b'\n', &mut line)
                    {
                        Ok(0) => {
                            let _ = tx.blocking_send(Ok(Value::Null));
                            break;
                        }
                        Ok(_) if line.len() as u64 <= MAX_LINE => {}
                        _ => {
                            let _ = tx
                                .blocking_send(Err(failure("Нарушен формат потока Claude", None)));
                            break;
                        }
                    }
                    let parsed = serde_json::from_slice(&line)
                        .map_err(|_| failure("Некорректный JSON от Claude CLI", None));
                    if tx.blocking_send(parsed).is_err() {
                        break;
                    }
                }
            })?;
        let mut state = mapping::StreamState::new(request.output_schema.is_some()).with_root(&root);
        let mut usage_emitted = false;
        loop {
            let next = tokio::select! {
                biased;
                _=cancel.cancelled()=>return Ok(()),
                next=lines.recv()=>next,
                followup=async { match &mut followups {Some(rx)=>rx.recv().await,None=>std::future::pending().await} }, if input_sender.is_some()=>{
                    if let Some(followup)=followup {
                        if pending.len()>=32 {let _=followup.acknowledged.send(Err(failure("Claude ещё не подтвердил предыдущие уточнения",None)));continue;}
                        let id=Uuid::new_v4().to_string();
                        let line=match input_line(&request.session,&followup.prompt,&followup.attachments,Some(&id)) {
                            Ok(line)=>line,Err(error)=>{let _=followup.acknowledged.send(Err(error));continue;}
                        };
                        let sender=input_sender.as_ref().ok_or(CoreError::Busy)?;
                        let sent=tokio::select! {biased;_=cancel.cancelled()=>return Ok(()),sent=sender.send(line)=>sent};
                        if sent.is_err() {let _=followup.acknowledged.send(Err(failure("Поток ввода Claude закрыт",None)));continue;}
                        pending.insert(id,followup.acknowledged);
                    } else {followups=None;}
                    continue;
                }
            };
            let Some(next) = next else { break };
            let value = next?;
            if value.is_null() {
                break;
            }
            let main = value.get("parent_tool_use_id").is_none_or(Value::is_null);
            if main && value["type"] == "user" {
                if let Some(id) = value["uuid"].as_str() {
                    if let Some(ack) = pending.remove(id) {
                        let _ = ack.send(Ok(()));
                    }
                }
            }
            let result = main && value["type"] == "result";
            if result {
                if let Some(id) = value["uuid"].as_str() {
                    if !seen_results.insert(id.to_string()) {
                        continue;
                    }
                    if seen_results.len() > 128 {
                        return Err(failure("Слишком много итогов Claude в одном запуске", None));
                    }
                }
            }
            if live
                && main
                && last_result
                && matches!(
                    value["type"].as_str(),
                    Some("assistant" | "stream_event" | "result")
                )
            {
                state = mapping::StreamState::new(false).with_root(&root);
                usage_emitted = false;
                last_result = false;
                let _ = events
                    .send(EventPayload::AssistantTextDelta {
                        text: "\n\n".into(),
                    })
                    .await;
            }
            if !usage_emitted {
                if let Some(usage) = mapping::reported_usage(&value) {
                    usage_emitted = true;
                    tokio::select! {_=cancel.cancelled()=>return Ok(()),sent=events.send(usage)=>{if sent.is_err(){return Ok(())}}}
                }
            }
            for payload in state.accept(value)? {
                tokio::select! {_=cancel.cancelled()=>return Ok(()),sent=events.send(payload)=>{if sent.is_err(){return Ok(())}}}
            }
            if result {
                last_result = true;
                // EOF drains messages already written to the native queue. Close the application
                // channel now, so a late correction cannot silently become another task.
                input_sender.take();
                if let Some(mut receiver) = followups.take() {
                    receiver.close();
                    while let Ok(followup) = receiver.try_recv() {
                        let _ = followup.acknowledged.send(Err(failure(
                            "Claude уже завершает ответ; уточнение не отправлено",
                            None,
                        )));
                    }
                }
            }
        }
        if !pending.is_empty() {
            return Err(failure(
                "Claude CLI не подтвердил получение части уточнений; автоматического повтора нет",
                None,
            ));
        }
        let end = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = owned.child.try_wait()? {
                if !status.success() {
                    return Err(failure(
                        "Claude CLI завершился с ошибкой. Проверьте вход, модель и лимит аккаунта.",
                        None,
                    ));
                }
                return state.finish();
            }
            if cancel.is_cancelled() {
                return Ok(());
            }
            if tokio::time::Instant::now() > end {
                return Err(failure("Claude CLI не завершился после ответа", None));
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

fn input_line(
    session: &Session,
    prompt: &str,
    files: &[crate::attachments::AttachedFile],
    id: Option<&str>,
) -> Result<String> {
    let mut content = vec![json!({"type":"text","text":prompt})];
    for file in files.iter().filter(|file| file.mime.starts_with("image/")) {
        let url = crate::attachments::image_data_url(session, file)?;
        let data = url.split_once(',').ok_or(CoreError::Busy)?.1;
        content.push(
            json!({"type":"image","source":{"type":"base64","media_type":file.mime,"data":data}}),
        );
    }
    let mut value = json!({"type":"user","message":{"role":"user","content":content},"parent_tool_use_id":null});
    if let Some(id) = id {
        value["uuid"] = json!(id);
    }
    Ok(format!("{value}\n"))
}
