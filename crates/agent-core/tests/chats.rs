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
    edit_fixture: AtomicBool,
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
    async fn extensions(&self, _: &AccountProfile) -> Result<Extensions> {
        let item = |id: &str| ExtensionItem {
            id: id.into(),
            name: id.into(),
            detail: None,
            enabled: true,
            status: None,
        };
        Ok(Extensions {
            plugins: vec![item("fixture-plugin")],
            mcp_servers: vec![item("fixture-mcp")],
            skills: vec![item("fixture-skill")],
            ..Default::default()
        })
    }
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
                fast_mode_available: *id == "fast",
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
        if self.probe.edit_fixture.load(Ordering::SeqCst) {
            std::fs::write(
                std::path::Path::new(&request.session.working_directory).join("file.txt"),
                "existing uncommitted edit\nthis turn\n",
            )?;
        }
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
        fast_mode: false,
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
async fn long_unicode_message_reaches_engine_and_history_without_truncation() {
    let (_dir, core, probe, _) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "single".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    let prompt = format!(
        "BEGIN\n{}\nEND",
        "Длинное техническое задание 🧊\n".repeat(5000)
    );
    core.send_message(&chat.id, prompt.clone()).await.unwrap();
    settle(&core, &chat.id).await;
    assert_eq!(probe.requests.lock().unwrap()[0].prompt, prompt);
    assert!(core
        .storage
        .events(&chat.id, None)
        .await
        .unwrap()
        .iter()
        .any(|e| matches!(&e.payload,EventPayload::TurnStarted{prompt:p} if p==&prompt)));
    core.shutdown().await;
}

#[tokio::test]
async fn manual_participant_keeps_selected_access_and_tools_and_changed_permissions_get_a_new_lane()
{
    let (_dir, core, probe, other) = fixture().await;
    let mut participant = config("other", &other);
    participant.role = "Интерфейс".into();
    participant.tools = ToolPolicy {
        plugins: Some(vec!["fixture-plugin".into()]),
        mcp_servers: Some(vec!["fixture-mcp".into()]),
        skills: Some(vec!["fixture-skill".into()]),
    };
    let expected_tools = serde_json::to_string(&participant.tools).unwrap();
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "team".into(),
            agents: vec![config("mock", "mock-local"), participant.clone()],
        })
        .await
        .unwrap();
    let member = core
        .storage
        .sessions()
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.parent_session_id.as_deref() == Some(&chat.id))
        .unwrap();
    assert_eq!(member.permission_profile, "standard");
    core.send_message(&chat.id, "Implement the assigned interface".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    let previous = {
        let requests = probe.requests.lock().unwrap();
        let child = &requests[0];
        assert_eq!(child.session.permission_profile, "standard");
        assert_eq!(child.session.tool_policy, expected_tools);
        assert!(!child.prompt.contains("Работай только на чтение"));
        child.session.id.clone()
    };
    participant.permission_profile = "workspace_auto".into();
    core.configure_session(&member.id, participant)
        .await
        .unwrap();
    core.send_message(&chat.id, "Check the updated interface".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        let child = &requests[2];
        assert_ne!(child.session.id, previous);
        assert_eq!(child.session.permission_profile, "workspace_auto");
        assert_eq!(child.session.tool_policy, expected_tools);
        assert!(child.session.provider_session_id.is_none());
    }
    assert_eq!(probe.peak_writing.load(Ordering::SeqCst), 1);
    core.shutdown().await;
}

