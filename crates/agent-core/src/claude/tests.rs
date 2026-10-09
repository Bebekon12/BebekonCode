use super::*;
use serde_json::json;

#[test]
fn requires_supported_official_cli() {
    assert!(!version_supported(None));
    assert!(!version_supported(Some("2.1.89")));
    assert!(version_supported(Some("2.1.294")));
}

#[cfg(windows)]
#[tokio::test]
async fn shell_and_mcp_always_require_fresh_consent_even_in_auto() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut session = turn(temp.path()).session;
    session.permission_profile = "workspace_auto".into();
    let mut access = access::Access::default();
    access.mcp = vec![crate::mcp::LocalMcpServer {
        name: "unity".into(),
        command: "C:/tools/unity.exe".into(),
        args: vec!["mcp".into()],
    }];
    let (tx, mut rx) = mpsc::channel(32);
    for tool in ["Bash", "PowerShell", "mcp__unity__capture_scene_view"] {
        let value = json!({"hook_event_name":"PreToolUse", "tool_name":tool, "tool_input":{"command":"Write-Output safe"}});
        let answer = guard::decide_with_access(
            &session,
            value.clone(),
            &tx,
            &CancellationToken::new(),
            &provider.pending,
            &provider.grants,
            &access,
        )
        .await;
        assert_eq!(answer["hookSpecificOutput"]["permissionDecision"], "ask");
        while let Ok(event) = rx.try_recv() {
            assert!(matches!(event, EventPayload::ApprovalResolved { .. }));
        }
        for _ in 0..2 {
            let mut value = value.clone();
            value["hook_event_name"] = json!("PermissionRequest");
            let p = provider.clone();
            let session_copy = session.clone();
            let tx = tx.clone();
            let access = access.clone();
            let job = tokio::spawn(async move {
                guard::decide_with_access(
                    &session_copy,
                    value,
                    &tx,
                    &CancellationToken::new(),
                    &p.pending,
                    &p.grants,
                    &access,
                )
                .await
            });
            let id = loop {
                match tokio::time::timeout(Duration::from_secs(5), rx.recv())
                    .await
                    .unwrap()
                    .unwrap()
                {
                    EventPayload::ApprovalRequested {
                        id,
                        available_decisions,
                        detail,
                        ..
                    } => {
                        assert_eq!(
                            available_decisions,
                            Some(vec![ApprovalDecision::AllowOnce, ApprovalDecision::Deny])
                        );
                        assert!(detail.contains("Write-Output safe"));
                        break id;
                    }
                    _ => continue,
                }
            };
            assert!(provider
                .resolve_approval(&session.id, &id, ApprovalDecision::AllowSession)
                .await
                .is_err());
            provider
                .resolve_approval(&session.id, &id, ApprovalDecision::AllowOnce)
                .await
                .unwrap();
            assert_eq!(
                job.await.unwrap()["hookSpecificOutput"]["decision"]["behavior"],
                "allow"
            );
        }
        let mut readonly = session.clone();
        readonly.permission_profile = "read_only".into();
        assert_eq!(
            guard::decide_with_access(
                &readonly,
                value,
                &tx,
                &CancellationToken::new(),
                &provider.pending,
                &provider.grants,
                &access
            )
            .await["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );
    }
    let unknown =
        json!({"hook_event_name":"PreToolUse","tool_name":"mcp__unselected__run","tool_input":{}});
    assert_eq!(
        guard::decide_with_access(
            &session,
            unknown,
            &tx,
            &CancellationToken::new(),
            &provider.pending,
            &provider.grants,
            &access
        )
        .await["hookSpecificOutput"]["permissionDecision"],
        "deny"
    );
    assert!(provider.grants.lock().unwrap().is_empty());
}

#[cfg(windows)]
#[test]
fn account_skills_are_selected_read_only_and_cannot_install_hooks() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    // A Windows account path can differ from canonical spelling (case or an 8.3 alias).
    let profile = PathBuf::from(temp.path().join("profile").to_string_lossy().to_lowercase());
    let project = temp.path().join("project");
    std::fs::create_dir_all(profile.join("skills/safe")).unwrap();
    std::fs::create_dir_all(profile.join("skills/hooks")).unwrap();
    std::fs::create_dir(&project).unwrap();
    std::fs::write(
        profile.join("skills/safe/SKILL.md"),
        "---\nname: safe\ndescription: test\n---\nRead only",
    )
    .unwrap();
    std::fs::write(
        profile.join("skills/hooks/SKILL.md"),
        "---\nname: hooks\nhooks:\n  PreToolUse: []\n---\nBlocked",
    )
    .unwrap();
    let mut request = turn(&project);
    request.account.config_dir = Some(profile.to_string_lossy().into_owned());
    let inventory = access::inventory(&request.account).unwrap();
    assert_eq!(inventory.skills.len(), 2);
    assert!(
        !inventory
            .skills
            .iter()
            .find(|s| s.name == "hooks")
            .unwrap()
            .enabled
    );
    let selected = access::Access::for_request(&request).unwrap();
    assert!(selected.guidance().contains("safe"));
    assert!(!selected.guidance().contains("/hooks/"));
    std::fs::write(profile.join("bebekon-mcp.json"), "[]").unwrap();
    assert!(selected
        .checked_file(
            temp.path(),
            &json!({"file_path":profile.join("bebekon-mcp.json")}),
            "Read"
        )
        .is_err());
    let input = json!({"file_path":profile.join("skills/safe/SKILL.md")});
    assert!(selected.skill_read(&input, "Read").is_ok());
    assert!(selected
        .skill_read(
            &json!({"file_path":profile.join("skills/safe/SKILL.md").canonicalize().unwrap()}),
            "Read"
        )
        .is_ok());
    assert!(selected.skill_read(&input, "Write").is_err());
    assert!(selected
        .skill_read(
            &json!({"file_path":profile.join(".credentials.json")}),
            "Read"
        )
        .is_err());
    request.session.tool_policy = serde_json::to_string(&ToolPolicy {
        skills: Some(vec![]),
        ..Default::default()
    })
    .unwrap();
    assert!(access::Access::for_request(&request)
        .unwrap()
        .skills
        .is_empty());
    assert!(access::Access::for_request(&request)
        .unwrap()
        .guidance()
        .is_empty());
    request.session.tool_policy = "{}".into();
    request.session.permission_profile = "read_only".into();
    assert!(access::Access::for_request(&request)
        .unwrap()
        .skills
        .is_empty());
}

