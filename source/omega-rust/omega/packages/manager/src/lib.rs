#![forbid(unsafe_code)]

//! Workflows, project declarations, source custody, graph construction, and
//! review for Omega's registry-free package manager.
//!
//! Start with [`workflows`] for complete user flows. Callers name the owner they
//! consume; this root deliberately does not flatten the subsystem into one
//! undifferentiated namespace.

pub mod graph;
pub mod package;
pub mod project;
pub mod review;
pub mod sources;
pub mod workflows;