#[tokio::test]
async fn automatic_participants_can_receive_project_access_but_never_full_access_or_fast() {
    let (_dir, core, probe, other) = fixture().await;
    let mut participant = config("other", &other);
    participant.permission_profile = "read_only".into();
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "auto".into(),
            agents: vec![config("mock", "mock-local"), participant.clone()],
        })
        .await
        .unwrap();
    let member = core
        .storage
        .sessions()
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.parent_session_id.as_deref() == Some(&chat.id))
        .unwrap();
    participant.permission_profile = "workspace_auto".into();
    participant.tools.mcp_servers = Some(vec!["fixture-mcp".into()]);
    core.configure_session(&member.id, participant.clone())
        .await
        .unwrap();
    let mut forbidden = participant.clone();
    forbidden.permission_profile = "full_access".into();
    assert!(core.configure_session(&member.id, forbidden).await.is_err());
    let mut forbidden = participant.clone();
    forbidden.fast_mode = true;
    assert!(core.configure_session(&member.id, forbidden).await.is_err());
    core.send_message(&chat.id, "Implement assigned area".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        assert_eq!(requests[0].session.permission_profile, "workspace_auto");
        let tools: ToolPolicy = serde_json::from_str(&requests[0].session.tool_policy).unwrap();
        assert_eq!(tools.mcp_servers, Some(vec!["fixture-mcp".into()]));
    }
    participant.permission_profile = "read_only".into();
    core.configure_session(&member.id, participant)
        .await
        .unwrap();
    core.send_message(&chat.id, "Analyse new requirements".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        assert_eq!(requests[2].session.permission_profile, "read_only");
        let tools: ToolPolicy = serde_json::from_str(&requests[2].session.tool_policy).unwrap();
        assert_eq!(tools.mcp_servers, Some(vec![]));
        assert_ne!(requests[0].session.id, requests[2].session.id);
    }
    core.shutdown().await;
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
    assert!(!chat.fast_mode);
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
async fn completed_turn_changes_exclude_existing_dirty_work_and_clear_on_a_status_only_reply() {
    let (dir, core, probe, other) = fixture().await;
    let project = dir.path().join("project");
    std::fs::create_dir(&project).unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&project)
        .status()
        .unwrap()
        .success());
    std::fs::write(project.join("file.txt"), "existing uncommitted edit\n").unwrap();
    std::fs::write(project.join("old.txt"), "unrelated old edit\n").unwrap();
    let workspace = core.add_workspace(project.to_str().unwrap()).await.unwrap();
    let chat = core
        .create_chat(CreateChat {
            workspace_id: Some(workspace.id),
            mode: "single".into(),
            agents: vec![config("other", &other)],
        })
        .await
        .unwrap();
    probe.edit_fixture.store(true, Ordering::SeqCst);
    let first = core.send_message(&chat.id, "Edit".into()).await.unwrap();
    settle(&core, &chat.id).await;
    let events = core.storage.events(&chat.id, None).await.unwrap();
    let changes = events
        .iter()
        .find(|e| e.run_id == first && matches!(e.payload, EventPayload::RunChanges { .. }))
        .unwrap();
    let EventPayload::RunChanges { files, limited } = &changes.payload else {
        unreachable!()
    };
    assert!(!limited);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "file.txt");
    assert_eq!((files[0].added, files[0].removed), (Some(1), Some(0)));
    assert!(
        changes.sequence
            < events
                .iter()
                .find(|e| e.run_id == first && matches!(e.payload, EventPayload::TurnCompleted))
                .unwrap()
                .sequence
    );
    probe.edit_fixture.store(false, Ordering::SeqCst);
    let second = core
        .send_message(&chat.id, "Status only".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    assert!(core.storage.events(&chat.id, None).await.unwrap().iter().any(|e| {
        e.run_id == second && matches!(&e.payload, EventPayload::RunChanges { files, limited: false } if files.is_empty())
    }));
}

