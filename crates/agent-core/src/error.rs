use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Не удалось выполнить операцию с локальным хранилищем")]
    Storage(#[from] sqlx::Error),
    #[error("Не удалось обновить структуру базы данных")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("Некорректный запрос: {0}")]
    Invalid(String),
    #[error("Запись не найдена")]
    NotFound,
    #[error("В этой сессии уже выполняется задача")]
    Busy,
    #[error("Этот провайдер недоступен. Проверьте установку CLI в настройках провайдеров.")]
    ProviderUnavailable,
    #[error("Не удалось выполнить операцию с файловой системой")]
    Io(#[from] std::io::Error),
    #[error("Не удалось сохранить событие")]
    Json(#[from] serde_json::Error),
    #[error("Ошибка операции Git или Git не установлен")]
    Git,
    #[error("Не удалось проверить обновления GitHub. Проверьте подключение и повторите попытку.")]
    UpdateNetwork,
    #[error("Достигнут лимит запросов GitHub API. Повторите попытку позже.")]
    UpdateRateLimit,
    #[error("GitHub вернул некорректные сведения о выпуске")]
    UpdateMetadata,
    #[error("Системное хранилище учётных данных недоступно")]
    CredentialStore,
    #[error("Провайдер этой возможности не настроен")]
    CapabilityUnavailable,
    /// A provider-reported failure, already redacted. `kind` lets the UI offer the right manual
    /// recovery (for example choosing another account); the core never retries elsewhere.
    #[error("{message}")]
    Provider {
        message: String,
        kind: Option<String>,
    },
}
pub type Result<T> = std::result::Result<T, CoreError>;
