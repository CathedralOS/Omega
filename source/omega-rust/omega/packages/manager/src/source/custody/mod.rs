//! Filesystem custody shared by local snapshots and Git cache entries.
//!
//! Read downward by responsibility: [`tree`] authenticates retained cache
//! trees, [`platform`] applies host ownership and ACL policy, [`lock`] owns
//! per-entry serialization, and [`publication`] owns private staging and
//! atomic publication.

pub(in crate::source) mod lock;
pub(in crate::source) mod platform;
pub(in crate::source) mod publication;
pub(in crate::source) mod tree;
