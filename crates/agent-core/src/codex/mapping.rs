//! Translation of Codex app-server notifications into normalized timeline events.
//! Pure functions only, so protocol drift is caught by unit tests rather than in the UI.

use crate::{model::EventPayload, redaction::redact};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

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
    last_message: Option<String>,
    has_text: bool,
    file_changes: HashMap<String, Vec<String>>,
}

impl TurnState {
    pub fn new(thread_id: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            turn_id: None,
            streamed: HashSet::new(),
            last_message: None,
            has_text: false,
            file_changes: HashMap::new(),
        }
    }

    /// Whether a notification or server request belongs to this thread and turn.
    pub fn owns(&self, params: &Value) -> bool {
        if params.get("threadId").and_then(Value::as_str) != Some(self.thread_id.as_str()) {
            return false;
        }
        match (
            self.turn_id.as_deref(),
            params.get("turnId").and_then(Value::as_str),
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
    if !state.owns(params) {
        return vec![];
    }
    match method {
        "item/agentMessage/delta" => {
            let (Some(item), Some(delta)) = (str_at(params, "itemId"), str_at(params, "delta"))
            else {
                return vec![];
            };
            state.streamed.insert(item.to_string());
            vec![Mapped::Event(state.text(item, delta))]
        }
        "item/started" => {
            if let Some(item) = params.get("item") {
                if item.get("type").and_then(Value::as_str) == Some("fileChange") {
                    remember_paths(item, state);
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
            vec![Mapped::Finished(end)]
        }
        "error" => {
            if params.get("willRetry").and_then(Value::as_bool) == Some(true) {
                return vec![Mapped::Event(EventPayload::ToolActivity {
                    label: "Повтор запроса".into(),
                    detail: "Codex повторяет запрос после временной ошибки".into(),
                })];
            }
            vec![Mapped::Finished(turn_failure(params.get("error")))]
        }
        _ => vec![],
    }
}

fn completed_item(item: &Value, state: &mut TurnState) -> Vec<Mapped> {
    let activity = |label: &str, detail: String| {
        vec![Mapped::Event(EventPayload::ToolActivity {
            label: label.into(),
            detail: clip(&redact(&detail), 400),
        })]
    };
    match item.get("type").and_then(Value::as_str).unwrap_or_default() {
        "agentMessage" => {
            let (Some(id), Some(text)) = (str_at(item, "id"), str_at(item, "text")) else {
                return vec![];
            };
            if state.streamed.contains(id) || text.is_empty() {
                return vec![];
            }
            vec![Mapped::Event(state.text(id, text))]
        }
        "commandExecution" => {
            let command = str_at(item, "command").unwrap_or("команда");
            let outcome = match (
                str_at(item, "status"),
                item.get("exitCode").and_then(Value::as_i64),
            ) {
                (Some("declined"), _) => " · отклонена".to_string(),
                (_, Some(0)) => String::new(),
                (_, Some(code)) => format!(" · код выхода {code}"),
                (Some("failed"), None) => " · ошибка".into(),
                _ => String::new(),
            };
            activity("Команда", format!("{command}{outcome}"))
        }
        "fileChange" => {
            remember_paths(item, state);
            let id = str_at(item, "id").unwrap_or_default();
            let paths = state.file_changes.remove(id).unwrap_or_default();
            let declined = str_at(item, "status") == Some("declined");
            let label = if declined {
                "Изменение файлов отклонено"
            } else {
                "Изменение файлов"
            };
            activity(label, summarize_paths(&paths))
        }
        "mcpToolCall" => {
            let server = str_at(item, "server").unwrap_or("MCP");
            let tool = str_at(item, "tool").unwrap_or("инструмент");
            let failed = str_at(item, "status") == Some("failed");
            activity(
                &format!("MCP · {server}"),
                format!("{tool}{}", if failed { " · ошибка" } else { "" }),
            )
        }
        "dynamicToolCall" => activity(
            "Инструмент",
            str_at(item, "tool").unwrap_or("инструмент").to_string(),
        ),
        "webSearch" => activity(
            "Поиск в интернете",
            str_at(item, "query").unwrap_or_default().to_string(),
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
        "contextCompaction" => activity("Сжатие контекста", "История сжата Codex".into()),
        "collabAgentToolCall" | "subAgentActivity" => {
            activity("Субагент", "Codex передал часть задачи субагенту".into())
        }
        _ => vec![],
    }
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
            vec![Mapped::Event(EventPayload::ToolActivity {
                label: "Команда".into(),
                detail: "cargo test · код выхода 101".into(),
            })]
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
        assert_eq!(
            map_notification("item/completed", &done, &mut state),
            vec![Mapped::Event(EventPayload::ToolActivity {
                label: "Изменение файлов".into(),
                detail: "src/a.rs, src/b.rs".into(),
            })]
        );
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