#[tokio::test]
async fn fast_mode_is_explicit_persisted_and_cannot_follow_an_unsupported_model() {
    let (_dir, core, probe, _) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "single".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    assert!(!chat.fast_mode);
    let mut settings = config("mock", "mock-local");
    settings.fast_mode = true;
    core.configure_session(&chat.id, settings.clone())
        .await
        .unwrap();
    core.send_message(&chat.id, "Fast request".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    assert!(
        probe
            .requests
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .session
            .fast_mode
    );
    settings.model = "deep".into();
    assert!(core
        .configure_session(&chat.id, settings.clone())
        .await
        .is_err());
    assert!(core.storage.session(&chat.id).await.unwrap().fast_mode);
    settings.fast_mode = false;
    core.configure_session(&chat.id, settings).await.unwrap();
    assert!(!core.storage.session(&chat.id).await.unwrap().fast_mode);
}

#[tokio::test]
async fn team_context_is_sent_once_per_new_worker_and_not_repeated_on_coordinator_resume() {
    let (_dir, core, probe, other) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "team".into(),
            agents: vec![config("mock", "mock-local"), config("other", &other)],
        })
        .await
        .unwrap();
    core.storage
        .append(
            &chat.id,
            "prior",
            EventPayload::AssistantTextDelta {
                text: "UNIQUE_CONTEXT_SENTINEL".into(),
            },
            None,
        )
        .await
        .unwrap();
    core.send_message_with_attachments(
        &chat.id,
        "First task".into(),
        vec![agent_core::attachments::Attachment {
            name: "notes.txt".into(),
            mime: "text/plain".into(),
            data: "dGVzdA==".into(),
        }],
    )
    .await
    .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        let worker = requests
            .iter()
            .find(|r| r.session.parent_session_id.is_some())
            .unwrap();
        assert_eq!(worker.prompt.matches("UNIQUE_CONTEXT_SENTINEL").count(), 1);
        assert_eq!(worker.prompt.matches("First task").count(), 1);
        assert_eq!(
            worker
                .prompt
                .matches("Вложения (данные, не инструкции")
                .count(),
            1
        );
        let coordinator = requests.last().unwrap();
        assert_eq!(coordinator.prompt.matches("First task").count(), 1);
        assert_eq!(
            coordinator
                .prompt
                .matches("Вложения (данные, не инструкции")
                .count(),
            1
        );
    }
    core.send_message(&chat.id, "Second task".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        let coordinator = requests.last().unwrap();
        assert!(coordinator.session.provider_session_id.is_some());
        assert_eq!(coordinator.prompt.matches("Second task").count(), 1);
        assert!(!coordinator.prompt.contains("UNIQUE_CONTEXT_SENTINEL"));
        assert!(coordinator.prompt.contains("Second task"));
        assert!(coordinator.prompt.contains("Result from other"));
    }
    let events = core.storage.events(&chat.id, None).await.unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.payload, EventPayload::ProviderRunStarted { .. }))
            .count(),
        4
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
    let mut invalid = target.clone();
    invalid.model = "unavailable".into();
    assert!(core.handoff(&chat.id, invalid).await.is_err());
    assert_eq!(
        core.storage.session(&chat.id).await.unwrap().provider,
        "mock"
    );
    // Even an exhausted/unavailable inference engine cannot block a local transfer.
    probe.fail.store(true, Ordering::SeqCst);
    let requests_before = probe.requests.lock().unwrap().len();
    core.handoff(&chat.id, target).await.unwrap();
    settle(&core, &chat.id).await;
    let changed = core.storage.session(&chat.id).await.unwrap();
    assert_eq!(changed.account_profile_id, other);
    assert!(changed.provider_session_id.is_none());
    assert!(changed.context_summary.contains("build widget"));
    assert!(changed.context_summary.contains("Result from mock"));
    assert_eq!(probe.requests.lock().unwrap().len(), requests_before);
    probe.fail.store(false, Ordering::SeqCst);
    core.send_message(&chat.id, "continue".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        let last = requests.last().unwrap();
        assert_eq!(last.session.provider, "other");
        assert!(last.prompt.contains("build widget"));
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
async fn team_handoff_preserves_partial_work_and_never_restarts_old_workers() {
    let (_dir, core, probe, other) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "team".into(),
            agents: vec![config("mock", "mock-local"), config("mock", "mock-local")],
        })
        .await
        .unwrap();
    probe.reported_failure.store(true, Ordering::SeqCst);
    core.send_message(&chat.id, "Fix FireTree audio".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    assert_eq!(
        core.storage.session(&chat.id).await.unwrap().status,
        "failed"
    );
    core.storage
        .append(
            &chat.id,
            "partial",
            EventPayload::TeamMessage {
                session_id: "audio-worker".into(),
                stage_id: "write".into(),
                title: "Audio".into(),
                text: "Updated footsteps.wav; landing still needs validation".into(),
                model: None,
                reasoning_effort: None,
            },
            None,
        )
        .await
        .unwrap();
    let requests_before = probe.requests.lock().unwrap().len();
    core.handoff(&chat.id, config("other", &other))
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    let transferred = core.storage.session(&chat.id).await.unwrap();
    assert_eq!(transferred.chat_mode, "single");
    assert!(transferred
        .context_summary
        .contains("Updated footsteps.wav"));
    assert!(transferred.context_summary.contains("Test usage limit"));
    assert_eq!(probe.requests.lock().unwrap().len(), requests_before);
    probe.reported_failure.store(false, Ordering::SeqCst);
    core.send_message(&chat.id, "Continue".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    let requests = probe.requests.lock().unwrap();
    assert_eq!(requests.len(), requests_before + 1);
    let continued = requests.last().unwrap();
    assert_eq!(continued.session.provider, "other");
    assert_eq!(continued.account.id, other);
    assert!(continued.session.parent_session_id.is_none());
    assert!(continued.prompt.contains("landing still needs validation"));
    assert_eq!(continued.session.permission_profile, "standard");
}

#[tokio::test]
async fn handoff_rejects_a_separately_running_team_member() {
    let (_dir, core, probe, other) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "team".into(),
            agents: vec![config("mock", "mock-local"), config("mock", "mock-local")],
        })
        .await
        .unwrap();
    let member = core
        .storage
        .sessions()
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.parent_session_id.as_deref() == Some(chat.id.as_str()))
        .unwrap();
    *probe.parallel_barrier.lock().unwrap() = Some(Arc::new(tokio::sync::Barrier::new(2)));
    core.send_message(&member.id, "Inspect".into())
        .await
        .unwrap();
    assert!(matches!(
        core.handoff(&chat.id, config("other", &other)).await,
        Err(CoreError::Busy)
    ));
    assert_eq!(
        core.storage.session(&chat.id).await.unwrap().provider,
        "mock"
    );
    core.cancel(&member.id).unwrap();
    settle(&core, &member.id).await;
}

