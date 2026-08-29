#![forbid(unsafe_code)]

//! Operations, declarations, discovery, graph construction, and review for
//! Omega's registry-free package manager.
//!
//! Start with [`operations`] for complete operations. Callers name the owner they
//! consume; this root deliberately does not flatten the subsystem into one
//! undifferentiated namespace.

pub mod declarations;
pub mod discovery;
pub mod graph;
pub mod identity;
pub mod operations;
pub mod review;
