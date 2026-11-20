//! Atlas's core: everything except the window. It does not depend on Tauri.
//!
//! The module map is in `AGENTS.md`.

pub mod auth;
mod core;
pub mod course_map;
pub mod domain;
mod features;
pub mod harness;
pub mod index_tree;
pub mod live;
pub mod placement;
pub mod ports;
pub mod protocol;
pub mod provider;
pub mod sanitise;
pub mod store;
pub mod testing;
pub mod usage;

pub use crate::core::{Core, CoreDeps};
