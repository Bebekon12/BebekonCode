use std::{env, path::PathBuf, process::Command};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    for file in [
        "scripts/claude-usage.mjs",
        "scripts/build-claude-usage.mjs",
        "package-lock.json",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(file).display());
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("claude-usage.mjs");
    let status = Command::new("node")
        .arg(root.join("scripts/build-claude-usage.mjs"))
        .arg(output)
        .status()
        .expect("Node.js and npm ci are required to build the official Claude usage adapter");
    assert!(status.success(), "Run npm ci before building agent-core");
}
