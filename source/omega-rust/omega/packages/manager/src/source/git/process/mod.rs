//! Sealed Git process construction and bounded execution.
//!
//! Command policy is assembled in [`command`], stable execution identities in
//! [`identity`], process capture and cleanup in [`capture`], Git-specific
//! invocation in [`invocation`], and joined outcome precedence in
//! [`reconciliation`].

pub(in crate::source) mod capture;
pub(in crate::source) mod command;
pub(in crate::source) mod identity;
pub(in crate::source) mod invocation;
pub(in crate::source) mod reconciliation;
