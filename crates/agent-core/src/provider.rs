use crate::{
    error::Result,
    model::{EventPayload, ProviderInfo, Session},
};
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct TurnRequest {
    pub prompt: String,
    pub session: Session,
}

#[async_trait]
pub trait AgentProvider: Send + Sync {
    fn info(&self) -> ProviderInfo;
    async fn run(
        &self,
        request: TurnRequest,
        events: mpsc::Sender<EventPayload>,
        cancel: CancellationToken,
    ) -> Result<()>;
}

pub struct MockProvider;

#[async_trait]
impl AgentProvider for MockProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "mock".into(),
            name: "Local demo".into(),
            available: true,
            detected_path: None,
            detail: "Deterministic local simulator. No AI requests, tools, or file changes.".into(),
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
            label: "Simulated planning step".into(),
            detail: "Demo activity only. No commands were executed and no project files were read."
                .into(),
        };
        if events.send(activity).await.is_err() {
            return Ok(());
        }
        let subject: String = request.prompt.chars().take(100).collect();
        let response = format!("This is a local demo response to: “{subject}”.\n\nThe workspace is ready for independent agent sessions. Each session keeps its provider, account and permission profile, and its timeline is stored locally in SQLite.\n\nThis simulator demonstrates streaming and cancellation. It does not inspect your repository, execute tools or modify files. Connect an official provider adapter in a later milestone to perform real coding tasks.");
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
    for (id, name, binary) in [
        ("openai", "OpenAI / Codex", "codex"),
        ("anthropic", "Anthropic / Claude Code", "claude"),
    ] {
        let path = which::which(binary)
            .ok()
            .map(|path| path.to_string_lossy().into_owned());
        let detail = if path.is_some() {
            "CLI detected. Official adapter and isolated sign-in are not integrated yet."
        } else {
            "CLI not detected. Install the official CLI; integration is planned for the next milestone."
        };
        providers.push(ProviderInfo {
            id: id.into(),
            name: name.into(),
            available: false,
            detected_path: path,
            detail: detail.into(),
            models: vec![],
        });
    }
    providers
}
