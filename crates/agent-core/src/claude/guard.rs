//! Documented hooks bridged through a local-only named pipe. No HTTP server or secrets.
use super::{failure, Pending, MAX_LINE};
use crate::{files, model::*, Result};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

type Approvals = Arc<Mutex<HashMap<String, Pending>>>;
pub(super) type Grants = Arc<Mutex<HashSet<(String, String, PathBuf)>>>;

/// A pending approval waits for the user; Stop still ends the run at once.
const HOOK_SECONDS: u64 = 7 * 24 * 3600;

/// Built-in tools without side effects outside the CLI: plans, task lists, tool search and
/// native subagents. Subagent tool calls still pass through this hook one by one.
const INTERNAL_TOOLS: [&str; 12] = [
    "TodoWrite",
    "TaskCreate",
    "TaskUpdate",
    "TaskList",
    "TaskGet",
    "TaskStop",
    "TaskOutput",
    "ToolSearch",
    "Agent",
    "Task",
    "ExitPlanMode",
    "EnterPlanMode",
];
/// Tools acting outside project files: shell, background monitors and the web.
const EXTERNAL_TOOLS: [&str; 5] = ["Bash", "PowerShell", "Monitor", "WebFetch", "WebSearch"];

pub(super) fn settings(pipe: &str, executable: &Path, profile: &str, subagents: bool) -> Value {
    let hook = json!({"type":"command", "command":executable, "args":["--claude-hook", pipe], "timeout":HOOK_SECONDS});
    let mut deny = vec![
        "Skill",
        "NotebookEdit",
        "Read(./.env*)",
        "Read(./**/.env*)",
        "Read(./**/*.key)",
        "Read(./**/*.pem)",
        "Read(./.git/**)",
        "Read(./.claude/**)",
        "Read(./**/.git/**)",
        "Read(./**/.claude/**)",
        "Read(./**/.codex/**)",
        "Read(./**/.ssh/**)",
        "Read(./**/.aws/**)",
        "Read(./**/.credentials.json)",
        "Read(./**/id_rsa)",
        "Read(./**/id_ed25519)",
        "Read(./**/*.pfx)",
        "Read(./**/*.p12)",
    ];
    if !subagents {
        deny.extend(["Agent", "Task"]);
    }
    if matches!(profile, "read_only" | "plan") {
        deny.extend(["Edit", "Write", "mcp__*", "Agent", "Task"]);
        deny.extend(EXTERNAL_TOOLS);
    }
    let automatic = matches!(profile, "workspace_auto" | "full_access" | "claude_auto");
    let ask = if automatic {
        vec![]
    } else {
        vec!["mcp__*", "Bash", "PowerShell", "Monitor", "Edit", "Write"]
    };
    // Only explicitly selected MCP servers are supplied by --strict-mcp-config.
    // dontAsk executes pre-authorized tools and rejects remaining permission requests.
    let allow = if profile == "workspace_auto" {
        vec![
            "Edit",
            "Write",
            "Bash",
            "PowerShell",
            "Monitor",
            "WebFetch",
            "WebSearch",
            "mcp__*",
        ]
    } else {
        vec![]
    };
    json!({"permissions":{"defaultMode":permission_mode(profile), "ask":ask, "allow":allow, "deny":deny}, "hooks": {"PreToolUse":[{"matcher":".*","hooks":[hook.clone()]}], "PermissionRequest":[{"matcher":".*","hooks":[hook]}]}, "enabledPlugins":{}, "disableAllHooks":false})
}

pub(super) fn permission_mode(profile: &str) -> &'static str {
    match profile {
        "workspace_auto" => "dontAsk",
        "full_access" => "bypassPermissions",
        "claude_auto" => "auto",
        "plan" => "plan",
        _ => "default",
    }
}

/// Path-like mentions of credential stores. Word boundaries keep code such as `process.env`
/// or `System.Environment` usable: those are not credential files.
pub(super) fn touches_credentials(text: &str) -> bool {
    let lower = text.to_lowercase();
    let word = |c: char| c.is_alphanumeric() || c == '_';
    [".credentials", ".env", ".ssh", ".aws", ".codex", ".claude"]
        .iter()
        .any(|needle| {
            lower.match_indices(needle).any(|(index, _)| {
                lower[..index].chars().next_back().is_none_or(|c| !word(c))
                    && lower[index + needle.len()..]
                        .chars()
                        .next()
                        .is_none_or(|c| !word(c))
            })
        })
        || ["cmdkey", "vaultcmd", "get-credential"]
            .iter()
            .any(|s| lower.contains(s))
}

