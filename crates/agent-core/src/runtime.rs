use crate::{
    model::*,
    provider::{detect_providers, AgentProvider, MockProvider, TurnRequest},
    storage::Storage,
    CoreError, Result,
};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct Core {
    pub storage: Storage,
    engines: HashMap<String, Arc<dyn AgentProvider>>,
    events: broadcast::Sender<Event>,
    runs: Mutex<HashMap<String, CancellationToken>>,
    providers: Mutex<Vec<ProviderInfo>>,
}

impl Core {
    pub async fn open(database: &Path) -> Result<Arc<Self>> {
        Self::open_with_providers(database, vec![Arc::new(MockProvider)]).await
    }
    pub async fn open_with_providers(
        database: &Path,
        providers: Vec<Arc<dyn AgentProvider>>,
    ) -> Result<Arc<Self>> {
        let storage = Storage::open(database).await?;
        let mut engines = HashMap::new();
        for provider in providers {
            if engines.insert(provider.info().id, provider).is_some() {
                return Err(CoreError::Invalid("Duplicate provider registration".into()));
            }
        }
        let mut detected = detect_providers();
        for engine in engines.values() {
            detected.retain(|info| info.id != engine.info().id);
            detected.insert(0, engine.info());
        }
        let (events, _) = broadcast::channel(512);
        Ok(Arc::new(Self {
            storage,
            engines,
            events,
            runs: Mutex::new(HashMap::new()),
            providers: Mutex::new(detected),
        }))
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }
    pub async fn snapshot(&self) -> Result<Snapshot> {
        let providers = self.providers.lock().map_err(|_| CoreError::Busy)?.clone();
        Ok(Snapshot {
            workspaces: self.storage.workspaces().await?,
            sessions: self.storage.sessions().await?,
            accounts: self.storage.accounts().await?,
            providers,
            settings: self.storage.settings().await?,
        })
    }
    pub fn refresh_providers(&self) -> Result<Vec<ProviderInfo>> {
        let mut detected = detect_providers();
        for engine in self.engines.values() {
            detected.retain(|info| info.id != engine.info().id);
            detected.insert(0, engine.info());
        }
        *self.providers.lock().map_err(|_| CoreError::Busy)? = detected.clone();
        Ok(detected)
    }
    pub async fn add_workspace(&self, root: &str) -> Result<Workspace> {
        if root.trim().is_empty() {
            return Err(CoreError::Invalid("Choose a project folder".into()));
        }
        let root = Path::new(root).canonicalize()?;
        if !root.is_dir() {
            return Err(CoreError::Invalid("Workspace must be a directory".into()));
        }
        let name = root
            .file_name()
            .map(|v| v.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Project".into());
        let root = root.to_string_lossy().into_owned();
        let workspace = Workspace {
            id: Uuid::new_v4().to_string(),
            name,
            root,
            created_at: now(),
        };
        sqlx::query("INSERT INTO workspaces VALUES (?, ?, ?, ?) ON CONFLICT(root) DO NOTHING")
            .bind(&workspace.id)
            .bind(&workspace.name)
            .bind(&workspace.root)
            .bind(workspace.created_at)
            .execute(&self.storage.pool)
            .await?;
        Ok(sqlx::query_as("SELECT * FROM workspaces WHERE root = ?")
            .bind(&workspace.root)
            .fetch_one(&self.storage.pool)
            .await?)
    }
    pub async fn create_session(&self, input: CreateSession) -> Result<Session> {
        let provider = self
            .engines
            .get(&input.provider)
            .ok_or(CoreError::ProviderUnavailable)?;
        if !provider.info().models.contains(&input.model) {
            return Err(CoreError::Invalid("Model is unavailable".into()));
        }
        let accounts = self.storage.accounts().await?;
        if !accounts.iter().any(|account| {
            account.id == input.account_profile_id && account.provider == input.provider
        }) {
            return Err(CoreError::Invalid(
                "Account does not belong to the selected provider".into(),
            ));
        }
        if !["standard", "read_only"].contains(&input.permission_profile.as_str()) {
            return Err(CoreError::Invalid("Unknown permission profile".into()));
        }
        let workspace = self.storage.workspace(&input.workspace_id).await?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        sqlx::query("INSERT INTO sessions (id, workspace_id, provider, account_profile_id, model, title, status, permission_profile, working_directory, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'New session', 'idle', ?, ?, ?, ?)")
            .bind(&id).bind(&workspace.id).bind(&input.provider).bind(&input.account_profile_id)
            .bind(&input.model).bind(&input.permission_profile).bind(&workspace.root)
            .bind(timestamp).bind(timestamp).execute(&self.storage.pool).await?;
        self.storage.session(&id).await
    }
    pub async fn send_message(
        self: &Arc<Self>,
        session_id: &str,
        prompt: String,
    ) -> Result<String> {
        if prompt.trim().is_empty() || prompt.len() > 32_000 {
            return Err(CoreError::Invalid(
                "Message must contain 1–32,000 bytes".into(),
            ));
        }
        let session = self.storage.session(session_id).await?;
        let provider = self
            .engines
            .get(&session.provider)
            .cloned()
            .ok_or(CoreError::ProviderUnavailable)?;
        let cancel = CancellationToken::new();
        {
            let mut runs = self.runs.lock().map_err(|_| CoreError::Busy)?;
            if runs.contains_key(session_id) {
                return Err(CoreError::Busy);
            }
            runs.insert(session_id.into(), cancel.clone());
        }
        let run = Uuid::new_v4().to_string();
        let started = self
            .storage
            .append(
                session_id,
                &run,
                EventPayload::TurnStarted {
                    prompt: prompt.clone(),
                },
                Some("running"),
            )
            .await;
        let event = match started {
            Ok(event) => event,
            Err(error) => {
                if let Ok(mut runs) = self.runs.lock() {
                    runs.remove(session_id);
                }
                return Err(error);
            }
        };
        let _ = self.events.send(event);
        if session.title == "New session" {
            let title: String = prompt.trim().chars().take(64).collect();
            // A failed title update must not strand a running turn.
            if sqlx::query("UPDATE sessions SET title = ? WHERE id = ?")
                .bind(title)
                .bind(session_id)
                .execute(&self.storage.pool)
                .await
                .is_err()
            {
                tracing::warn!(code = "session_title_write_failed");
            }
        }
        let core = Arc::clone(self);
        let result = run.clone();
        tokio::spawn(async move {
            core.run_turn(provider, session, run, prompt, cancel).await;
        });
        Ok(result)
    }
    async fn run_turn(
        &self,
        provider: Arc<dyn AgentProvider>,
        session: Session,
        run: String,
        prompt: String,
        cancel: CancellationToken,
    ) {
        let (tx, mut rx) = mpsc::channel(64);
        let session_id = session.id.clone();
        let request = TurnRequest { prompt, session };
        let session = session_id;
        let child_cancel = cancel.clone();
        let producer = tokio::spawn(async move { provider.run(request, tx, child_cancel).await });
        let mut failed = false;
        while let Some(payload) = rx.recv().await {
            if cancel.is_cancelled() {
                break;
            }
            match self.storage.append(&session, &run, payload, None).await {
                Ok(event) => {
                    let _ = self.events.send(event);
                }
                Err(_) => {
                    failed = true;
                    cancel.cancel();
                    break;
                }
            }
        }
        drop(rx);
        let result = producer.await;
        failed |= !matches!(result, Ok(Ok(())));
        let (payload, status) = if failed {
            (
                EventPayload::ProviderError {
                    message: "The turn failed. Local storage or provider runtime is unavailable."
                        .into(),
                },
                "failed",
            )
        } else if cancel.is_cancelled() {
            (EventPayload::SessionStopped, "stopped")
        } else {
            (EventPayload::TurnCompleted, "completed")
        };
        match self
            .storage
            .append(&session, &run, payload, Some(status))
            .await
        {
            Ok(event) => {
                let _ = self.events.send(event);
            }
            Err(_) => {
                tracing::error!(code = "turn_finalization_failed");
            }
        }
        if let Ok(mut runs) = self.runs.lock() {
            runs.remove(&session);
        }
    }
    pub fn cancel(&self, session: &str) -> Result<()> {
        if let Some(token) = self.runs.lock().map_err(|_| CoreError::Busy)?.get(session) {
            token.cancel();
        }
        Ok(())
    }
    pub fn cancel_all(&self) {
        if let Ok(runs) = self.runs.lock() {
            for token in runs.values() {
                token.cancel();
            }
        }
    }
}
