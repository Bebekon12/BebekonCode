use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub root: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AccountProfile {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub credential_ref: Option<String>,
    pub config_dir: Option<String>,
    pub auth_status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: String,
    pub workspace_id: String,
    pub provider: String,
    pub account_profile_id: String,
    pub model: String,
    pub title: String,
    pub status: String,
    pub permission_profile: String,
    pub working_directory: String,
    pub provider_session_id: Option<String>,
    pub worktree_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub fast_mode: bool,
    pub tool_policy: String,
    pub parent_session_id: Option<String>,
    pub chat_mode: String,
    pub role: String,
    pub context_summary: String,
    #[serde(default = "empty_settings")]
    #[sqlx(default)]
    pub conversation_settings: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolPolicy {
    pub plugins: Option<Vec<String>>,
    pub mcp_servers: Option<Vec<String>>,
    pub skills: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub provider: String,
    pub account_profile_id: String,
    pub model: String,
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub fast_mode: bool,
    pub permission_profile: String,
    #[serde(default)]
    pub tools: ToolPolicy,
    #[serde(default)]
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChat {
    pub workspace_id: Option<String>,
    pub mode: String,
    pub agents: Vec<AgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSession {
    pub workspace_id: String,
    pub provider: String,
    pub account_profile_id: String,
    pub model: String,
    pub permission_profile: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventPayload {
    ContextUsage {
        used_tokens: Option<u64>,
        window_tokens: Option<u64>,
        source: String,
    },
    ContextCompacted {},
    UserFollowup {
        prompt: String,
        files: Vec<crate::attachments::AttachedFile>,
    },
    QueueChanged,
    TurnStarted {
        prompt: String,
    },
    AssistantTextDelta {
        text: String,
    },
    ProgressDelta {
        text: String,
    },
    TeamMessage {
        session_id: String,
        #[serde(default)]
        stage_id: String,
        title: String,
        text: String,
        model: Option<String>,
        reasoning_effort: Option<String>,
    },
    ModelResolved {
        model: String,
    },
    UserAttachments {
        files: Vec<crate::attachments::AttachedFile>,
    },
    AgentConfiguration {
        provider: String,
        model: String,
        account_profile_id: String,
        reasoning_effort: Option<String>,
        #[serde(default)]
        fast_mode: bool,
    },
    ToolActivity {
        label: String,
        detail: String,
    },
    /// One tool call with a short redacted summary (path, pattern or command line).
    /// File contents and tool output are never stored.
    ToolStarted {
        id: String,
        tool: String,
        /// command, read, edit, create, write, search, web, agent, mcp or other.
        kind: String,
        summary: String,
        #[serde(default)]
        added: Option<u32>,
        #[serde(default)]
        removed: Option<u32>,
    },
    ToolFinished {
        id: String,
        failed: bool,
    },
    /// The agent's own task list (TodoWrite replaces it, Task tools update single items).
    TodoUpdate {
        replace: bool,
        items: Vec<TodoItem>,
    },
    RunChanges {
        files: Vec<ChangedFile>,
        limited: bool,
    },
    ProviderRunStarted {
        provider: String,
        model: String,
        purpose: String,
        resumed: bool,
        prompt_bytes: u64,
    },
    ProviderUsage {
        provider: String,
        model_requests: Option<u64>,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
        cache_creation_tokens: Option<u64>,
        #[serde(default)]
        reasoning_tokens: Option<u64>,
        #[serde(default)]
        incomplete: bool,
    },
    /// One bounded, content-free report per Claude adapter invocation, including failures.
    ProviderDiagnostic {
        report: Box<ProviderRunDiagnostic>,
    },
    /// The agent asks the user before acting. Shown verbatim; never auto-approved by the app.
    ApprovalRequested {
        id: String,
        kind: String,
        title: String,
        detail: String,
        cwd: Option<String>,
        reason: Option<String>,
        #[serde(default)]
        available_decisions: Option<Vec<ApprovalDecision>>,
    },
    ApprovalResolved {
        id: String,
        decision: String,
    },
    TurnCompleted,
    SessionStopped,
    ProviderError {
        message: String,
        /// `usage_limit`, `auth` or absent. Never triggers an automatic account switch.
        #[serde(default)]
        kind: Option<String>,
    },
    /// Internal: binds the session to the provider's own conversation id. Never persisted.
    ProviderSession {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRunDiagnostic {
    pub request_id: String,
    pub provider: String,
    pub account_profile_id: String,
    pub session_id: String,
    pub provider_session_id: Option<String>,
    pub requested_model: String,
    pub resolved_model: Option<String>,
    pub effort: Option<String>,
    pub permission_profile: String,
    pub fast_mode: bool,
    pub resumed: bool,
    pub prompt_bytes: u64,
    pub elapsed_ms: u64,
    pub tool_requests: u64,
    pub approval_requests: u64,
    pub reported_retries: u64,
    pub status: String,
    pub error_kind: Option<String>,
    pub usage_reported: bool,
    #[serde(default)]
    pub usage_incomplete: bool,
    #[serde(default)]
    pub subagent_events: u64,
    pub model_requests: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    AllowOnce,
    AllowSession,
    Deny,
}

/// Live account state reported by the provider. Values the provider does not report stay empty
/// instead of being estimated.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountStatus {
    #[serde(default)]
    pub sandbox: Option<SandboxStatus>,
    #[serde(default)]
    pub usage_detail: Option<String>,
    #[serde(default)]
    pub usage_error: Option<String>,
    #[serde(default)]
    pub auth_mode: Option<String>,
    pub account_id: String,
    /// `signed_in`, `signed_out`, `not_required`, `unavailable` or `error`.
    pub state: String,
    pub email: Option<String>,
    pub plan: Option<String>,
    pub usage: Vec<UsageWindow>,
    pub limit_reached: Option<String>,
    pub credits: Option<String>,
    pub message: Option<String>,
    /// Whether requests are authorized to use the user's ChatGPT plan (granted OAuth scope).
    pub plan_usage_enabled: Option<bool>,
    /// Official page where the provider shows usage and limits for this account.
    pub manage_usage_url: Option<String>,
    pub checked_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxStatus {
    /// ready, not_configured, update_required, setting_up, unavailable or error.
    pub state: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageWindow {
    #[serde(default)]
    pub label: Option<String>,
    pub window_minutes: Option<i64>,
    pub used_percent: f64,
    pub resets_at: Option<i64>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_default: bool,
    pub reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: Option<String>,
    #[serde(default)]
    pub fast_mode_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TodoItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    /// pending, in_progress, completed or deleted.
    #[serde(default)]
    pub status: Option<String>,
}

/// Counts only; file content and patches are never persisted for change-card snapshots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Extensions {
    pub plugins: Vec<ExtensionItem>,
    pub mcp_servers: Vec<ExtensionItem>,
    pub skills: Vec<ExtensionItem>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionItem {
    pub id: String,
    pub name: String,
    pub detail: Option<String>,
    pub enabled: bool,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginStart {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountEvent {
    pub account_id: String,
    /// `login_completed`, `login_failed`, `updated`, `usage` or `notice`.
    pub kind: String,
    pub status: Option<AccountStatus>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub sequence: i64,
    pub session_id: String,
    pub run_id: String,
    pub timestamp: i64,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
    pub detected_path: Option<String>,
    pub detail: String,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub check_updates_on_start: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub workspaces: Vec<Workspace>,
    pub sessions: Vec<Session>,
    pub accounts: Vec<AccountProfile>,
    pub providers: Vec<ProviderInfo>,
    pub settings: Settings,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

fn empty_settings() -> String {
    "{}".into()
}