fn external_preview(tool: &str, input: &Value) -> String {
    match tool {
        "Bash" | "PowerShell" | "Monitor" => input["command"].as_str().unwrap_or("").to_string(),
        "WebFetch" => input["url"].as_str().unwrap_or("").to_string(),
        "WebSearch" => input["query"].as_str().unwrap_or("").to_string(),
        _ => input.to_string(),
    }
}

fn allow_tool(event: &str) -> Value {
    if event == "PermissionRequest" {
        response(event, true)
    } else {
        json!({"hookSpecificOutput":{"hookEventName":"PreToolUse", "permissionDecision":"allow", "permissionDecisionReason":"Память Claude Code"}})
    }
}

fn response(event: &str, allowed: bool) -> Value {
    if event == "PermissionRequest" {
        let decision = if allowed {
            json!({"behavior":"allow"})
        } else {
            json!({"behavior":"deny", "message":"Запрос отклонён пользователем BebekonCode или ограничениями проекта"})
        };
        json!({"hookSpecificOutput":{"hookEventName":"PermissionRequest", "decision":decision}})
    } else {
        // The reason reaches the model: say plainly whether this is a question or a refusal.
        let reason = if allowed {
            "BebekonCode спросит пользователя"
        } else {
            "BebekonCode: закрыто режимом доступа чата (вне проекта, учётные данные или служебные файлы)"
        };
        json!({"hookSpecificOutput":{"hookEventName":"PreToolUse", "permissionDecision":if allowed {"ask"} else {"deny"}, "permissionDecisionReason":reason}})
    }
}

pub(super) fn checked_path(root: &Path, input: &Value, tool: &str) -> Result<PathBuf> {
    if !["Read", "Glob", "Grep", "Edit", "Write"].contains(&tool) {
        return Err(failure("Инструмент Claude недоступен", None));
    }
    if tool == "Glob" {
        let pattern = input["pattern"].as_str().unwrap_or("");
        if pattern.contains("..") || pattern.contains(':') || pattern.starts_with(['/', '\\']) {
            return Err(failure("Шаблон должен оставаться внутри проекта", None));
        }
    }
    let supplied = input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if ["Read", "Edit", "Write"].contains(&tool) && supplied.is_empty() {
        return Err(failure("Отсутствует путь файла", None));
    }
    // Windows may spell the same root with a short 8.3 alias or different case. Retain
    // the caller's spelling for lexical prefix stripping, then resolve the relative
    // suffix through the canonical project root and the existing reparse-point guard.
    let supplied_root = root;
    let root = root.canonicalize()?;
    let plain_root = root
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    let supplied = Path::new(supplied);
    let relative = if supplied.is_absolute() {
        supplied
            .strip_prefix(&root)
            .or_else(|_| supplied.strip_prefix(Path::new(&plain_root)))
            .or_else(|_| supplied.strip_prefix(supplied_root))
            .map_err(|_| failure("Путь за пределами проекта", None))?
    } else {
        supplied
    };
    let name = relative.to_string_lossy().replace('\\', "/");
    let name = name.strip_prefix("./").unwrap_or(&name);
    if name.split('/').any(|part| {
        let part = part.to_ascii_lowercase();
        [
            ".git",
            ".claude",
            ".codex",
            ".ssh",
            ".aws",
            ".credentials.json",
            "id_rsa",
            "id_ed25519",
        ]
        .contains(&part.as_str())
            || part.starts_with(".env")
            || [".pem", ".key", ".pfx", ".p12"]
                .iter()
                .any(|extension| part.ends_with(extension))
    }) {
        return Err(failure(
            "Доступ к учётным данным и служебным файлам запрещён",
            None,
        ));
    }
    files::resolve(&root, if name == "." { "" } else { name }, tool == "Write")
}

