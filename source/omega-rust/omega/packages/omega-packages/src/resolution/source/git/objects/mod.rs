//! Git object parsing, graph authentication, and bounded blob transfer.

mod authentication;
mod batch;
mod identity;
mod tree;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;

#[allow(unused_imports)]
pub(in crate::resolution::source) use authentication::{
    authenticate_git_commit, authenticate_git_commit_payload, authenticate_git_tree,
    verify_exact_git_revision,
};
#[cfg(test)]
pub(in crate::resolution::source) use batch::read_git_blobs_batch_from_path;
// Preserve the former module's crate-internal facade; several consumers are test-only.
#[allow(unused_imports)]
pub(in crate::resolution::source) use batch::{
    PendingGitBatchRequest, assign_git_batch_output, git_batch_output_limit,
};
#[allow(unused_imports)]
pub(in crate::resolution::source) use identity::{
    finalize_checked_sha1, git_object_algorithm, git_object_identity, git_object_invalid,
    hex_digit, is_object_id, verify_git_object_identity,
};
#[allow(unused_imports)]
pub(in crate::resolution::source) use tree::{
    git_directory_paths, git_tree_invalid, parse_git_tree_entries, validate_git_symlink_target,
};

use crate::resolution::source::error::SourceResolveError;
use crate::resolution::source::git::cache::{VerifiedGitRepository, cache_invalid};
use crate::resolution::source::git::execution::GitExecutor;
use crate::resolution::source::limits::LocalSourceLimits;

use self::batch::read_git_blobs_batch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolution::source) struct GitTreeEntry {
    pub(in crate::resolution::source) relative_bytes: Vec<u8>,
    pub(in crate::resolution::source) relative_path: PathBuf,
    pub(in crate::resolution::source) oid: String,
    pub(in crate::resolution::source) size: u64,
    pub(in crate::resolution::source) kind: GitTreeEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolution::source) enum GitTreeEntryKind {
    Tree,
    File {
        executable: bool,
        bytes: GitBlobBytes,
    },
    Symlink {
        target_bytes: GitBlobBytes,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolution::source) struct GitBlobBytes {
    pub(in crate::resolution::source) batch: Arc<Vec<u8>>,
    pub(in crate::resolution::source) start: usize,
    pub(in crate::resolution::source) end: usize,
}

impl GitBlobBytes {
    pub(in crate::resolution::source) fn empty() -> Self {
        Self {
            batch: Arc::new(Vec::new()),
            start: 0,
            end: 0,
        }
    }

    pub(in crate::resolution::source) fn as_slice(&self) -> &[u8] {
        &self.batch[self.start..self.end]
    }
}

pub(in crate::resolution::source) fn inspect_git_tree(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    limits: LocalSourceLimits,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    if !is_object_id(tree) {
        return Err(cache_invalid(
            repository.path(),
            "Git returned an invalid tree object ID",
        ));
    }
    let listing = repository.run_git_bytes_stdout(
        executor,
        [
            OsStr::new("ls-tree"),
            OsStr::new("--full-tree"),
            OsStr::new("-r"),
            OsStr::new("-t"),
            OsStr::new("-l"),
            OsStr::new("-z"),
            OsStr::new(tree),
        ],
    )?;
    let mut entries = parse_git_tree_entries(&listing, repository.path(), limits)?;
    read_git_blobs_batch(executor, repository, &mut entries, limits)?;
    Ok(entries)
}
