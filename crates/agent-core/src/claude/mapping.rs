use super::{failure, tools::ToolTracker};
use crate::{model::EventPayload, Result};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};

/// A bounded replay window: repeated text can be legitimate, so deduplicate only by ID.
#[derive(Default)]
struct RecentIds {
    ids: HashSet<String>,
    order: VecDeque<String>,
}
impl RecentIds {
    fn duplicate(&mut self, id: Option<&str>) -> bool {
        let Some(id) = id.filter(|id| !id.is_empty() && id.len() <= 256) else {
            return false;
        };
        if !self.ids.insert(id.into()) {
            return true;
        }
        self.order.push_back(id.into());
        if self.order.len() > 8192 {
            if let Some(old) = self.order.pop_front() {
                self.ids.remove(&old);
            }
        }
        false
    }
}

/// The public result's `usage` is per-turn main-agent usage. `modelUsage` and cost
/// can include prior resumed turns, so never sum those cumulative fields here.
pub(super) fn reported_usage(value: &Value) -> Option<EventPayload> {
    if value["type"] != "result"
        || value
            .get("parent_tool_use_id")
            .is_some_and(|id| !id.is_null())
    {
        return None;
    }
    let counter = |field: &Value| field.as_u64().filter(|n| *n <= 1_000_000_000_000);
    let usage = &value["usage"];
    let model_requests = counter(&value["num_turns"]);
    let input_tokens = counter(&usage["input_tokens"]);
    let output_tokens = counter(&usage["output_tokens"]);
    let cache_read_tokens = counter(&usage["cache_read_input_tokens"]);
    let cache_creation_tokens = counter(&usage["cache_creation_input_tokens"]);
    if [
        model_requests,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
    ]
    .iter()
    .all(Option::is_none)
    {
        return None;
    }
    Some(EventPayload::ProviderUsage {
        provider: "anthropic".into(),
        model_requests,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        reasoning_tokens: None,
        incomplete: false,
    })
}

