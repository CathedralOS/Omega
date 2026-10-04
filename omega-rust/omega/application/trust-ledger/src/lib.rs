#![forbid(unsafe_code)]

//! CLI-owned filesystem custody for standalone/native compiler admission policy
//! in `omega.admissions`. The package owner retains `omega.lock`.
//!
//! Compile and run commands read policy here and pass admissions to compilation.
//! The compiler does not depend on this package or read the policy file.
//! Filesystem-free obligation reconstruction lives in `trust-model`.
//! This crate reads policy for ordinary checks and mutates it only through an
//! explicit acceptance operation owned by command orchestration. The filesystem
//! lifecycle is in `admission_policy.rs`.

mod admission_policy;

pub use admission_policy::{accept_trust_admissions, read_trust_admissions};
