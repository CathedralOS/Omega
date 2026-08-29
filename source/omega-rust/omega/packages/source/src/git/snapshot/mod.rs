//! Authenticated immutable source snapshots.
//!
//! Git tree authentication and materialization enter through
//! [`resolve_git_snapshot`]. Shared snapshot construction, metadata,
//! permission finalization, and atomic publication live in focused sibling
//! modules because local-source snapshots use the same custody machinery.

pub(crate) mod construction;
pub(crate) mod materialization;
pub(crate) mod metadata;
pub(crate) mod permissions;
pub(crate) mod publication;
