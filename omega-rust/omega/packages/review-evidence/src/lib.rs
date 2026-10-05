#![forbid(unsafe_code)]

//! Compiler-issued package review evidence.
//!
//! The crate root is intentionally only the public entrance: stable policy
//! vocabulary lives in [`record`], compiler-to-policy conversion in
//! [`capture`], and canonical persistence/recovery in [`encoding`]. This is a
//! review surface, not accepted package admission evidence.

mod capture;
pub mod encoding;
pub mod record;
pub use encoding::PACKAGE_CALLABLE_POLICY_VERSION;

pub use capture::{project_checked_package_policy, project_checked_terminal_permission_policy};
