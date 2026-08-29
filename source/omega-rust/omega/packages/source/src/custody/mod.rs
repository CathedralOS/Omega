//! Filesystem custody shared by local snapshots and Git cache entries.
//!
//! Read downward by responsibility: [`tree`] authenticates retained cache
//! trees, [`platform`] applies host ownership and ACL policy, [`lock`] owns
//! per-entry serialization, and [`publication`] owns private staging and
//! atomic publication.

pub(crate) mod lock;
pub(crate) mod platform;
pub(crate) mod publication;
pub(crate) mod tree;

#[cfg(test)]
mod tests;
