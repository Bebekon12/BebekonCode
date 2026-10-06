//! Child process hygiene shared by every provider adapter.
//!
//! * Children receive an allowlisted environment, so API keys and config overrides meant for
//!   one provider or account never leak into another provider's process.
//! * On Windows each provider process tree lives in its own kill-on-close Job Object. Dropping
//!   the [`ProcessGroup`] (account stopped, app exit or crash) ends the whole tree, including
//!   helpers the CLI starts itself (for example `git`). Processes BebekonCode did not start are
//!   never touched.

use std::ffi::OsString;

/// Variables a CLI needs to locate the user profile, temp space, shells and proxies.
/// Anything that can carry provider credentials or redirect provider state is excluded.
const INHERITED: &[&str] = &[
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMW6432",
    "COMMONPROGRAMFILES",
    "COMMONPROGRAMFILES(X86)",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "OS",
    "USERNAME",
    "USERDOMAIN",
    "COMPUTERNAME",
    "HOME",
    "USER",
    "LANG",
    "LC_ALL",
    "SHELL",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "ALL_PROXY",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NODE_EXTRA_CA_CERTS",
];

/// Builds the environment for a provider child process from an allowlist plus explicit values.
pub fn child_env(extra: &[(&str, OsString)]) -> Vec<(OsString, OsString)> {
    child_env_from(std::env::vars_os(), extra)
}

fn child_env_from(
    current: impl Iterator<Item = (OsString, OsString)>,
    extra: &[(&str, OsString)],
) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = current
        .filter(|(key, _)| {
            let key = key.to_string_lossy().to_ascii_uppercase();
            INHERITED.contains(&key.as_str())
        })
        .collect();
    for (key, value) in extra {
        env.retain(|(existing, _)| !existing.eq_ignore_ascii_case(key));
        env.push((OsString::from(key), value.clone()));
    }
    env
}

/// Applies the hardened defaults to a provider command: no inherited environment and no
/// console window. Stdio is configured by the caller.
pub fn harden(command: &mut std::process::Command, env: Vec<(OsString, OsString)>) {
    command.env_clear().envs(env);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Owns a provider process tree. Dropping it terminates every process in the tree.
pub struct ProcessGroup {
    #[cfg(windows)]
    job: job::Job,
}

impl ProcessGroup {
    /// Places a freshly spawned child into a new kill-on-close group. Returns `None` when the
    /// platform offers no such mechanism; callers then still kill the direct child themselves.
    pub fn adopt(child: &std::process::Child) -> Option<Self> {
        #[cfg(windows)]
        {
            let job = job::Job::new()?;
            job.assign(child).then_some(Self { job })
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            None
        }
    }

    /// Ends every process in the tree immediately.
    pub fn terminate(&self) {
        #[cfg(windows)]
        self.job.terminate();
    }
}

#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
    };

    pub struct Job(HANDLE);
    // SAFETY: a job handle is a kernel handle owned exclusively by this value; Win32 job APIs
    // are thread-safe and the handle is closed exactly once in `Drop`.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub fn new() -> Option<Self> {
            // SAFETY: plain Win32 calls with valid arguments; `info` is a zeroed POD struct of
            // the size passed, and the handle is closed if configuration fails.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let configured = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if configured == 0 {
                    CloseHandle(job);
                    return None;
                }
                Some(Self(job))
            }
        }

        pub fn assign(&self, child: &std::process::Child) -> bool {
            // SAFETY: both handles are valid for the duration of the call; the process handle
            // belongs to a child this process spawned and still owns.
            unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) != 0 }
        }

        pub fn terminate(&self) {
            // SAFETY: valid job handle owned by `self`.
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the handle was created by `new` and is closed exactly once. Closing the
            // last handle kills all processes in the job (KILL_ON_JOB_CLOSE).
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_secrets_are_not_inherited() {
        let current = [
            ("Path", "C:\\bin"),
            ("OPENAI_API_KEY", "sk-private"),
            ("ANTHROPIC_API_KEY", "sk-ant-private"),
            ("CLAUDE_CONFIG_DIR", "C:\\other-account"),
            ("CODEX_HOME", "C:\\other-account"),
            ("USERPROFILE", "C:\\Users\\me"),
        ]
        .map(|(key, value)| (OsString::from(key), OsString::from(value)));
        let env = child_env_from(
            current.into_iter(),
            &[("CODEX_HOME", OsString::from("C:\\profiles\\a"))],
        );
        let keys: Vec<String> = env
            .iter()
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        assert!(keys.iter().any(|key| key == "Path"));
        assert!(keys.iter().any(|key| key == "USERPROFILE"));
        assert!(!keys.iter().any(|key| key.contains("API_KEY")));
        assert!(!keys.iter().any(|key| key == "CLAUDE_CONFIG_DIR"));
        let homes: Vec<_> = env
            .iter()
            .filter(|(key, _)| key == "CODEX_HOME")
            .map(|(_, value)| value.clone())
            .collect();
        assert_eq!(homes, vec![OsString::from("C:\\profiles\\a")]);
    }

    #[cfg(windows)]
    #[test]
    fn dropping_the_group_ends_grandchildren() {
        // cmd waits a second (so it is already in the group), starts a long-running grandchild
        // that inherits the pipe, then exits; dropping the group must still end the grandchild.
        let mut command = std::process::Command::new("cmd.exe");
        command
            .args([
                "/C",
                "ping -n 2 127.0.0.1 >NUL & start /B ping -n 30 127.0.0.1",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        harden(&mut command, child_env(&[]));
        let mut child = command.spawn().expect("spawn");
        let group = ProcessGroup::adopt(&child).expect("job");
        let mut stdout = child.stdout.take().expect("stdout");
        let reader = std::thread::spawn(move || {
            use std::io::Read;
            let mut sink = Vec::new();
            let _ = stdout.read_to_end(&mut sink);
        });
        let _ = child.wait();
        drop(group);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !reader.is_finished() {
            assert!(
                std::time::Instant::now() < deadline,
                "grandchild kept the pipe open"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}
