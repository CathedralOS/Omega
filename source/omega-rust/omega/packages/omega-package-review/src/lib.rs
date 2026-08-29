#![forbid(unsafe_code)]

//! Compiler-issued package review evidence.
//!
//! The crate root is intentionally only the public entrance: stable review
//! vocabulary lives in [`evidence`], compiler-to-review conversion in
//! [`projection`], canonical persistence/recovery in [`encoding`], and local
//! reconstruction questions in [`obligations`]. This is a review surface,
//! not accepted package admission evidence.

pub mod encoding;
pub mod evidence;
pub mod obligations;
mod projection;

pub use projection::project_checked_package_review;
