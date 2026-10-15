use crate::{Error, Result};
use std::{mem::size_of, os::windows::ffi::OsStrExt};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::*,
        Security::Authorization::*,
        Security::*,
        System::{Diagnostics::ToolHelp::*, RemoteDesktop::ProcessIdToSessionId, Threading::*},
    },
};

pub fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(Some(0))
        .collect()
}
pub fn interactive_desktop() -> Result<()> {
    use windows::Win32::System::StationsAndDesktops::*;
    let desktop = super::win(unsafe {
        OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS)
    })?;
    let mut name = [0u16; 128];
    let result = super::win(unsafe {
        GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            (name.len() * 2) as u32,
            None,
        )
    });
    unsafe {
        let _ = CloseDesktop(desktop);
    }
    result?;
    let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
    if !String::from_utf16_lossy(&name[..end]).eq_ignore_ascii_case("Default") {
        return Err(Error::Denied);
    }
    Ok(())
}
pub struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
pub fn process(pid: u32) -> Result<Handle> {
    super::win(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }).map(Handle)
}
pub fn alive(pid: u32) -> bool {
    process(pid)
        .and_then(|p| {
            let mut code = 0;
            super::win(unsafe { GetExitCodeProcess(p.0, &mut code) })?;
            Ok(code == 259)
        })
        .unwrap_or(false)
}
fn token(pid: u32) -> Result<Handle> {
    let p = process(pid)?;
    let mut t = HANDLE::default();
    super::win(unsafe { OpenProcessToken(p.0, TOKEN_QUERY, &mut t) })?;
    Ok(Handle(t))
}
pub fn sid(pid: u32) -> Result<String> {
    let t = token(pid)?;
    let mut size = 0;
    unsafe {
        let _ = GetTokenInformation(t.0, TokenUser, None, 0, &mut size);
    }
    if size == 0 || size > 65536 {
        return Err(Error::Denied);
    }
    let mut data = vec![0u64; (size as usize).div_ceil(8)];
    super::win(unsafe {
        GetTokenInformation(
            t.0,
            TokenUser,
            Some(data.as_mut_ptr().cast()),
            size,
            &mut size,
        )
    })?;
    let user = unsafe { &*data.as_ptr().cast::<TOKEN_USER>() };
    let mut out = PWSTR::null();
    super::win(unsafe { ConvertSidToStringSidW(user.User.Sid, &mut out) })?;
    let value = unsafe { out.to_string() }.map_err(|_| Error::Denied);
    unsafe { LocalFree(Some(HLOCAL(out.0.cast()))) };
    value
}
pub fn elevation(pid: u32) -> Result<bool> {
    let t = token(pid)?;
    let mut e = TOKEN_ELEVATION::default();
    let mut size = 0;
    super::win(unsafe {
        GetTokenInformation(
            t.0,
            TokenElevation,
            Some((&mut e as *mut TOKEN_ELEVATION).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
    })?;
    Ok(e.TokenIsElevated != 0)
}
pub fn session(pid: u32) -> Result<u32> {
    let mut id = 0;
    super::win(unsafe { ProcessIdToSessionId(pid, &mut id) })?;
    Ok(id)
}
pub fn path(pid: u32) -> Result<std::path::PathBuf> {
    let p = process(pid)?;
    let mut buf = vec![0u16; 32768];
    let mut len = buf.len() as u32;
    super::win(unsafe {
        QueryFullProcessImageNameW(p.0, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
    })?;
    Ok(String::from_utf16_lossy(&buf[..len as usize]).into())
}
pub fn same_user(pid: u32) -> Result<()> {
    let me = std::process::id();
    if sid(pid)? != sid(me)? || session(pid)? != session(me)? || elevation(pid)? {
        return Err(Error::Denied);
    }
    Ok(())
}
pub fn same_executable(pid: u32) -> Result<()> {
    same_user(pid)?;
    if !path(pid)?
        .to_string_lossy()
        .eq_ignore_ascii_case(&path(std::process::id())?.to_string_lossy())
    {
        return Err(Error::Denied);
    }
    Ok(())
}
pub fn parent(pid: u32) -> Result<u32> {
    let snapshot = Handle(super::win(unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    })?);
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    super::win(unsafe { Process32FirstW(snapshot.0, &mut entry) })?;
    loop {
        if entry.th32ProcessID == pid {
            return Ok(entry.th32ParentProcessID);
        }
        if unsafe { Process32NextW(snapshot.0, &mut entry) }.is_err() {
            break;
        }
    }
    Err(Error::Denied)
}
pub fn child_of(mut pid: u32, ancestor: u32) -> bool {
    for _ in 0..5 {
        let Ok(p) = parent(pid) else {
            return false;
        };
        if p == ancestor {
            return true;
        }
        if p == 0 || p == pid {
            return false;
        }
        pid = p;
    }
    false
}

pub struct SecurityDescriptor(PSECURITY_DESCRIPTOR);
impl SecurityDescriptor {
    pub fn new() -> Result<Self> {
        let s = wide(&format!("D:P(A;;GA;;;{})", sid(std::process::id())?));
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        super::win(unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(s.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        })?;
        Ok(Self(descriptor))
    }
    pub fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0 .0,
            bInheritHandle: false.into(),
        }
    }
}
impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(Some(HLOCAL(self.0 .0)));
        }
    }
}

/// Exclusive cross-process controller for this interactive logon session.
pub struct Lease(Handle);
impl Lease {
    pub fn acquire() -> Result<Self> {
        let d = SecurityDescriptor::new()?;
        let a = d.attributes();
        let name = wide(&format!(
            "Local\\BebekonSmartComputer-{}-{}",
            sid(std::process::id())?,
            session(std::process::id())?
        ));
        let handle = Handle(super::win(unsafe {
            CreateMutexW(Some(&a), false, PCWSTR(name.as_ptr()))
        })?);
        let wait = unsafe { WaitForSingleObject(handle.0, 0) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            return Err(Error::Busy);
        }
        Ok(Self(handle))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0 .0);
        }
    }
}
