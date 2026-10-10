use super::failure;
use crate::{model::EventPayload, Result};
use serde_json::Value;

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
    streamed_message: bool,
    emitted: bool,
    completed: bool,
}
impl StreamState {
    pub fn new(structured: bool) -> Self {
        Self {
            structured,
            streamed_message: false,
            emitted: false,
            completed: false,
        }
    }
    pub fn accept(&mut self, value: Value) -> Result<Vec<EventPayload>> {
        let mut events = Vec::new();
        if value
            .get("parent_tool_use_id")
            .is_some_and(|id| !id.is_null())
        {
            return Ok(events);
        }
        match value.get("type").and_then(Value::as_str) {
            Some("system") => {
                match value.get("subtype").and_then(Value::as_str) {
                    Some("init") => {
                        if let Some(model) = value.get("model").and_then(Value::as_str) {
                            events.push(EventPayload::ModelResolved { model: model.into() });
                        }
                        if let Some(id) = value.get("session_id").and_then(Value::as_str) {
                            uuid::Uuid::parse_str(id).map_err(|_| failure("Некорректный ID сессии Claude", None))?;
                            events.push(EventPayload::ProviderSession { id: id.into() });
                        }
                    }
                    Some("compact_boundary") => events.push(EventPayload::ToolActivity { label: "Сжатие контекста".into(), detail: "Claude Code сжал контекст диалога.".into() }),
                    Some("api_retry") => events.push(EventPayload::ToolActivity { label: "Повтор запроса Claude".into(), detail: "Официальный CLI повторяет запрос на том же аккаунте; привязка не изменяется.".into() }),
                    _ => {}
                }
            }
            Some("stream_event") if !self.structured => {
                let event = &value["event"];
                if event["type"] == "message_start" { self.streamed_message = false; }
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
                            // Never include raw tool inputs: they may contain credentials/content.
                            let name = block["name"].as_str().unwrap_or("Инструмент");
                            events.push(EventPayload::ToolActivity { label: format!("Claude · {name}"), detail: "Запрос инструмента; выполнение регулируется разрешениями.".into() });
                        }
                        _ => {}
                    }
                }
            }
            Some("result") => {
                if let Some(usage) = value.get("modelUsage").and_then(Value::as_object) {
                    if usage.len() == 1 {
                        if let Some(model) = usage.keys().next() {
                            events.push(EventPayload::ModelResolved { model: model.clone() });
                        }
                    }
                }
                if self.completed { return Err(failure("Повторный итог Claude CLI", None)); }
                let subtype = value["subtype"].as_str().unwrap_or("");
                if value["is_error"].as_bool() == Some(true) || subtype != "success" {
                    let message = value["result"].as_str().or_else(|| value["errors"].as_array().and_then(|errors| errors.first()).and_then(Value::as_str)).unwrap_or("Claude не завершил задачу успешно");
                    let lower = message.to_ascii_lowercase();
                    let kind = if lower.contains("rate limit") || lower.contains("usage limit") || lower.contains("session limit") || lower.contains("hit your limit") { Some("usage_limit") } else if lower.contains("auth") || lower.contains("not logged") || lower.contains("login") { Some("auth") } else { None };
                    return Err(failure(message, kind));
                }
                if self.structured {
                    let structured = value.get("structured_output").filter(|v| v.is_object()).ok_or_else(|| failure("Claude не вернул структурированный план", None))?;
                    events.push(EventPayload::AssistantTextDelta { text: structured.to_string() });
                } else if !self.emitted {
                    if let Some(text) = value["result"].as_str() { events.push(EventPayload::AssistantTextDelta { text: text.into() }); }
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
