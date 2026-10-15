//! Short, redacted tool summaries for the chat: paths, patterns and command lines only.
use crate::model::{EventPayload, TodoItem};
use serde_json::Value;
use std::{collections::HashMap, path::Path};

const SUMMARY: usize = 160;

/// Tools the user does not need to see as separate rows.
const QUIET: [&str; 5] = [
    "ToolSearch",
    "TaskList",
    "TaskGet",
    "TaskOutput",
    "TodoWrite",
];

#[derive(Default)]
pub(super) struct ToolTracker {
    root: String,
    created_tasks: HashMap<String, String>,
}

fn short(text: &str) -> String {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let line = crate::redaction::redact(line.trim());
    let mut result: String = line.chars().take(SUMMARY).collect();
    if line.chars().count() > SUMMARY || text.trim().lines().count() > 1 {
        result.push('…');
    }
    result
}

fn lines(text: &str) -> u32 {
    if text.is_empty() {
        0
    } else {
        text.lines().count().min(u32::MAX as usize) as u32
    }
}

fn status(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|s| ["pending", "in_progress", "completed", "deleted"].contains(s))
        .map(str::to_string)
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

impl ToolTracker {
    pub fn new(root: Option<&Path>) -> Self {
        let root = root
            .map(|root| {
                root.to_string_lossy()
                    .trim_start_matches(r"\\?\")
                    .replace('\\', "/")
                    .to_lowercase()
            })
            .unwrap_or_default();
        Self {
            root,
            ..Self::default()
        }
    }

    fn display_path(&self, path: &str) -> String {
        let normalized = path.trim_start_matches(r"\\?\").replace('\\', "/");
        let lower = normalized.to_lowercase();
        let relative = if !self.root.is_empty() && lower.starts_with(&self.root) {
            normalized[self.root.len()..]
                .trim_start_matches('/')
                .to_string()
        } else {
            normalized
        };
        short(&relative)
    }

    fn exists(&self, path: &str) -> bool {
        let path = Path::new(path);
        if path.is_absolute() || self.root.is_empty() {
            path.exists()
        } else {
            Path::new(&self.root).join(path).exists()
        }
    }

    pub fn started(&mut self, block: &Value) -> Vec<EventPayload> {
        let id = block["id"].as_str().unwrap_or("").to_string();
        let name = block["name"].as_str().unwrap_or("").to_string();
        let input = &block["input"];
        match name.as_str() {
            "TodoWrite" => {
                let items = input["todos"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(100)
                    .map(|todo| TodoItem {
                        id: None,
                        content: todo["content"].as_str().map(short),
                        status: status(&todo["status"]),
                    })
                    .collect();
                return vec![EventPayload::TodoUpdate {
                    replace: true,
                    items,
                }];
            }
            "TaskCreate" => {
                if let Some(subject) = input["subject"].as_str() {
                    self.created_tasks.insert(id, short(subject));
                }
                return vec![];
            }
            "TaskUpdate" => {
                let Some(task) = input["taskId"]
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| input["taskId"].as_u64().map(|n| n.to_string()))
                else {
                    return vec![];
                };
                return vec![EventPayload::TodoUpdate {
                    replace: false,
                    items: vec![TodoItem {
                        id: Some(task),
                        content: input["subject"].as_str().map(short),
                        status: status(&input["status"]),
                    }],
                }];
            }
            _ if QUIET.contains(&name.as_str()) => return vec![],
            _ => {}
        }
        let path = input["file_path"]
            .as_str()
            .or_else(|| input["notebook_path"].as_str())
            .unwrap_or("");
        let (kind, summary, added, removed) = match name.as_str() {
            "Bash" | "PowerShell" | "Monitor" => (
                "command",
                short(
                    input["description"]
                        .as_str()
                        .filter(|d| !d.trim().is_empty())
                        .or_else(|| input["command"].as_str())
                        .unwrap_or(""),
                ),
                None,
                None,
            ),
            "Read" => ("read", self.display_path(path), None, None),
            "Edit" => (
                "edit",
                self.display_path(path),
                Some(lines(input["new_string"].as_str().unwrap_or(""))),
                Some(lines(input["old_string"].as_str().unwrap_or(""))),
            ),
            "Write" => (
                if self.exists(path) { "write" } else { "create" },
                self.display_path(path),
                Some(lines(input["content"].as_str().unwrap_or(""))),
                None,
            ),
            "Glob" | "Grep" => (
                "search",
                short(input["pattern"].as_str().unwrap_or("")),
                None,
                None,
            ),
            "WebSearch" => (
                "web",
                short(input["query"].as_str().unwrap_or("")),
                None,
                None,
            ),
            "WebFetch" => (
                "web",
                short(input["url"].as_str().unwrap_or("")),
                None,
                None,
            ),
            "Agent" | "Task" => (
                "agent",
                short(input["description"].as_str().unwrap_or("")),
                None,
                None,
            ),
            other if other.starts_with("mcp__") => {
                let mut parts = other.splitn(3, "__").skip(1);
                let server = parts.next().unwrap_or("");
                let tool = parts.next().unwrap_or("");
                ("mcp", short(&format!("{server} · {tool}")), None, None)
            }
            other => ("other", short(other), None, None),
        };
        vec![EventPayload::ToolStarted {
            id,
            tool: short(&name),
            kind: kind.into(),
            summary,
            added,
            removed,
        }]
    }

    pub fn finished(&mut self, block: &Value) -> Vec<EventPayload> {
        let Some(id) = block["tool_use_id"].as_str() else {
            return vec![];
        };
        let failed = block["is_error"].as_bool() == Some(true);
        let mut events = Vec::new();
        if let Some(subject) = self.created_tasks.remove(id) {
            let text = text_of(&block["content"]);
            let task = text
                .split('#')
                .nth(1)
                .map(|rest| {
                    rest.chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>()
                })
                .filter(|digits| !digits.is_empty());
            if let (false, Some(task)) = (failed, task) {
                events.push(EventPayload::TodoUpdate {
                    replace: false,
                    items: vec![TodoItem {
                        id: Some(task),
                        content: Some(subject),
                        status: Some("pending".into()),
                    }],
                });
            }
            return events;
        }
        events.push(EventPayload::ToolFinished {
            id: id.into(),
            failed,
        });
        events
    }
}
