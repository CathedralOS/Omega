#![forbid(unsafe_code)]

//! Compiler-issued package review evidence.
//!
//! The crate root is intentionally only the public entrance: stable policy
//! vocabulary lives in [`record`], compiler-to-policy conversion in
//! [`capture`], and canonical persistence/recovery in [`encoding`]. This is a
//! review surface, not accepted package admission evidence.

mod capture;
pub mod encoding;
pub mod permission_acceptance;
pub mod record;

pub use capture::{PackageReviewProjectionError, project_package_review};
pub use capture::{
    SemanticBindingProjectionError, candidate_hosted_exit_bindings, candidate_service_bindings,
    candidate_target_entry_binding,
};
pub use capture::{project_checked_package_policy, project_checked_terminal_permission_policy};
