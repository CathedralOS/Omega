#![forbid(unsafe_code)]

//! Commands, manifest handling, resolution, and review for Omega's registry-free
//! package manager.
//!
//! Start with [`commands`] for complete operations. Callers name the owner they
//! consume; this root deliberately does not flatten the subsystem into one
//! undifferentiated namespace.

pub mod commands;
pub mod identity;
pub mod manifest;
pub mod resolution;
pub mod review;
