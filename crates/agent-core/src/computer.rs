//! Desktop capability belongs to the core; Tauri only forwards explicit UI decisions.
use crate::{Core, CoreError, Result};
pub use smart_computer::{
    manager::entry,
    protocol::{Status, WindowInfo},
};
impl Core {
    pub async fn computer_status(&self) -> Result<Status> {
        match self.computer.status().await {
            Ok(s) => Ok(s),
            Err(smart_computer::Error::Unavailable) => Ok(Status {
                available: cfg!(windows),
                detail: "Управление компьютером выключено".into(),
                ..Status::default()
            }),
            Err(e) => Err(CoreError::Invalid(e.to_string())),
        }
    }
    pub async fn computer_windows(&self) -> Result<Vec<WindowInfo>> {
        self.computer
            .choices()
            .await
            .map_err(|e| CoreError::Invalid(e.to_string()))
    }
    pub async fn configure_computer(&self, session_id: &str, windows: Vec<String>) -> Result<()> {
        let _lease = self.lease(session_id, tokio_util::sync::CancellationToken::new())?;
        let session = self.storage.session(session_id).await?;
        if session.provider != "openai"
            || session.parent_session_id.is_some()
            || session.chat_mode != "single"
            || !session.role.is_empty()
            || session.status == "running"
        {
            return Err(CoreError::Invalid(
                "Smart Computer доступен для остановленного обычного чата Codex".into(),
            ));
        }
        let account = self.account(&session.account_profile_id).await?;
        if account.provider != "openai" {
            return Err(CoreError::Invalid(
                "Аккаунт чата не поддерживает Smart Computer".into(),
            ));
        }
        self.computer
            .configure(
                account.id,
                session.id,
                windows,
                session.permission_profile == "read_only",
            )
            .await
            .map_err(|e| CoreError::Invalid(e.to_string()))
    }
    pub async fn stop_computer(&self) -> Result<()> {
        self.computer
            .stop()
            .await
            .map_err(|e| CoreError::Invalid(e.to_string()))
    }
    pub async fn approve_computer(&self, id: String, allow: bool, sensitive: bool) -> Result<()> {
        self.computer
            .approve(id, allow, sensitive)
            .await
            .map_err(|e| CoreError::Invalid(e.to_string()))
    }
}