pub(super) async fn expire(
    pending: &Approvals,
    session: &str,
    events: &mpsc::Sender<EventPayload>,
) {
    let removed = if let Ok(mut pending) = pending.lock() {
        let ids: Vec<_> = pending
            .iter()
            .filter(|(_, p)| p.session == session)
            .map(|(id, _)| id.clone())
            .collect();
        ids.into_iter()
            .filter_map(|id| pending.remove(&id).map(|p| (id, p)))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    for (id, pending) in removed {
        let _ = pending.answer.send(ApprovalDecision::Deny);
        let _ = events
            .send(EventPayload::ApprovalResolved {
                id,
                decision: "deny".into(),
            })
            .await;
    }
}

pub(super) struct Bridge {
    pub pipe: String,
    stop: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.cancel();
        self.task.abort();
    }
}

#[cfg(windows)]
impl Bridge {
    pub async fn start(
        request: &super::TurnRequest,
        access: &super::access::Access,
        events: mpsc::Sender<EventPayload>,
        stop: CancellationToken,
        pending: Approvals,
        grants: Grants,
    ) -> Result<Self> {
        use tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt},
            net::windows::named_pipe::ServerOptions,
        };
        let pipe = format!(r"\\.\pipe\bebekon-claude-{}", Uuid::new_v4());
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(&pipe)?;
        let next_pipe = pipe.clone();
        let child_stop = stop.clone();
        let session = request.session.clone();
        let access = access.clone();
        let task = tokio::spawn(async move {
            loop {
                if tokio::select! { _ = child_stop.cancelled() => true, result = server.connect() => result.is_err() }
                {
                    break;
                }
                let next = match ServerOptions::new()
                    .reject_remote_clients(true)
                    .create(&next_pipe)
                {
                    Ok(next) => next,
                    Err(_) => break,
                };
                let connected = std::mem::replace(&mut server, next);
                let stop = child_stop.clone();
                let pending = pending.clone();
                let grants = grants.clone();
                let session = session.clone();
                let access = access.clone();
                let events = events.clone();
                tokio::spawn(async move {
                    let mut stream = tokio::io::BufReader::new(connected);
                    let mut bytes = Vec::new();
                    use tokio::io::AsyncReadExt;
                    let mut limited = (&mut stream).take(MAX_LINE + 1);
                    let read = tokio::select! { _ = stop.cancelled() => return, read = limited.read_until(b'\n', &mut bytes) => read };
                    if read.is_err() || bytes.len() as u64 > MAX_LINE {
                        return;
                    }
                    let Ok(input) = serde_json::from_slice::<Value>(&bytes) else {
                        return;
                    };
                    let answer = decide_with_access(
                        &session, input, &events, &stop, &pending, &grants, &access,
                    )
                    .await;
                    let mut answer = answer.to_string();
                    answer.push('\n');
                    let _ = stream.get_mut().write_all(answer.as_bytes()).await;
                    let _ = stream.get_mut().flush().await;
                });
            }
        });
        Ok(Self { pipe, stop, task })
    }
}
#[cfg(not(windows))]
impl Bridge {
    pub async fn start(
        _request: &super::TurnRequest,
        _access: &super::access::Access,
        _events: mpsc::Sender<EventPayload>,
        _stop: CancellationToken,
        _pending: Approvals,
        _grants: Grants,
    ) -> Result<Self> {
        Err(failure(
            "Защищённый адаптер Claude доступен только в Windows",
            None,
        ))
    }
}

#[cfg(test)]
pub(super) async fn decide(
    session: &Session,
    value: Value,
    events: &mpsc::Sender<EventPayload>,
    cancel: &CancellationToken,
    pending: &Approvals,
    grants: &Grants,
) -> Value {
    decide_with_access(
        session,
        value,
        events,
        cancel,
        pending,
        grants,
        &super::access::Access::default(),
    )
    .await
}

