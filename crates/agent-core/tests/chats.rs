use agent_core::{
    model::*,
    provider::{AgentProvider, TurnRequest},
    Core, CoreError, Result,
};
use async_trait::async_trait;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
#[path = "../src/test_support.rs"]
mod test_support;

#[derive(Default)]
struct Probe {
    requests: Mutex<Vec<TurnRequest>>,
    fail: AtomicBool,
    reported_failure: AtomicBool,
    active: AtomicUsize,
    peak: AtomicUsize,
    writing: AtomicUsize,
    peak_writing: AtomicUsize,
    parallel_barrier: Mutex<Option<Arc<tokio::sync::Barrier>>>,
}
struct Engine {
    id: &'static str,
    probe: Arc<Probe>,
}
#[async_trait]
impl AgentProvider for Engine {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: self.id.into(),
            name: self.id.into(),
            available: true,
            detected_path: None,
            detail: "Test only".into(),
            models: vec!["fast".into(), "deep".into()],
        }
    }
    fn manages_accounts(&self) -> bool {
        true
    }
    async fn models(&self, _: &AccountProfile) -> Result<Vec<ModelInfo>> {
        Ok(["fast", "deep"]
            .iter()
            .map(|id| ModelInfo {
                id: (*id).into(),
                name: (*id).into(),
                description: String::new(),
                is_default: *id == "fast",
                reasoning_efforts: vec!["low".into(), "high".into()],
                default_reasoning_effort: Some("low".into()),
            })
            .collect())
    }
    async fn run(
        &self,
        request: TurnRequest,
        tx: mpsc::Sender<EventPayload>,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.probe.requests.lock().unwrap().push(request.clone());
        if self.probe.fail.load(Ordering::SeqCst) {
            return Err(CoreError::Invalid("Test provider failed".into()));
        }
        let active = self.probe.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.probe.peak.fetch_max(active, Ordering::SeqCst);
        let writing = request.session.permission_profile != "read_only";
        if writing {
            let count = self.probe.writing.fetch_add(1, Ordering::SeqCst) + 1;
            self.probe.peak_writing.fetch_max(count, Ordering::SeqCst);
        }
        let barrier = self.probe.parallel_barrier.lock().unwrap().clone();
        if request.session.parent_session_id.is_some() {
            if let Some(barrier) = barrier {
                tokio::select! { _ = cancel.cancelled() => {}, _ = barrier.wait() => {} }
            }
        }
        tokio::select! { _=cancel.cancelled()=>{}, _=tokio::time::sleep(std::time::Duration::from_millis(80))=>{} }
        self.probe.active.fetch_sub(1, Ordering::SeqCst);
        if writing {
            self.probe.writing.fetch_sub(1, Ordering::SeqCst);
        }
        if cancel.is_cancelled() {
            return Ok(());
        }
        if self.probe.reported_failure.load(Ordering::SeqCst) {
            let _ = tx
                .send(EventPayload::ProviderError {
                    message: "Test usage limit".into(),
                    kind: Some("usage_limit".into()),
                })
                .await;
            return Ok(());
        }
        let text = if request.output_schema.is_some() {
            r#"{"tasks":[{"title":"Analysis","prompt":"Inspect","agent":0},{"title":"Review","prompt":"Check","agent":0}]}"#.into()
        } else if request.session.id.starts_with("context-") {
            "Goal: build widget; done: layout; next: tests".into()
        } else {
            format!("Result from {}: {}", self.id, request.session.model)
        };
        let _ = tx
            .send(EventPayload::ProviderSession {
                id: format!("provider-{}", request.session.id),
            })
            .await;
        let _ = tx.send(EventPayload::AssistantTextDelta { text }).await;
        Ok(())
    }
}
fn config(provider: &str, account: &str) -> AgentConfig {
    AgentConfig {
        provider: provider.into(),
        account_profile_id: account.into(),
        model: "fast".into(),
        reasoning_effort: Some("low".into()),
        permission_profile: "standard".into(),
        tools: ToolPolicy::default(),
        role: String::new(),
    }
}
async fn fixture() -> (test_support::TestDirectory, Arc<Core>, Arc<Probe>, String) {
    let dir = test_support::TestDirectory::new().unwrap();
    let probe = Arc::new(Probe::default());
    let core = Core::open_with_providers(
        &dir.path().join("chats.db"),
        vec![
            Arc::new(Engine {
                id: "mock",
                probe: probe.clone(),
            }),
            Arc::new(Engine {
                id: "other",
                probe: probe.clone(),
            }),
        ],
    )
    .await
    .unwrap();
    let account = core
        .add_account("other", "Other test profile")
        .await
        .unwrap();
    (dir, core, probe, account.id)
}
async fn settle(core: &Core, id: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if core.storage.session(id).await.unwrap().status != "running" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(15)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn upgrade_preserves_existing_conversation_and_provider_thread() {
    let dir = test_support::TestDirectory::new().unwrap();
    let old_migrations = dir.path().join("old-migrations");
    std::fs::create_dir(&old_migrations).unwrap();
    std::fs::write(
        old_migrations.join("0001_initial.sql"),
        include_str!("../migrations/0001_initial.sql"),
    )
    .unwrap();
    let database = dir.path().join("upgrade.db");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::migrate::Migrator::new(old_migrations.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO workspaces VALUES ('old-project','Старый проект',?,1)")
        .bind(dir.path().to_string_lossy().as_ref())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sessions VALUES ('old-chat','old-project','mock','mock-local','fast','Сохранённый чат','completed','standard',?,'upstream-before-upgrade',NULL,1,2)")
        .bind(dir.path().to_string_lossy().as_ref()).execute(&pool).await.unwrap();
    let payload = EventPayload::AssistantTextDelta {
        text: "Ответ до обновления".into(),
    };
    sqlx::query(
        "INSERT INTO events(session_id,run_id,timestamp,payload) VALUES ('old-chat','old-run',2,?)",
    )
    .bind(serde_json::to_string(&payload).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let storage = agent_core::storage::Storage::open(&database).await.unwrap();
    let chat = storage.session("old-chat").await.unwrap();
    assert_eq!(chat.title, "Сохранённый чат");
    assert_eq!(chat.status, "completed");
    assert_eq!(
        chat.provider_session_id.as_deref(),
        Some("upstream-before-upgrade")
    );
    assert_eq!(chat.chat_mode, "single");
    assert!(chat.parent_session_id.is_none());
    assert!(chat.reasoning_effort.is_none());
    assert_eq!(chat.tool_policy, "{}");
    assert!(chat.context_summary.is_empty());
    let events = storage.events("old-chat", None).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].run_id, "old-run");
    assert_eq!(
        serde_json::to_value(&events[0].payload).unwrap(),
        serde_json::to_value(payload).unwrap()
    );
}

#[tokio::test]
async fn settings_apply_to_next_turn_and_invalid_effort_does_not_mutate_history() {
    let (_dir, core, probe, _) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "single".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    assert_eq!(chat.workspace_id, "chat-scratch");
    core.send_message(&chat.id, "build widget".into())
        .await
        .unwrap();
    assert!(matches!(
        core.configure_session(&chat.id, config("mock", "mock-local"))
            .await,
        Err(CoreError::Busy)
    ));
    settle(&core, &chat.id).await;
    let mut settings = config("mock", "mock-local");
    settings.model = "deep".into();
    settings.reasoning_effort = Some("high".into());
    settings.permission_profile = "read_only".into();
    core.configure_session(&chat.id, settings.clone())
        .await
        .unwrap();
    settings.reasoning_effort = Some("invalid".into());
    assert!(core.configure_session(&chat.id, settings).await.is_err());
    core.send_message(&chat.id, "continue".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        assert_eq!(requests[1].session.model, "deep");
        assert_eq!(
            requests[1].session.reasoning_effort.as_deref(),
            Some("high")
        );
        assert_eq!(
            requests[1].session.provider_session_id.as_deref(),
            Some(format!("provider-{}", chat.id).as_str())
        );
    }
    assert_eq!(
        core.storage
            .events(&chat.id, None)
            .await
            .unwrap()
            .iter()
            .filter(|e| matches!(e.payload, EventPayload::TurnStarted { .. }))
            .count(),
        2
    );
}

#[tokio::test]
async fn handoff_is_atomic_preserves_chat_and_injects_summary_into_target() {
    let (dir, core, probe, other) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "single".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    core.send_message(&chat.id, "build widget".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    let target = config("other", &other);
    probe.fail.store(true, Ordering::SeqCst);
    core.handoff(&chat.id, target.clone()).await.unwrap();
    settle(&core, &chat.id).await;
    assert_eq!(
        core.storage.session(&chat.id).await.unwrap().provider,
        "mock"
    );
    probe.fail.store(false, Ordering::SeqCst);
    core.handoff(&chat.id, target).await.unwrap();
    settle(&core, &chat.id).await;
    let changed = core.storage.session(&chat.id).await.unwrap();
    assert_eq!(changed.account_profile_id, other);
    assert!(changed.provider_session_id.is_none());
    assert!(changed.context_summary.contains("next: tests"));
    core.send_message(&chat.id, "continue".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        let last = requests.last().unwrap();
        assert_eq!(last.session.provider, "other");
        assert!(last.prompt.contains("next: tests"));
        assert_eq!(last.account.id, other);
    }
    core.shutdown().await;
    drop(core);
    let reopened = Core::open_with_providers(
        &dir.path().join("chats.db"),
        vec![Arc::new(Engine { id: "other", probe })],
    )
    .await
    .unwrap();
    assert_eq!(
        reopened.storage.session(&chat.id).await.unwrap().provider,
        "other"
    );
    assert!(reopened.storage.events(&chat.id, None).await.unwrap().len() > 8);
}

#[tokio::test]
async fn auto_contexts_are_parallel_read_only_persisted_and_share_the_goal() {
    let (_dir, core, probe, _) = fixture().await;
    // Assert actual overlap without relying on scheduler timing or an 80 ms sleep.
    *probe.parallel_barrier.lock().unwrap() = Some(Arc::new(tokio::sync::Barrier::new(2)));
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "auto".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    core.send_message(&chat.id, "build widget".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    assert_eq!(
        core.storage.session(&chat.id).await.unwrap().status,
        "completed"
    );
    let tasks: Vec<_> = core
        .snapshot()
        .await
        .unwrap()
        .sessions
        .into_iter()
        .filter(|s| s.parent_session_id.as_deref() == Some(&chat.id))
        .collect();
    assert_eq!(tasks.len(), 2);
    assert!(tasks.iter().all(|s| s.permission_profile == "read_only"
        && s.context_summary.contains("build widget")
        && s.status == "completed"));
    assert_eq!(probe.peak.load(Ordering::SeqCst), 2);
    let requests = probe.requests.lock().unwrap();
    assert!(requests.last().unwrap().prompt.contains("Result from mock"));
    assert_ne!(tasks[0].provider_session_id, tasks[1].provider_session_id);
}

#[tokio::test]
async fn team_exchanges_results_and_cancellation_stops_all_workers() {
    let (_dir, core, probe, other) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "team".into(),
            agents: vec![
                config("mock", "mock-local"),
                config("other", &other),
                config("mock", "mock-local"),
            ],
        })
        .await
        .unwrap();
    core.send_message(&chat.id, "review widget".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        assert!(requests
            .iter()
            .all(|r| r.prompt.contains("мультиагентном чате BebekonCode")));
        assert_eq!(
            probe.peak_writing.load(Ordering::SeqCst),
            1,
            "writers must not overlap"
        );
        assert!(requests
            .iter()
            .filter(|r| r.prompt.contains("Обсудите результаты коллег"))
            .all(|r| r.session.permission_profile == "read_only"));
        assert_eq!(
            requests.first().unwrap().session.permission_profile,
            "standard"
        );
        assert!(requests.iter().all(|r| r
            .prompt
            .contains("прямого канала или инструмента вызова коллег у тебя нет")));
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.prompt.contains("Обсудите результаты коллег"))
                .count(),
            2
        );
    }
    core.send_message(&chat.id, "cancel task".into())
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while probe.active.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    core.cancel(&chat.id).unwrap();
    settle(&core, &chat.id).await;
    assert_eq!(
        core.storage.session(&chat.id).await.unwrap().status,
        "stopped"
    );
    assert_eq!(probe.active.load(Ordering::SeqCst), 0);
    assert!(!core
        .snapshot()
        .await
        .unwrap()
        .sessions
        .iter()
        .any(|s| s.status == "running"));
}

