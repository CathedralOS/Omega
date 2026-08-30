#![forbid(unsafe_code)]

//! Workflows, manifests, resolution, and review for Omega's registry-free
//! package manager.
//!
//! Start with [`workflows`] for complete user flows. Callers name the owner they
//! consume; this root deliberately does not flatten the subsystem into one
//! undifferentiated namespace.

pub mod manifest;
pub mod resolution;
pub mod review;
pub mod workflows;
