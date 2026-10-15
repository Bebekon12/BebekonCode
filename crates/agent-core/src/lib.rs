pub mod attachments;
pub mod capabilities;
pub mod catalog;
mod changes;
mod chats;
pub mod claude;
pub mod codex;
pub mod computer;
pub mod conversation;
pub mod credentials;
pub mod error;
pub mod files;
pub mod git;
pub mod mcp;
pub mod model;
pub mod permissions;
pub mod process;
pub mod provider;
mod provider_diagnostics;
pub mod redaction;
pub mod releases;
pub mod review;
mod runtime;
pub mod secret_file;
pub mod storage;
mod team;
mod unity_adapter;
pub mod workflows;

pub use error::{CoreError, Result};
pub use runtime::Core;

#[cfg(test)]
mod test_support;