#[tokio::test]
async fn worker_error_event_is_not_replaced_with_success_or_another_account() {
    let (_dir, core, probe, other) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "team".into(),
            agents: vec![config("mock", "mock-local"), config("other", &other)],
        })
        .await
        .unwrap();
    probe.reported_failure.store(true, Ordering::SeqCst);
    core.send_message(&chat.id, "review".into()).await.unwrap();
    settle(&core, &chat.id).await;
    let stored = core.storage.session(&chat.id).await.unwrap();
    assert_eq!(stored.status, "failed");
    assert_eq!(stored.account_profile_id, "mock-local");
    let events = core.storage.events(&chat.id, None).await.unwrap();
    assert!(
        matches!(&events.last().unwrap().payload,EventPayload::ProviderError{kind:Some(kind),..} if kind=="usage_limit")
    );
    assert_eq!(probe.active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancelled_handoff_keeps_source_thread_and_binding() {
    let (_dir, core, probe, other) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "single".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    core.send_message(&chat.id, "build widget".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    let previous = core.storage.session(&chat.id).await.unwrap();
    core.handoff(&chat.id, config("other", &other))
        .await
        .unwrap();
    core.cancel(&chat.id).unwrap();
    settle(&core, &chat.id).await;
    let after = core.storage.session(&chat.id).await.unwrap();
    assert_eq!(after.status, "stopped");
    assert_eq!(after.account_profile_id, previous.account_profile_id);
    assert_eq!(after.provider_session_id, previous.provider_session_id);
    assert!(after.context_summary.is_empty());
    assert_eq!(probe.active.load(Ordering::SeqCst), 0);
}
