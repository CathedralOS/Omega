//! Resolver-neutral source-tree traversal and canonical tree results.
//!
//! [`capture`] owns bounded, no-follow traversal. [`model`] carries the exact
//! resolved tree and verified entries shared by local and Git acquisition.

pub(crate) mod capture;
pub(crate) mod filesystem;
pub(crate) mod identity;
mod resolved_tree;

pub use resolved_tree::{
    ResolvedLocalSource, ResolvedSourceTree, VerifiedPackageSourceEntry,
    VerifiedPackageSourceEntryKind,
};
