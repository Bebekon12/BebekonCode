use crate::{
    model::{EventPayload, ProviderRunDiagnostic},
    provider::TurnRequest,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

async fn send(
    output: &mpsc::Sender<EventPayload>,
    event: EventPayload,
    cancel: &CancellationToken,
) -> bool {
    // Backpressure must not keep a cancelled CLI alive. Terminal reports are best-effort
    // if the consumer stopped draining; ordinary core consumers keep draining on cancel.
    let permit = tokio::select! {
        biased;
        _ = cancel.cancelled() => None,
        permit = output.reserve() => permit.ok(),
    };
    if let Some(permit) = permit {
        permit.send(event);
        true
    } else {
        output.try_send(event).is_ok()
    }
}

pub(crate) async fn finish(
    output: &mpsc::Sender<EventPayload>,
    report: ProviderRunDiagnostic,
    cancel: &CancellationToken,
) {
    let _ = send(
        output,
        EventPayload::ProviderDiagnostic {
            report: Box::new(report),
        },
        cancel,
    )
    .await;
}

/// Observe the existing stream. No model calls, payload copies, or content-derived tokens.
pub(crate) async fn forward(
    request: &TurnRequest,
    request_id: &str,
    mut input: mpsc::Receiver<EventPayload>,
    output: &mpsc::Sender<EventPayload>,
    cancel: &CancellationToken,
) -> ProviderRunDiagnostic {
    let mut report = ProviderRunDiagnostic {
        request_id: request_id.into(),
        provider: request.session.provider.clone(),
        account_profile_id: request.account.id.clone(),
        session_id: request.session.id.clone(),
        provider_session_id: request.session.provider_session_id.clone(),
        requested_model: request.session.model.clone(),
        resolved_model: None,
        effort: request.session.reasoning_effort.clone(),
        permission_profile: request.session.permission_profile.clone(),
        fast_mode: request.session.fast_mode,
        resumed: request.session.provider_session_id.is_some(),
        prompt_bytes: request.prompt.len() as u64,
        elapsed_ms: 0,
        tool_requests: 0,
        approval_requests: 0,
        reported_retries: 0,
        status: String::new(),
        error_kind: None,
        usage_reported: false,
        usage_incomplete: false,
        subagent_events: 0,
        model_requests: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_creation_tokens: None,
    };
    while let Some(event) = input.recv().await {
        match &event {
            EventPayload::ProviderSession { id } => report.provider_session_id = Some(id.clone()),
            EventPayload::ModelResolved { model } => report.resolved_model = Some(model.clone()),
            EventPayload::ProviderUsage {
                model_requests,
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
                incomplete,
                ..
            } => {
                let add = |old: Option<u64>, new: Option<u64>| {
                    if report.usage_reported {
                        old.zip(new).map(|(a, b)| a.saturating_add(b))
                    } else {
                        new
                    }
                };
                report.usage_incomplete |= *incomplete;
                report.model_requests = add(report.model_requests, *model_requests);
                report.input_tokens = add(report.input_tokens, *input_tokens);
                report.output_tokens = add(report.output_tokens, *output_tokens);
                report.cache_read_tokens = add(report.cache_read_tokens, *cache_read_tokens);
                report.cache_creation_tokens =
                    add(report.cache_creation_tokens, *cache_creation_tokens);
                report.usage_reported = true;
            }
            EventPayload::ApprovalRequested { .. } => report.approval_requests += 1,
            EventPayload::ToolStarted { kind, .. } => {
                report.tool_requests += 1;
                if kind == "agent" {
                    report.subagent_events += 1;
                }
            }
            EventPayload::ToolActivity { label, .. } if label == "Субагент" => {
                report.subagent_events += 1
            }
            EventPayload::ToolActivity { label, .. }
                if label == "Повтор запроса Claude" || label == "Повтор запроса" =>
            {
                report.reported_retries += 1;
            }
            EventPayload::ToolActivity { label, .. }
                if label.starts_with("Claude · ")
                    || label.starts_with("MCP · ")
                    || [
                        "Команда",
                        "Изменение файлов",
                        "Инструмент",
                        "Поиск в интернете",
                        "Просмотр изображения",
                    ]
                    .contains(&label.as_str()) =>
            {
                report.tool_requests += 1;
            }
            _ => {}
        }
        if !send(output, event, cancel).await {
            cancel.cancel();
        }
    }
    report
}
