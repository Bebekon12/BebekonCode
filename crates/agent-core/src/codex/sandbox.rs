//! Windows sandbox lifecycle uses only the official app-server protocol. No auto-approval,
//! host ACL probing, credential inspection or fallback to an unprotected process.
use super::*;

fn status(state: &str, detail: &str) -> SandboxStatus {
    SandboxStatus {
        state: state.into(),
        detail: detail.into(),
    }
}

pub(super) fn parse_readiness(value: &Value) -> SandboxStatus {
    match str_at(value, "status") {
        Some("ready") => status("ready", "Песочница Codex готова. Команды в разрешённых границах выполняются автоматически."),
        Some("notConfigured") => status("not_configured", "Настройте песочницу Windows для этого аккаунта. Без неё автоматическое выполнение команд недоступно."),
        Some("updateRequired") => status("update_required", "Codex требует обновить настройку песочницы Windows."),
        _ => status("unavailable", "CLI не подтвердил готовность песочницы. Обновите официальный Codex CLI."),
    }
}

pub(super) async fn readiness(server: &AppServer) -> SandboxStatus {
    if !cfg!(windows) {
        return status("ready", "Используется штатная песочница Codex для этой ОС.");
    }
    if let Some(value) = server.sandbox.lock().ok().and_then(|value| value.clone()) {
        if value.state == "setting_up" {
            return value;
        }
    }
    match server
        .peer
        .request("windowsSandbox/readiness", Value::Null, REQUEST_TIMEOUT)
        .await
    {
        Ok(value) => parse_readiness(&value),
        Err(_) => status(
            "unavailable",
            "Не удалось проверить песочницу Windows. Обновите Codex CLI и повторите проверку.",
        ),
    }
}

pub(super) async fn ensure_ready(server: &AppServer) -> Result<()> {
    let value = readiness(server).await;
    if value.state == "ready" {
        return Ok(());
    }
    Err(CoreError::Provider {
        message: format!("{} Откройте «Аккаунты → ChatGPT → Песочница Windows». После настройки отправьте задачу повторно.", value.detail),
        kind: Some("sandbox_setup".into()),
    })
}

pub(super) async fn setup(server: &AppServer) -> Result<SandboxStatus> {
    if !cfg!(windows) {
        return Err(CoreError::Invalid(
            "Эта настройка предназначена для Windows".into(),
        ));
    }
    // Do not reconfigure a server with active workers or race a newly starting turn.
    let _operation = server.operation.try_lock().map_err(|_| CoreError::Busy)?;
    if server
        .active
        .lock()
        .map_err(|_| CoreError::Busy)?
        .iter()
        .next()
        .is_some()
    {
        return Err(CoreError::Busy);
    }
    let mut incoming = server.peer.subscribe();
    let setting_up = status(
        "setting_up",
        "Codex настраивает песочницу. Подтвердите системный запрос Windows, если он появится.",
    );
    *server.sandbox.lock().map_err(|_| CoreError::Busy)? = Some(setting_up);
    let result = async {
        let started = server.peer.request("windowsSandbox/setupStart", json!({"mode":"elevated"}), REQUEST_TIMEOUT).await
            .map_err(|error| {
                // A lost response leaves setup outcome unknown. Do not reuse this process.
                if matches!(error, RpcError::Timeout | RpcError::Closed) { server.stop(); }
                provider_error(error, None)
            })?;
        if started.get("started") != Some(&Value::Bool(true)) {
            return Err(CoreError::Invalid("Codex не запустил настройку песочницы".into()));
        }
        let closed = server.peer.closed();
        let completed = tokio::time::timeout(Duration::from_secs(180), async {
            loop {
                let message = tokio::select! {
                    _ = closed.cancelled() => return Err(CoreError::Invalid("Codex завершился во время настройки".into())),
                    message = incoming.recv() => message,
                };
                match message {
                    Ok(Incoming::Notification { method, params }) if method == "windowsSandbox/setupCompleted" => {
                        if str_at(&params, "mode") != Some("elevated") { continue; }
                        if params.get("success") == Some(&Value::Bool(true)) { return Ok(()); }
                        let detail = str_at(&params, "error").unwrap_or("Windows не завершила настройку");
                        return Err(CoreError::Invalid(format!("Песочница не настроена: {}. Проверьте подтверждение Windows и ограничения системы.", clip(&redact(detail), 600))));
                    }
                    Err(_) => {
                        server.stop();
                        return Err(CoreError::Invalid("Связь с Codex потеряна во время настройки".into()));
                    }
                    _ => {}
                }
            }
        }).await;
        match completed {
            Ok(result) => result,
            Err(_) => {
                // Setup outcome is unknown. End this idle process rather than allow commands
                // or a second concurrent setup. A new connection rechecks official readiness.
                server.stop();
                Err(CoreError::Invalid("Codex не подтвердил настройку за 3 минуты. Проверьте системное окно Windows и повторите проверку.".into()))
            }
        }
    }.await;
    *server.sandbox.lock().map_err(|_| CoreError::Busy)? = None;
    result?;
    let value = readiness(server).await;
    if value.state != "ready" {
        return Err(CoreError::Invalid(format!(
            "Настройка завершена, но готовность не подтверждена. {}",
            value.detail
        )));
    }
    Ok(value)
}

