//! Sealed Git command construction and bounded execution.
//!
//! Command policy is assembled in [`command`], process capture and cleanup in
//! [`capture`], Git-specific invocation in [`invocation`], and joined outcome
//! precedence in [`reconciliation`].

pub(crate) mod capture;
pub(crate) mod command;
pub(crate) mod invocation;
pub(crate) mod reconciliation;

#[cfg(test)]
mod tests;
