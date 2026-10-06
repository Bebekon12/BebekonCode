#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use agent_core::{model::*, releases::ReleaseCheck, Core};
use std::{path::Path, sync::Arc};
use tauri::{Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

struct AppState {
    core: Arc<Core>,
    release: tokio::sync::Mutex<Option<ReleaseCheck>>,
    files: tokio::sync::Mutex<()>,
}
type IpcResult<T> = Result<T, String>;

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum FileOperation {
    List {
        path: String,
    },
    Read {
        path: String,
    },
    Save {
        path: String,
        expected: String,
        content: String,
    },
    Create {
        path: String,
        directory: bool,
    },
    Rename {
        path: String,
        destination: String,
    },
    Delete {
        path: String,
    },
}

#[tauri::command]
async fn file_operation(
    workspace_id: String,
    operation: FileOperation,
    state: State<'_, AppState>,
) -> IpcResult<serde_json::Value> {
    let workspace = state
        .core
        .storage
        .workspace(&workspace_id)
        .await
        .map_err(|error| error.to_string())?;
    let _guard = state.files.lock().await;
    tauri::async_runtime::spawn_blocking(move || -> agent_core::Result<serde_json::Value> {
        use agent_core::files;
        let root = Path::new(&workspace.root);
        match operation {
            FileOperation::List { path } => Ok(serde_json::to_value(files::list(root, &path)?)?),
            FileOperation::Read { path } => Ok(serde_json::to_value(files::read(root, &path)?)?),
            FileOperation::Save {
                path,
                expected,
                content,
            } => {
                files::save(root, &path, &expected, &content)?;
                Ok(serde_json::Value::Null)
            }
            FileOperation::Create { path, directory } => {
                files::create(root, &path, directory)?;
                Ok(serde_json::Value::Null)
            }
            FileOperation::Rename { path, destination } => {
                files::rename(root, &path, &destination)?;
                Ok(serde_json::Value::Null)
            }
            FileOperation::Delete { path } => {
                files::remove(root, &path)?;
                Ok(serde_json::Value::Null)
            }
        }
    })
    .await
    .map_err(|_| "File operation could not finish".to_string())?
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn open_project(
    workspace_id: String,
    terminal: bool,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> IpcResult<()> {
    let workspace = state
        .core
        .storage
        .workspace(&workspace_id)
        .await
        .map_err(|error| error.to_string())?;
    let root = agent_core::files::resolve(Path::new(&workspace.root), "", false)
        .map_err(|error| error.to_string())?;
    if terminal {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Explicit user click opens a visible console. No shell interpolation or agent execution.
            std::process::Command::new("powershell.exe")
                .arg("-NoExit")
                .current_dir(root)
                .creation_flags(0x00000010)
                .spawn()
                .map_err(|_| "Could not open PowerShell".to_string())?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err("Terminal launcher currently supports Windows".into())
        }
    } else {
        app.opener()
            .open_path(root.to_string_lossy(), None::<&str>)
            .map_err(|_| "Could not open Explorer".to_string())
    }
}

#[tauri::command]
async fn snapshot(state: State<'_, AppState>) -> IpcResult<Snapshot> {
    state
        .core
        .snapshot()
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn add_workspace(root: String, state: State<'_, AppState>) -> IpcResult<Workspace> {
    state
        .core
        .add_workspace(&root)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn create_session(input: CreateSession, state: State<'_, AppState>) -> IpcResult<Session> {
    state
        .core
        .create_session(input)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn send_message(
    session_id: String,
    prompt: String,
    state: State<'_, AppState>,
) -> IpcResult<String> {
    state
        .core
        .send_message(&session_id, prompt)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
fn cancel_session(session_id: String, state: State<'_, AppState>) -> IpcResult<()> {
    state
        .core
        .cancel(&session_id)
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn session_events(
    session_id: String,
    before: Option<i64>,
    state: State<'_, AppState>,
) -> IpcResult<Vec<Event>> {
    state
        .core
        .storage
        .events(&session_id, before)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
fn refresh_providers(state: State<'_, AppState>) -> IpcResult<Vec<ProviderInfo>> {
    state
        .core
        .refresh_providers()
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn save_settings(settings: Settings, state: State<'_, AppState>) -> IpcResult<()> {
    state
        .core
        .storage
        .save_settings(settings)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn git_status(
    workspace_id: String,
    state: State<'_, AppState>,
) -> IpcResult<agent_core::git::GitStatus> {
    let workspace = state
        .core
        .storage
        .workspace(&workspace_id)
        .await
        .map_err(|error| error.to_string())?;
    agent_core::git::status(Path::new(&workspace.root))
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn git_diff(workspace_id: String, state: State<'_, AppState>) -> IpcResult<String> {
    let workspace = state
        .core
        .storage
        .workspace(&workspace_id)
        .await
        .map_err(|error| error.to_string())?;
    agent_core::git::diff(Path::new(&workspace.root))
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn check_releases(
    force: bool,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> IpcResult<ReleaseCheck> {
    // Single-flight and six-hour cache. No recurring background polling.
    let mut cached = state.release.lock().await;
    if !force {
        if let Some(check) = cached.as_ref() {
            if agent_core::model::now() - check.checked_at < 6 * 60 * 60 {
                return Ok(check.clone());
            }
        }
    }
    let release = agent_core::releases::check(&app.package_info().version.to_string())
        .await
        .map_err(|error| error.to_string())?;
    *cached = Some(release.clone());
    Ok(release)
}
#[tauri::command]
async fn open_releases(state: State<'_, AppState>, app: tauri::AppHandle) -> IpcResult<()> {
    let repo = agent_core::releases::repository().map_err(|error| error.to_string())?;
    let url = state
        .release
        .lock()
        .await
        .as_ref()
        .map(|release| release.release_url.clone())
        .unwrap_or_else(|| format!("https://github.com/{repo}/releases"));
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "Could not open the release page".to_string())
}

fn main() {
    // Only allowlisted static codes are logged. No provider payloads, prompt text or errors.
    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .with_target(false)
        .with_ansi(false)
        .with_writer(agent_core::redaction::LogWriter::default)
        .init();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let mut directory = app.path().app_local_data_dir()?;
            let mut arguments = std::env::args_os().skip(1);
            while let Some(argument) = arguments.next() {
                if argument == "--data-dir" {
                    let path = arguments
                        .next()
                        .map(std::path::PathBuf::from)
                        .ok_or("--data-dir requires an absolute folder")?;
                    if !path.is_absolute() {
                        return Err("--data-dir requires an absolute folder".into());
                    }
                    directory = path;
                }
            }
            std::fs::create_dir_all(&directory)?;
            let core = tauri::async_runtime::block_on(Core::open(&directory.join("workspace.db")))?;
            let mut events = core.subscribe();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match events.recv().await {
                        Ok(event) => {
                            let _ = handle.emit("agent-event", event);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let _ = handle.emit("agent-resync", ());
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
            app.manage(AppState {
                core,
                release: tokio::sync::Mutex::new(None),
                files: tokio::sync::Mutex::new(()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            add_workspace,
            create_session,
            send_message,
            cancel_session,
            session_events,
            refresh_providers,
            save_settings,
            git_status,
            git_diff,
            check_releases,
            open_releases,
            file_operation,
            open_project
        ])
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<AppState>().core.cancel_all();
            }
        }),
        Err(_) => {
            tracing::error!(code = "desktop_startup_failed");
            std::process::exit(1);
        }
    }
}
