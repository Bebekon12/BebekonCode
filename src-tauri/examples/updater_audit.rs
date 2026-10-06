//! Download/signature audit only. This example never calls install or opens user data.
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;

fn main() {
    let endpoint = std::env::args().nth(1).expect("manifest URL required");
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let loopback = endpoint.starts_with("http://127.0.0.1:");
    if loopback {
        context.config_mut().plugins.0.get_mut("updater").unwrap()
            ["dangerousInsecureTransportProtocol"] = serde_json::json!(true);
    } else {
        assert!(endpoint.starts_with("https://github.com/Bebekon12/BebekonCode/releases/"));
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            let updater = app
                .updater_builder()
                .endpoints(vec![endpoint.parse()?])?
                .target("windows-x86_64")
                .timeout(Duration::from_secs(60))
                .version_comparator(|_, _| true)
                .build()?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let result = async {
                    let update = updater
                        .check()
                        .await?
                        .expect("manifest returned no package");
                    let bytes = update.download(|_, _| {}, || {}).await?;
                    assert!(bytes.starts_with(b"MZ"), "Windows executable required");
                    println!(
                        "PASS: official updater verified {} bytes for {}",
                        bytes.len(),
                        update.version
                    );
                    let mut wrong_version = update.clone();
                    wrong_version.version = "9.0.0".into();
                    assert!(
                        wrong_version.download(|_, _| {}, || {}).await.is_err(),
                        "signature accepted for another version"
                    );
                    println!("PASS: official updater rejected a mismatched signed version");
                    if loopback {
                        let mut altered = update.clone();
                        altered.download_url.set_path("/tampered.exe");
                        assert!(
                            altered.download(|_, _| {}, || {}).await.is_err(),
                            "tampered package accepted"
                        );
                        println!("PASS: official updater rejected a modified installer");
                    }
                    Ok::<_, tauri_plugin_updater::Error>(())
                }
                .await;
                if let Err(error) = result {
                    eprintln!("Updater audit failed: {error}");
                    handle.exit(1);
                } else {
                    handle.exit(0);
                }
            });
            Ok(())
        })
        .build(context)
        .expect("audit app startup failed")
        .run(|_, _| {});
}
