#[path = "../src/test_support.rs"]
mod test_support;

fn git(root: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git installed");
    assert!(status.success());
}

#[tokio::test]
async fn real_git_status_and_diff_handle_staged_and_untracked_paths() {
    let temp = test_support::TestDirectory::new().expect("temp directory");
    git(temp.path(), &["init", "--initial-branch=main"]);
    std::fs::write(temp.path().join("file with spaces.txt"), "first\n").expect("file");
    git(temp.path(), &["add", "--", "file with spaces.txt"]);
    std::fs::write(temp.path().join("file with spaces.txt"), "first\nsecond\n").expect("edit");
    std::fs::write(temp.path().join("untracked.txt"), "untracked").expect("untracked");
    let status = agent_core::git::status(temp.path()).await.expect("status");
    assert_eq!(status.branch, "main");
    assert!(status
        .files
        .iter()
        .any(|file| file.path == "file with spaces.txt" && file.status == "AM"));
    assert!(status
        .files
        .iter()
        .any(|file| file.path == "untracked.txt" && file.status == "??"));
    let diff = agent_core::git::diff(temp.path()).await.expect("diff");
    assert!(diff.contains("+first"));
    assert!(diff.contains("+second"));
    assert!(!diff.contains("+untracked"));
}
