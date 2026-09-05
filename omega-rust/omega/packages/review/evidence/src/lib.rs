#![forbid(unsafe_code)]

//! Compiler-issued package review evidence.
//!
//! The crate root is intentionally only the public entrance: stable review
//! vocabulary lives in [`record`], compiler-to-review conversion in
//! [`capture`], canonical persistence/recovery in [`encoding`], and local
//! reconstruction questions in [`ledger`]. This is a review surface,
//! not accepted package admission evidence.

mod capture;
pub mod encoding;
pub mod ledger;
pub mod record;

pub use capture::{
    project_checked_calling_policy, project_checked_conformance_policy,
    project_checked_package_review, project_checked_representation_policy,
    project_checked_selected_provider_policy, project_non_executable_quotient_package_review,
};
