#[tokio::main]
async fn main() {
    match agent_core::releases::check(env!("CARGO_PKG_VERSION")).await {
        Ok(release) => println!(
            "Current: {}\nLatest: {}\nUpdate available: {}\nRelease: {}",
            release.current_version,
            release.latest_version.as_deref().unwrap_or("none"),
            release.available,
            release.release_url
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
