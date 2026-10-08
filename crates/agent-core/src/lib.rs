pub mod capabilities;
mod chats;
pub mod claude;
pub mod codex;
pub mod credentials;
pub mod error;
pub mod files;
pub mod git;
pub mod model;
pub mod permissions;
pub mod process;
pub mod provider;
pub mod redaction;
pub mod releases;
mod runtime;
pub mod secret_file;
pub mod storage;

pub use error::{CoreError, Result};
pub use runtime::Core;

#[cfg(test)]
mod test_support;
