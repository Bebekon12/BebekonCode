//! Application-owned host lifecycle and explicit chat binding. UI adapters forward here.
use crate::{
    ipc::{self, Control, Message},
    protocol::*,
    Error, Result,
};
use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::Mutex,
};
pub struct Manager {
    host: Mutex<Option<(Child, String)>>,
    executable: PathBuf,
}
impl Manager {
    pub fn new(executable: PathBuf) -> Self {
        Self {
            host: Mutex::new(None),
            executable,
        }
    }
    async fn command(&self, command: Control) -> Result<serde_json::Value> {
        #[cfg(windows)]
        {
            let (pipe, pid) = {
                let host = self.host.lock().map_err(|_| Error::Busy)?;
                let (child, pipe) = host.as_ref().ok_or(Error::Unavailable)?;
                (pipe.clone(), child.id())
            };
            ipc::exchange(&pipe, Some(pid), Message::Control { command }).await
        }
        #[cfg(not(windows))]
        {
            let _ = command;
            Err(Error::Unavailable)
        }
    }
    pub async fn start(&self) -> Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let mut host = self.host.lock().map_err(|_| Error::Busy)?;
            if let Some((child, _)) = host.as_mut() {
                if child.try_wait()?.is_none() {
                    return Ok(());
                }
            }
            let pipe = format!(r"\\.\pipe\BebekonSmartComputer-{}", uuid::Uuid::new_v4());
            let child = Command::new(&self.executable)
                .args([
                    "--smart-computer-host",
                    &pipe,
                    &std::process::id().to_string(),
                ])
                .creation_flags(0x08000000)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
            *host = Some((child, pipe));
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err(Error::Unavailable)
        }
    }
    pub async fn status(&self) -> Result<Status> {
        Ok(serde_json::from_value(
            self.command(Control::Status).await?,
        )?)
    }
    pub async fn choices(&self) -> Result<Vec<WindowInfo>> {
        self.start().await?;
        Ok(serde_json::from_value(
            self.command(Control::Windows).await?,
        )?)
    }
    pub async fn configure(
        &self,
        account: String,
        session: String,
        windows: Vec<String>,
        read_only: bool,
    ) -> Result<()> {
        self.start().await?;
        self.command(Control::Configure {
            account,
            session,
            windows,
            read_only,
        })
        .await?;
        Ok(())
    }
    pub async fn approve(&self, id: String, allow: bool, sensitive: bool) -> Result<()> {
        #[cfg(windows)]
        if allow {
            if let Some((child, _)) = self.host.lock().map_err(|_| Error::Busy)?.as_ref() {
                crate::windows::win(unsafe {
                    windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(child.id())
                })?;
            }
        }
        self.command(Control::Approve {
            id,
            allow,
            sensitive,
        })
        .await?;
        Ok(())
    }
    pub async fn stop(&self) -> Result<()> {
        self.command(Control::Stop).await?;
        Ok(())
    }
    pub fn revoke_session(self: &std::sync::Arc<Self>, session: &str) {
        let manager = self.clone();
        let session = session.to_string();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = manager.command(Control::Revoke { session }).await;
            });
        }
    }
    /// Per-thread official MCP configuration; never writes global Codex configuration.
    pub async fn config(
        &self,
        account: &str,
        session: &str,
        agent_pid: u32,
    ) -> Result<Option<serde_json::Value>> {
        let status = match self.status().await {
            Ok(s) => s,
            Err(Error::Unavailable) => return Ok(None),
            Err(e) => return Err(e),
        };
        if !status.enabled || status.stopped || status.owner_session.as_deref() != Some(session) {
            return Ok(None);
        }
        self.command(Control::Agent { pid: agent_pid }).await?;
        let pipe = self
            .host
            .lock()
            .map_err(|_| Error::Busy)?
            .as_ref()
            .ok_or(Error::Unavailable)?
            .1
            .clone();
        Ok(Some(
            serde_json::json!({"command":self.executable,"args":["--smart-computer-mcp",pipe,account,session],"enabled":true,"startup_timeout_sec":10,"tool_timeout_sec":125}),
        ))
    }
    pub fn shutdown(&self) {
        if let Ok(mut host) = self.host.lock() {
            if let Some((mut child, _)) = host.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    pub async fn shutdown_async(&self) {
        let _ = self.stop().await;
        let until = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        while tokio::time::Instant::now() < until {
            match self.status().await {
                Ok(s) if s.executing => {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await
                }
                _ => break,
            }
        }
        self.shutdown();
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        self.shutdown();
    }
}
/// Must run before GUI initialization, logging and single-instance handling.
pub fn entry() -> Option<i32> {
    let args: Vec<_> = std::env::args().collect();
    let kind = args.get(1)?.as_str();
    let result = match kind {
        "--smart-computer-mcp" if args.len() == 5 => {
            crate::mcp::serve(&args[2], &args[3], &args[4])
        }
        #[cfg(windows)]
        "--smart-computer-host" if args.len() == 4 => args[3]
            .parse()
            .map_err(|_| Error::Denied)
            .and_then(|pid| ipc::host(&args[2], pid)),
        "--smart-computer-host" | "--smart-computer-mcp" => Err(Error::Denied),
        _ => return None,
    };
    Some(if result.is_ok() { 0 } else { 1 })
}
