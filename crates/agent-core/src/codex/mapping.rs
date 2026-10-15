//! Translation of Codex app-server notifications into normalized timeline events.
//! Pure functions only, so protocol drift is caught by unit tests rather than in the UI.

use crate::{model::EventPayload, redaction::redact};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[cfg(test)]
#[test]
fn context_reports_latest_request_not_lifetime_usage_and_rejects_other_turns() {
    let mut state = TurnState::new("thread");
    state.turn_id = Some("current".into());
    let mut params = serde_json::json!({"threadId":"thread","turnId":"current","tokenUsage":{"total":{"totalTokens":9000000},"last":{"totalTokens":12500},"modelContextWindow":200000}});
    assert_eq!(
        map_notification("thread/tokenUsage/updated", &params, &mut state),
        vec![Mapped::Event(EventPayload::ContextUsage {
            used_tokens: Some(12500),
            window_tokens: Some(200000),
            source: "codex_last_request".into()
        })]
    );
    params["turnId"] = serde_json::json!("old");
    assert!(map_notification("thread/tokenUsage/updated", &params, &mut state).is_empty());
    let compact = serde_json::json!({"threadId":"thread","turnId":"current","item":{"type":"contextCompaction","id":"c"}});
    assert!(!map_notification("item/started", &compact, &mut state)
        .contains(&Mapped::Event(EventPayload::ContextCompacted {})));
    assert_eq!(
        map_notification("item/completed", &compact, &mut state),
        vec![Mapped::Event(EventPayload::ContextCompacted {})]
    );
}

#[derive(Debug, PartialEq)]
pub enum Mapped {
    Event(EventPayload),
    Finished(TurnEnd),
}

#[derive(Debug, PartialEq)]
pub enum TurnEnd {
    Completed,
    Interrupted,
    Failed {
        message: String,
        kind: Option<String>,
    },
}

/// Per-turn bookkeeping needed to render a coherent transcript.
#[derive(Debug)]
pub struct TurnState {
    pub thread_id: String,
    pub turn_id: Option<String>,
    streamed: HashSet<String>,
    commentary: HashSet<String>,
    completed: HashSet<String>,
    last_message: Option<String>,
    has_text: bool,
    file_changes: HashMap<String, Vec<String>>,
    tools_started: HashSet<String>,
    pub(super) usage: super::usage::Tracker,
}

impl TurnState {
    pub fn new(thread_id: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            turn_id: None,
            streamed: HashSet::new(),
            commentary: HashSet::new(),
            completed: HashSet::new(),
            last_message: None,
            has_text: false,
            file_changes: HashMap::new(),
            tools_started: HashSet::new(),
            usage: super::usage::Tracker::default(),
        }
    }

    /// Whether a notification or server request belongs to this thread and turn.
    pub fn owns(&self, params: &Value) -> bool {
        if params.get("threadId").and_then(Value::as_str) != Some(self.thread_id.as_str()) {
            return false;
        }
        match (
            self.turn_id.as_deref(),
            params
                .get("turnId")
                .or_else(|| params.pointer("/turn/id"))
                .and_then(Value::as_str),
        ) {
            (Some(ours), Some(theirs)) => ours == theirs,
            _ => true,
        }
    }

    /// Paths of a pending file change, used to describe a file-change approval.
    pub fn file_change_paths(&self, item_id: &str) -> Option<&[String]> {
        self.file_changes.get(item_id).map(Vec::as_slice)
    }

    fn text(&mut self, item: &str, text: &str) -> EventPayload {
        let mut chunk = String::new();
        if self.last_message.as_deref() != Some(item) {
            if self.has_text {
                chunk.push_str("\n\n");
            }
            self.last_message = Some(item.to_string());
        }
        chunk.push_str(text);
        self.has_text = true;
        EventPayload::AssistantTextDelta { text: chunk }
    }
}

