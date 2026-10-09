//! Explicit official Codex login; the CLI owns credentials in its per-home OS keyring.
use super::*;

const MODE_FILE: &str = "bebekon-auth-mode.json";

pub(super) fn selected(account: &AccountProfile) -> Result<bool> {
    let path = CodexProvider::profile(account)?.join(MODE_FILE);
    match std::fs::read_to_string(path) {
        Ok(value) if value.trim() == "\"codex\"" => Ok(true),
        Ok(_) => Err(CoreError::Invalid("Неизвестный способ входа Codex".into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn select(account: &AccountProfile) -> Result<()> {
    let profile = CodexProvider::profile(account)?;
    std::fs::create_dir_all(&profile)?;
    std::fs::write(profile.join(MODE_FILE), "\"codex\"\n")?;
    Ok(())
}

pub(super) async fn read(peer: &RpcPeer, status: &mut AccountStatus) -> Result<()> {
    let value = peer
        .request(
            "account/read",
            json!({"refreshToken":false}),
            REQUEST_TIMEOUT,
        )
        .await
        .map_err(|e| provider_error(e, None))?;
    let account = &value["account"];
    if account.is_null() {
        status.state = "signed_out".into();
        return Ok(());
    }
    if account["type"].as_str() != Some("chatgpt") {
        return Err(CoreError::Invalid(
            "Для лимитов подписки нужен вход ChatGPT через Codex".into(),
        ));
    }
    status.state = "signed_in".into();
    status.email = str_at(account, "email").map(str::to_string);
    status.plan = str_at(account, "planType").map(str::to_string);
    status.plan_usage_enabled = Some(true);
    status.message = Some("Вход через официальный Codex. Учётные данные хранит CLI в защищённом хранилище ОС отдельного профиля.".into());
    Ok(())
}

pub(super) async fn login(peer: &RpcPeer) -> Result<LoginStart> {
    let value = peer
        .request(
            "account/login/start",
            json!({"type":"chatgpt"}),
            REQUEST_TIMEOUT,
        )
        .await
        .map_err(|e| provider_error(e, None))?;
    let url = str_at(&value, "authUrl")
        .ok_or_else(|| CoreError::Invalid("Codex не передал страницу входа".into()))?;
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| CoreError::Invalid("Некорректная страница входа Codex".into()))?;
    if parsed.scheme() != "https"
        || !matches!(parsed.host_str(), Some("auth.openai.com" | "chatgpt.com"))
    {
        return Err(CoreError::Invalid(
            "Codex передал неподдерживаемую страницу входа".into(),
        ));
    }
    Ok(LoginStart { url: url.into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selecting_a_profile_never_selects_another_account_or_falls_back_on_corruption() {
        let temp = crate::test_support::TestDirectory::new().unwrap();
        let account = |name: &str| AccountProfile {
            id: name.into(),
            provider: "openai".into(),
            label: name.into(),
            credential_ref: None,
            config_dir: Some(temp.path().join(name).to_string_lossy().into_owned()),
            auth_status: "signed_out".into(),
            created_at: now(),
        };
        let one = account("one");
        let two = account("two");
        assert!(!selected(&one).unwrap());
        select(&one).unwrap();
        assert!(selected(&one).unwrap());
        assert!(!selected(&two).unwrap());
        std::fs::write(
            CodexProvider::profile(&one).unwrap().join(MODE_FILE),
            "invalid",
        )
        .unwrap();
        assert!(selected(&one).is_err());
    }
}
