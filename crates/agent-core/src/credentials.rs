use crate::{CoreError, Result};

/// No frontend command exposes this store. SQLite stores only a profile reference.
pub struct CredentialStore;
impl CredentialStore {
    fn entry(profile: &str) -> Result<keyring::Entry> {
        uuid::Uuid::parse_str(profile)
            .map_err(|_| CoreError::Invalid("Invalid account profile ID".into()))?;
        keyring::Entry::new("com.bebekon.agent-workspace", profile)
            .map_err(|_| CoreError::CredentialStore)
    }
    pub fn save(profile: &str, secret: &str) -> Result<()> {
        Self::entry(profile)?
            .set_password(secret)
            .map_err(|_| CoreError::CredentialStore)
    }
    pub fn load(profile: &str) -> Result<String> {
        Self::entry(profile)?
            .get_password()
            .map_err(|_| CoreError::CredentialStore)
    }
    pub fn remove(profile: &str) -> Result<()> {
        Self::entry(profile)?
            .delete_credential()
            .map_err(|_| CoreError::CredentialStore)
    }
}
