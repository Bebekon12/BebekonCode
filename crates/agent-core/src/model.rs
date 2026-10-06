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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventPayload {
    TurnStarted { prompt: String },
    AssistantTextDelta { text: String },
    ToolActivity { label: String, detail: String },
    TurnCompleted,
    SessionStopped,
    ProviderError { message: String },
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