#[test]
fn streaming_ignores_thinking_children_and_duplicate_final_text() {
    let mut state = mapping::StreamState::new(false);
    assert!(state.accept(json!({"type":"stream_event","event":{"delta":{"type":"thinking_delta","thinking":"private"}}})).unwrap().is_empty());
    assert!(state.accept(json!({"type":"stream_event","parent_tool_use_id":"worker","event":{"delta":{"type":"text_delta","text":"child"}}})).unwrap().is_empty());
    let events = state
        .accept(
            json!({"type":"stream_event","event":{"delta":{"type":"text_delta","text":"Ответ"}}}),
        )
        .unwrap();
    assert_eq!(
        events,
        vec![EventPayload::AssistantTextDelta {
            text: "Ответ".into()
        }]
    );
    assert!(state
        .accept(json!({"type":"assistant","message":{"content":[{"type":"text","text":"Ответ"}]}}))
        .unwrap()
        .is_empty());
    assert!(state
        .accept(json!({"type":"result","subtype":"success","result":"Ответ"}))
        .unwrap()
        .is_empty());
    assert!(state.finish().is_ok());
}

#[test]
fn errors_and_incomplete_streams_never_become_success() {
    let mut state = mapping::StreamState::new(false);
    assert!(state.finish().is_err());
    let error = state.accept(json!({"type":"result","subtype":"error_during_execution","is_error":true,"result":"You hit your usage limit"})).unwrap_err();
    assert!(matches!(error, CoreError::Provider { kind: Some(kind), .. } if kind == "usage_limit"));
    assert!(state.finish().is_err());
    let mut structured = mapping::StreamState::new(true);
    assert!(structured
        .accept(
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"not JSON"}]}})
        )
        .unwrap()
        .is_empty());
    assert!(structured
        .accept(json!({"type":"result","subtype":"success","result":"invented plan"}))
        .is_err());
    let events = structured
        .accept(json!({"type":"result","subtype":"success","structured_output":{"tasks":[]}}))
        .unwrap();
    assert_eq!(
        events,
        vec![EventPayload::AssistantTextDelta {
            text: "{\"tasks\":[]}".into()
        }]
    );
}