pub fn map_notification(method: &str, params: &Value, state: &mut TurnState) -> Vec<Mapped> {
    if method == "thread/tokenUsage/updated" {
        if str_at(params, "threadId") != Some(state.thread_id.as_str()) {
            return vec![];
        }
        let (Some(ours), Some(theirs)) = (state.turn_id.as_deref(), str_at(params, "turnId"))
        else {
            return vec![];
        };
        if ours == theirs {
            state.usage.observe(&params["tokenUsage"]);
            let used_tokens = params["tokenUsage"]["last"]["totalTokens"]
                .as_u64()
                .filter(|n| *n <= 10_000_000);
            let window_tokens = params["tokenUsage"]["modelContextWindow"]
                .as_u64()
                .filter(|n| *n > 0 && *n <= 10_000_000);
            if used_tokens.is_some() || window_tokens.is_some() {
                return vec![Mapped::Event(EventPayload::ContextUsage {
                    used_tokens,
                    window_tokens,
                    source: "codex_last_request".into(),
                })];
            }
        }
        // Wrong-turn frames cannot establish a trustworthy cold-resume baseline.
        return vec![];
    }
    if !state.owns(params) {
        return vec![];
    }
    match method {
        "turn/started" => {
            if state.turn_id.is_none() {
                state.turn_id = params
                    .pointer("/turn/id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            vec![]
        }
        "model/rerouted" => str_at(params, "toModel")
            .map(|model| {
                vec![
                    Mapped::Event(EventPayload::ModelResolved {
                        model: clip(&redact(model), 120),
                    }),
                    Mapped::Event(EventPayload::ToolActivity {
                        label: "Маршрутизация модели Codex".into(),
                        detail: format!(
                            "Провайдер сообщил модель {}. Аккаунт не изменялся.",
                            clip(&redact(model), 120)
                        ),
                    }),
                ]
            })
            .unwrap_or_default(),
        "item/reasoning/summaryTextDelta" => str_at(params, "delta")
            .map(|text| {
                vec![Mapped::Event(EventPayload::ProgressDelta {
                    text: text.into(),
                })]
            })
            .unwrap_or_default(),
        "item/reasoning/summaryPartAdded" => vec![Mapped::Event(EventPayload::ProgressDelta {
            text: "\n\n".into(),
        })],
        "item/agentMessage/delta" => {
            let (Some(item), Some(delta)) = (str_at(params, "itemId"), str_at(params, "delta"))
            else {
                return vec![];
            };
            state.streamed.insert(item.to_string());
            if state.commentary.contains(item) {
                return vec![Mapped::Event(EventPayload::ProgressDelta {
                    text: delta.into(),
                })];
            }
            vec![Mapped::Event(state.text(item, delta))]
        }
        "item/started" => {
            if params["item"]["type"] == "contextCompaction" {
                return vec![Mapped::Event(EventPayload::ContextUsage {
                    used_tokens: None,
                    window_tokens: None,
                    source: "compacting".into(),
                })];
            }
            if let Some(item) = params.get("item") {
                if str_at(item, "type") == Some("agentMessage")
                    && str_at(item, "phase") == Some("commentary")
                {
                    if let Some(id) = str_at(item, "id") {
                        state.commentary.insert(id.into());
                    }
                }
                if item.get("type").and_then(Value::as_str) == Some("fileChange") {
                    remember_paths(item, state);
                }
                if let Some(started) = tool_started(item) {
                    if let Some(id) = str_at(item, "id") {
                        state.tools_started.insert(id.into());
                    }
                    return vec![Mapped::Event(started)];
                }
            }
            vec![]
        }
        "item/completed" => params
            .get("item")
            .map(|item| completed_item(item, state))
            .unwrap_or_default(),
        "turn/completed" => {
            let turn = params.get("turn").cloned().unwrap_or(Value::Null);
            let end = match turn.get("status").and_then(Value::as_str) {
                Some("completed") => TurnEnd::Completed,
                Some("interrupted") => TurnEnd::Interrupted,
                _ => turn_failure(turn.get("error")),
            };
            if end != TurnEnd::Completed {
                state.usage.mark_incomplete();
            }
            finish(end, state)
        }
        "error" => {
            if params.get("willRetry").and_then(Value::as_bool) == Some(true) {
                return vec![Mapped::Event(EventPayload::ToolActivity {
                    label: "Повтор запроса".into(),
                    detail: "Codex повторяет запрос после временной ошибки".into(),
                })];
            }
            state.usage.mark_incomplete();
            finish(turn_failure(params.get("error")), state)
        }
        _ => vec![],
    }
}

fn finish(end: TurnEnd, state: &mut TurnState) -> Vec<Mapped> {
    state
        .usage
        .take()
        .into_iter()
        .map(Mapped::Event)
        .chain(std::iter::once(Mapped::Finished(end)))
        .collect()
}

fn completed_item(item: &Value, state: &mut TurnState) -> Vec<Mapped> {
    if let Some(id) = str_at(item, "id").filter(|id| !id.is_empty()) {
        if !state.completed.insert(id.into()) {
            return vec![];
        }
    }
    let activity = |label: &str, detail: String| {
        vec![Mapped::Event(EventPayload::ToolActivity {
            label: label.into(),
            detail: clip(&redact(&detail), 400),
        })]
    };
    match item.get("type").and_then(Value::as_str).unwrap_or_default() {
        "contextCompaction" => vec![Mapped::Event(EventPayload::ContextCompacted {})],
        "agentMessage" => {
            let (Some(id), Some(text)) = (str_at(item, "id"), str_at(item, "text")) else {
                return vec![];
            };
            if state.streamed.contains(id) || text.is_empty() {
                return vec![];
            }
            if str_at(item, "phase") == Some("commentary") {
                return vec![Mapped::Event(EventPayload::ProgressDelta {
                    text: text.into(),
                })];
            }
            vec![Mapped::Event(state.text(id, text))]
        }
        "commandExecution" | "mcpToolCall" | "webSearch" => {
            let id = str_at(item, "id").unwrap_or_default().to_string();
            let failed = matches!(str_at(item, "status"), Some("failed" | "declined"))
                || item
                    .get("exitCode")
                    .and_then(Value::as_i64)
                    .is_some_and(|code| code != 0);
            let mut events = Vec::new();
            if !state.tools_started.remove(&id) {
                events.extend(tool_started(item).map(Mapped::Event));
            }
            events.push(Mapped::Event(EventPayload::ToolFinished { id, failed }));
            events
        }
        "fileChange" => {
            remember_paths(item, state);
            let id = str_at(item, "id").unwrap_or_default();
            let paths = state.file_changes.remove(id).unwrap_or_default();
            if str_at(item, "status") == Some("declined") {
                return activity("Изменение файлов отклонено", summarize_paths(&paths));
            }
            let changes = item
                .get("changes")
                .and_then(Value::as_array)
                .filter(|changes| !changes.is_empty());
            let rows: Vec<(String, &str, Option<u32>, Option<u32>)> = match changes {
                Some(changes) => changes
                    .iter()
                    .filter_map(|change| {
                        let path = str_at(change, "path")?;
                        let kind = if change.pointer("/kind/type").and_then(Value::as_str)
                            == Some("add")
                        {
                            "create"
                        } else {
                            "edit"
                        };
                        let (added, removed) = diff_counts(str_at(change, "diff").unwrap_or(""));
                        Some((path.to_string(), kind, added, removed))
                    })
                    .collect(),
                None => paths
                    .into_iter()
                    .map(|path| (path, "edit", None, None))
                    .collect(),
            };
            rows.into_iter()
                .enumerate()
                .flat_map(|(index, (path, kind, added, removed))| {
                    let id = format!("{id}:{index}");
                    [
                        Mapped::Event(EventPayload::ToolStarted {
                            id: id.clone(),
                            tool: "fileChange".into(),
                            kind: kind.into(),
                            summary: clip(&redact(&path), 200),
                            added,
                            removed,
                        }),
                        Mapped::Event(EventPayload::ToolFinished { id, failed: false }),
                    ]
                })
                .collect()
        }
        "dynamicToolCall" => activity(
            "Инструмент",
            str_at(item, "tool").unwrap_or("инструмент").to_string(),
        ),
        "imageView" => activity(
            "Просмотр изображения",
            str_at(item, "path").unwrap_or_default().to_string(),
        ),
        "imageGeneration" => activity(
            "Генерация изображения",
            str_at(item, "savedPath")
                .or_else(|| str_at(item, "revisedPrompt"))
                .unwrap_or_default()
                .to_string(),
        ),
        "plan" => activity("План", str_at(item, "text").unwrap_or_default().to_string()),
        "collabAgentToolCall" | "subAgentActivity" => {
            activity("Субагент", "Codex передал часть задачи субагенту".into())
        }
        _ => vec![],
    }
}

/// Commands, MCP calls and web searches as short rows; outputs are never stored.
fn tool_started(item: &Value) -> Option<EventPayload> {
    let id = str_at(item, "id")?.to_string();
    let (tool, kind, summary) = match str_at(item, "type")? {
        "commandExecution" => (
            "shell",
            "command",
            str_at(item, "command").unwrap_or("команда").to_string(),
        ),
        "mcpToolCall" => (
            "mcp",
            "mcp",
            format!(
                "{} · {}",
                str_at(item, "server").unwrap_or("MCP"),
                str_at(item, "tool").unwrap_or("инструмент")
            ),
        ),
        "webSearch" => (
            "webSearch",
            "web",
            str_at(item, "query").unwrap_or_default().to_string(),
        ),
        _ => return None,
    };
    let line = summary.lines().next().unwrap_or("").to_string();
    Some(EventPayload::ToolStarted {
        id,
        tool: tool.into(),
        kind: kind.into(),
        summary: clip(&redact(&line), 160),
        added: None,
        removed: None,
    })
}

fn diff_counts(diff: &str) -> (Option<u32>, Option<u32>) {
    if diff.is_empty() {
        return (None, None);
    }
    let count = |prefix: char, header: &str| {
        diff.lines()
            .filter(|line| line.starts_with(prefix) && !line.starts_with(header))
            .count()
            .min(u32::MAX as usize) as u32
    };
    (Some(count('+', "+++")), Some(count('-', "---")))
}

fn remember_paths(item: &Value, state: &mut TurnState) {
    let Some(id) = str_at(item, "id") else { return };
    let paths: Vec<String> = item
        .get("changes")
        .and_then(Value::as_array)
        .map(|changes| {
            changes
                .iter()
                .filter_map(|change| str_at(change, "path").map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if !paths.is_empty() {
        state.file_changes.insert(id.to_string(), paths);
    }
}

pub fn summarize_paths(paths: &[String]) -> String {
    match paths.len() {
        0 => "Файлы не указаны".into(),
        1..=4 => paths.join(", "),
        count => format!("{} и ещё {}", paths[..3].join(", "), count - 3),
    }
}

/// Maps a provider error to a user-facing message. Limits are surfaced as-is; recovery is a
/// manual user decision.
pub fn turn_failure(error: Option<&Value>) -> TurnEnd {
    let message = error
        .and_then(|error| str_at(error, "message"))
        .map(|message| clip(&redact(message), 600))
        .unwrap_or_else(|| "Codex завершил ход с ошибкой".into());
    let info = error.and_then(|error| error.get("codexErrorInfo"));
    let kind = match info.and_then(Value::as_str) {
        Some("usageLimitExceeded" | "rateLimitExceeded") => Some("usage_limit".to_string()),
        Some("unauthorized") => Some("auth".to_string()),
        Some("contextWindowExceeded") => Some("context".to_string()),
        // Documented ChatGPT-plan error codes surface in the message text.
        _ if message.contains("subscription_sharing_usage_limit_exceeded") => {
            Some("usage_limit".to_string())
        }
        _ if message.contains("subscription_sharing_invalid_user") => Some("auth".to_string()),
        _ => None,
    };
    TurnEnd::Failed { message, kind }
}

pub fn str_at<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut clipped: String = text.chars().take(limit).collect();
    clipped.push('…');
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn state() -> TurnState {
        let mut state = TurnState::new("thread-1");
        state.turn_id = Some("turn-1".into());
        state
    }

    fn text(mapped: &[Mapped]) -> String {
        mapped
            .iter()
            .filter_map(|item| match item {
                Mapped::Event(EventPayload::AssistantTextDelta { text }) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn streams_text_and_skips_the_duplicate_final_message() {
        let mut state = state();
        let base = json!({"threadId": "thread-1", "turnId": "turn-1", "itemId": "m1"});
        let mut delta = base.clone();
        delta["delta"] = json!("Привет");
        let streamed = map_notification("item/agentMessage/delta", &delta, &mut state);
        assert_eq!(text(&streamed), "Привет");
        let done = json!({"threadId": "thread-1", "turnId": "turn-1",
            "item": {"type": "agentMessage", "id": "m1", "text": "Привет"}});
        assert!(map_notification("item/completed", &done, &mut state).is_empty());
        let second = json!({"threadId": "thread-1", "turnId": "turn-1",
            "item": {"type": "agentMessage", "id": "m2", "text": "Готово"}});
        assert_eq!(
            text(&map_notification("item/completed", &second, &mut state)),
            "\n\nГотово"
        );
    }

    #[test]
    fn ignores_other_threads_and_turns() {
        let mut state = state();
        for params in [
            json!({"threadId": "thread-2", "turnId": "turn-1", "itemId": "m", "delta": "x"}),
            json!({"threadId": "thread-1", "turnId": "turn-0", "itemId": "m", "delta": "x"}),
        ] {
            assert!(map_notification("item/agentMessage/delta", &params, &mut state).is_empty());
        }
    }

    #[test]
    fn describes_commands_and_file_changes() {
        let mut state = state();
        let command = json!({"threadId": "thread-1", "turnId": "turn-1", "item": {
            "type": "commandExecution", "id": "c1", "command": "cargo test",
            "status": "completed", "exitCode": 101}});
        assert_eq!(
            map_notification("item/completed", &command, &mut state),
            vec![
                Mapped::Event(EventPayload::ToolStarted {
                    id: "c1".into(),
                    tool: "shell".into(),
                    kind: "command".into(),
                    summary: "cargo test".into(),
                    added: None,
                    removed: None,
                }),
                Mapped::Event(EventPayload::ToolFinished {
                    id: "c1".into(),
                    failed: true,
                }),
            ]
        );
        let started = json!({"threadId": "thread-1", "turnId": "turn-1", "item": {
            "type": "fileChange", "id": "f1", "status": "inProgress",
            "changes": [{"path": "src/a.rs", "diff": "", "kind": {"type": "update"}},
                        {"path": "src/b.rs", "diff": "", "kind": {"type": "add"}}]}});
        map_notification("item/started", &started, &mut state);
        assert_eq!(
            state.file_change_paths("f1"),
            Some(&["src/a.rs".to_string(), "src/b.rs".to_string()][..])
        );
        let done = json!({"threadId": "thread-1", "turnId": "turn-1", "item": {
            "type": "fileChange", "id": "f1", "status": "completed", "changes": []}});
        let mapped = map_notification("item/completed", &done, &mut state);
        assert_eq!(mapped.len(), 4);
        assert!(
            matches!(&mapped[2], Mapped::Event(EventPayload::ToolStarted { summary, .. }) if summary == "src/b.rs")
        );
        let diff = json!({"threadId": "thread-1", "turnId": "turn-1", "item": {
            "type": "fileChange", "id": "f2", "status": "completed",
            "changes": [{"path": "src/c.rs", "diff": "--- a\n+++ b\n-old\n+new\n+more", "kind": {"type": "update"}}]}});
        assert!(matches!(
            &map_notification("item/completed", &diff, &mut state)[0],
            Mapped::Event(EventPayload::ToolStarted {
                added: Some(2),
                removed: Some(1),
                ..
            })
        ));
    }

    #[test]
    fn usage_limits_are_reported_not_worked_around() {
        let mut state = state();
        let params = json!({"threadId": "thread-1", "turnId": "turn-1", "turn": {
            "id": "turn-1", "status": "failed", "items": [],
            "error": {"message": "You've hit your usage limit.", "codexErrorInfo": "usageLimitExceeded"}}});
        assert_eq!(
            map_notification("turn/completed", &params, &mut state),
            vec![Mapped::Finished(TurnEnd::Failed {
                message: "You've hit your usage limit.".into(),
                kind: Some("usage_limit".into()),
            })]
        );
    }

    #[test]
    fn provider_errors_are_redacted() {
        let failure = turn_failure(Some(
            &json!({"message": "request failed: Authorization: Bearer secret-token"}),
        ));
        let TurnEnd::Failed { message, .. } = failure else {
            panic!("expected failure")
        };
        assert!(!message.contains("secret-token"));
    }

    #[test]
    fn long_path_lists_are_summarized() {
        let paths: Vec<String> = (1..=6).map(|i| format!("f{i}.rs")).collect();
        assert_eq!(summarize_paths(&paths), "f1.rs, f2.rs, f3.rs и ещё 3");
    }
}

#[cfg(test)]
mod efficient_stream_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn replayed_tool_completion_is_one_activity() {
        let mut state = TurnState::new("thread");
        let frame = json!({"threadId":"thread","item":{"type":"mcpToolCall","id":"call-1","server":"fixture","tool":"read","status":"completed"}});
        assert_eq!(
            map_notification("item/completed", &frame, &mut state).len(),
            2 // started + finished rows for one call
        );
        assert!(map_notification("item/completed", &frame, &mut state).is_empty());
    }
    #[test]
    fn old_turns_cannot_inflate_a_cold_resume_bill() {
        let mut state = TurnState::new("thread");
        state.turn_id = Some("current".into());
        let usage = |n| json!({"inputTokens":n,"cachedInputTokens":0,"outputTokens":0,"reasoningOutputTokens":0});
        for n in [1000, 1800] {
            assert!(map_notification("thread/tokenUsage/updated",&json!({"threadId":"thread","turnId":"old","tokenUsage":{"total":usage(n),"last":usage(20)}}),&mut state).is_empty());
        }
        map_notification(
            "thread/tokenUsage/updated",
            &json!({"threadId":"thread","turnId":"current","tokenUsage":{"total":usage(1820),"last":usage(20)}}),
            &mut state,
        );
        let final_events = map_notification(
            "turn/completed",
            &json!({"threadId":"thread","turn":{"id":"current","status":"completed"}}),
            &mut state,
        );
        assert!(final_events.iter().any(|e| matches!(
            e,
            Mapped::Event(EventPayload::ProviderUsage {
                input_tokens: Some(20),
                incomplete: true,
                ..
            })
        )));
    }
    #[test]
    fn public_commentary_is_progress_not_a_duplicate_final_answer() {
        let mut state = TurnState::new("thread");
        state.turn_id = Some("current".into());
        map_notification(
            "item/started",
            &json!({"threadId":"thread","turnId":"current","item":{"type":"agentMessage","id":"progress","phase":"commentary"}}),
            &mut state,
        );
        let progress = map_notification(
            "item/agentMessage/delta",
            &json!({"threadId":"thread","turnId":"current","itemId":"progress","delta":"Checking files"}),
            &mut state,
        );
        assert!(
            matches!(&progress[0],Mapped::Event(EventPayload::ProgressDelta{text}) if text=="Checking files")
        );
        assert!(map_notification("item/completed",&json!({"threadId":"thread","turnId":"current","item":{"type":"agentMessage","id":"progress","phase":"commentary","text":"Checking files"}}),&mut state).is_empty());
        let final_text = map_notification(
            "item/agentMessage/delta",
            &json!({"threadId":"thread","turnId":"current","itemId":"final","delta":"Done"}),
            &mut state,
        );
        assert!(
            matches!(&final_text[0],Mapped::Event(EventPayload::AssistantTextDelta{text}) if text=="Done")
        );
        assert!(map_notification(
            "model/rerouted",
            &json!({"threadId":"other","toModel":"wrong"}),
            &mut state
        )
        .is_empty());
        assert!(
            matches!(&map_notification("model/rerouted",&json!({"threadId":"thread","turnId":"current","toModel":"actual"}),&mut state)[0],Mapped::Event(EventPayload::ModelResolved{model}) if model=="actual")
        );
    }
}
