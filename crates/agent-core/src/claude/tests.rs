use super::*;
use serde_json::json;

#[cfg(windows)]
#[tokio::test]
async fn streamed_followup_is_acknowledged_in_same_cli_and_all_native_results_are_counted() {
    use base64::Engine;
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut request = turn(temp.path());
    let profile = temp.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    request.account.config_dir = Some(profile.to_string_lossy().into());
    request.prompt = "[LIVE]".into();
    let mut image = vec![0u8; 3 * 1024 * 1024];
    image[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    let files = crate::attachments::prepare(
        &request.session,
        (0..2)
            .map(|index| crate::attachments::Attachment {
                name: format!("picture-{index}.png"),
                mime: "image/png".into(),
                data: base64::engine::general_purpose::STANDARD.encode(&image),
            })
            .collect(),
    )
    .unwrap();
    let (input, rx) = mpsc::channel(8);
    request.live_input = Some(Arc::new(tokio::sync::Mutex::new(rx)));
    let (ack, acknowledged) = oneshot::channel();
    input
        .send(crate::conversation::Followup {
            prompt: "Live addition".into(),
            attachments: files,
            acknowledged: ack,
        })
        .await
        .unwrap();
    let (tx, mut events) = mpsc::channel(64);
    tokio::time::timeout(
        Duration::from_secs(10),
        provider.run(request, tx, CancellationToken::new()),
    )
    .await
    .unwrap()
    .unwrap();
    acknowledged.await.unwrap().unwrap();
    assert!(input.is_closed());
    let mut usage = 0;
    let mut output = String::new();
    let mut diagnostic = false;
    while let Some(event) = events.recv().await {
        match event {
            EventPayload::ProviderUsage {
                input_tokens: Some(n),
                ..
            } => usage += n,
            EventPayload::AssistantTextDelta { text } => output.push_str(&text),
            EventPayload::ProviderDiagnostic { report } => {
                assert_eq!(report.input_tokens, Some(15));
                assert_eq!(report.model_requests, Some(3));
                diagnostic = true;
            }
            _ => {}
        }
    }
    assert_eq!(usage, 15);
    assert!(diagnostic);
    assert_eq!(output, "First result\n\nClaude fixture result");
    assert_eq!(
        std::fs::read_to_string(profile.join("fixture-invocations.jsonl"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[cfg(windows)]
#[tokio::test]
async fn closed_claude_stdout_does_not_wait_for_the_open_input_channel() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut request = turn(temp.path());
    let profile = temp.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    request.account.config_dir = Some(profile.to_string_lossy().into());
    request.prompt = "[EXIT_WITHOUT_RESULT]".into();
    let (_input, rx) = mpsc::channel(8);
    request.live_input = Some(Arc::new(tokio::sync::Mutex::new(rx)));
    let (tx, _events) = mpsc::channel(64);
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        provider.run(request, tx, CancellationToken::new()),
    )
    .await
    .unwrap();
    assert!(result.is_err());
}

#[test]
fn requires_supported_official_cli() {
    assert!(!version_supported(None));
    assert!(!version_supported(Some("2.1.89")));
    assert!(version_supported(Some("2.1.294")));
}

#[cfg(windows)]
#[tokio::test]
async fn manual_shell_and_mcp_require_fresh_consent() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut session = turn(temp.path()).session;
    session.permission_profile = "standard".into();
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
fn replayed_events_and_tool_ids_do_not_duplicate_visible_activity() {
    let mut state = mapping::StreamState::new(false);
    let chunk = json!({"type":"stream_event","uuid":"chunk-1","event":{"delta":{"type":"text_delta","text":"ha"}}});
    assert_eq!(state.accept(chunk.clone()).unwrap().len(), 1);
    assert!(state.accept(chunk).unwrap().is_empty());
    // Equal text with a new event ID is real output, not a duplicate.
    assert_eq!(state.accept(json!({"type":"stream_event","uuid":"chunk-2","event":{"delta":{"type":"text_delta","text":"ha"}}})).unwrap().len(), 1);
    let tool = json!({"type":"assistant","uuid":"assistant-1","message":{"content":[{"type":"tool_use","id":"tool-1","name":"Write","input":{"file_path":"notes.txt","content":"private-content
second"}}]}});
    let activity = state.accept(tool.clone()).unwrap();
    assert_eq!(activity.len(), 1);
    let shown = serde_json::to_string(&activity).unwrap();
    assert!(shown.contains("notes.txt") && !shown.contains("private-content"));
    assert!(matches!(
        &activity[0],
        EventPayload::ToolStarted { added: Some(2), .. }
    ));
    assert!(state.accept(tool.clone()).unwrap().is_empty());
    let mut repeated = tool;
    repeated["uuid"] = json!("assistant-2");
    assert!(state.accept(repeated).unwrap().is_empty());
}

#[test]
fn session_limit_result_is_reported_as_a_usage_limit() {
    let mut state = mapping::StreamState::new(false);
    let error = state
        .accept(json!({
            "type": "result", "subtype": "error_during_execution", "is_error": true,
            "result": "You've hit your session limit · resets 10:10pm (Europe/Moscow)"
        }))
        .unwrap_err();
    assert!(matches!(error, CoreError::Provider { kind: Some(kind), .. } if kind == "usage_limit"));
    assert!(state.finish().is_err());
}

#[test]
fn usage_uses_per_turn_counters_and_never_sums_resumed_cumulative_cost_or_usage() {
    let event = mapping::reported_usage(&json!({
        "type":"result", "num_turns": 3,
        "usage": {"input_tokens": 123,"output_tokens":45,"cache_read_input_tokens":900,"cache_creation_input_tokens":12},
        "modelUsage": {"claude-opus-5-5":{"inputTokens":99999,"costUSD":999}},
        "total_cost_usd":999, "private":"password=do-not-persist"
    })).unwrap();
    assert_eq!(
        event,
        EventPayload::ProviderUsage {
            provider: "anthropic".into(),
            model_requests: Some(3),
            input_tokens: Some(123),
            output_tokens: Some(45),
            cache_read_tokens: Some(900),
            cache_creation_tokens: Some(12),
            reasoning_tokens: None,
            incomplete: false,
        }
    );
    assert!(
        mapping::reported_usage(&json!({"type":"result","usage":{"input_tokens":-1}})).is_none()
    );
    assert!(mapping::reported_usage(
        &json!({"type":"result","parent_tool_use_id":"child","num_turns":3})
    )
    .is_none());
    assert!(mapping::reported_usage(
        &json!({"type":"result","modelUsage":{"opus":{"inputTokens":999}}})
    )
    .is_none());
    assert!(!serde_json::to_string(&event)
        .unwrap()
        .contains("do-not-persist"));
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
        live_input: None,
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
            conversation_settings: "{}".into(),
            context_summary: String::new(),
            fast_mode: false,
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
    request.session.model = "claude-opus-5-5".into();
    request.session.fast_mode = true;
    request.prompt = "[WRITE]".into();
    let (tx, mut rx) = mpsc::channel(64);
    let engine = provider.clone();
    let sent = request.clone();
    let task = tokio::spawn(async move { engine.run(sent, tx, CancellationToken::new()).await });
    let mut thread = None;
    let mut text = String::new();
    let mut usage_events = 0;
    let mut reports = Vec::new();
    while let Some(event) = tokio::time::timeout(Duration::from_secs(15), rx.recv())
        .await
        .unwrap()
    {
        match event {
            EventPayload::ProviderDiagnostic { report } => reports.push(report),
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
            EventPayload::ProviderUsage {
                input_tokens,
                cache_read_tokens,
                model_requests,
                ..
            } => {
                usage_events += 1;
                assert_eq!(input_tokens, Some(10));
                assert_eq!(cache_read_tokens, Some(20));
                assert_eq!(model_requests, Some(2));
            }
            _ => {}
        }
    }
    task.await.unwrap().unwrap();
    assert_eq!(text, "Claude fixture result");
    assert_eq!(usage_events, 1);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].status, "completed");
    assert_eq!(reports[0].approval_requests, 1);
    assert_eq!(reports[0].input_tokens, Some(10));
    assert_eq!(reports[0].account_profile_id, request.account.id);
    assert_eq!(reports[0].provider_session_id, thread);
    assert!(!reports[0].resumed);
    assert_eq!(
        std::fs::read_to_string(temp.path().join("approved.txt")).unwrap(),
        "Approved through the local pipe"
    );
    request.session.provider_session_id = thread.clone();
    request.session.fast_mode = false;
    request.prompt = "Continue".into();
    let (tx, mut rx) = mpsc::channel(64);
    provider
        .run(request.clone(), tx, CancellationToken::new())
        .await
        .unwrap();
    let mut resumed = None;
    while let Some(event) = rx.recv().await {
        if let EventPayload::ProviderDiagnostic { report } = &event {
            assert!(report.resumed);
            assert_eq!(report.provider_session_id, thread);
            assert_ne!(report.request_id, reports[0].request_id);
            assert_eq!(report.approval_requests, 0);
        }
        if let EventPayload::ProviderSession { id } = event {
            resumed = Some(id);
        }
    }
    assert_eq!(thread, resumed);
    let launches: Vec<serde_json::Value> =
        std::fs::read_to_string(profile.join("fixture-invocations.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    assert_eq!(launches.len(), 2);
    assert_eq!(launches[0]["fast_mode"], true);
    assert_eq!(launches[1]["fast_mode"], false);
    assert_eq!(launches[0]["id"], launches[1]["id"]);
    assert_eq!(launches[0]["model"], "claude-opus-5-5");
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
    let (tx, mut failure_rx) = mpsc::channel(64);
    let error = provider
        .run(request, tx, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(error, CoreError::Provider { kind:Some(kind), .. } if kind == "usage_limit"));
    let mut reports = Vec::new();
    while let Some(event) = failure_rx.recv().await {
        if let EventPayload::ProviderDiagnostic { report } = event {
            reports.push(report);
        }
    }
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].status, "failed");
    assert_eq!(reports[0].error_kind.as_deref(), Some("usage_limit"));
    assert!(!reports[0].usage_reported);
    assert_eq!(reports[0].input_tokens, None);
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
    // A handoff can keep the local chat ID while changing the bound account.
    let mut rebound = session.clone();
    rebound.account_profile_id = "other-profile".into();
    let waiting = launch(rebound.clone(), input.clone(), CancellationToken::new());
    let id = next_approval(&mut rx).await;
    provider
        .resolve_approval(&rebound.id, &id, ApprovalDecision::Deny)
        .await
        .unwrap();
    assert_eq!(
        waiting.await.unwrap()["hookSpecificOutput"]["decision"]["behavior"],
        "deny"
    );
    // Non-interactive mode rejects unexpected provider asks without an approval card.
    while let Ok(event) = rx.try_recv() {
        assert!(!matches!(event, EventPayload::ApprovalRequested { .. }));
    }
    let mut auto = session.clone();
    auto.permission_profile = "workspace_auto".into();
    let waiting = launch(auto.clone(), input.clone(), CancellationToken::new());
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), waiting)
            .await
            .unwrap()
            .unwrap()["hookSpecificOutput"]["decision"]["behavior"],
        "deny"
    );
    assert!(rx.try_recv().is_err());
    let mut unrelated = turn(temp.path()).account;
    unrelated.id = "unrelated".into();
    provider.release_account(&unrelated).await;
    assert_eq!(provider.grants.lock().unwrap().len(), 1);
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
        fast_mode: false,
        provider: "anthropic".into(),
        account_profile_id: account.id,
        model: "sonnet".into(),
        reasoning_effort: Some("high".into()),
        permission_profile: "standard".into(),
        tools: ToolPolicy::default(),
        role: "Claude reviewer".into(),
    };
    let demo = AgentConfig {
        fast_mode: false,
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
    assert!(!core
        .storage
        .sessions()
        .await
        .unwrap()
        .iter()
        .any(|s| s.parent_session_id.as_deref() == Some(&auto.id) && s.chat_mode == "task"));
    let auto_events = core.storage.events(&auto.id, None).await.unwrap();
    assert_eq!(
        auto_events
            .iter()
            .filter(|e| matches!(e.payload, EventPayload::ProviderRunStarted { .. }))
            .count(),
        1
    );
    core.handoff(&team.id, demo).await.unwrap();
    settle(&core, &team.id).await;
    let handed = core.storage.session(&team.id).await.unwrap();
    assert_eq!(handed.provider, "mock");
    assert!(handed.provider_session_id.is_none());
    assert!(!handed.context_summary.is_empty());
    core.shutdown().await;
}

#[cfg(windows)]
#[tokio::test]
async fn accept_edits_reaches_cli_and_applies_only_to_guarded_project_files() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut request = turn(temp.path());
    let profile = temp.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    request.account.config_dir = Some(profile.to_string_lossy().into_owned());
    request.session.permission_profile = "workspace_auto".into();
    request.prompt = "[WRITE]".into();
    let (tx, mut rx) = mpsc::channel(64);
    tokio::time::timeout(
        Duration::from_secs(15),
        provider.run(request, tx, CancellationToken::new()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(temp.path().join("approved.txt").exists());
    let mut report_count = 0;
    while let Some(event) = rx.recv().await {
        assert!(!matches!(event, EventPayload::ApprovalRequested { .. }));
        if let EventPayload::ProviderDiagnostic { report } = event {
            assert_eq!(report.permission_profile, "workspace_auto");
            assert_eq!(report.approval_requests, 0);
            report_count += 1;
        }
    }
    assert_eq!(report_count, 1);
}

#[cfg(windows)]
#[tokio::test]
async fn cancellation_does_not_wait_for_a_full_diagnostic_consumer() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let (tx, _rx) = mpsc::channel(1);
    tx.send(EventPayload::TurnStarted {
        prompt: "queued".into(),
    })
    .await
    .unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    tokio::time::timeout(
        Duration::from_secs(2),
        provider.run(turn(temp.path()), tx, cancel),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(provider.running.lock().unwrap().is_empty());
}

#[cfg(windows)]
#[tokio::test]
async fn native_modes_and_persistent_rules_reach_cli_without_changing_account() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let profile = temp.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    let mut request = turn(temp.path());
    request.account.config_dir = Some(profile.to_string_lossy().into_owned());
    request.prompt = "[WRITE]".into();
    for mode in ["workspace_auto", "full_access", "claude_auto", "plan"] {
        let provider = fixture_provider();
        request.session.permission_profile = mode.into();
        let output = temp.path().join("approved.txt");
        let _ = std::fs::remove_file(&output);
        let (tx, mut rx) = mpsc::channel(64);
        tokio::time::timeout(
            Duration::from_secs(15),
            provider.run(request.clone(), tx, CancellationToken::new()),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(output.exists(), mode != "plan");
        while let Some(event) = rx.recv().await {
            assert!(!matches!(event, EventPayload::ApprovalRequested { .. }));
        }
    }
    request.session.permission_profile = "standard".into();
    let rule = rules::FileRule {
        root: temp
            .path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        path: "approved.txt".into(),
        tool: "Write".into(),
    };
    rules::save(&request.account, &request.session, vec![rule.clone()]).unwrap();
    assert_eq!(
        rules::read(&request.account, &request.session)
            .unwrap()
            .len(),
        1
    );
    let new_provider = fixture_provider();
    let (tx, mut rx) = mpsc::channel(64);
    tokio::time::timeout(
        Duration::from_secs(15),
        new_provider.run(request.clone(), tx, CancellationToken::new()),
    )
    .await
    .unwrap()
    .unwrap();
    while let Some(event) = rx.recv().await {
        assert!(!matches!(event, EventPayload::ApprovalRequested { .. }));
    }
    assert!(temp.path().join("approved.txt").exists());
    let mut unsafe_rule = rule;
    unsafe_rule.path = ".env".into();
    assert!(rules::save(&request.account, &request.session, vec![unsafe_rule]).is_err());
    rules::save(&request.account, &request.session, vec![]).unwrap();
    assert!(rules::read(&request.account, &request.session)
        .unwrap()
        .is_empty());
}

#[cfg(windows)]
#[tokio::test]
async fn automatic_modes_do_not_prompt_for_selected_tools_or_escalations() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut session = turn(temp.path()).session;
    let mut access = access::Access::default();
    access.mcp = vec![crate::mcp::LocalMcpServer {
        name: "unity".into(),
        command: "fixture".into(),
        args: vec![],
    }];
    let (tx, mut rx) = mpsc::channel(64);
    for mode in ["workspace_auto", "full_access", "claude_auto"] {
        session.permission_profile = mode.into();
        let settings = guard::settings("fixture", Path::new("fixture.exe"), mode, true);
        assert_eq!(settings["permissions"]["ask"], json!([]));
        if mode == "workspace_auto" {
            assert_eq!(settings["permissions"]["defaultMode"], "dontAsk");
            for tool in ["Bash", "PowerShell", "Edit", "Write", "mcp__*"] {
                assert!(settings["permissions"]["allow"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(tool)));
            }
        }
        for (tool, input) in [
            ("Bash", json!({"command":"echo safe"})),
            ("PowerShell", json!({"command":"Write-Output safe"})),
            ("mcp__unity__capture_scene_view", json!({"scene":"test"})),
            ("Write", json!({"file_path":"safe.txt", "content":"test"})),
        ] {
            for _ in 0..2 {
                let value =
                    json!({"hook_event_name":"PreToolUse", "tool_name":tool, "tool_input":input});
                let result = tokio::time::timeout(
                    Duration::from_secs(1),
                    guard::decide_with_access(
                        &session,
                        value.clone(),
                        &tx,
                        &CancellationToken::new(),
                        &provider.pending,
                        &provider.grants,
                        &access,
                    ),
                )
                .await
                .unwrap();
                assert_eq!(result, json!({}), "{mode}/{tool}");
                if mode != "claude_auto" {
                    let mut permission = value;
                    permission["hook_event_name"] = json!("PermissionRequest");
                    let result = tokio::time::timeout(
                        Duration::from_secs(1),
                        guard::decide_with_access(
                            &session,
                            permission,
                            &tx,
                            &CancellationToken::new(),
                            &provider.pending,
                            &provider.grants,
                            &access,
                        ),
                    )
                    .await
                    .unwrap();
                    assert_eq!(result["hookSpecificOutput"]["decision"]["behavior"], "deny");
                }
            }
        }
        for (tool, input) in [
            ("Bash", json!({"command":"cat ~/.claude/.credentials.json"})),
            ("mcp__unselected__run", json!({"scene":"test"})),
        ] {
            let answer = guard::decide_with_access(
                &session,
                json!({"hook_event_name":"PreToolUse","tool_name":tool,"tool_input":input}),
                &tx,
                &CancellationToken::new(),
                &provider.pending,
                &provider.grants,
                &access,
            )
            .await;
            assert_eq!(answer["hookSpecificOutput"]["permissionDecision"], "deny");
        }
        assert!(provider.pending.lock().unwrap().is_empty());
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn saved_file_rule_remains_revocable_when_its_parent_is_removed() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let mut request = turn(temp.path());
    let profile = temp.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    request.account.config_dir = Some(profile.to_string_lossy().into_owned());
    let nested = temp.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    rules::save(
        &request.account,
        &request.session,
        vec![rules::FileRule {
            root: temp
                .path()
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            path: "nested/new.txt".into(),
            tool: "Write".into(),
        }],
    )
    .unwrap();
    std::fs::remove_dir(nested).unwrap();
    assert_eq!(
        rules::read(&request.account, &request.session)
            .unwrap()
            .len(),
        1
    );
    rules::save(&request.account, &request.session, vec![]).unwrap();
    assert!(rules::read(&request.account, &request.session)
        .unwrap()
        .is_empty());
}
#[test]
fn context_counts_latest_input_with_cache_not_cumulative_billing_and_resets_on_compact() {
    let mut state = mapping::StreamState::new(false);
    let events=state.accept(json!({"type":"assistant","message":{"usage":{"input_tokens":10,"cache_read_input_tokens":90,"cache_creation_input_tokens":20},"content":[]}})).unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        EventPayload::ContextUsage {
            used_tokens: Some(120),
            window_tokens: None,
            ..
        }
    )));
    let events=state.accept(json!({"type":"result","subtype":"success","is_error":false,"modelUsage":{"test-model":{"contextWindow":200000,"inputTokens":5000000}},"usage":{"input_tokens":999999}})).unwrap();
    assert!(events.iter().any(|e| matches!(
        e,
        EventPayload::ContextUsage {
            used_tokens: Some(120),
            window_tokens: Some(200000),
            ..
        }
    )));
    let events = state
        .accept(json!({"type":"system","subtype":"compact_boundary"}))
        .unwrap();
    assert!(events.contains(&EventPayload::ContextCompacted {}));
    let events=state.accept(json!({"type":"assistant","parent_tool_use_id":"child","message":{"usage":{"input_tokens":777},"content":[]}})).unwrap();
    assert!(events.is_empty());
}
#[cfg(windows)]
#[tokio::test]
async fn compact_uses_native_command_with_no_tools_and_explicit_context_settings() {
    let temp = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut request = turn(temp.path());
    let profile = temp.path().join("profile");
    std::fs::create_dir(&profile).unwrap();
    request.account.config_dir = Some(profile.to_string_lossy().into());
    request.session.provider_session_id = Some(Uuid::new_v4().to_string());
    request.session.conversation_settings =
        r#"{"compact_tokens":200000,"claude_compact_percent":70}"#.into();
    request.prompt = "/compact".into();
    let (tx, mut rx) = mpsc::channel(32);
    provider
        .run(request, tx, CancellationToken::new())
        .await
        .unwrap();
    let mut compacted = false;
    while let Some(event) = rx.recv().await {
        compacted |= matches!(event, EventPayload::ContextCompacted {});
    }
    assert!(compacted);
    let record: Value = serde_json::from_str(
        std::fs::read_to_string(profile.join("fixture-invocations.jsonl"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    assert_eq!(record["compact_window"], "200000");
    assert_eq!(record["compact_percent"], "70");
    assert_eq!(record["tools"], "");
}

#[test]
fn tool_rows_are_short_redacted_and_task_lists_are_tracked() {
    let mut state = mapping::StreamState::new(false);
    let events = state
        .accept(json!({"type":"assistant","message":{"content":[
            {"type":"tool_use","id":"t1","name":"Bash","input":{"command":"npm test","description":"Run tests"}},
            {"type":"tool_use","id":"t2","name":"Bash","input":{"command":"curl -H 'Authorization: Bearer abc123'\nsecond line"}},
            {"type":"tool_use","id":"t3","name":"Edit","input":{"file_path":"src/a.rs","old_string":"a","new_string":"b\nc"}},
            {"type":"tool_use","id":"t4","name":"TodoWrite","input":{"todos":[{"content":"Step one","status":"completed"},{"content":"Step two","status":"in_progress"}]}},
            {"type":"tool_use","id":"t5","name":"TaskCreate","input":{"subject":"Build","description":"private"}},
            {"type":"tool_use","id":"t6","name":"ToolSearch","input":{"query":"unity"}},
            {"type":"tool_use","id":"t7","name":"mcp__unity__editor_status","input":{}}
        ]}}))
        .unwrap();
    let shown = serde_json::to_string(&events).unwrap();
    assert!(
        !shown.contains("abc123") && !shown.contains("second line") && !shown.contains("private")
    );
    assert!(
        matches!(&events[0], EventPayload::ToolStarted { kind, summary, .. } if kind == "command" && summary == "Run tests")
    );
    assert!(
        matches!(&events[2], EventPayload::ToolStarted { kind, added: Some(2), removed: Some(1), .. } if kind == "edit")
    );
    assert!(
        matches!(&events[3], EventPayload::TodoUpdate { replace: true, items } if items.len() == 2 && items[1].status.as_deref() == Some("in_progress"))
    );
    assert!(
        matches!(&events[4], EventPayload::ToolStarted { kind, summary, .. } if kind == "mcp" && summary == "unity · editor_status")
    );
    assert_eq!(events.len(), 5);
    let finished = state
        .accept(json!({"type":"user","message":{"content":[
            {"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"failed"},
            {"type":"tool_result","tool_use_id":"t5","content":[{"type":"text","text":"Task #3 created successfully: Build"}]}
        ]}}))
        .unwrap();
    assert_eq!(
        finished,
        vec![
            EventPayload::ToolFinished {
                id: "t1".into(),
                failed: true
            },
            EventPayload::TodoUpdate {
                replace: false,
                items: vec![crate::model::TodoItem {
                    id: Some("3".into()),
                    content: Some("Build".into()),
                    status: Some("pending".into()),
                }],
            },
        ]
    );
}

#[test]
fn credential_check_ignores_code_that_only_mentions_environment() {
    for safe in [
        "node -e \"console.log(process.env.HOME)\"",
        "var p = System.Environment.GetFolderPath(0);",
        "python -c 'import os; print(os.environ)'",
        "cat CLAUDE.md",
    ] {
        assert!(!guard::touches_credentials(safe), "{safe}");
    }
    for blocked in [
        "type .env",
        "cat config/.env.local",
        "cat ~/.claude/.credentials.json",
        "ls C:\\Users\\me\\.ssh",
        "cmdkey /list",
    ] {
        assert!(guard::touches_credentials(blocked), "{blocked}");
    }
}

#[cfg(windows)]
#[tokio::test]
async fn automatic_modes_accept_long_commands_internal_tools_and_memory() {
    let profile = crate::test_support::TestDirectory::new().unwrap();
    let project = crate::test_support::TestDirectory::new().unwrap();
    let provider = fixture_provider();
    let mut request = turn(project.path());
    request.account.config_dir = Some(profile.path().to_string_lossy().into_owned());
    let (tx, _rx) = mpsc::channel(64);
    let decide = |session: Session, access: access::Access, tool: &str, input: Value| {
        let provider = provider.clone();
        let tx = tx.clone();
        let value = json!({"hook_event_name":"PreToolUse","tool_name":tool,"tool_input":input});
        async move {
            guard::decide_with_access(
                &session,
                value,
                &tx,
                &CancellationToken::new(),
                &provider.pending,
                &provider.grants,
                &access,
            )
            .await
        }
    };
    let long = format!("echo {}", "x".repeat(6000));
    for mode in [
        "standard",
        "workspace_auto",
        "full_access",
        "claude_auto",
        "read_only",
    ] {
        request.session.permission_profile = mode.into();
        let access = access::Access::for_request(&request).unwrap();
        let projects = profile.path().join("projects");
        let key = std::fs::read_dir(&projects)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let memory = key.path().join("memory").join("MEMORY.md");
        std::fs::write(&memory, "- note").unwrap();
        let session = request.session.clone();
        let write = decide(
            session.clone(),
            access.clone(),
            "Write",
            json!({"file_path":memory,"content":"- note"}),
        )
        .await;
        let read = decide(
            session.clone(),
            access.clone(),
            "Read",
            json!({"file_path":memory}),
        )
        .await;
        assert_eq!(
            read["hookSpecificOutput"]["permissionDecision"], "allow",
            "{mode}"
        );
        for tool in ["TodoWrite", "ToolSearch"] {
            assert_eq!(
                decide(session.clone(), access.clone(), tool, json!({})).await,
                json!({})
            );
        }
        if mode == "read_only" {
            assert_eq!(write["hookSpecificOutput"]["permissionDecision"], "deny");
            continue;
        }
        assert_eq!(
            write["hookSpecificOutput"]["permissionDecision"], "allow",
            "{mode}"
        );
        for command in [
            long.as_str(),
            "node -e \"process.env.X\"",
            "echo password=1",
        ] {
            let answer = decide(
                session.clone(),
                access.clone(),
                "Bash",
                json!({ "command": command }),
            )
            .await;
            let expected = if mode == "standard" {
                json!("ask")
            } else {
                Value::Null
            };
            assert_eq!(
                answer["hookSpecificOutput"]["permissionDecision"],
                expected,
                "{mode}: {}",
                &command[..20.min(command.len())]
            );
        }
        let other = decide(
            session.clone(),
            access.clone(),
            "Write",
            json!({"file_path":profile.path().join("settings.json"),"content":"{}"}),
        )
        .await;
        assert_eq!(
            other["hookSpecificOutput"]["permissionDecision"], "deny",
            "{mode}"
        );
    }
    let settings = guard::settings("fixture", Path::new("fixture.exe"), "full_access", false);
    assert!(settings["permissions"]["deny"]
        .as_array()
        .unwrap()
        .contains(&json!("Agent")));
}