#[test]
fn file_guard_rejects_escape_credentials_and_commands() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    std::fs::write(temp.path().join("a.txt"), "old").unwrap();
    assert!(guard::checked_path(temp.path(), &json!({"file_path":"a.txt"}), "Read").is_ok());
    assert!(guard::checked_path(temp.path(), &json!({"file_path":"new.txt"}), "Write").is_ok());
    for path in [
        "../outside.txt",
        ".env",
        "private.key",
        ".git/config",
        ".claude/.credentials.json",
        "a.txt:stream",
    ] {
        assert!(
            guard::checked_path(temp.path(), &json!({"file_path":path}), "Write").is_err(),
            "{path}"
        );
    }
    assert!(guard::checked_path(temp.path(), &json!({"command":"whoami"}), "Bash").is_err());
    assert!(guard::checked_path(temp.path(), &json!({"pattern":"../*"}), "Glob").is_err());
    assert!(guard::checked_path(
        temp.path(),
        &json!({"path":temp.path().join("a.txt")}),
        "Read"
    )
    .is_ok());
}

#[cfg(windows)]
#[test]
fn file_guard_accepts_absolute_paths_using_the_original_windows_root_spelling() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    std::fs::write(temp.path().join("a.txt"), "safe").unwrap();
    let alias = PathBuf::from(temp.path().to_string_lossy().to_lowercase());
    assert!(guard::checked_path(&alias, &json!({"file_path":alias.join("a.txt")}), "Read").is_ok());
    assert!(
        guard::checked_path(&alias, &json!({"file_path":alias.join("new.txt")}), "Write").is_ok()
    );
    assert!(guard::checked_path(&alias, &json!({"file_path":alias.join(".env")}), "Read").is_err());
    assert!(guard::checked_path(
        &alias,
        &json!({"file_path":alias.join("../outside.txt")}),
        "Write"
    )
    .is_err());
}

#[cfg(windows)]
fn fixture_provider() -> Arc<ClaudeProvider> {
    let (notifications, _) = broadcast::channel(16);
    let provider = Arc::new(ClaudeProvider::new(notifications));
    *provider.detection.write().unwrap() = Detection {
        binary: Some(which::which("node").expect("Node.js is a development prerequisite")),
        version: Some("2.1.294".into()),
        prefix: vec![Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/claude-cli.mjs")
            .to_string_lossy()
            .into_owned()],
    };
    provider
}

#[cfg(windows)]
fn turn(root: &Path) -> TurnRequest {
    let account = AccountProfile {
        id: "claude-a".into(),
        provider: "anthropic".into(),
        label: "Fixture".into(),
        credential_ref: None,
        config_dir: Some(root.to_string_lossy().into_owned()),
        auth_status: "signed_in".into(),
        created_at: now(),
    };
    TurnRequest {
        attachments: vec![],
        prompt: "Read project".into(),
        account,
        output_schema: None,
        session: Session {
            id: Uuid::new_v4().to_string(),
            workspace_id: "test".into(),
            provider: "anthropic".into(),
            account_profile_id: "claude-a".into(),
            model: "sonnet".into(),
            title: "Test".into(),
            status: "idle".into(),
            permission_profile: "standard".into(),
            working_directory: root.to_string_lossy().into_owned(),
            provider_session_id: None,
            worktree_id: None,
            created_at: now(),
            updated_at: now(),
            reasoning_effort: Some("high".into()),
            tool_policy: "{}".into(),
            parent_session_id: None,
            chat_mode: "single".into(),
            role: String::new(),
            context_summary: String::new(),
        },
    }
}

