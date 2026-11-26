// In-process Git object-graph parsing, SHA-1/SHA-256 cryptographic
// authentication, selective member projection, and bounded `cat-file --batch`
// transfer.

pub(crate) mod authentication;
pub(crate) mod batch;
pub(crate) mod identity;
mod inspection;
mod object_rows;
mod projection;
pub(crate) mod tree;

use self::authentication::authenticate_git_tree_graph;
use crate::error::SourceResolveError;
pub(in crate::git) use batch::{
    ExactGitObjectAvailability, ExactGitObjectKind, probe_exact_git_object,
};
pub(crate) use inspection::{
    AuthenticatedGitTreeInspection, inspect_git_tree, inspect_git_tree_graph,
};
pub(crate) use object_rows::{GitBlobBytes, GitTreeEntry, GitTreeEntryKind};
pub(crate) use projection::GitTreeProjectionRequest;

#[cfg(test)]
mod tests;

// Authenticated, payload-free view (`AuthenticatedGitTreeGraph`) of one
// complete recursive Git tree graph, supporting binary-search lookup by exact
// slash-separated path bytes.

#[derive(Debug)]
struct AuthenticatedGitTreeGraph {
    root_tree_oid: String,
    entries: Vec<GitTreeEntry>,
}

impl AuthenticatedGitTreeGraph {
    pub(super) fn authenticate(
        root_tree_oid: &str,
        entries: Vec<GitTreeEntry>,
    ) -> Result<Self, SourceResolveError> {
        authenticate_git_tree_graph(root_tree_oid, &entries)?;
        Ok(Self {
            root_tree_oid: root_tree_oid.to_owned(),
            entries,
        })
    }

    pub(super) fn root_tree_oid(&self) -> &str {
        &self.root_tree_oid
    }

    pub(super) fn entries(&self) -> &[GitTreeEntry] {
        &self.entries
    }

    pub(super) fn entry(&self, exact_path: &[u8]) -> Option<&GitTreeEntry> {
        self.entries
            .binary_search_by(|entry| entry.relative_bytes.as_slice().cmp(exact_path))
            .ok()
            .map(|index| &self.entries[index])
    }
}
