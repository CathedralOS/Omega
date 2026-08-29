//! Filesystem custody shared by local snapshots and Git cache entries.
//!
//! Read downward by responsibility: [`tree`] authenticates retained cache
//! trees, [`platform`] applies host ownership and ACL policy, [`lock`] owns
//! per-entry serialization, and [`publication`] owns private staging and
//! atomic publication.

use super::*;

mod lock;
mod platform;
mod publication;
mod tree;

pub(super) use lock::*;
pub(super) use platform::*;
pub(super) use publication::*;
pub(super) use tree::*;
