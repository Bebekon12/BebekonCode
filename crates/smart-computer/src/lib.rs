//! Local, approval-gated desktop capability. No model, HTTP endpoint or credentials.
pub mod engine;
pub mod ipc;
pub mod manager;
pub mod mcp;
pub mod protocol;
pub mod vision;
#[cfg(windows)]
pub mod windows;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("Доступ к компьютеру отклонён")]
    Denied,
    #[error("Desktop Host недоступен или остановлен")]
    Unavailable,
    #[error("Управление занято другим агентом")]
    Busy,
    #[error("Операция остановлена или истекло время ожидания")]
    Cancelled,
    #[error("Ошибка локального IPC")]
    Io(#[from] std::io::Error),
    #[error("Некорректный запрос Smart Computer")]
    Json(#[from] serde_json::Error),
}
pub fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}
