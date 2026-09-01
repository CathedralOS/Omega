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
mod package;
mod providers;
mod quotients;
mod representation;
mod semantics;
mod source;
mod terminal_authority_permissions;

pub use package::project_checked_package_review;
pub use quotients::project_non_executable_quotient_package_review;