pub(super) struct StreamState {
    structured: bool,
    last_input: Option<u64>,
    window: Option<u64>,
    streamed_message: bool,
    emitted: bool,
    completed: bool,
    events: RecentIds,
    tools: RecentIds,
    tracker: ToolTracker,
}
impl StreamState {
    pub fn new(structured: bool) -> Self {
        Self {
            structured,
            last_input: None,
            window: None,
            streamed_message: false,
            emitted: false,
            completed: false,
            events: RecentIds::default(),
            tools: RecentIds::default(),
            tracker: ToolTracker::default(),
        }
    }
    /// Shows project paths relative to the working directory.
    pub fn with_root(mut self, root: &std::path::Path) -> Self {
        self.tracker = ToolTracker::new(Some(root));
        self
    }
    pub fn accept(&mut self, value: Value) -> Result<Vec<EventPayload>> {
        let mut events = Vec::new();
        if value
            .get("parent_tool_use_id")
            .is_some_and(|id| !id.is_null())
        {
            return Ok(events);
        }
        if self
            .events
            .duplicate(value.get("uuid").and_then(Value::as_str))
        {
            return Ok(events);
        }
        let usage = if value["type"] == "assistant" {
            &value["message"]["usage"]
        } else if value["type"] == "stream_event" && value["event"]["type"] == "message_start" {
            &value["event"]["message"]["usage"]
        } else {
            &Value::Null
        };
        if let Some(input) = usage["input_tokens"].as_u64() {
            self.last_input = input
                .checked_add(usage["cache_read_input_tokens"].as_u64().unwrap_or(0))
                .and_then(|n| {
                    n.checked_add(usage["cache_creation_input_tokens"].as_u64().unwrap_or(0))
                })
                .filter(|n| *n <= 10_000_000);
            events.push(EventPayload::ContextUsage {
                used_tokens: self.last_input,
                window_tokens: self.window,
                source: "claude_last_request_input".into(),
            });
        }
        match value.get("type").and_then(Value::as_str) {
            Some("system") => match value.get("subtype").and_then(Value::as_str) {
                Some("init") => {
                    if let Some(model) = value.get("model").and_then(Value::as_str) {
                        events.push(EventPayload::ModelResolved {
                            model: model.into(),
                        });
                    }
                    if let Some(id) = value.get("session_id").and_then(Value::as_str) {
                        uuid::Uuid::parse_str(id)
                            .map_err(|_| failure("Некорректный ID сессии Claude", None))?;
                        events.push(EventPayload::ProviderSession { id: id.into() });
                    }
                }
                Some("compact_boundary") => {
                    self.last_input = None;
                    events.push(EventPayload::ContextCompacted {});
                    events.push(EventPayload::ToolActivity {
                        label: "Контекст сжат".into(),
                        detail: String::new(),
                    });
                }
                Some("api_retry") => events.push(EventPayload::ToolActivity {
                    label: "Повтор запроса".into(),
                    detail: String::new(),
                }),
                _ => {}
            },
            Some("stream_event") if !self.structured => {
                let event = &value["event"];
                if event["type"] == "message_start" {
                    self.streamed_message = false;
                }
                if event["delta"]["type"] == "text_delta" {
                    if let Some(text) = event["delta"]["text"].as_str() {
                        self.streamed_message = true;
                        self.emitted = true;
                        events.push(EventPayload::AssistantTextDelta { text: text.into() });
                    }
                }
                // Thinking/signatures are intentionally never forwarded or persisted.
            }
            Some("assistant") => {
                for block in value["message"]["content"].as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") if !self.structured && !self.streamed_message => {
                            if let Some(text) = block["text"].as_str() {
                                self.emitted = true;
                                events.push(EventPayload::AssistantTextDelta { text: text.into() });
                            }
                        }
                        Some("tool_use") => {
                            if self
                                .tools
                                .duplicate(block.get("id").and_then(Value::as_str))
                            {
                                continue;
                            }
                            // Only a short redacted summary: never file contents or full inputs.
                            events.extend(self.tracker.started(block));
                        }
                        _ => {}
                    }
                }
            }
            Some("user") => {
                for block in value["message"]["content"].as_array().into_iter().flatten() {
                    if block["type"] == "tool_result" {
                        events.extend(self.tracker.finished(block));
                    }
                }
            }
            Some("result") => {
                if let Some(usage) = value.get("modelUsage").and_then(Value::as_object) {
                    if usage.len() == 1 {
                        if let Some(model) = usage.keys().next() {
                            self.window = usage[model]["contextWindow"]
                                .as_u64()
                                .filter(|n| *n > 0 && *n <= 10_000_000);
                            events.push(EventPayload::ContextUsage {
                                used_tokens: self.last_input,
                                window_tokens: self.window,
                                source: "claude_last_request_input".into(),
                            });
                            events.push(EventPayload::ModelResolved {
                                model: model.clone(),
                            });
                        }
                    }
                }
                if self.completed {
                    return Err(failure("Повторный итог Claude CLI", None));
                }
                let subtype = value["subtype"].as_str().unwrap_or("");
                if value["is_error"].as_bool() == Some(true) || subtype != "success" {
                    let message = value["result"]
                        .as_str()
                        .or_else(|| {
                            value["errors"]
                                .as_array()
                                .and_then(|errors| errors.first())
                                .and_then(Value::as_str)
                        })
                        .unwrap_or("Claude не завершил задачу успешно");
                    let lower = message.to_ascii_lowercase();
                    let kind = if lower.contains("rate limit")
                        || lower.contains("usage limit")
                        || lower.contains("session limit")
                        || lower.contains("hit your limit")
                    {
                        Some("usage_limit")
                    } else if lower.contains("auth")
                        || lower.contains("not logged")
                        || lower.contains("login")
                    {
                        Some("auth")
                    } else {
                        None
                    };
                    return Err(failure(message, kind));
                }
                if self.structured {
                    let structured = value
                        .get("structured_output")
                        .filter(|v| v.is_object())
                        .ok_or_else(|| {
                            failure("Claude не вернул результат по запрошенной схеме", None)
                        })?;
                    events.push(EventPayload::AssistantTextDelta {
                        text: structured.to_string(),
                    });
                } else if !self.emitted {
                    if let Some(text) = value["result"].as_str() {
                        events.push(EventPayload::AssistantTextDelta { text: text.into() });
                    }
                }
                self.completed = true;
            }
            _ => {}
        }
        Ok(events)
    }
    pub fn finish(&self) -> Result<()> {
        if self.completed {
            Ok(())
        } else {
            Err(failure(
                "Поток Claude оборвался до итогового результата",
                None,
            ))
        }
    }
}
