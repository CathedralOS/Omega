//! Authenticated immutable source snapshots.
//!
//! Git tree authentication and materialization enter through
//! [`resolve_git_snapshot`]. Shared snapshot construction, metadata,
//! permission finalization, and atomic publication live in focused sibling
//! modules because local-source snapshots use the same custody machinery.

pub(in crate::source) mod construction;
pub(in crate::source) mod materialization;
pub(in crate::source) mod metadata;
pub(in crate::source) mod permissions;
pub(in crate::source) mod publication;
