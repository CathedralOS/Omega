//! Git repository cache creation, retained custody, and invalidation.
//!
//! Read downward by responsibility: [`creation`] constructs canonical cache
//! entries, [`repository`] exposes verified retained repositories,
//! [`snapshots`] retains immutable snapshot collections, [`custody`] proves
//! repository-tree custody, [`invalidation`] disables rejected entries, and
//! [`identity`] defines stable cache records and diagnostics.

pub(crate) mod configuration;
pub(crate) mod creation;
pub(crate) mod custody;
pub(crate) mod identity;
pub(crate) mod invalidation;
pub(crate) mod repository;
pub(crate) mod snapshots;
