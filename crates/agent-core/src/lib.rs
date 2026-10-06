pub mod capabilities;
pub mod credentials;
pub mod error;
pub mod files;
pub mod git;
pub mod model;
pub mod permissions;
pub mod provider;
pub mod redaction;
pub mod releases;
mod runtime;
pub mod storage;

pub use error::{CoreError, Result};
pub use runtime::Core;

#[cfg(test)]
mod test_support;
