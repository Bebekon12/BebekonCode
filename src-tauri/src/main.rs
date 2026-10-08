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

#[tauri::command]
async fn plugin_catalog() -> IpcResult<agent_core::catalog::Catalog> {
    agent_core::catalog::fetch()
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn open_catalog_source(provider: String, app: tauri::AppHandle) -> IpcResult<()> {
    let url = agent_core::catalog::source_url(&provider).map_err(|e| e.to_string())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn review_document(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> IpcResult<agent_core::review::ReviewDocument> {
    state
        .core
        .review_document(&workspace_id, &path)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn review_comments(
    workspace_id: String,
    path: String,
    state: State<'_, AppState>,
) -> IpcResult<Vec<agent_core::review::ReviewComment>> {
    state
        .core
        .review_comments(&workspace_id, &path)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn add_review_comment(
    workspace_id: String,
    path: String,
    input: agent_core::review::NewComment,
    state: State<'_, AppState>,
) -> IpcResult<agent_core::review::ReviewComment> {
    state
        .core
        .add_review_comment(&workspace_id, &path, input)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn resolve_review_comment(
    workspace_id: String,
    id: String,
    resolved: bool,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    state
        .core
        .resolve_review_comment(&workspace_id, &id, resolved)
        .await
        .map_err(|e| e.to_string())
}

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
    .map_err(|_| "Не удалось завершить операцию с файлом".to_string())?
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn prepare_update(state: State<'_, AppState>) -> IpcResult<()> {
    let snapshot = state
        .core
        .snapshot()
        .await
        .map_err(|error| error.to_string())?;
    if snapshot
        .sessions
        .iter()
        .any(|session| session.status == "running")
    {
        return Err("Остановите активные сессии перед установкой обновления".into());
    }
    state.core.shutdown().await;
    Ok(())
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
                .map_err(|_| "Не удалось открыть PowerShell".to_string())?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err("Запуск терминала пока поддерживается только в Windows".into())
        }
    } else {
        app.opener()
            .open_path(root.to_string_lossy(), None::<&str>)
            .map_err(|_| "Не удалось открыть Проводник".to_string())
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
async fn create_chat(input: CreateChat, state: State<'_, AppState>) -> IpcResult<Session> {
    state
        .core
        .create_chat(input)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn configure_session(
    session_id: String,
    config: AgentConfig,
    state: State<'_, AppState>,
) -> IpcResult<Session> {
    state
        .core
        .configure_session(&session_id, config)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn handoff_session(
    session_id: String,
    config: AgentConfig,
    state: State<'_, AppState>,
) -> IpcResult<String> {
    state
        .core
        .handoff(&session_id, config)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn send_message(
    session_id: String,
    prompt: String,
    attachments: Option<Vec<agent_core::attachments::Attachment>>,
    state: State<'_, AppState>,
) -> IpcResult<String> {
    state
        .core
        .send_message_with_attachments(&session_id, prompt, attachments.unwrap_or_default())
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
async fn refresh_providers(state: State<'_, AppState>) -> IpcResult<Vec<ProviderInfo>> {
    state
        .core
        .refresh_providers()
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn add_account(
    provider: String,
    label: String,
    state: State<'_, AppState>,
) -> IpcResult<AccountProfile> {
    state
        .core
        .add_account(&provider, &label)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn rename_account(
    account_id: String,
    label: String,
    state: State<'_, AppState>,
) -> IpcResult<AccountProfile> {
    state
        .core
        .rename_account(&account_id, &label)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn remove_account(account_id: String, state: State<'_, AppState>) -> IpcResult<()> {
    state
        .core
        .remove_account(&account_id)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn account_status(
    account_id: String,
    state: State<'_, AppState>,
) -> IpcResult<AccountStatus> {
    state
        .core
        .account_status(&account_id)
        .await
        .map_err(|error| error.to_string())
}
/// Runs the official sandbox setup only after an explicit user action.
#[tauri::command]
async fn setup_sandbox(account_id: String, state: State<'_, AppState>) -> IpcResult<SandboxStatus> {
    state
        .core
        .setup_sandbox(&account_id)
        .await
        .map_err(|error| error.to_string())
}

/// Starts sign-in and opens the provider's official page in the default browser.
#[tauri::command]
async fn account_login(
    account_id: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> IpcResult<()> {
    let login = state
        .core
        .account_login(&account_id)
        .await
        .map_err(|error| error.to_string())?;
    if login.url.is_empty() {
        return Ok(());
    }
    app.opener()
        .open_url(login.url, None::<&str>)
        .map_err(|_| "Не удалось открыть браузер для входа".to_string())
}
#[tauri::command]
async fn account_logout(account_id: String, state: State<'_, AppState>) -> IpcResult<()> {
    state
        .core
        .account_logout(&account_id)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn account_models(
    account_id: String,
    state: State<'_, AppState>,
) -> IpcResult<Vec<ModelInfo>> {
    state
        .core
        .account_models(&account_id)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn account_extensions(
    account_id: String,
    state: State<'_, AppState>,
) -> IpcResult<Extensions> {
    state
        .core
        .account_extensions(&account_id)
        .await
        .map_err(|error| error.to_string())
}
/// Opens the provider's official usage page. Only allowlisted provider URLs can be opened.
#[tauri::command]
fn open_usage(provider: String, app: tauri::AppHandle) -> IpcResult<()> {
    let url = match provider.as_str() {
        "openai" => agent_core::codex::siwc::USAGE_URL,
        "anthropic" => agent_core::claude::USAGE_URL,
        _ => return Err("У этого провайдера нет страницы использования".into()),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "Не удалось открыть браузер".to_string())
}
#[tauri::command]
async fn resolve_approval(
    session_id: String,
    approval_id: String,
    decision: ApprovalDecision,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    state
        .core
        .resolve_approval(&session_id, &approval_id, decision)
        .await
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
        .map_err(|_| "Не удалось открыть страницу выпуска".to_string())
}

fn main() {
    if agent_core::claude::hook_entry() {
        return;
    }
    // Only allowlisted static codes are logged. No provider payloads, prompt text or errors.
    tracing_subscriber::fmt()
        .with_env_filter("warn")
        .with_target(false)
        .with_ansi(false)
        .with_writer(agent_core::redaction::LogWriter::default)
        .init();

    let mut context = tauri::generate_context!();
    let profile = match requested_profile() {
        Ok(profile) => profile,
        Err(_) => {
            tracing::error!(code = "desktop_profile_invalid");
            std::process::exit(1);
        }
    };
    if let Some(directory) = &profile {
        use sha2::{Digest, Sha256};
        // Single-instance/window state namespaces follow an explicitly selected local profile.
        // The default application identifier and existing user data remain unchanged.
        #[cfg(windows)]
        let bytes: Vec<u8> = {
            use std::os::windows::ffi::OsStrExt;
            directory
                .as_os_str()
                .encode_wide()
                .flat_map(u16::to_le_bytes)
                .collect()
        };
        #[cfg(not(windows))]
        let bytes = directory.as_os_str().as_encoded_bytes();
        context
            .config_mut()
            .identifier
            .push_str(&format!(".profile{:x}", Sha256::digest(bytes)));
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // Signed updates only: the updater verifies every package against the embedded key.
        .plugin(
            tauri_plugin_updater::Builder::new()
                .target("windows-x86_64")
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .setup(move |app| {
            let directory = match profile {
                Some(directory) => directory,
                None => app.path().app_local_data_dir()?,
            };
            std::fs::create_dir_all(&directory)?;
            let core = tauri::async_runtime::block_on(Core::open(&directory.join("workspace.db")))?;
            let mut accounts = core.subscribe_accounts();
            let account_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match accounts.recv().await {
                        Ok(event) => {
                            let _ = account_handle.emit("account-event", event);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
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
            create_chat,
            configure_session,
            handoff_session,
            send_message,
            cancel_session,
            session_events,
            refresh_providers,
            add_account,
            rename_account,
            remove_account,
            account_status,
            setup_sandbox,
            account_login,
            account_logout,
            account_models,
            account_extensions,
            resolve_approval,
            open_usage,
            plugin_catalog,
            open_catalog_source,
            save_settings,
            git_status,
            git_diff,
            check_releases,
            open_releases,
            file_operation,
            review_document,
            review_comments,
            add_review_comment,
            resolve_review_comment,
            open_project,
            prepare_update
        ])
        .build(context);
    match app {
        Ok(app) => app.run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                // Stop provider processes cleanly; the kill-on-close job object is the backstop.
                let core = std::sync::Arc::clone(&app.state::<AppState>().core);
                tauri::async_runtime::block_on(core.shutdown());
            }
        }),
        Err(_) => {
            tracing::error!(code = "desktop_startup_failed");
            std::process::exit(1);
        }
    }
}

fn requested_profile() -> std::io::Result<Option<std::path::PathBuf>> {
    let mut selected = None;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument != "--data-dir" {
            continue;
        }
        let path = arguments
            .next()
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Для --data-dir укажите абсолютный путь к папке",
                )
            })?;
        std::fs::create_dir_all(&path)?;
        selected = Some(path.canonicalize()?);
    }
    Ok(selected)
}
