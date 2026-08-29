//! Git repository cache creation, retained custody, and invalidation.
//!
//! Read downward by responsibility: [`creation`] constructs canonical cache
//! entries, [`repository`] exposes verified retained repositories,
//! [`snapshots`] retains immutable snapshot collections, [`custody`] proves
//! repository-tree custody, [`invalidation`] disables rejected entries, and
//! [`identity`] defines stable cache records and diagnostics.

pub(in crate::source) mod creation;
pub(in crate::source) mod custody;
pub(in crate::source) mod identity;
pub(in crate::source) mod invalidation;
pub(in crate::source) mod repository;
pub(in crate::source) mod snapshots;