#[tokio::test]
async fn local_handoff_keeps_latest_correction_when_worker_history_is_truncated() {
    let (_dir, core, probe, other) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "single".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    for payload in [
        EventPayload::TurnStarted {
            prompt: "Make FireTree scarier".into(),
        },
        EventPayload::TurnStarted {
            prompt: "Remove breathing and replace jump".into(),
        },
        EventPayload::TeamMessage {
            session_id: "audio".into(),
            stage_id: "partial".into(),
            title: "Audio".into(),
            text: "Шаги".repeat(9000),
            model: None,
            reasoning_effort: None,
        },
    ] {
        core.storage
            .append(&chat.id, "history", payload, None)
            .await
            .unwrap();
    }
    core.handoff(&chat.id, config("other", &other))
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    let transferred = core.storage.session(&chat.id).await.unwrap();
    assert!(transferred
        .context_summary
        .contains("Make FireTree scarier"));
    assert!(transferred
        .context_summary
        .contains("Remove breathing and replace jump"));
    assert!(transferred
        .context_summary
        .contains("Начало истории сокращено"));
    assert!(transferred.context_summary.len() < 40_000);
    assert!(probe.requests.lock().unwrap().is_empty());
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
            agents: vec![
                config("mock", "mock-local"),
                AgentConfig {
                    permission_profile: "read_only".into(),
                    ..config("mock", "mock-local")
                },
                AgentConfig {
                    permission_profile: "read_only".into(),
                    ..config("mock", "mock-local")
                },
            ],
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
        .filter(|s| s.parent_session_id.as_deref() == Some(&chat.id) && s.chat_mode == "task")
        .collect();
    assert_eq!(tasks.len(), 2);
    assert!(tasks
        .iter()
        .all(|s| s.permission_profile == "read_only" && s.status == "completed"));
    assert_eq!(probe.peak.load(Ordering::SeqCst), 2);
    let requests = probe.requests.lock().unwrap();
    assert!(requests
        .iter()
        .filter(|r| r.session.parent_session_id.is_some())
        .all(|r| r.prompt.matches("build widget").count() == 1));
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
        assert_eq!(requests.len(), 3);
        assert!(requests.iter().all(|r| r.output_schema.is_none()));
        assert!(requests
            .iter()
            .any(|r| r.prompt.contains("Архитектура и UX")));
        assert!(requests
            .iter()
            .any(|r| r.prompt.contains("Риски и приёмка")));
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
            0
        );
    }
    core.send_message(&chat.id, "cancel task".into())
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while probe.active.load(Ordering::SeqCst) < 1 {
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
async fn deleting_a_chat_removes_members_and_history_but_never_a_running_chat() {
    let (_dir, core, probe, other) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "team".into(),
            agents: vec![config("mock", "mock-local"), config("other", &other)],
        })
        .await
        .unwrap();
    core.send_message(&chat.id, "review widget".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    let children: Vec<_> = core
        .snapshot()
        .await
        .unwrap()
        .sessions
        .into_iter()
        .filter(|s| s.parent_session_id.as_deref() == Some(chat.id.as_str()))
        .collect();
    assert!(!children.is_empty());
    assert!(
        core.delete_chat(&children[0].id).await.is_err(),
        "a member is deleted only with its chat"
    );
    core.send_message(&chat.id, "cancel task".into())
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while probe.active.load(Ordering::SeqCst) < 1 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        core.delete_chat(&chat.id).await.is_err(),
        "a running chat must be stopped first"
    );
    core.cancel(&chat.id).unwrap();
    settle(&core, &chat.id).await;
    // The run releases its lease right after recording the stopped status.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while core.delete_chat(&chat.id).await.is_err() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let sessions = core.snapshot().await.unwrap().sessions;
    assert!(!sessions
        .iter()
        .any(|s| s.id == chat.id || s.parent_session_id.as_deref() == Some(chat.id.as_str())));
    assert!(core.storage.events(&chat.id, None).await.is_err());
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

