//! Unmodified Claude Code CLI. Authentication is owned by the CLI, never imported.
mod access;
mod guard;
mod mapping;
#[cfg(test)]
mod tests;
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
    collections::HashMap,
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
const MAX_LINE: u64 = 8 * 1024 * 1024;
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
    grants: Arc<Mutex<std::collections::HashSet<(String, PathBuf)>>>,
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
        Ok(AccountStatus { account_id: account.id.clone(), state: if signed_in { "signed_in" } else { "signed_out" }.into(), email: value.get("email").and_then(Value::as_str).map(str::to_string), plan: value.get("subscriptionType").and_then(Value::as_str).map(str::to_string), manage_usage_url: Some(USAGE_URL.into()), message: Some("Авторизация хранится только официальным CLI в отдельном профиле. Навыки доступны. Полный доступ включается отдельно в чате: обычные правки и shell без вопросов, MCP с подтверждением каждого вызова. Песочницы ОС Windows нет. Плагины недоступны.".into()), checked_at: now(), ..AccountStatus::default() })
    }
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
                format!("Claude Code {}. Отдельный вход через Anthropic. Файлы и навыки; полный доступ включается для отдельного чата. В обычном и авторежиме shell с подтверждением; MCP с подтверждением в любом режиме. Без песочницы ОС Windows. Плагины недоступны.", d.version.unwrap_or_default())
            } else {
                format!("Нужен официальный нативный Claude Code {MIN_VERSION}+ для Windows. Выполните claude update и нажмите «Обновить».")
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
                "Псевдоним CLI: точная версия появится в ответе."
            } else {
                "Фиксированная версия. Доступность проверяет Anthropic при запросе."
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
        // Access is chosen per chat; only documented profiles are accepted.
        crate::permissions::validate_profile("anthropic", &request.session.permission_profile)?;
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
        let result = async {
            let status = tokio::select! { biased; _ = cancel.cancelled() => return Ok(()), status = self.auth_status(&request.account) => status? };
            if status.state != "signed_in" {
                return Err(failure("Войдите в аккаунт Claude в настройках", Some("auth")));
            }
            self.drive(&request, events.clone(), cancel.clone()).await
        }.await;
        self.running
            .lock()
            .map_err(|_| CoreError::Busy)?
            .remove(&key);
        guard::expire(&self.pending, &request.session.id, &events).await;
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
        // Grants are memory-only and are cleared on sign-out.
        if let Ok(mut grants) = self.grants.lock() {
            grants.clear();
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
        let settings = guard::settings(
            &bridge.pipe,
            &std::env::current_exe()?,
            &request.session.permission_profile,
        );
        let mode = guard::permission_mode(&request.session.permission_profile);
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
        // Restricted mode refuses bypassPermissions; full access keeps the hook and deny rules.
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
        command.arg(if request.session.permission_profile == "read_only" {
            "Read,Glob,Grep"
        } else {
            "Read,Glob,Grep,Edit,Write,Bash,PowerShell"
        });
        command.arg("--disable-slash-commands");
        if access.mcp.is_empty() {
            command.args(["--disallowedTools", "mcp__*"]);
        }
        for directory in access.skills.values() {
            command.arg("--add-dir").arg(directory);
        }
        if !access.skills.is_empty() {
            command.args([
                "--append-system-prompt",
                &access.guidance(mode == "bypassPermissions"),
            ]);
        }
        // CLI 2.1.294 forces the default permission mode while the scrub is on. The child
        // environment is already an allowlist, so the scrub is dropped only for automatic modes.
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
        let mut content = vec![json!({"type":"text", "text": request.prompt})];
        for file in request
            .attachments
            .iter()
            .filter(|file| file.mime.starts_with("image/"))
        {
            use base64::Engine;
            let bytes = std::fs::read(crate::files::resolve(
                &root,
                Path::new(&file.path)
                    .strip_prefix(&root)
                    .or_else(|_| {
                        Path::new(&file.path)
                            .strip_prefix(root.to_string_lossy().trim_start_matches(r"\\?\"))
                    })
                    .map_err(|_| failure("Вложение за пределами проекта", None))?
                    .to_string_lossy()
                    .replace('\\', "/")
                    .as_str(),
                false,
            )?)?;
            content.push(json!({"type":"image","source":{"type":"base64","media_type":file.mime,"data":base64::engine::general_purpose::STANDARD.encode(bytes)}}));
        }
        let prompt = format!(
            "{}\n",
            json!({"type":"user","message":{"role":"user","content":content}})
        );
        std::thread::Builder::new()
            .name("claude-prompt".into())
            .spawn(move || {
                let mut stdin = stdin;
                let _ = stdin.write_all(prompt.as_bytes());
            })?;
        let (tx, mut lines) = mpsc::channel::<Result<Value>>(64);
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
                        Ok(0) => break,
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
        let mut state = mapping::StreamState::new(request.output_schema.is_some());
        let deadline = tokio::time::sleep(Duration::from_secs(3600));
        tokio::pin!(deadline);
        loop {
            let next = tokio::select! { biased; _ = cancel.cancelled() => return Ok(()), _ = &mut deadline => return Err(failure("Claude превысил время выполнения запроса", None)), next = lines.recv() => next };
            let Some(next) = next else { break };
            let value = next?;
            if value["type"] == "system"
                && value["subtype"] == "init"
                && value["permissionMode"]
                    .as_str()
                    .is_some_and(|actual| actual != mode)
            {
                return Err(failure("Claude не применил выбранный режим доступа. Проверьте политику Claude Code; режим автоматически не заменяется.", None));
            }
            for payload in state.accept(value)? {
                tokio::select! { _ = cancel.cancelled() => return Ok(()), sent = events.send(payload) => { if sent.is_err() { return Ok(()); } } }
            }
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