/// Absent = legacy protocol with the three normal choices; present = exact offered subset.
pub(super) fn approval_decisions(params: &Value) -> Option<Vec<ApprovalDecision>> {
    params
        .get("availableDecisions")
        .filter(|value| !value.is_null())
        .map(|value| {
            value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|value| match value.as_str() {
                    Some("accept") => Some(ApprovalDecision::AllowOnce),
                    Some("acceptForSession") => Some(ApprovalDecision::AllowSession),
                    Some("decline") => Some(ApprovalDecision::Deny),
                    _ => None,
                })
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    fn fake_server(script: Vec<(&'static str, Value, Option<Value>)>) -> Arc<AppServer> {
        let (client_io, server_io) = tokio::io::duplex(16384);
        let (reader, writer) = tokio::io::split(client_io);
        let (reader2, mut writer2) = tokio::io::split(server_io);
        let peer = RpcPeer::start(reader, writer);
        let closed = peer.closed();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader2).lines();
            for (method, result, notification) in script {
                let request: Value =
                    serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
                assert_eq!(request["method"], method);
                if method == "windowsSandbox/setupStart" {
                    assert_eq!(request["params"], json!({"mode":"elevated"}));
                }
                // Exercise completion arriving before the setupStart response too.
                if let Some(notification) = notification {
                    writer2
                        .write_all(format!("{notification}\n").as_bytes())
                        .await
                        .unwrap();
                }
                let response = json!({"id":request["id"],"result":result});
                writer2
                    .write_all(format!("{response}\n").as_bytes())
                    .await
                    .unwrap();
            }
            closed.cancelled().await;
        });
        Arc::new(AppServer {
            usage_totals: Mutex::default(),
            peer,
            operation: tokio::sync::Mutex::new(()),
            sandbox: Mutex::default(),
            child: Mutex::default(),
            group: Mutex::default(),
            active: Mutex::default(),
            token_expires_at: None,
            loaded: Mutex::default(),
            stderr: Arc::default(),
        })
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn not_configured_fails_before_inference_without_starting_setup() {
        let server = fake_server(vec![(
            "windowsSandbox/readiness",
            json!({"status":"notConfigured"}),
            None,
        )]);
        let error = ensure_ready(&server).await.unwrap_err();
        assert!(
            matches!(error, CoreError::Provider { kind: Some(kind), .. } if kind == "sandbox_setup")
        );
        server.stop();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn setup_waits_for_completion_and_rechecks_readiness() {
        let server = fake_server(vec![
            (
                "windowsSandbox/setupStart",
                json!({"started":true}),
                Some(
                    json!({"method":"windowsSandbox/setupCompleted","params":{"mode":"elevated","success":true,"error":null}}),
                ),
            ),
            ("windowsSandbox/readiness", json!({"status":"ready"}), None),
        ]);
        assert_eq!(setup(&server).await.unwrap().state, "ready");
        assert!(server.sandbox.lock().unwrap().is_none());
        server.stop();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn failed_setup_does_not_fall_back_or_enable_commands() {
        let server = fake_server(vec![
            (
                "windowsSandbox/setupStart",
                json!({"started":true}),
                Some(
                    json!({"method":"windowsSandbox/setupCompleted","params":{"mode":"elevated","success":false,"error":"UAC declined"}}),
                ),
            ),
            (
                "windowsSandbox/readiness",
                json!({"status":"notConfigured"}),
                None,
            ),
        ]);
        assert!(setup(&server)
            .await
            .unwrap_err()
            .to_string()
            .contains("UAC declined"));
        assert!(ensure_ready(&server).await.is_err());
        server.stop();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn setup_never_reconfigures_an_active_account() {
        let server = fake_server(vec![]);
        server.active.lock().unwrap().insert("worker".into());
        assert!(matches!(setup(&server).await, Err(CoreError::Busy)));
        server.active.lock().unwrap().clear();
        let _operation = server.operation.lock().await;
        assert!(matches!(setup(&server).await, Err(CoreError::Busy)));
        server.stop();
    }

    #[tokio::test]
    async fn unavailable_or_wrong_agent_approval_keeps_the_original_request_pending() {
        let server = fake_server(vec![]);
        let (account_events, _) = broadcast::channel(16);
        let provider = CodexProvider::new(account_events);
        let (events, _rx) = mpsc::channel(16);
        provider.approvals.lock().unwrap().insert(
            "request".into(),
            PendingApproval {
                session_id: "worker".into(),
                server: server.clone(),
                rpc_id: json!(99),
                events,
                available_decisions: Some(vec![
                    ApprovalDecision::AllowOnce,
                    ApprovalDecision::Deny,
                ]),
            },
        );
        assert!(matches!(
            provider
                .resolve_approval("worker", "request", ApprovalDecision::AllowSession)
                .await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            provider
                .resolve_approval("another", "request", ApprovalDecision::AllowOnce)
                .await,
            Err(CoreError::NotFound)
        ));
        assert!(provider.approvals.lock().unwrap().contains_key("request"));
        server.stop();
    }

    #[tokio::test]
    async fn read_only_review_cannot_escalate_through_an_approval_card() {
        let server = fake_server(vec![]);
        let (account_events, _) = broadcast::channel(16);
        let provider = CodexProvider::new(account_events);
        let (events, mut rx) = mpsc::channel(16);
        provider
            .handle_request(
                &server,
                &TurnState::new("thread"),
                "reviewer",
                true,
                &events,
                ServerRequest {
                    id: json!(10),
                    method: "item/fileChange/requestApproval".into(),
                    params: json!({"grantRoot":"C:/outside"}),
                },
            )
            .await;
        assert!(provider.approvals.lock().unwrap().is_empty());
        assert!(
            matches!(rx.recv().await, Some(EventPayload::ToolActivity { label, .. }) if label == "Дополнительный доступ отклонён")
        );
        server.stop();
    }
    #[test]
    fn readiness_requires_explicit_confirmation() {
        assert_eq!(parse_readiness(&json!({"status":"ready"})).state, "ready");
        assert_eq!(
            parse_readiness(&json!({"status":"notConfigured"})).state,
            "not_configured"
        );
        assert_eq!(
            parse_readiness(&json!({"status":"updateRequired"})).state,
            "update_required"
        );
        assert_eq!(
            parse_readiness(&json!({"configured":true})).state,
            "unavailable"
        );
    }
    #[test]
    fn approval_choices_never_invent_a_session_grant() {
        assert_eq!(
            approval_decisions(&json!({"availableDecisions":["accept","decline"]})),
            Some(vec![ApprovalDecision::AllowOnce, ApprovalDecision::Deny])
        );
        assert_eq!(
            approval_decisions(&json!({"availableDecisions":[]})),
            Some(vec![])
        );
        assert_eq!(
            approval_decisions(
                &json!({"availableDecisions":[{"acceptWithExecpolicyAmendment":{}}]})
            ),
            Some(vec![])
        );
        assert!(approval_decisions(&json!({})).is_none());
    }
}