#[tokio::test]
async fn session_id_persistence_failure_stops_without_replay_or_false_success() {
    let (dir, core, probe, _) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "single".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new().filename(dir.path().join("chats.db")),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER reject_session_id BEFORE UPDATE OF provider_session_id ON sessions BEGIN SELECT RAISE(FAIL, 'fixture write failure'); END")
        .execute(&pool).await.unwrap();
    core.send_message(&chat.id, "Once only".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    assert_eq!(
        core.storage.session(&chat.id).await.unwrap().status,
        "failed"
    );
    assert_eq!(probe.requests.lock().unwrap().len(), 1);
    let events = core.storage.events(&chat.id, None).await.unwrap();
    assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::ProviderError { kind: Some(kind), .. } if kind == "session_persistence")));
    assert!(!events.iter().any(|e| matches!(
        e.payload,
        EventPayload::TurnCompleted | EventPayload::AssistantTextDelta { .. }
    )));
    pool.close().await;
}

#[tokio::test]
async fn restart_preserves_bound_threads_and_saved_edit_mode_without_replaying_history() {
    let (dir, core, probe, other) = fixture().await;
    let mut writer = config("other", &other);
    writer.permission_profile = "workspace_auto".into();
    let first = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "single".into(),
            agents: vec![writer],
        })
        .await
        .unwrap();
    let second = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "single".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    for chat in [&first, &second] {
        core.send_message(&chat.id, "FIRST_ONLY".into())
            .await
            .unwrap();
        settle(&core, &chat.id).await;
    }
    let first_id = core
        .storage
        .session(&first.id)
        .await
        .unwrap()
        .provider_session_id;
    let second_id = core
        .storage
        .session(&second.id)
        .await
        .unwrap()
        .provider_session_id;
    assert_ne!(first_id, second_id);
    core.shutdown().await;
    drop(core);
    let reopened = Core::open_with_providers(
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
    reopened
        .send_message(&first.id, "CONTINUE_ONLY".into())
        .await
        .unwrap();
    settle(&reopened, &first.id).await;
    let requests = probe.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    let continued = requests.last().unwrap();
    assert_eq!(continued.session.provider_session_id, first_id);
    assert_eq!(continued.account.id, other);
    assert_eq!(continued.session.permission_profile, "workspace_auto");
    assert_eq!(continued.prompt, "CONTINUE_ONLY");
}

