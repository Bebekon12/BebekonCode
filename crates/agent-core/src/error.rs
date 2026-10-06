use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Local storage operation failed")]
    Storage(#[from] sqlx::Error),
    #[error("Database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("Invalid request: {0}")]
    Invalid(String),
    #[error("Record not found")]
    NotFound,
    #[error("This session already has an active turn")]
    Busy,
    #[error("This provider is not integrated yet. Select the local demo provider.")]
    ProviderUnavailable,
    #[error("Filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("Event encoding failed")]
    Json(#[from] serde_json::Error),
    #[error("Git operation failed or Git is not installed")]
    Git,
    #[error("GitHub release check failed. Check your connection and try again.")]
    UpdateNetwork,
    #[error("GitHub API limit reached. Try again later.")]
    UpdateRateLimit,
    #[error("GitHub returned invalid release metadata")]
    UpdateMetadata,
    #[error("OS credential store is unavailable")]
    CredentialStore,
    #[error("Capability provider is not configured")]
    CapabilityUnavailable,
}
pub type Result<T> = std::result::Result<T, CoreError>;
