use crate::{CoreError, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Deserialize)]
struct Product {
    repository: String,
}
#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
    prerelease: bool,
    draft: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseCheck {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub available: bool,
    pub notes: String,
    pub published_at: Option<String>,
    pub release_url: String,
    pub checked_at: i64,
}

pub fn repository() -> Result<String> {
    let product: Product = serde_json::from_str(include_str!("../../../product.json"))?;
    Ok(product.repository)
}

fn interpret(release: Option<GithubRelease>, current: &str, repo: &str) -> Result<ReleaseCheck> {
    let current_version = Version::parse(current).map_err(|_| CoreError::UpdateMetadata)?;
    let mut check = ReleaseCheck {
        current_version: current.into(),
        latest_version: None,
        available: false,
        notes: "No published stable release yet.".into(),
        published_at: None,
        release_url: format!("https://github.com/{repo}/releases"),
        checked_at: crate::model::now(),
    };
    if let Some(release) = release {
        let version = Version::parse(
            release
                .tag_name
                .strip_prefix('v')
                .unwrap_or(&release.tag_name),
        )
        .map_err(|_| CoreError::UpdateMetadata)?;
        if release.draft || release.prerelease || !version.pre.is_empty() {
            return Err(CoreError::UpdateMetadata);
        }
        check.available = version > current_version;
        check.latest_version = Some(version.to_string());
        check.notes = release
            .body
            .unwrap_or_default()
            .chars()
            .take(32_000)
            .collect();
        check.published_at = release.published_at;
        // Construct the URL from a validated SemVer; never open a server-supplied URL.
        check.release_url = format!(
            "https://github.com/{repo}/releases/tag/{}",
            release.tag_name
        );
    }
    Ok(check)
}

pub async fn check(current: &str) -> Result<ReleaseCheck> {
    let repo = repository()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("agent-workspace/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| CoreError::UpdateNetwork)?;
    let mut response = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|_| CoreError::UpdateNetwork)?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return interpret(None, current, &repo);
    }
    if [
        reqwest::StatusCode::FORBIDDEN,
        reqwest::StatusCode::TOO_MANY_REQUESTS,
    ]
    .contains(&response.status())
    {
        return Err(CoreError::UpdateRateLimit);
    }
    if !response.status().is_success() {
        return Err(CoreError::UpdateNetwork);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| CoreError::UpdateNetwork)?
    {
        if bytes.len() + chunk.len() > 256 * 1024 {
            return Err(CoreError::UpdateMetadata);
        }
        bytes.extend_from_slice(&chunk);
    }
    let release = serde_json::from_slice(&bytes).map_err(|_| CoreError::UpdateMetadata)?;
    interpret(Some(release), current, &repo)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str) -> GithubRelease {
        GithubRelease {
            tag_name: tag.into(),
            body: Some("Changes".into()),
            published_at: None,
            prerelease: false,
            draft: false,
        }
    }
    #[test]
    fn semantic_versions_and_missing_release() {
        assert!(
            interpret(Some(release("v0.10.0")), "0.9.0", "owner/repo")
                .expect("check")
                .available
        );
        assert!(
            !interpret(Some(release("v0.1.0")), "0.2.0", "owner/repo")
                .expect("check")
                .available
        );
        assert!(
            !interpret(None, "0.1.0", "owner/repo")
                .expect("check")
                .available
        );
    }
    #[test]
    fn invalid_or_preview_tags_are_rejected() {
        assert!(interpret(Some(release("../../bad")), "0.1.0", "owner/repo").is_err());
        assert!(interpret(Some(release("v0.2.0-beta.1")), "0.1.0", "owner/repo").is_err());
    }
}
