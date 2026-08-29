//! Compiler-owned translation into inert package-review evidence.
//!
//! The entrance coordinates checked semantic interpretation, public API and
//! behavior projection, provider selection, representation disclosure, and
//! source custody. Evidence types and canonical encoding remain separate owners.

mod api;
mod authority;
mod behavior;
mod callables;
mod contracts;
mod providers;
mod representation;
mod review;
mod semantics;
mod source;

pub use review::project_checked_package_review;
