//! Native Windows implementation. All callers pass through the host approval engine.
mod desktop;
mod indicator;
pub mod security;
mod unity;
pub use desktop::{window_choices, NativeDesktop};
pub use indicator::{restore_target, run_indicator};
pub fn win<T>(result: windows::core::Result<T>) -> crate::Result<T> {
    result.map_err(|_| crate::Error::Unavailable)
}
