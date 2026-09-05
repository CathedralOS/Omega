//! Compiler-owned projection of inert calling-contract components.

pub(crate) mod application;
pub(crate) mod callbacks;
mod opaque;
mod physical;

pub use application::project_checked_calling_policy;
