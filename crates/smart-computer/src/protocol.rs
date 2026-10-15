use crate::{invalid, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const SERVER_NAME: &str = "smart-computer";
pub const MAX_MESSAGE: usize = 4 * 1024 * 1024;
pub const MAX_ELEMENTS: usize = 80;
pub const MAX_TEXT: usize = 12_000;
pub const MAX_SEQUENCE: usize = 12;
pub const MAX_FRAME_PIXELS: u64 = 20_000_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
impl Rect {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        i64::from(x) >= i64::from(self.x)
            && i64::from(y) >= i64::from(self.y)
            && i64::from(x) < i64::from(self.x) + i64::from(self.width)
            && i64::from(y) < i64::from(self.y) + i64::from(self.height)
    }
    pub fn validate(&self) -> Result<()> {
        if self.width == 0
            || self.height == 0
            || u64::from(self.width) * u64::from(self.height) > MAX_FRAME_PIXELS
        {
            return Err(invalid("Некорректная или слишком большая область"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub application: String,
    pub pid: u32,
    pub bounds: Rect,
    pub dpi: u32,
    pub focused: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    pub name: Option<String>,
    pub automation_id: Option<String>,
    pub control_type: Option<String>,
    pub element_id: Option<String>,
}
impl Selector {
    pub fn validate(&self) -> Result<()> {
        if [
            &self.name,
            &self.automation_id,
            &self.control_type,
            &self.element_id,
        ]
        .into_iter()
        .flatten()
        .any(|s| s.len() > 256 || s.is_empty())
        {
            return Err(invalid("Слишком большой или пустой селектор"));
        }
        Ok(())
    }
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.automation_id.is_none()
            && self.control_type.is_none()
            && self.element_id.is_none()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Element {
    pub id: String,
    pub name: String,
    pub automation_id: String,
    pub control_type: String,
    pub bounds: Rect,
    pub enabled: bool,
    pub focused: bool,
    pub patterns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Click {
        x: i32,
        y: i32,
        #[serde(default)]
        right: bool,
    },
    DoubleClick {
        x: i32,
        y: i32,
    },
    Move {
        x: i32,
        y: i32,
    },
    Scroll {
        x: i32,
        y: i32,
        dx: i32,
        dy: i32,
    },
    Drag {
        points: Vec<[i32; 2]>,
    },
    Keypress {
        keys: Vec<String>,
    },
    Type {
        text: String,
    },
    Invoke {
        selector: Selector,
    },
    SetValue {
        selector: Selector,
        text: String,
    },
    Select {
        selector: Selector,
    },
    Toggle {
        selector: Selector,
    },
    Expand {
        selector: Selector,
        expanded: bool,
    },
    Window {
        operation: WindowAction,
    },
    Unity {
        command: UnityCommand,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowAction {
    Activate,
    Minimize,
    Maximize,
    Restore,
    Close,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UnityCommand {
    Select {
        object_id: i32,
        generation: String,
    },
    Rename {
        object_id: i32,
        generation: String,
        name: String,
    },
    SetActive {
        object_id: i32,
        generation: String,
        active: bool,
    },
    SetTransform {
        object_id: i32,
        generation: String,
        position: Option<[f32; 3]>,
        rotation: Option<[f32; 3]>,
        scale: Option<[f32; 3]>,
    },
}
impl Action {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Click { .. } => "click",
            Self::DoubleClick { .. } => "double_click",
            Self::Move { .. } => "move",
            Self::Scroll { .. } => "scroll",
            Self::Drag { .. } => "drag",
            Self::Keypress { .. } => "keypress",
            Self::Type { .. } => "type",
            Self::Invoke { .. } => "invoke",
            Self::SetValue { .. } => "set_value",
            Self::Select { .. } => "select",
            Self::Toggle { .. } => "toggle",
            Self::Expand { .. } => "expand",
            Self::Window { .. } => "window",
            Self::Unity { .. } => "unity",
        }
    }
    pub fn validate(&self) -> Result<()> {
        if let Self::Unity { command } = self {
            let generation = match command {
                UnityCommand::Select { generation, .. }
                | UnityCommand::Rename { generation, .. }
                | UnityCommand::SetActive { generation, .. }
                | UnityCommand::SetTransform { generation, .. } => generation,
            };
            if generation.len() != 36 || uuid::Uuid::parse_str(generation).is_err() {
                return Err(invalid("Unity: нужна актуальная generation из наблюдения"));
            }
        }
        match self {
            Self::Drag { points } if points.len() < 2 || points.len() > 64 => {
                return Err(invalid("Перетаскивание: от 2 до 64 точек"))
            }
            Self::Type { text } | Self::SetValue { text, .. } if text.len() > 4096 => {
                return Err(invalid("Слишком большой ввод"))
            }
            Self::Keypress { keys }
                if keys.is_empty() || keys.len() > 5 || keys.iter().any(|k| k.len() > 24) =>
            {
                return Err(invalid("Некорректное сочетание клавиш"))
            }
            Self::Scroll { dx, dy, .. } if dx.unsigned_abs() > 2400 || dy.unsigned_abs() > 2400 => {
                return Err(invalid("Слишком большая прокрутка"))
            }
            _ => {}
        }
        match self {
            Self::Invoke { selector }
            | Self::SetValue { selector, .. }
            | Self::Select { selector }
            | Self::Toggle { selector }
            | Self::Expand { selector, .. } => {
                selector.validate()?;
                if selector.is_empty() {
                    return Err(invalid("Нужен селектор элемента"));
                }
            }
            Self::Unity {
                command: UnityCommand::Rename { name, .. },
            } if name.is_empty() || name.len() > 128 => {
                return Err(invalid("Некорректное имя объекта"))
            }
            Self::Unity {
                command:
                    UnityCommand::SetTransform {
                        position,
                        rotation,
                        scale,
                        ..
                    },
            } if (position.is_none() && rotation.is_none() && scale.is_none())
                || [position, rotation, scale]
                    .into_iter()
                    .flatten()
                    .flatten()
                    .any(|x| !x.is_finite() || x.abs() > 1_000_000.) =>
            {
                return Err(invalid("Некорректный Transform"));
            }
            _ => {}
        }
        Ok(())
    }
    pub fn sensitive(&self) -> bool {
        !matches!(
            self,
            Self::Move { .. }
                | Self::Window {
                    operation: WindowAction::Activate
                        | WindowAction::Minimize
                        | WindowAction::Maximize
                        | WindowAction::Restore
                }
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    ElementExists { selector: Selector },
    ElementAbsent { selector: Selector },
    ElementFocused { selector: Selector },
    TextContains { selector: Selector, text: String },
    WindowFocused,
    WindowClosed,
    ImageChanged { frame_id: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub action: Action,
    pub verify: Option<Condition>,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    #[default]
    Window,
    Screen,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterView {
    State,
    Hierarchy,
    Inspector,
    Console,
    Selection,
    Scene,
    Game,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    ComputerStatus,
    ComputerWindows,
    ComputerObserve {
        adapter_view: Option<AdapterView>,
        window: String,
        #[serde(default)]
        visual: bool,
        roi: Option<Rect>,
        goal: Option<String>,
        #[serde(default)]
        selector: Selector,
        #[serde(default = "element_limit")]
        limit: usize,
    },
    ComputerFind {
        window: String,
        selector: Selector,
        #[serde(default = "element_limit")]
        limit: usize,
    },
    ComputerAct {
        window: String,
        action: Action,
        verify: Option<Condition>,
    },
    ComputerSequence {
        window: String,
        steps: Vec<Step>,
    },
    ComputerWait {
        window: String,
        condition: Condition,
        #[serde(default = "wait_timeout")]
        timeout_ms: u64,
    },
    ComputerCapture {
        #[serde(default)]
        surface: Surface,
        window: String,
        roi: Option<Rect>,
        #[serde(default = "capture_limit")]
        max_edge: u32,
        #[serde(default)]
        if_changed: Option<String>,
    },
}
fn element_limit() -> usize {
    30
}
fn wait_timeout() -> u64 {
    3000
}
fn capture_limit() -> u32 {
    1280
}
impl Request {
    pub fn parse(name: &str, arguments: Value) -> Result<Self> {
        let mut obj = arguments
            .as_object()
            .cloned()
            .ok_or_else(|| invalid("Нужен объект arguments"))?;
        if obj.contains_key("tool") {
            return Err(invalid("Поле tool недопустимо в arguments"));
        }
        obj.insert("tool".into(), json!(name));
        let request: Self = serde_json::from_value(Value::Object(obj))?;
        request.validate()?;
        Ok(request)
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ComputerObserve {
                window,
                roi,
                goal,
                selector,
                limit,
                ..
            } => {
                valid_window(window)?;
                selector.validate()?;
                if let Some(roi) = roi {
                    roi.validate()?;
                }
                if *limit == 0
                    || *limit > MAX_ELEMENTS
                    || goal.as_ref().is_some_and(|g| g.len() > 512)
                {
                    return Err(invalid("Наблюдение превышает ограничения"));
                }
            }
            Self::ComputerFind {
                window,
                selector,
                limit,
            } => {
                valid_window(window)?;
                selector.validate()?;
                if selector.is_empty() || *limit == 0 || *limit > MAX_ELEMENTS {
                    return Err(invalid("Нужен селектор и ограниченный limit"));
                }
            }
            Self::ComputerAct {
                window,
                action,
                verify,
            } => {
                valid_window(window)?;
                action.validate()?;
                validate_condition(verify.as_ref())?;
            }
            Self::ComputerSequence { window, steps } => {
                valid_window(window)?;
                if steps.is_empty() || steps.len() > MAX_SEQUENCE {
                    return Err(invalid("Последовательность: от 1 до 12 шагов"));
                }
                for step in steps {
                    step.action.validate()?;
                    validate_condition(step.verify.as_ref())?;
                }
            }
            Self::ComputerWait {
                window,
                condition,
                timeout_ms,
            } => {
                valid_window(window)?;
                validate_condition(Some(condition))?;
                if *timeout_ms == 0 || *timeout_ms > 5000 {
                    return Err(invalid("Ожидание: от 1 до 5000 мс"));
                }
            }
            Self::ComputerCapture {
                window,
                roi,
                max_edge,
                if_changed,
                ..
            } => {
                valid_window(window)?;
                if let Some(roi) = roi {
                    roi.validate()?;
                }
                if !(256..=1600).contains(max_edge)
                    || if_changed.as_ref().is_some_and(|s| s.len() > 128)
                {
                    return Err(invalid("Некорректные параметры кадра"));
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::ComputerStatus => "computer_status",
            Self::ComputerWindows => "computer_windows",
            Self::ComputerObserve { .. } => "computer_observe",
            Self::ComputerFind { .. } => "computer_find",
            Self::ComputerAct { .. } => "computer_act",
            Self::ComputerSequence { .. } => "computer_sequence",
            Self::ComputerWait { .. } => "computer_wait",
            Self::ComputerCapture { .. } => "computer_capture",
        }
    }
    pub fn window(&self) -> Option<&str> {
        match self {
            Self::ComputerObserve { window, .. }
            | Self::ComputerFind { window, .. }
            | Self::ComputerAct { window, .. }
            | Self::ComputerSequence { window, .. }
            | Self::ComputerWait { window, .. }
            | Self::ComputerCapture { window, .. } => Some(window),
            _ => None,
        }
    }
}
fn valid_window(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 64 {
        Err(invalid("Некорректный window"))
    } else {
        Ok(())
    }
}
fn validate_condition(condition: Option<&Condition>) -> Result<()> {
    if let Some(condition) = condition {
        match condition {
            Condition::ElementExists { selector }
            | Condition::ElementAbsent { selector }
            | Condition::ElementFocused { selector }
            | Condition::TextContains { selector, .. } => {
                selector.validate()?;
                if selector.is_empty() {
                    return Err(invalid("Нужен селектор условия"));
                }
            }
            Condition::ImageChanged { frame_id } if frame_id.len() > 128 => {
                return Err(invalid("Некорректный frame_id"))
            }
            _ => {}
        }
        if let Condition::TextContains { text, .. } = condition {
            if text.is_empty() || text.len() > 512 {
                return Err(invalid("Некорректный текст условия"));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub tools: u64,
    pub screenshots: u64,
    pub images_sent: u64,
    pub actions: u64,
    pub verified: u64,
    pub denied: u64,
    pub failures: u64,
    pub context_bytes: u64,
    pub image_bytes: u64,
    pub latency_ms: u64,
    pub cache_hits: u64,
    pub uia_calls: u64,
    pub ocr_calls: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Audit {
    pub at_ms: u64,
    pub tool: String,
    pub action: Option<String>,
    pub execution: String,
    pub verification: String,
    pub latency_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pending {
    pub id: String,
    pub account: String,
    pub session: String,
    pub window: Option<String>,
    pub tool: String,
    pub action: Option<String>,
    pub sensitive: bool,
    pub created_ms: u64,
    pub description: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Status {
    pub executing: bool,
    pub available: bool,
    pub enabled: bool,
    pub stopped: bool,
    pub owner_session: Option<String>,
    pub windows: Vec<WindowInfo>,
    pub pending: Option<Pending>,
    pub metrics: Metrics,
    pub audit: Vec<Audit>,
    pub detail: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolResult {
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_actions_and_bounds() {
        assert!(Request::parse(
            "computer_act",
            json!({"window":"x","action":{"kind":"type","text":"ok"},"verify":null})
        )
        .is_ok());
        assert!(Request::parse("computer_act",json!({"window":"x","action":{"kind":"type","text":"ok","execute":"evil"},"verify":null})).is_err());
        assert!(Request::parse(
            "computer_capture",
            json!({"window":"x","max_edge":20000,"roi":null})
        )
        .is_err());
        assert!(Request::parse(
            "computer_find",
            json!({"window":"x","selector":{},"limit":20})
        )
        .is_err());
        assert!(Rect {
            x: -2000,
            y: 0,
            width: 2000,
            height: 1000
        }
        .contains(-1, 4));
    }
}
