//! Git object parsing, authentication, and selective bounded transfer.

pub(crate) mod authentication;
pub(crate) mod batch;
mod graph;
pub(crate) mod identity;
mod inspection;
mod model;
mod projection;
pub(crate) mod tree;

#[cfg(test)]
pub(crate) use inspection::inspect_git_tree_projection;
pub(crate) use inspection::{inspect_git_tree, inspect_git_tree_graph};
pub(crate) use model::{GitBlobBytes, GitTreeEntry, GitTreeEntryKind};
pub(crate) use projection::GitTreeProjectionRequest;
