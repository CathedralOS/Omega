#![forbid(unsafe_code)]

//! Coordinator-facing custody for standalone/native compiler admission policy
//! in `omega.admissions`. The package owner retains `omega.lock`.
//!
//! Filesystem-free obligation reconstruction lives in `trust-model`.
//! This crate reads policy for ordinary checks and mutates it only through an
//! explicit acceptance operation owned by command orchestration. The filesystem
//! lifecycle is in `admission_policy.rs`.

mod admission_policy;
#[cfg(test)]
mod admission_policy_test_support;

pub use admission_policy::{accept_trust_admissions, read_trust_admissions};
