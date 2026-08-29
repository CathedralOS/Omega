//! Git repository cache creation, retained custody, and invalidation.
//!
//! Read downward by responsibility: [`creation`] constructs canonical cache
//! entries, [`repository`] exposes verified retained repositories,
//! [`snapshots`] retains immutable snapshot collections, [`custody`] proves
//! repository-tree custody, [`invalidation`] disables rejected entries, and
//! [`identity`] defines stable cache records and diagnostics.

mod creation;
mod custody;
mod identity;
mod invalidation;
mod repository;
mod snapshots;

pub(in crate::source) use creation::create_git_cache_entry;
#[allow(unused_imports)]
pub(in crate::source) use creation::parse_git_remote_object_format;
pub(in crate::source) use identity::{
    append_framed_bytes, cache_invalid, git_cache_identity, git_cache_metadata,
    local_snapshot_invalid,
};
pub(in crate::source) use invalidation::invalidate_git_cache_entry_from_open_parent;
#[cfg(test)]
pub(in crate::source) use invalidation::invalidate_git_cache_entry_from_retained_parent;
pub(in crate::source) use repository::VerifiedGitRepository;
pub(in crate::source) use snapshots::RetainedGitSnapshots;
