use crate::{CoreError, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Ask,
    Deny,
}
#[derive(Debug, Clone, Copy)]
pub enum Action {
    ReadWorkspace,
    WriteWorkspace,
    WriteExternal,
    Delete,
    Execute,
    GitRead,
    GitCommit,
    GitPush,
    Network,
    Credentials,
    SystemSettings,
}

/// Access profiles a chat may use. Full access is opt-in per chat and only where the provider
/// documents it (Codex `dangerFullAccess`, Claude `bypassPermissions`).
pub fn validate_profile(provider: &str, profile: &str) -> Result<()> {
    match profile {
        "standard" | "read_only" | "workspace_auto" => Ok(()),
        "full_access" if matches!(provider, "openai" | "anthropic" | "mock") => Ok(()),
        "full_access" => Err(CoreError::Invalid(
            "Полный доступ недоступен для этого провайдера".into(),
        )),
        _ => Err(CoreError::Invalid("Неизвестный профиль доступа".into())),
    }
}

pub fn evaluate(profile: &str, action: Action) -> Decision {
    use Action::*;
    use Decision::*;
    match (profile, action) {
        (_, Credentials | SystemSettings) => Deny,
        ("full_access", _) => Allow,
        ("read_only", ReadWorkspace | GitRead) => Allow,
        ("read_only", _) => Deny,
        ("standard" | "workspace_auto", ReadWorkspace | WriteWorkspace | GitRead) => Allow,
        ("standard" | "workspace_auto", _) => Ask,
        _ => Deny,
    }
}

/// Both paths must exist; junctions and symlinks are resolved before containment is tested.
pub fn resolve_inside(root: &Path, candidate: &Path) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let candidate = candidate.canonicalize()?;
    if !candidate.starts_with(&root) {
        return Err(CoreError::Invalid(
            "Путь находится за пределами рабочей области".into(),
        ));
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restrictive_defaults() {
        assert_eq!(evaluate("unknown", Action::ReadWorkspace), Decision::Deny);
        assert_eq!(evaluate("standard", Action::Credentials), Decision::Deny);
        assert_eq!(evaluate("standard", Action::GitPush), Decision::Ask);
        assert_eq!(
            evaluate("read_only", Action::WriteWorkspace),
            Decision::Deny
        );
        assert_eq!(evaluate("full_access", Action::GitPush), Decision::Allow);
        assert_eq!(evaluate("full_access", Action::Credentials), Decision::Deny);
    }
    #[test]
    fn full_access_needs_a_documented_provider_mode() {
        assert!(validate_profile("openai", "full_access").is_ok());
        assert!(validate_profile("anthropic", "full_access").is_ok());
        assert!(validate_profile("other", "full_access").is_err());
        assert!(validate_profile("anthropic", "workspace_auto").is_ok());
        assert!(validate_profile("openai", "everything").is_err());
    }
    #[test]
    fn sibling_prefix_does_not_escape() {
        let temp = crate::test_support::TestDirectory::new().expect("temp dir");
        let root = temp.path().join("repo");
        let other = temp.path().join("repo-secret");
        std::fs::create_dir(&root).expect("root");
        std::fs::create_dir(&other).expect("other");
        assert!(resolve_inside(&root, &other).is_err());
    }
}
