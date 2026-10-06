use agent_core::{
    model::{CreateSession, EventPayload},
    Core, CoreError,
};
#[path = "../src/test_support.rs"]
mod test_support;

async fn fixture() -> (test_support::TestDirectory, std::sync::Arc<Core>, String) {
    let temp = test_support::TestDirectory::new().expect("temp");
    let core = Core::open(&temp.path().join("test.db"))
        .await
        .expect("core");
    let workspace = core
        .add_workspace(temp.path().to_str().expect("path"))
        .await
        .expect("workspace");
    let session = core
        .create_session(CreateSession {
            workspace_id: workspace.id,
            provider: "mock".into(),
            account_profile_id: "mock-local".into(),
            model: "mock-stream-v1".into(),
            permission_profile: "standard".into(),
        })
        .await
        .expect("session");
    (temp, core, session.id)
}

#[tokio::test]
async fn parallel_sessions_cancel_independently_and_persist() {
    let (_temp, core, id) = fixture().await;
    let snapshot = core.snapshot().await.expect("snapshot");
    let second = core
        .create_session(CreateSession {
            workspace_id: snapshot.workspaces[0].id.clone(),
            provider: "mock".into(),
            account_profile_id: "mock-local".into(),
            model: "mock-stream-v1".into(),
            permission_profile: "read_only".into(),
        })
        .await
        .expect("second");
    let mut events = core.subscribe();
    core.send_message(&id, "First task".into())
        .await
        .expect("first turn");
    assert!(matches!(
        core.send_message(&id, "Duplicate".into()).await,
        Err(CoreError::Busy)
    ));
    core.send_message(&second.id, "Second task".into())
        .await
        .expect("second turn");
    core.cancel(&id).expect("cancel");
    tokio::time::timeout(std::time::Duration::from_secs(12), async {
        let mut stopped = false;
        let mut completed = false;
        while !stopped || !completed {
            let event = events.recv().await.expect("event");
            stopped |=
                event.session_id == id && matches!(event.payload, EventPayload::SessionStopped);
            completed |= event.session_id == second.id
                && matches!(event.payload, EventPayload::TurnCompleted);
        }
    })
    .await
    .expect("turns finished");
    let saved = core
        .storage
        .events(&second.id, None)
        .await
        .expect("history");
    assert!(saved
        .iter()
        .any(|e| matches!(e.payload, EventPayload::AssistantTextDelta { .. })));
    assert_eq!(
        core.storage.session(&id).await.expect("session").status,
        "stopped"
    );
    assert_eq!(
        core.storage
            .session(&second.id)
            .await
            .expect("session")
            .status,
        "completed"
    );
}

#[tokio::test]
async fn unavailable_provider_and_wrong_account_are_rejected() {
    let (_temp, core, _) = fixture().await;
    let workspace_id = core.snapshot().await.expect("snapshot").workspaces[0]
        .id
        .clone();
    let mut input = CreateSession {
        workspace_id,
        provider: "gemini".into(),
        account_profile_id: "mock-local".into(),
        model: "mock-stream-v1".into(),
        permission_profile: "standard".into(),
    };
    assert!(matches!(
        core.create_session(input.clone()).await,
        Err(CoreError::ProviderUnavailable)
    ));
    input.provider = "mock".into();
    input.account_profile_id = "other-account".into();
    assert!(matches!(
        core.create_session(input).await,
        Err(CoreError::Invalid(_))
    ));
}

#[tokio::test]
async fn database_reopen_recovers_incomplete_turn_without_replay() {
    let (temp, core, id) = fixture().await;
    core.storage
        .append(
            &id,
            "aborted-run",
            EventPayload::TurnStarted {
                prompt: "Recover me".into(),
            },
            Some("running"),
        )
        .await
        .expect("start");
    drop(core);
    let core = Core::open(&temp.path().join("test.db"))
        .await
        .expect("reopen");
    assert_eq!(
        core.storage.session(&id).await.expect("session").status,
        "interrupted"
    );
    assert_eq!(
        core.storage.events(&id, None).await.expect("events").len(),
        1
    );
}

#[tokio::test]
async fn account_profiles_are_isolated_and_removal_is_guarded() {
    let (temp, core, _) = fixture().await;
    let first = core
        .add_account("openai", "  Личный  ")
        .await
        .expect("first account");
    let second = core
        .add_account("openai", "Рабочий")
        .await
        .expect("second account");
    assert_eq!(first.label, "Личный");
    assert_eq!(first.auth_status, "signed_out");
    let (Some(first_dir), Some(second_dir)) = (&first.config_dir, &second.config_dir) else {
        panic!("profiles need their own directories");
    };
    assert_ne!(first_dir, second_dir);
    for directory in [first_dir, second_dir] {
        let directory = std::path::Path::new(directory);
        assert!(directory.is_dir());
        assert!(directory.starts_with(temp.path().join("providers").join("openai")));
    }
    assert!(core.add_account("mock", "Демо").await.is_err());
    assert!(core.add_account("openai", "   ").await.is_err());
    let renamed = core
        .rename_account(&second.id, "Второй")
        .await
        .expect("rename");
    assert_eq!(renamed.label, "Второй");
    core.remove_account(&second.id).await.expect("remove");
    assert!(!std::path::Path::new(second_dir).exists());
    assert!(core.remove_account("mock-local").await.is_err());
    let accounts = core.snapshot().await.expect("snapshot").accounts;
    assert!(accounts.iter().any(|account| account.id == first.id));
    assert!(!accounts.iter().any(|account| account.id == second.id));
}