pub(super) async fn decide_with_access(
    session: &Session,
    value: Value,
    events: &mpsc::Sender<EventPayload>,
    cancel: &CancellationToken,
    pending: &Approvals,
    grants: &Grants,
    access: &super::access::Access,
) -> Value {
    let event = value["hook_event_name"].as_str().unwrap_or("");
    let tool = value["tool_name"].as_str().unwrap_or("");
    if !["PreToolUse", "PermissionRequest"].contains(&event) {
        return response(event, false);
    }
    if cancel.is_cancelled() {
        return response(event, false);
    }
    if event == "PermissionRequest"
        && matches!(
            session.permission_profile.as_str(),
            "workspace_auto" | "full_access"
        )
    {
        // Do not override provider/organization restrictions or wait for a user in a
        // non-interactive mode. Normal allowed calls resolve before this hook.
        return response(event, false);
    }
    let writable = ["standard", "workspace_auto", "full_access", "claude_auto"]
        .contains(&session.permission_profile.as_str());
    if INTERNAL_TOOLS.contains(&tool) {
        // Native deny rules still remove subagents from read-only, plan and team runs.
        return if event == "PreToolUse" {
            json!({})
        } else {
            response(event, session.permission_profile == "standard")
        };
    }
    // Claude Code's own per-project memory inside the bound account profile.
    if let Some(path) = access.memory_file(&value["tool_input"], tool) {
        let read = ["Read", "Glob", "Grep"].contains(&tool);
        return if (read || writable) && !touches_credentials(&path.to_string_lossy()) {
            allow_tool(event)
        } else {
            response(event, false)
        };
    }
    let external = EXTERNAL_TOOLS.contains(&tool) || access.is_mcp_tool(tool);
    let full = session.permission_profile == "full_access";
    let auto = session.permission_profile == "claude_auto";
    if external {
        if !writable {
            return response(event, false);
        }
        let preview = external_preview(tool, &value["tool_input"]);
        if preview.trim().is_empty() || touches_credentials(&preview) {
            return response(event, false);
        }
        if event == "PreToolUse" && (full || auto || session.permission_profile == "workspace_auto")
        {
            return json!({}); // Native allow/classifier/bypass and deny rules decide.
        }
        if event == "PreToolUse" {
            return response(event, true);
        }
        return manual_decision(session, value, None, events, cancel, pending, grants).await;
    }
    let checked = if full {
        access.checked_full_file(
            Path::new(&session.working_directory),
            &value["tool_input"],
            tool,
        )
    } else {
        access.checked_file(
            Path::new(&session.working_directory),
            &value["tool_input"],
            tool,
        )
    };
    let Ok(path) = checked else {
        return response(event, false);
    };
    if ["Read", "Glob", "Grep"].contains(&tool) {
        return if event == "PreToolUse" {
            json!({})
        } else {
            response(event, false)
        };
    }
    if !["standard", "workspace_auto", "full_access", "claude_auto"]
        .contains(&session.permission_profile.as_str())
        || cancel.is_cancelled()
    {
        return response(event, false);
    }
    if (full || auto) && event == "PreToolUse" {
        return json!({});
    }
    if session.permission_profile == "workspace_auto" {
        // File boundary checks precede the native permissions.allow rules.
        return json!({});
    }
    if event == "PreToolUse" {
        if session.permission_profile == "standard"
            && access.file_rules.iter().any(|rule| {
                rule.tool == tool
                    && checked_path(
                        Path::new(&session.working_directory),
                        &json!({"file_path":rule.path}),
                        tool,
                    )
                    .is_ok_and(|approved| approved == path)
            })
        {
            return json!({});
        } // The documented allow rule decides after all path/deny checks.
        return response(event, true);
    }
    manual_decision(session, value, Some(path), events, cancel, pending, grants).await
}

