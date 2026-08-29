//! Git object parsing, authentication, and selective bounded transfer.

pub(crate) mod authentication;
pub(crate) mod batch;
// The selective API is deliberately landed before its resolve-layer caller.
#[allow(dead_code)]
mod graph;
pub(crate) mod identity;
#[allow(dead_code)]
mod inspection;
mod model;
#[allow(dead_code)]
mod projection;
pub(crate) mod tree;

pub(crate) use inspection::inspect_git_tree;
#[allow(unused_imports)]
pub(crate) use inspection::inspect_git_tree_projection;
pub(crate) use model::{GitBlobBytes, GitTreeEntry, GitTreeEntryKind};
#[allow(unused_imports)]
pub(crate) use projection::{
    AuthenticatedGitMemberTree, AuthenticatedGitTreeProjection, GitTreeProjectionRequest,
};
