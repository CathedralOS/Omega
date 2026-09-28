//! Authenticated, payload-free view of one complete recursive Git tree graph.

use crate::error::SourceResolveError;

use super::GitTreeEntry;
use super::authentication::authenticate_git_tree_graph;

#[derive(Debug)]
pub(super) struct AuthenticatedGitTreeGraph {
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
