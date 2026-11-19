//! Shared by the core's integration tests: a scripted HTTP server on
//! `127.0.0.1` and a sample course as typed values.
//!
//! Each test binary uses a different part of this module.
#![allow(dead_code)]

pub mod sample_course;
pub mod server;