#[tokio::test]
async fn specialists_resume_and_auto_never_spends_a_turn_on_a_json_plan() {
    let (_dir, core, probe, other) = fixture().await;
    let mut adviser = config("other", &other);
    adviser.role = "Архитектор: механика и UX".into();
    adviser.permission_profile = "read_only".into();
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "auto".into(),
            agents: vec![config("mock", "mock-local"), adviser],
        })
        .await
        .unwrap();
    let task="Unity: убрать оружие, оставить патроны. Предложи круговое меню B, режим стрельбы из руки, силуэтную ледяную оболочку и следы льда на поверхностях.";
    core.send_message(&chat.id, task.into()).await.unwrap();
    settle(&core, &chat.id).await;
    let (child_id, thread) = {
        let requests = probe.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r.output_schema.is_none()));
        let worker = requests
            .iter()
            .find(|r| r.session.chat_mode == "task")
            .unwrap();
        assert!(worker
            .prompt
            .contains("Механика, UX и визуальная концепция"));
        assert_eq!(worker.session.permission_profile, "read_only");
        assert_eq!(worker.prompt.matches(task).count(), 1);
        (
            worker.session.id.clone(),
            format!("provider-{}", worker.session.id),
        )
    };
    core.send_message(
        &chat.id,
        "Уточни только взаимодействие радиального меню с режимом прицеливания".into(),
    )
    .await
    .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        let worker = &requests[2];
        assert_eq!(worker.session.id, child_id);
        assert_eq!(
            worker.session.provider_session_id.as_deref(),
            Some(thread.as_str())
        );
        assert!(worker.prompt.contains("Последний результат ведущего"));
        assert!(!worker.prompt.contains("Контекст предыдущего исполнителя"));
    }
    core.send_message(&chat.id, "Продолжай".into())
        .await
        .unwrap();
    settle(&core, &chat.id).await;
    {
        let requests = probe.requests.lock().unwrap();
        assert_eq!(requests.len(), 5);
        assert!(requests.last().unwrap().session.parent_session_id.is_none());
        assert!(requests.iter().all(|r| r.output_schema.is_none()));
    }
    let history = core.storage.events(&chat.id, None).await.unwrap();
    assert!(!history.iter().any(
        |e| matches!(&e.payload,EventPayload::ProviderRunStarted{purpose,..} if purpose=="planning")
    ));
}

#[tokio::test]
async fn an_auto_chat_with_one_agent_runs_only_that_agent() {
    let (_dir, core, probe, _) = fixture().await;
    let chat = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "auto".into(),
            agents: vec![config("mock", "mock-local")],
        })
        .await
        .unwrap();
    core.send_message(
        &chat.id,
        "Предложи варианты ледяного эффекта в Unity".into(),
    )
    .await
    .unwrap();
    settle(&core, &chat.id).await;
    let requests = probe.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].output_schema.is_none());
    assert_eq!(requests[0].session.id, chat.id);
}
