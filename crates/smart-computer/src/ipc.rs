//! Local named-pipe transport. Identity comes from the kernel, never request fields.
use crate::{invalid, protocol::*, Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "channel", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    Proxy {
        account: String,
        session: String,
        request: Box<Request>,
    },
    Control {
        command: Control,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Control {
    Status,
    Windows,
    Configure {
        account: String,
        session: String,
        windows: Vec<String>,
        read_only: bool,
    },
    Agent {
        pid: u32,
    },
    Approve {
        id: String,
        allow: bool,
        sensitive: bool,
    },
    Stop,
    Revoke {
        session: String,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    value: Option<serde_json::Value>,
    error: Option<ReplyError>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplyError {
    kind: String,
    message: String,
}
impl From<Error> for ReplyError {
    fn from(error: Error) -> Self {
        let kind = match error {
            Error::Denied => "denied",
            Error::Cancelled => "cancelled",
            Error::Busy => "busy",
            Error::Unavailable => "unavailable",
            _ => "invalid",
        };
        Self {
            kind: kind.into(),
            message: error.to_string(),
        }
    }
}
impl From<ReplyError> for Error {
    fn from(error: ReplyError) -> Self {
        match error.kind.as_str() {
            "denied" => Error::Denied,
            "cancelled" => Error::Cancelled,
            "busy" => Error::Busy,
            "unavailable" => Error::Unavailable,
            _ => invalid(&error.message),
        }
    }
}
pub fn validate_pipe(pipe: &str) -> Result<()> {
    if !pipe.starts_with(r"\\.\pipe\BebekonSmartComputer-")
        || pipe.len() > 128
        || pipe[32.min(pipe.len())..].contains(['\\', '/'])
    {
        return Err(invalid("Некорректный локальный канал"));
    }
    Ok(())
}

#[cfg(windows)]
pub async fn exchange(
    pipe: &str,
    server_pid: Option<u32>,
    message: Message,
) -> Result<serde_json::Value> {
    use std::os::windows::io::AsRawHandle;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::windows::named_pipe::ClientOptions,
    };
    use windows::Win32::{Foundation::HANDLE, System::Pipes::GetNamedPipeServerProcessId};
    validate_pipe(pipe)?;
    let until = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut stream = loop {
        match ClientOptions::new().open(pipe) {
            Ok(p) => break p,
            Err(error)
                if error.raw_os_error() == Some(231)
                    || error.kind() == std::io::ErrorKind::NotFound =>
            {
                if tokio::time::Instant::now() >= until {
                    return Err(Error::Unavailable);
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            Err(error) => return Err(error.into()),
        }
    };
    let mut pid = 0;
    crate::windows::win(unsafe {
        GetNamedPipeServerProcessId(HANDLE(stream.as_raw_handle()), &mut pid)
    })?;
    if server_pid.is_some_and(|expected| expected != pid) {
        return Err(Error::Denied);
    }
    crate::windows::security::same_executable(pid)?;
    let bytes = serde_json::to_vec(&message)?;
    if bytes.len() > MAX_MESSAGE {
        return Err(invalid("Слишком большой запрос"));
    }
    stream.write_u32_le(bytes.len() as u32).await?;
    stream.write_all(&bytes).await?;
    let response = tokio::time::timeout(std::time::Duration::from_secs(125), async {
        let size = stream.read_u32_le().await? as usize;
        if size > MAX_MESSAGE {
            return Err(invalid("Слишком большой ответ"));
        }
        let mut bytes = vec![0; size];
        stream.read_exact(&mut bytes).await?;
        let reply: Reply = serde_json::from_slice(&bytes)?;
        if let Some(error) = reply.error {
            return Err(error.into());
        }
        reply.value.ok_or(Error::Unavailable)
    })
    .await
    .map_err(|_| Error::Cancelled)??;
    Ok(response)
}
pub async fn proxy_exchange(
    pipe: &str,
    account: &str,
    session: &str,
    request: Request,
) -> Result<ToolResult> {
    #[cfg(windows)]
    {
        let value = exchange(
            pipe,
            None,
            Message::Proxy {
                account: account.into(),
                session: session.into(),
                request: Box::new(request),
            },
        )
        .await?;
        Ok(serde_json::from_value(value)?)
    }
    #[cfg(not(windows))]
    {
        let _ = (pipe, account, session, request);
        Err(Error::Unavailable)
    }
}

#[cfg(windows)]
pub fn host(pipe: &str, parent_pid: u32) -> Result<()> {
    use crate::{
        engine,
        windows::{security, NativeDesktop},
    };
    security::same_executable(parent_pid)?;
    if security::parent(std::process::id())? != parent_pid || security::session(parent_pid)? == 0 {
        return Err(Error::Denied);
    }
    validate_pipe(pipe)?;
    let shared = engine::shared(parent_pid);
    shared.0.lock().map_err(|_| Error::Busy)?.status.available = false;
    let indicator = shared.clone();
    std::thread::spawn(move || {
        if crate::windows::run_indicator(indicator.clone()).is_err() {
            engine::stop(&indicator);
        }
    });
    let (send, receive) = std::sync::mpsc::sync_channel::<Work>(4);
    let worker_state = shared.clone();
    std::thread::spawn(move || {
        let Ok(desktop) = NativeDesktop::new() else {
            engine::stop(&worker_state);
            return;
        };
        if let Ok(mut state) = worker_state.0.lock() {
            state.desktop_ready = true;
            state.status.available = state.indicator_ready;
        }
        let mut engine = engine::Engine::new(desktop, worker_state);
        while let Ok(work) = receive.recv() {
            if let Ok(mut state) = engine.shared.0.lock() {
                state.status.executing = true;
            }
            let active = engine.shared.0.lock().is_ok_and(|s| s.epoch == work.epoch);
            let answer = if !active {
                Err(Error::Cancelled)
            } else {
                engine.call_bound(&work.account, &work.session, work.request, Some(work.epoch))
            }
            .and_then(|v| serde_json::to_value(v).map_err(Into::into));
            let _ = work.reply.send(answer);
            if let Ok(mut state) = engine.shared.0.lock() {
                state.status.executing = false;
            }
        }
    });
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    tokio::task::LocalSet::new().block_on(&runtime, run_host(pipe, parent_pid, shared, send))
}
#[cfg(windows)]
struct Work {
    epoch: u64,
    account: String,
    session: String,
    request: Request,
    reply: tokio::sync::oneshot::Sender<Result<serde_json::Value>>,
}

#[cfg(windows)]
async fn run_host(
    pipe: &str,
    parent_pid: u32,
    shared: crate::engine::Shared,
    send: std::sync::mpsc::SyncSender<Work>,
) -> Result<()> {
    use crate::{engine, windows::security};
    use std::sync::Arc;
    use tokio::{io::AsyncWriteExt, net::windows::named_pipe::ServerOptions};
    let descriptor = security::SecurityDescriptor::new()?;
    let mut attributes = descriptor.attributes();
    let mut first = true;
    let slots = Arc::new(tokio::sync::Semaphore::new(8));
    let lease = std::rc::Rc::new(std::cell::RefCell::new(None::<security::Lease>));
    loop {
        if !security::alive(parent_pid) {
            engine::stop(&shared);
            return Ok(());
        }
        if !shared.0.lock().map_err(|_| Error::Busy)?.status.enabled {
            lease.borrow_mut().take();
        }
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .max_instances(9)
            .in_buffer_size(65536)
            .out_buffer_size(65536);
        let server = unsafe {
            options.create_with_security_attributes_raw(
                pipe,
                (&mut attributes as *mut windows::Win32::Security::SECURITY_ATTRIBUTES).cast(),
            )
        }?;
        first = false;
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(200), server.connect())
                .await
            {
                Ok(result) => {
                    result?;
                    break;
                }
                Err(_) => {
                    if !security::alive(parent_pid) {
                        engine::stop(&shared);
                        return Ok(());
                    }
                    if !shared.0.lock().map_err(|_| Error::Busy)?.status.enabled {
                        lease.borrow_mut().take();
                    }
                }
            }
        }
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            drop(server);
            continue;
        };
        let shared = shared.clone();
        let send = send.clone();
        let lease = lease.clone();
        tokio::task::spawn_local(async move {
            let _permit = permit;
            let mut stream = server;
            let answer = serve_client(&mut stream, parent_pid, &shared, &send, &lease).await;
            let reply = match answer {
                Ok(value) => Reply {
                    value: Some(value),
                    error: None,
                },
                Err(error) => Reply {
                    value: None,
                    error: Some(error.into()),
                },
            };
            if let Ok(bytes) = serde_json::to_vec(&reply) {
                if bytes.len() <= MAX_MESSAGE {
                    let _ = stream.write_u32_le(bytes.len() as u32).await;
                    let _ = stream.write_all(&bytes).await;
                }
            }
        });
    }
}

#[cfg(windows)]
async fn serve_client(
    stream: &mut tokio::net::windows::named_pipe::NamedPipeServer,
    parent_pid: u32,
    shared: &crate::engine::Shared,
    send: &std::sync::mpsc::SyncSender<Work>,
    lease: &std::rc::Rc<std::cell::RefCell<Option<crate::windows::security::Lease>>>,
) -> Result<serde_json::Value> {
    use crate::{engine, windows::security};
    use std::os::windows::io::AsRawHandle;
    use tokio::io::AsyncReadExt;
    use windows::Win32::{Foundation::HANDLE, System::Pipes::GetNamedPipeClientProcessId};
    let mut pid = 0;
    crate::windows::win(unsafe {
        GetNamedPipeClientProcessId(HANDLE(stream.as_raw_handle()), &mut pid)
    })?;
    security::same_executable(pid)?;
    let message: Message =
        tokio::time::timeout(std::time::Duration::from_secs(3), read_message(stream))
            .await
            .map_err(|_| Error::Cancelled)??;
    match message {
        Message::Control { command } => {
            if pid != parent_pid {
                return Err(Error::Denied);
            }
            match command {
                Control::Status => Ok(serde_json::to_value(
                    &shared.0.lock().map_err(|_| Error::Busy)?.status,
                )?),
                Control::Windows => Ok(serde_json::to_value(crate::windows::window_choices()?)?),
                Control::Configure {
                    account,
                    session,
                    windows,
                    read_only,
                } => {
                    if !shared.0.lock().map_err(|_| Error::Busy)?.status.available {
                        return Err(Error::Unavailable);
                    }
                    let choices = crate::windows::window_choices()?;
                    let selected: Vec<_> = windows
                        .iter()
                        .filter_map(|id| choices.iter().find(|w| &w.id == id).cloned())
                        .collect();
                    if selected.len() != windows.len() {
                        return Err(Error::Denied);
                    }
                    let mut guard = lease.borrow_mut();
                    if guard.is_none() {
                        *guard = Some(security::Lease::acquire()?);
                    }
                    engine::configure(shared, account, session, selected, read_only)?;
                    Ok(serde_json::json!({"enabled":true}))
                }
                Control::Agent { pid } => {
                    security::same_user(pid)?;
                    if !security::child_of(pid, parent_pid) {
                        return Err(Error::Denied);
                    }
                    shared.0.lock().map_err(|_| Error::Busy)?.agent_pid = pid;
                    Ok(serde_json::json!({"registered":true}))
                }
                Control::Approve {
                    id,
                    allow,
                    sensitive,
                } => {
                    {
                        let state = shared.0.lock().map_err(|_| Error::Busy)?;
                        let pending = state.status.pending.as_ref().ok_or(Error::Denied)?;
                        if pending.id != id || allow && pending.sensitive && !sensitive {
                            return Err(Error::Denied);
                        }
                    }
                    if allow {
                        crate::windows::restore_target(shared)?;
                    }
                    engine::approve(shared, &id, allow, sensitive)?;
                    Ok(serde_json::json!({"decided":true}))
                }
                Control::Stop => {
                    engine::stop(shared);
                    lease.borrow_mut().take();
                    Ok(serde_json::json!({"stopped":true}))
                }
                Control::Revoke { session } => {
                    if shared
                        .0
                        .lock()
                        .map_err(|_| Error::Busy)?
                        .status
                        .owner_session
                        .as_deref()
                        == Some(&session)
                    {
                        engine::stop(shared);
                        lease.borrow_mut().take();
                    }
                    Ok(serde_json::json!({"revoked":true}))
                }
            }
        }
        Message::Proxy {
            account,
            session,
            request,
        } => {
            let (agent, epoch) = {
                let state = shared.0.lock().map_err(|_| Error::Busy)?;
                (state.agent_pid, state.epoch)
            };
            if agent == 0 || !security::child_of(pid, agent) {
                return Err(Error::Denied);
            }
            request.validate()?;
            let (reply, result) = tokio::sync::oneshot::channel();
            send.try_send(Work {
                epoch,
                account,
                session,
                request: *request,
                reply,
            })
            .map_err(|_| Error::Busy)?;
            let mut disconnected = [0u8; 1];
            tokio::select! {
                result = result => result.map_err(|_|Error::Unavailable)?,
                _ = stream.read(&mut disconnected) => { engine::stop(shared); Err(Error::Cancelled) }
            }
        }
    }
}
#[cfg(windows)]
async fn read_message(
    stream: &mut tokio::net::windows::named_pipe::NamedPipeServer,
) -> Result<Message> {
    use tokio::io::AsyncReadExt;
    let size = stream.read_u32_le().await? as usize;
    if size > MAX_MESSAGE {
        return Err(invalid("Слишком большой запрос"));
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pipes_are_local_and_schema_rejects_control_injection() {
        assert!(validate_pipe(r"\\.\pipe\BebekonSmartComputer-123").is_ok());
        assert!(validate_pipe(r"\\server\pipe\BebekonSmartComputer-123").is_err());
        assert!(serde_json::from_value::<Message>(
            serde_json::json!({"channel":"control","command":{"command":"stop"},"account":"a"})
        )
        .is_err());
    }
}
