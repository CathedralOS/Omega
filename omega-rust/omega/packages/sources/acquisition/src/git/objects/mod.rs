//! Git object parsing, authentication, and selective bounded transfer.

pub(crate) mod authentication;
pub(crate) mod batch;
mod graph;
pub(crate) mod identity;
mod inspection;
mod model;
mod projection;
pub(crate) mod tree;

pub(in crate::git) use batch::{
    ExactGitObjectAvailability, ExactGitObjectKind, probe_exact_git_object,
};
pub(crate) use inspection::{inspect_git_tree, inspect_git_tree_graph};
pub(crate) use model::{GitBlobBytes, GitTreeEntry, GitTreeEntryKind};
pub(crate) use projection::GitTreeProjectionRequest;

#[cfg(test)]
mod tests;