#[cfg(windows)]
#[tokio::test]
#[ignore = "requires official native Claude CLI; fresh unsigned profile, no inference"]
async fn installed_cli_accepts_restricted_launch_and_reports_missing_auth() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let (notifications, _) = broadcast::channel(16);
    let provider = ClaudeProvider::new(notifications);
    provider.refresh().await;
    provider.ready().unwrap();
    let request = turn(temp.path());
    std::fs::create_dir_all(temp.path().join("skills/preflight")).unwrap();
    std::fs::write(
        temp.path().join("skills/preflight/SKILL.md"),
        "---\nname: preflight\ndescription: isolated flag check\n---\nDo not execute commands.",
    )
    .unwrap();
    assert_eq!(
        provider.auth_status(&request.account).await.unwrap().state,
        "signed_out"
    );
    let (tx, _rx) = mpsc::channel(64);
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        provider.drive(&request, tx, CancellationToken::new()),
    )
    .await
    .unwrap();
    assert!(
        matches!(result, Err(CoreError::Provider { kind: Some(kind), .. }) if kind == "auth"),
        "official CLI must parse all configured flags and refuse inference without our own login"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn real_subprocess_pipe_approval_resume_and_account_binding() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut request = turn(temp.path());
    let profile = temp.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    request.account.config_dir = Some(profile.to_string_lossy().into_owned());
    request.prompt = "[WRITE]".into();
    let (tx, mut rx) = mpsc::channel(64);
    let engine = provider.clone();
    let sent = request.clone();
    let task = tokio::spawn(async move { engine.run(sent, tx, CancellationToken::new()).await });
    let mut thread = None;
    let mut text = String::new();
    while let Some(event) = tokio::time::timeout(Duration::from_secs(15), rx.recv())
        .await
        .unwrap()
    {
        match event {
            EventPayload::ApprovalRequested { id, .. } => {
                assert!(!temp.path().join("approved.txt").exists());
                assert!(provider
                    .resolve_approval("wrong-session", &id, ApprovalDecision::AllowOnce)
                    .await
                    .is_err());
                provider
                    .resolve_approval(&request.session.id, &id, ApprovalDecision::AllowOnce)
                    .await
                    .unwrap();
            }
            EventPayload::ProviderSession { id } => thread = Some(id),
            EventPayload::AssistantTextDelta { text: delta } => text.push_str(&delta),
            _ => {}
        }
    }
    task.await.unwrap().unwrap();
    assert_eq!(text, "Claude fixture result");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("approved.txt")).unwrap(),
        "Approved through the local pipe"
    );
    request.session.provider_session_id = thread.clone();
    request.prompt = "Continue".into();
    let (tx, mut rx) = mpsc::channel(64);
    provider
        .run(request.clone(), tx, CancellationToken::new())
        .await
        .unwrap();
    let mut resumed = None;
    while let Some(event) = rx.recv().await {
        if let EventPayload::ProviderSession { id } = event {
            resumed = Some(id);
        }
    }
    assert_eq!(thread, resumed);
    request.account.id = "other-account".into();
    let (tx, _) = mpsc::channel(4);
    assert!(provider
        .run(request, tx, CancellationToken::new())
        .await
        .is_err());
}

#[cfg(windows)]
#[tokio::test]
async fn read_only_denial_cancellation_and_usage_limit_stop_the_process() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut request = turn(temp.path());
    request.session.permission_profile = "read_only".into();
    request.prompt = "[WRITE]".into();
    let (tx, mut rx) = mpsc::channel(64);
    provider
        .run(request.clone(), tx, CancellationToken::new())
        .await
        .unwrap();
    while let Some(event) = rx.recv().await {
        assert!(!matches!(event, EventPayload::ApprovalRequested { .. }));
    }
    assert!(!temp.path().join("approved.txt").exists());
    request.prompt = "[WAIT]".into();
    let (tx, mut rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();
    let next = cancel.clone();
    let engine = provider.clone();
    let sent = request.clone();
    let task = tokio::spawn(async move { engine.run(sent, tx, next).await });
    tokio::time::timeout(Duration::from_secs(15), rx.recv())
        .await
        .unwrap()
        .unwrap();
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    request.prompt = "[LIMIT]".into();
    let (tx, _rx) = mpsc::channel(64);
    let error = provider
        .run(request, tx, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, CoreError::Provider { kind:Some(kind), .. } if kind == "usage_limit"));
}

