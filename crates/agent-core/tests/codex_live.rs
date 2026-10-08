//! Opt-in checks against the locally installed official Codex CLI. They never complete a real
//! sign-in and never send a prompt, so they spend no plan usage. Run with:
//! `cargo test -p agent-core --test codex_live -- --ignored`
use agent_core::{codex::siwc, model::now, Core, CoreError};
#[path = "../src/test_support.rs"]
mod test_support;

#[tokio::test]
#[ignore = "requires the official Codex CLI on PATH"]
async fn sign_in_with_chatgpt_profile_and_documented_app_server_configuration() {
    let temp = test_support::TestDirectory::new().expect("temp");
    let core = Core::open(&temp.path().join("live.db"))
        .await
        .expect("core");
    let codex = core
        .snapshot()
        .await
        .expect("snapshot")
        .providers
        .into_iter()
        .find(|provider| provider.id == "openai")
        .expect("openai provider");
    assert!(codex.available, "{}", codex.detail);

    let account = core
        .add_account("openai", "Проверка")
        .await
        .expect("account");
    let status = core.account_status(&account.id).await.expect("status");
    assert_eq!(status.state, "signed_out");
    assert!(status.usage.is_empty(), "limits must not be invented");
    assert_eq!(
        status.manage_usage_url.as_deref(),
        Some("https://chatgpt.com/settings/usage")
    );

    // Signed-out accounts never start Codex.
    assert!(matches!(
        core.account_models(&account.id).await,
        Err(CoreError::Provider { kind: Some(kind), .. }) if kind == "auth"
    ));

    let login = core.account_login(&account.id).await.expect("login url");
    assert!(login
        .url
        .starts_with("https://auth.openai.com/api/accounts/authorize?"));
    assert!(login.url.contains("client_id=dynamic_agent_client"));
    assert!(login.url.contains("agent_name_hint=BebekonCode"));

    // A placeholder token (never valid) proves the documented provider configuration starts
    // and serves the bundled model catalog; no inference request is made.
    let profile = std::path::PathBuf::from(account.config_dir.clone().expect("profile"));
    siwc::save(
        &profile,
        &siwc::Credentials {
            client_id: Some("oaiapp_placeholder".into()),
            access_token: Some("placeholder-not-a-token".into()),
            expires_at: Some(now() + 3600),
            scopes: vec![siwc::PLAN_SCOPE.into()],
            ..siwc::Credentials::default()
        },
    )
    .expect("save placeholder");
    let models = core.account_models(&account.id).await.expect("models");
    assert!(!models.is_empty(), "bundled model catalog expected");
    assert!(
        models
            .iter()
            .any(|model| !model.reasoning_efforts.is_empty()),
        "reasoning levels must come from the official catalog"
    );
    let extensions = core
        .account_extensions(&account.id)
        .await
        .expect("official extension inventory");
    assert!(
        extensions.errors.is_empty(),
        "extension inventory failed: {:?}",
        extensions.errors
    );

    let status = core
        .account_status(&account.id)
        .await
        .expect("sandbox status");
    let sandbox = status.sandbox.expect("official readiness response");
    assert!(
        matches!(
            sandbox.state.as_str(),
            "ready" | "not_configured" | "update_required"
        ),
        "{}",
        sandbox.detail
    );
    // Never invoke setup here: it can require UAC and change Windows accounts/firewall rules.
    core.shutdown().await;
    siwc::save(&profile, &siwc::Credentials::default()).expect("clear");
    core.remove_account(&account.id).await.expect("remove");
}
