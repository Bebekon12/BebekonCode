use crate::{
    error::{CoreError, Result},
    model::{
        AccountProfile, AccountStatus, ApprovalDecision, EventPayload, Extensions, LoginStart,
        ModelInfo, ProviderInfo, Session,
    },
};
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct TurnRequest {
    pub prompt: String,
    pub session: Session,
    /// The account bound to the session at creation. Providers must never substitute another.
    pub account: AccountProfile,
}

/// A provider engine. Everything beyond `info` and `run` has a conservative default, so a new
/// provider can be added without touching the session manager and unsupported features stay
/// visibly unavailable.
#[async_trait]
pub trait AgentProvider: Send + Sync {
    fn info(&self) -> ProviderInfo;

    /// Re-detects the provider binary and version. Called on startup and on explicit refresh.
    async fn refresh(&self) {}

    async fn run(
        &self,
        request: TurnRequest,
        events: mpsc::Sender<EventPayload>,
        cancel: CancellationToken,
    ) -> Result<()>;

    /// Whether accounts of this provider are created by the user and need sign-in.
    fn manages_accounts(&self) -> bool {
        false
    }

    async fn models(&self, _account: &AccountProfile) -> Result<Vec<ModelInfo>> {
        Ok(self
            .info()
            .models
            .into_iter()
            .map(|id| ModelInfo {
                name: id.clone(),
                id,
                description: String::new(),
                is_default: false,
            })
            .collect())
    }

    async fn account_status(&self, account: &AccountProfile) -> Result<AccountStatus> {
        Ok(AccountStatus {
            account_id: account.id.clone(),
            state: "not_required".into(),
            checked_at: crate::model::now(),
            ..AccountStatus::default()
        })
    }

    async fn login(&self, _account: &AccountProfile) -> Result<LoginStart> {
        Err(CoreError::Invalid(
            "Этот провайдер не поддерживает вход".into(),
        ))
    }

    async fn logout(&self, _account: &AccountProfile) -> Result<()> {
        Ok(())
    }

    async fn extensions(&self, _account: &AccountProfile) -> Result<Extensions> {
        Ok(Extensions::default())
    }

    async fn resolve_approval(
        &self,
        _session_id: &str,
        _approval_id: &str,
        _decision: ApprovalDecision,
    ) -> Result<()> {
        Err(CoreError::NotFound)
    }

    /// Stops background processes owned for this account, e.g. before the account is removed.
    async fn release_account(&self, _account: &AccountProfile) {}

    async fn shutdown(&self) {}
}

pub struct MockProvider;

#[async_trait]
impl AgentProvider for MockProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "mock".into(),
            name: "Локальное демо".into(),
            available: true,
            detected_path: None,
            detail:
                "Локальный симулятор. Без запросов к ИИ, запуска инструментов и изменений файлов."
                    .into(),
            models: vec!["mock-stream-v1".into()],
        }
    }
    async fn run(
        &self,
        request: TurnRequest,
        events: mpsc::Sender<EventPayload>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let activity = EventPayload::ToolActivity {
            label: "Демонстрация планирования".into(),
            detail: "Демонстрация действия. Команды не выполнялись, файлы проекта не читались."
                .into(),
        };
        if events.send(activity).await.is_err() {
            return Ok(());
        }
        let subject: String = request.prompt.chars().take(100).collect();
        let response = format!("Это ответ локального демо на задачу: «{subject}».\n\nРабочая область поддерживает независимые сессии. Каждая сессия привязана к своему провайдеру, аккаунту и профилю разрешений, а её история хранится локально в SQLite.\n\nСимулятор демонстрирует потоковый вывод и остановку. Он не читает репозиторий, не запускает инструменты и не меняет файлы. Для настоящих задач программирования потребуется официальный адаптер провайдера.");
        for word in response.split_inclusive(' ') {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => return Ok(()),
                _ = tokio::time::sleep(std::time::Duration::from_millis(35)) => {}
            }
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                result = events.send(EventPayload::AssistantTextDelta { text: word.into() }) => {
                    if result.is_err() { return Ok(()); }
                }
            }
        }
        Ok(())
    }
}

pub fn detect_providers() -> Vec<ProviderInfo> {
    let mut providers = vec![MockProvider.info()];
    let path = which::which("claude")
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    let detail = if path.is_some() {
        "Claude Code найден. Адаптер с изолированными профилями — следующий этап."
    } else {
        "Claude Code не найден. Установите официальный CLI."
    };
    providers.push(ProviderInfo {
        id: "anthropic".into(),
        name: "Anthropic / Claude Code".into(),
        available: false,
        detected_path: path,
        detail: detail.into(),
        models: vec![],
    });
    providers
}
