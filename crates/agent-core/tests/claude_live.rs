//! Real official CLI preflight. A fresh profile, no sign-in, credentials or inference.
//! cargo test -p agent-core --test claude_live -- --ignored
use agent_core::Core;
#[path = "../src/test_support.rs"]
mod test_support;

#[tokio::test]
#[ignore = "requires native official Claude Code 2.1.293+ on Windows PATH"]
async fn isolated_signed_out_profile_and_documented_catalog() {
    let temp = test_support::TestDirectory::new().expect("temp");
    let core = Core::open(&temp.path().join("claude.db"))
        .await
        .expect("core");
    let snapshot = core.snapshot().await.expect("snapshot");
    let claude = snapshot
        .providers
        .iter()
        .find(|p| p.id == "anthropic")
        .expect("Claude");
    assert!(claude.available, "{}", claude.detail);
    let account = core
        .add_account("anthropic", "Изолированная проверка")
        .await
        .expect("account");
    assert!(std::path::Path::new(account.config_dir.as_deref().unwrap()).starts_with(temp.path()));
    let status = core
        .account_status(&account.id)
        .await
        .expect("official status");
    assert_eq!(
        status.state, "signed_out",
        "must not import the default CLI login"
    );
    assert!(status.email.is_none());
    assert!(status.usage.is_empty());
    assert_eq!(
        status.manage_usage_url.as_deref(),
        Some("https://claude.ai/settings/usage")
    );
    let models = core.account_models(&account.id).await.expect("aliases");
    assert!([
        "sonnet",
        "opus",
        "haiku",
        "claude-opus-5-5",
        "claude-opus-4-8"
    ]
    .iter()
    .all(|id| models.iter().any(|model| model.id == *id)));
    assert!(!models
        .iter()
        .find(|model| model.id == "claude-opus-4-6")
        .unwrap()
        .reasoning_efforts
        .iter()
        .any(|effort| effort == "xhigh"));
    let extensions = core
        .account_extensions(&account.id)
        .await
        .expect("extensions");
    assert!(
        extensions.plugins.is_empty()
            && extensions.mcp_servers.is_empty()
            && extensions.skills.is_empty()
    );
    assert!(extensions.errors.is_empty());
    core.shutdown().await;
    core.remove_account(&account.id)
        .await
        .expect("remove own empty profile");
}
