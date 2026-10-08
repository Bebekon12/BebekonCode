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

pub fn evaluate(profile: &str, action: Action) -> Decision {
    use Action::*;
    use Decision::*;
    match (profile, action) {
        (_, Credentials | SystemSettings) => Deny,
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
