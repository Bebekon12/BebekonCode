//! Context and follow-up controls are owned by the core, independent of the client.
use crate::{model::*, provider::TurnRequest, Core, CoreError, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, oneshot};

pub fn public_link(value: &str) -> Result<String> {
    let url =
        reqwest::Url::parse(value).map_err(|_| CoreError::Invalid("Некорректная ссылка".into()))?;
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    if value.len() > 4096
        || url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|port| port != 443)
        || host.contains(':')
        || !host.contains('.')
        || host.parse::<std::net::IpAddr>().is_ok()
        || [".localhost", ".local", ".internal", ".test", ".invalid"]
            .iter()
            .any(|suffix| host.ends_with(suffix))
    {
        return Err(CoreError::Invalid(
            "Доступны только публичные HTTPS-ссылки".into(),
        ));
    }
    Ok(url.to_string())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConversationSettings {
    pub compact_tokens: Option<u64>,
    pub claude_compact_percent: Option<u8>,
    pub response_style: ResponseStyle,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStyle {
    #[default]
    Standard,
    Concise,
    Detailed,
    Visual,
}
impl ConversationSettings {
    pub fn read(session: &Session) -> Result<Self> {
        let value: Self = serde_json::from_str(&session.conversation_settings)?;
        value.validate(&session.provider)?;
        Ok(value)
    }
    pub fn validate(&self, provider: &str) -> Result<()> {
        if self
            .compact_tokens
            .is_some_and(|n| !(100_000..=1_000_000).contains(&n))
        {
            return Err(CoreError::Invalid(
                "Ориентир сжатия: от 100 000 до 1 000 000 токенов".into(),
            ));
        }
        if self
            .claude_compact_percent
            .is_some_and(|n| !(1..=95).contains(&n))
        {
            return Err(CoreError::Invalid(
                "Порог Claude: от 1 до 95 процентов".into(),
            ));
        }
        if (self.compact_tokens.is_some() && !["openai", "anthropic"].contains(&provider))
            || (self.claude_compact_percent.is_some() && provider != "anthropic")
        {
            return Err(CoreError::Invalid(
                "Эта настройка контекста недоступна провайдеру".into(),
            ));
        }
        Ok(())
    }
    pub fn guidance(&self) -> &'static str {
        match self.response_style {
            ResponseStyle::Standard => "",
            ResponseStyle::Concise => "Предпочтение пользователя: отвечай кратко, начинай с результата, сохраняй существенные ограничения и доказательства. Не перечисляй служебные шаги без необходимости.",
            ResponseStyle::Detailed => "Предпочтение пользователя: давай связный, структурированный ответ с объяснениями и конкретными примерами по необходимости. Отделяй проверенные факты от предположений, используй ясные заголовки и таблицы для сравнений; избегай повторов.",
            ResponseStyle::Visual => "Предпочтение пользователя: ясный, красиво структурированный ответ; сначала главный результат, затем полезные детали. Используй Markdown, таблицы и изображения только когда они помогают. Для существующего изображения используй ![подпись](https://реальный-адрес), укажи источник ссылкой; никогда не выдумывай URL или данные и не генерируй изображение без необходимости. Картинка и следующий абзац могут отображаться рядом. Для графика из известных данных можно дать ```bebekon-chart с JSON {\"title\":\"Название\",\"items\":[{\"label\":\"Категория\",\"value\":4}]} на новой строке. Для линейного графика используй bebekon-line с такой же схемой. Не превращай короткий ответ в отчёт и не показывай скрытые рассуждения.",
        }
    }
}

