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
type Grants = Arc<Mutex<HashSet<(String, PathBuf)>>>;

pub(super) fn settings(pipe: &str, executable: &Path, profile: &str) -> Value {
    let hook = json!({"type":"command", "command":executable, "args":["--claude-hook", pipe], "timeout":300});
    let mut deny = vec![
        "Bash",
        "PowerShell",
        "Monitor",
        "Agent",
        "Skill",
        "NotebookEdit",
        "WebFetch",
        "WebSearch",
        "mcp__*",
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
    if profile == "read_only" {
        deny.extend(["Edit", "Write"]);
    }
    json!({"permissions":{"defaultMode":if profile == "workspace_auto" { "acceptEdits" } else { "default" }, "ask":if profile == "workspace_auto" { vec![] } else { vec!["Edit","Write"] }, "deny":deny}, "hooks": {"PreToolUse":[{"matcher":".*","hooks":[hook.clone()]}], "PermissionRequest":[{"matcher":".*","hooks":[hook]}]}, "enabledPlugins":{}, "disableAllHooks":false})
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
        json!({"hookSpecificOutput":{"hookEventName":"PreToolUse", "permissionDecision":if allowed {"ask"} else {"deny"}, "permissionDecisionReason":"Запись требует подтверждения в BebekonCode; пути ограничены проектом"}})
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
                    let answer = decide(&session, input, &events, &stop, &pending, &grants).await;
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

pub(super) async fn decide(
    session: &Session,
    value: Value,
    events: &mpsc::Sender<EventPayload>,
    cancel: &CancellationToken,
    pending: &Approvals,
    grants: &Grants,
) -> Value {
    let event = value["hook_event_name"].as_str().unwrap_or("");
    let tool = value["tool_name"].as_str().unwrap_or("");
    if !["PreToolUse", "PermissionRequest"].contains(&event) {
        return response(event, false);
    }
    let Ok(path) = checked_path(
        Path::new(&session.working_directory),
        &value["tool_input"],
        tool,
    ) else {
        return response(event, false);
    };
    if ["Read", "Glob", "Grep"].contains(&tool) {
        return if event == "PreToolUse" {
            json!({})
        } else {
            response(event, false)
        };
    }
    if !["standard", "workspace_auto"].contains(&session.permission_profile.as_str())
        || cancel.is_cancelled()
    {
        return response(event, false);
    }
    if session.permission_profile == "workspace_auto" {
        return if event == "PreToolUse" {
            // Keep provider deny rules effective; acceptEdits decides after our path check.
            json!({})
        } else {
            // An unexpected provider approval is still forwarded to the user below.
            // Automatic edits should not normally produce PermissionRequest.
            return manual_decision(session, value, path, events, cancel, pending, grants).await;
        };
    }
    if event == "PreToolUse" {
        return response(event, true);
    }
    manual_decision(session, value, path, events, cancel, pending, grants).await
}

async fn manual_decision(
    session: &Session,
    value: Value,
    path: PathBuf,
    events: &mpsc::Sender<EventPayload>,
    cancel: &CancellationToken,
    pending: &Approvals,
    grants: &Grants,
) -> Value {
    let event = "PermissionRequest";
    let tool = value["tool_name"].as_str().unwrap_or("");
    if grants
        .lock()
        .is_ok_and(|grants| grants.contains(&(session.id.clone(), path.clone())))
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
                answer: tx,
            },
        );
    } else {
        return response(event, false);
    }
    let input = &value["tool_input"];
    let preview = if tool == "Edit" {
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
        path.display(),
        crate::redaction::redact(&preview)
            .chars()
            .take(4000)
            .collect::<String>()
    );
    let sent = events
        .send(EventPayload::ApprovalRequested {
            id: id.clone(),
            kind: "file_change".into(),
            title: format!(
                "Claude просит {} файл",
                if tool == "Write" {
                    "записать"
                } else {
                    "изменить"
                }
            ),
            detail,
            cwd: Some(session.working_directory.clone()),
            reason: Some(
                "Разрешение действует только внутри проекта. «Разрешить на сессию» относится только к этому файлу и хранится до выхода из аккаунта или перезапуска приложения. Команды оболочки недоступны.".into(),
            ),
        })
        .await;
    let decision = if sent.is_err() {
        ApprovalDecision::Deny
    } else {
        tokio::select! { biased; _ = cancel.cancelled() => ApprovalDecision::Deny, _ = tokio::time::sleep(std::time::Duration::from_secs(240)) => ApprovalDecision::Deny, result = rx => result.unwrap_or(ApprovalDecision::Deny) }
    };
    if let Ok(mut pending) = pending.lock() {
        pending.remove(&id);
    }
    if decision == ApprovalDecision::AllowSession {
        if let Ok(mut grants) = grants.lock() {
            grants.insert((session.id.clone(), path));
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
            std::time::Duration::from_secs(250),
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
