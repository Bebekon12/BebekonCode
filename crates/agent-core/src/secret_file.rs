//! Small secret records protected with the OS user's credentials.
//!
//! On Windows the bytes are encrypted with DPAPI (user scope) before they touch disk, so the file
//! is useless to other users or when copied to another machine. Writes are atomic. Secrets never
//! enter SQLite, logs or the frontend; callers only ever pass them to the provider process.

use crate::{CoreError, Result};
use std::path::Path;

pub fn write(path: &Path, plaintext: &[u8]) -> Result<()> {
    let protected = protect(plaintext)?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, protected)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

pub fn read(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => unprotect(&bytes).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn remove(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

#[cfg(windows)]
fn protect(plaintext: &[u8]) -> Result<Vec<u8>> {
    dpapi::run(plaintext, true)
}
#[cfg(windows)]
fn unprotect(bytes: &[u8]) -> Result<Vec<u8>> {
    dpapi::run(bytes, false)
}

#[cfg(not(windows))]
fn protect(_: &[u8]) -> Result<Vec<u8>> {
    // V1 targets Windows; other platforms must add an OS-backed store before enabling this.
    Err(CoreError::CredentialStore)
}
#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> Result<Vec<u8>> {
    Err(CoreError::CredentialStore)
}

#[cfg(windows)]
mod dpapi {
    use super::{CoreError, Result};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };

    pub fn run(input: &[u8], encrypt: bool) -> Result<Vec<u8>> {
        let source = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(input.len()).map_err(|_| CoreError::CredentialStore)?,
            pbData: input.as_ptr().cast_mut(),
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        // SAFETY: `source` points at `input`, which outlives the call and is only read. On
        // success DPAPI allocates `output` with LocalAlloc; it is copied and freed exactly once.
        unsafe {
            let ok = if encrypt {
                CryptProtectData(
                    &source,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            } else {
                CryptUnprotectData(
                    &source,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            };
            if ok == 0 || output.pbData.is_null() {
                return Err(CoreError::CredentialStore);
            }
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData.cast());
            Ok(bytes)
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn records_round_trip_and_are_not_plaintext_on_disk() {
        let directory = std::env::temp_dir().join(format!("secret-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).expect("dir");
        let path = directory.join("record.bin");
        super::write(&path, b"refresh_token=private-value").expect("write");
        let raw = std::fs::read(&path).expect("raw");
        assert!(!String::from_utf8_lossy(&raw).contains("private-value"));
        assert_eq!(
            super::read(&path).expect("read").as_deref(),
            Some(&b"refresh_token=private-value"[..])
        );
        super::remove(&path).expect("remove");
        assert_eq!(super::read(&path).expect("missing"), None);
        let _ = std::fs::remove_dir_all(directory);
    }
}
