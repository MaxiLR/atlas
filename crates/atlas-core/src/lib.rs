//! Atlas's core: everything except the window. It does not depend on Tauri.

mod core;
pub mod harness;
pub mod ports;
pub mod protocol;
pub mod provider;
pub mod store;

pub use crate::core::{Core, CoreDeps};
