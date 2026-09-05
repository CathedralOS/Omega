#![forbid(unsafe_code)]

//! Coordinator-facing custody for the legacy standalone trust receipt section
//! of `omega.lock`.
//!
//! Filesystem-free obligation reconstruction lives in `trust-model`.
//! This crate reads policy for ordinary checks and mutates it only through an
//! explicit acceptance operation owned by command orchestration.

mod custody;

pub use custody::{accept_trust_admissions, read_trust_admissions};