pub struct Followup {
    pub prompt: String,
    pub attachments: Vec<crate::attachments::AttachedFile>,
    pub acknowledged: oneshot::Sender<Result<()>>,
}
pub type LiveReceiver = Arc<tokio::sync::Mutex<mpsc::Receiver<Followup>>>;
pub(crate) type LiveRegistry = Arc<Mutex<HashMap<String, (String, mpsc::Sender<Followup>)>>>;
pub(crate) struct LiveLease {
    registry: LiveRegistry,
    session: String,
    run: String,
}
impl Drop for LiveLease {
    fn drop(&mut self) {
        if let Ok(mut entries) = self.registry.lock() {
            if entries
                .get(&self.session)
                .is_some_and(|(run, _)| run == &self.run)
            {
                entries.remove(&self.session);
            }
        }
    }
}
impl Core {
    pub async fn configure_conversation(
        &self,
        id: &str,
        settings: ConversationSettings,
    ) -> Result<Session> {
        let _lease = self.lease(id, Default::default())?;
        let session = self.storage.session(id).await?;
        if let Some(parent) = &session.parent_session_id {
            if self
                .runs
                .lock()
                .map_err(|_| CoreError::Busy)?
                .contains_key(parent)
            {
                return Err(CoreError::Busy);
            }
        }
        settings.validate(&session.provider)?;
        sqlx::query("UPDATE sessions SET conversation_settings=? WHERE id=?")
            .bind(serde_json::to_string(&settings)?)
            .bind(id)
            .execute(&self.storage.pool)
            .await?;
        self.storage.session(id).await
    }
    pub(crate) fn live_input(&self, request: &mut TurnRequest, run: &str) -> Option<LiveLease> {
        // Only the implementing lead receives live input. Providers decide the safe delivery
        // boundary; application-level next-turn queuing remains a separate operation.
        if request.prompt == "/compact"
            || !["openai", "anthropic"].contains(&request.session.provider.as_str())
            || request.session.parent_session_id.is_some()
        {
            return None;
        }
        let (tx, rx) = mpsc::channel(8);
        self.live_inputs
            .lock()
            .ok()?
            .insert(request.session.id.clone(), (run.into(), tx));
        request.live_input = Some(Arc::new(tokio::sync::Mutex::new(rx)));
        Some(LiveLease {
            registry: self.live_inputs.clone(),
            session: request.session.id.clone(),
            run: run.into(),
        })
    }
    pub async fn steer_message(
        &self,
        id: &str,
        expected_run: &str,
        prompt: String,
        attachments: Vec<crate::attachments::Attachment>,
    ) -> Result<()> {
        if prompt.trim().is_empty() || crate::redaction::redact(&prompt) != prompt {
            return Err(CoreError::Invalid(
                "Пустое сообщение или защищённые данные".into(),
            ));
        }
        let session = self.storage.session(id).await?;
        if !["openai", "anthropic"].contains(&session.provider.as_str())
            || session.parent_session_id.is_some()
        {
            return Err(CoreError::Invalid("Живое уточнение доступно ведущему Claude/Codex. Используйте очередь следующего сообщения.".into()));
        }
        let tx=self.live_inputs.lock().map_err(|_|CoreError::Busy)?.get(id)
            .filter(|(run,_)|run==expected_run).map(|(_,tx)|tx.clone())
            .ok_or_else(||CoreError::Invalid("Активный ход уже завершён или ещё не готов принять уточнение. Сообщение осталось в редакторе.".into()))?;
        let files =
            tokio::task::spawn_blocking(move || crate::attachments::prepare(&session, attachments))
                .await
                .map_err(|_| CoreError::Busy)??;
        let (ack, rx) = oneshot::channel();
        tx.try_send(Followup {
            prompt: crate::attachments::prompt_with_files(&prompt, &files),
            attachments: files.clone(),
            acknowledged: ack,
        })
        .map_err(|_| {
            CoreError::Invalid(
                "Ход завершился или очередь уточнений заполнена. Сообщение не отправлено.".into(),
            )
        })?;
        rx.await.map_err(|_| {
            CoreError::Invalid(
                "Провайдер не подтвердил уточнение. Автоматического повтора нет.".into(),
            )
        })??;
        self.emit(
            id,
            expected_run,
            EventPayload::UserFollowup { prompt, files },
            None,
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_options_do_not_invent_provider_support_or_window_capacity() {
        let mut settings = ConversationSettings::default();
        assert!(settings.validate("mock").is_ok());
        settings.compact_tokens = Some(99_999);
        assert!(settings.validate("anthropic").is_err());
        settings.compact_tokens = Some(200_000);
        assert!(settings.validate("openai").is_ok());
        assert!(settings.validate("mock").is_err());
        settings.claude_compact_percent = Some(70);
        assert!(settings.validate("openai").is_err());
        assert!(settings.validate("anthropic").is_ok());
        settings.claude_compact_percent = Some(0);
        assert!(settings.validate("anthropic").is_err());
    }
    #[test]
    fn response_links_never_open_local_files_commands_or_credentials() {
        for url in [
            "file:///C:/secret",
            "javascript:alert(1)",
            "http://example.com",
            "https://127.0.0.1/a",
            "https://[::1]/",
            "https://host.local/a",
            "https://u:secret@example.com/a",
            "https://example.com:8443/a",
        ] {
            assert!(public_link(url).is_err(), "{url}");
        }
        assert!(public_link("https://images.example.com/photo.png").is_ok());
    }
    #[tokio::test]
    async fn settings_persist_and_busy_chat_rejects_mutation() {
        let temp = crate::test_support::TestDirectory::new().unwrap();
        let core = Core::open_with_providers(
            &temp.path().join("workspace.db"),
            vec![Arc::new(crate::provider::MockProvider)],
        )
        .await
        .unwrap();
        let workspace = core
            .add_workspace(&temp.path().to_string_lossy())
            .await
            .unwrap();
        let session = core
            .create_session(CreateSession {
                workspace_id: workspace.id,
                provider: "mock".into(),
                account_profile_id: "mock-local".into(),
                model: "mock-stream-v1".into(),
                permission_profile: "standard".into(),
            })
            .await
            .unwrap();
        let settings = ConversationSettings {
            response_style: ResponseStyle::Visual,
            ..Default::default()
        };
        core.configure_conversation(&session.id, settings.clone())
            .await
            .unwrap();
        assert!(matches!(
            ConversationSettings::read(&core.storage.session(&session.id).await.unwrap())
                .unwrap()
                .response_style,
            ResponseStyle::Visual
        ));
        let lease = core.lease(&session.id, Default::default()).unwrap();
        assert!(core
            .configure_conversation(&session.id, settings)
            .await
            .is_err());
        assert!(core
            .steer_message(&session.id, "stale", "Wrong turn".into(), vec![])
            .await
            .is_err());
        drop(lease);
        core.shutdown().await;
    }
}
