#![forbid(unsafe_code)]

//! Commands, declarations, resolution, and review for Omega's registry-free
//! package manager.
//!
//! Start with [`commands`] for complete operations. Callers name the owner they
//! consume; this root deliberately does not flatten the subsystem into one
//! undifferentiated namespace.

pub mod commands;
pub mod declarations;
pub mod resolution;
pub mod review;
