//! Core-only types: what Atlas knows about the candidate and the course memory
//! that keeps a long course coherent (ADR-0006, ADR-0013).
//!
//! Nothing here implements `ts_rs::TS`, so no request, response or event can
//! carry it: the interface never sees the ladder, the profile's internals, the
//! course map, the conventions, the section records or an answer key. They
//! serialise to JSON for the store and, for the section record, as the model
//! writes it.

pub mod conventions;
pub mod course_map;
pub mod ladder;
pub mod profile;
pub mod questions;
pub mod record;