#[cfg(windows)]
#[tokio::test]
async fn cancelling_or_releasing_an_account_stops_auth_preflight() {
    for release_account in [false, true] {
        let temp = crate::test_support::TestDirectory::new().unwrap();
        std::fs::write(temp.path().join("fixture-auth-wait"), "test only").unwrap();
        let provider = fixture_provider();
        let request = turn(temp.path());
        let account = request.account.clone();
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let engine = provider.clone();
        let (tx, _rx) = mpsc::channel(64);
        let task = tokio::spawn(async move { engine.run(request, tx, token).await });
        tokio::time::timeout(Duration::from_secs(10), async {
            while !temp.path().join("fixture-auth-started").exists() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        if release_account {
            provider.release_account(&account).await;
        } else {
            cancel.cancel();
        }
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(provider.running.lock().unwrap().is_empty());
    }
}

#[cfg(windows)]
#[tokio::test]
async fn session_grants_cover_one_file_and_cancellation_denies_pending_writes() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let session = turn(temp.path()).session;
    let (tx, mut rx) = mpsc::channel(64);
    let input = json!({"hook_event_name":"PermissionRequest", "tool_name":"Write", "tool_input":{"file_path":"one.txt", "content":"data"}});
    let cancel = CancellationToken::new();
    let launch = |session: Session, input: Value, cancel: CancellationToken| {
        let engine = provider.clone();
        let events = tx.clone();
        tokio::spawn(async move {
            guard::decide(
                &session,
                input,
                &events,
                &cancel,
                &engine.pending,
                &engine.grants,
            )
            .await
        })
    };
    async fn next_approval(rx: &mut mpsc::Receiver<EventPayload>) -> String {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(EventPayload::ApprovalRequested { id, .. }) = rx.recv().await {
                    return id;
                }
            }
        })
        .await
        .unwrap()
    }
    let first = launch(session.clone(), input.clone(), cancel.clone());
    let id = next_approval(&mut rx).await;
    provider
        .resolve_approval(&session.id, &id, ApprovalDecision::AllowSession)
        .await
        .unwrap();
    assert_eq!(
        first.await.unwrap()["hookSpecificOutput"]["decision"]["behavior"],
        "allow"
    );
    let same = guard::decide(
        &session,
        input.clone(),
        &tx,
        &cancel,
        &provider.pending,
        &provider.grants,
    )
    .await;
    assert_eq!(same["hookSpecificOutput"]["decision"]["behavior"], "allow");
    let mut other = input.clone();
    other["tool_input"]["file_path"] = json!("two.txt");
    let waiting = launch(session.clone(), other, cancel.clone());
    let _ = next_approval(&mut rx).await;
    cancel.cancel();
    assert_eq!(
        waiting.await.unwrap()["hookSpecificOutput"]["decision"]["behavior"],
        "deny"
    );
    assert!(provider.pending.lock().unwrap().is_empty());
    let mut another = session.clone();
    another.id = Uuid::new_v4().to_string();
    let waiting = launch(another.clone(), input.clone(), CancellationToken::new());
    let id = next_approval(&mut rx).await;
    provider
        .resolve_approval(&another.id, &id, ApprovalDecision::Deny)
        .await
        .unwrap();
    assert_eq!(
        waiting.await.unwrap()["hookSpecificOutput"]["decision"]["behavior"],
        "deny"
    );
    let mut read_only = session;
    read_only.permission_profile = "read_only".into();
    assert_eq!(
        guard::decide(
            &read_only,
            input,
            &tx,
            &CancellationToken::new(),
            &provider.pending,
            &provider.grants
        )
        .await["hookSpecificOutput"]["decision"]["behavior"],
        "deny"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn mixed_team_auto_and_handoff_use_claude_adapter_and_preserve_results() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let claude = fixture_provider();
    let core = crate::Core::open_with_providers(
        &temp.path().join("data.db"),
        vec![Arc::new(crate::provider::MockProvider), claude],
    )
    .await
    .unwrap();
    let account = core
        .add_account("anthropic", "Claude fixture")
        .await
        .unwrap();
    let config = AgentConfig {
        provider: "anthropic".into(),
        account_profile_id: account.id,
        model: "sonnet".into(),
        reasoning_effort: Some("high".into()),
        permission_profile: "standard".into(),
        tools: ToolPolicy::default(),
        role: "Claude reviewer".into(),
    };
    let demo = AgentConfig {
        provider: "mock".into(),
        account_profile_id: "mock-local".into(),
        model: "mock-stream-v1".into(),
        reasoning_effort: None,
        permission_profile: "standard".into(),
        tools: ToolPolicy::default(),
        role: "Coordinator fixture".into(),
    };
    async fn settle(core: &crate::Core, id: &str) {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if core.storage.session(id).await.unwrap().status != "running" {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(core.storage.session(id).await.unwrap().status, "completed");
    }
    let team = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "team".into(),
            agents: vec![config.clone(), demo.clone()],
        })
        .await
        .unwrap();
    core.send_message(&team.id, "Prepare shared result".into())
        .await
        .unwrap();
    settle(&core, &team.id).await;
    let history = core.storage.events(&team.id, None).await.unwrap();
    assert_eq!(
        history
            .iter()
            .filter_map(
                |e| if let EventPayload::AssistantTextDelta { text } = &e.payload {
                    Some(text.as_str())
                } else {
                    None
                }
            )
            .collect::<String>(),
        "Claude fixture result"
    );
    let auto = core
        .create_chat(CreateChat {
            workspace_id: None,
            mode: "auto".into(),
            agents: vec![config],
        })
        .await
        .unwrap();
    core.send_message(&auto.id, "Plan and analyse".into())
        .await
        .unwrap();
    settle(&core, &auto.id).await;
    let task = core
        .storage
        .sessions()
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.parent_session_id.as_deref() == Some(&auto.id) && s.chat_mode == "task")
        .unwrap();
    assert_eq!(task.provider, "anthropic");
    assert_eq!(task.permission_profile, "read_only");
    core.handoff(&team.id, demo).await.unwrap();
    settle(&core, &team.id).await;
    let handed = core.storage.session(&team.id).await.unwrap();
    assert_eq!(handed.provider, "mock");
    assert!(handed.provider_session_id.is_none());
    assert!(!handed.context_summary.is_empty());
    core.shutdown().await;
}
