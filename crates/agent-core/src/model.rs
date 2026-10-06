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
    TurnStarted {
        prompt: String,
    },
    AssistantTextDelta {
        text: String,
    },
    ToolActivity {
        label: String,
        detail: String,
    },
    /// The agent asks the user before acting. Shown verbatim; never auto-approved by the app.
    ApprovalRequested {
        id: String,
        kind: String,
        title: String,
        detail: String,
        cwd: Option<String>,
        reason: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageWindow {
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