async fn manual_decision(
    session: &Session,
    value: Value,
    path: Option<PathBuf>,
    events: &mpsc::Sender<EventPayload>,
    cancel: &CancellationToken,
    pending: &Approvals,
    grants: &Grants,
) -> Value {
    let event = "PermissionRequest";
    let tool = value["tool_name"].as_str().unwrap_or("");
    if session.permission_profile == "standard"
        && path.as_ref().is_some_and(|path| {
            grants.lock().is_ok_and(|grants| {
                grants.contains(&(
                    session.account_profile_id.clone(),
                    session.id.clone(),
                    path.clone(),
                ))
            })
        })
    {
        return response(event, true);
    }
    let id = Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    if let Ok(mut pending) = pending.lock() {
        pending.insert(
            id.clone(),
            Pending {
                session: session.id.clone(),
                allow_session: path.is_some(),
                answer: tx,
            },
        );
    } else {
        return response(event, false);
    }
    let input = &value["tool_input"];
    let preview = if path.is_none() {
        external_preview(tool, input)
    } else if tool == "Edit" {
        format!(
            "Было:\n{}\n\nСтанет:\n{}",
            input["old_string"].as_str().unwrap_or(""),
            input["new_string"].as_str().unwrap_or("")
        )
    } else {
        input["content"].as_str().unwrap_or("").to_string()
    };
    let detail = format!(
        "{}\n\n{}",
        path.as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| tool.to_string()),
        crate::redaction::redact(&preview)
            .chars()
            .take(4000)
            .collect::<String>()
    );
    let sent = events
        .send(EventPayload::ApprovalRequested {
            available_decisions: if path.is_some() {
                None
            } else {
                Some(vec![ApprovalDecision::AllowOnce, ApprovalDecision::Deny])
            },
            id: id.clone(),
            kind: if path.is_some() {
                "file_change"
            } else {
                "command"
            }
            .into(),
            title: if path.is_none() {
                format!("Claude просит выполнить {tool}")
            } else {
                format!(
                    "Claude просит {} файл",
                    if tool == "Write" {
                        "записать"
                    } else {
                        "изменить"
                    }
                )
            },
            detail,
            cwd: Some(session.working_directory.clone()),
            reason: Some(
                if path.is_some() {
                    "Только этот файл в этом чате."
                } else {
                    "Один вызов. Без песочницы Windows."
                }
                .into(),
            ),
        })
        .await;
    let decision = if sent.is_err() {
        ApprovalDecision::Deny
    } else {
        // No deadline: the run waits for the user's decision or for Stop.
        tokio::select! { biased; _ = cancel.cancelled() => ApprovalDecision::Deny, result = rx => result.unwrap_or(ApprovalDecision::Deny) }
    };
    if let Ok(mut pending) = pending.lock() {
        pending.remove(&id);
    }
    if decision == ApprovalDecision::AllowSession {
        if let (Some(path), Ok(mut grants)) = (path, grants.lock()) {
            grants.insert((session.account_profile_id.clone(), session.id.clone(), path));
        }
    }
    let _ = events
        .send(EventPayload::ApprovalResolved {
            id,
            decision: serde_json::to_value(decision)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| "deny".into()),
        })
        .await;
    response(
        event,
        decision != ApprovalDecision::Deny && !cancel.is_cancelled(),
    )
}

/// Called before any desktop initialization. Every IPC/hook error returns an explicit deny.
pub fn hook_entry() -> bool {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--claude-hook") {
        return false;
    }
    let mut event = "PreToolUse".to_string();
    let answer = (|| -> Option<Value> {
        let pipe = args.next()?;
        if !pipe.starts_with(r"\\.\pipe\bebekon-claude-") {
            return None;
        }
        use std::io::Read;
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(MAX_LINE + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > MAX_LINE {
            return None;
        }
        let value: Value = serde_json::from_slice(&bytes).ok()?;
        event = value["hook_event_name"]
            .as_str()
            .unwrap_or("PreToolUse")
            .to_string();
        hook_exchange(&pipe, &value)
    })();
    let answer = answer.unwrap_or_else(|| response(&event, false));
    println!("{answer}");
    true
}

#[cfg(windows)]
fn hook_exchange(pipe: &str, value: &Value) -> Option<Value> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    runtime.block_on(async {
        let mut client = None;
        for _ in 0..50 {
            if let Ok(connected) = tokio::net::windows::named_pipe::ClientOptions::new().open(pipe)
            {
                client = Some(connected);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let mut client = client?;
        let mut request = value.to_string();
        request.push('\n');
        client.write_all(request.as_bytes()).await.ok()?;
        let mut reader = tokio::io::BufReader::new(client);
        let mut bytes = Vec::new();
        use tokio::io::AsyncReadExt;
        tokio::time::timeout(
            std::time::Duration::from_secs(HOOK_SECONDS),
            (&mut reader).take(65536).read_until(b'\n', &mut bytes),
        )
        .await
        .ok()?
        .ok()?;
        serde_json::from_slice(&bytes).ok()
    })
}
#[cfg(not(windows))]
fn hook_exchange(_pipe: &str, _value: &Value) -> Option<Value> {
    None
}
