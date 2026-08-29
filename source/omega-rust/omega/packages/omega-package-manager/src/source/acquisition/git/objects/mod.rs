//! Git object parsing, graph authentication, and bounded blob transfer.

mod authentication;
mod batch;
mod identity;
mod tree;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;

#[allow(unused_imports)]
pub(in crate::source::acquisition) use authentication::{
    authenticate_git_commit, authenticate_git_commit_payload, authenticate_git_tree,
    verify_exact_git_revision,
};
#[cfg(test)]
pub(in crate::source::acquisition) use batch::read_git_blobs_batch_from_path;
// Preserve the former module's crate-internal facade; several consumers are test-only.
#[allow(unused_imports)]
pub(in crate::source::acquisition) use batch::{
    PendingGitBatchRequest, assign_git_batch_output, git_batch_output_limit,
};
#[allow(unused_imports)]
pub(in crate::source::acquisition) use identity::{
    finalize_checked_sha1, git_object_algorithm, git_object_identity, git_object_invalid,
    hex_digit, is_object_id, verify_git_object_identity,
};
#[allow(unused_imports)]
pub(in crate::source::acquisition) use tree::{
    git_directory_paths, git_tree_invalid, parse_git_tree_entries, validate_git_symlink_target,
};

use crate::source::acquisition::error::SourceResolveError;
use crate::source::acquisition::git::cache::{VerifiedGitRepository, cache_invalid};
use crate::source::acquisition::git::execution::GitExecutor;
use crate::source::acquisition::limits::LocalSourceLimits;

use self::batch::read_git_blobs_batch;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::source::acquisition) struct GitTreeEntry {
    pub(in crate::source::acquisition) relative_bytes: Vec<u8>,
    pub(in crate::source::acquisition) relative_path: PathBuf,
    pub(in crate::source::acquisition) oid: String,
    pub(in crate::source::acquisition) size: u64,
    pub(in crate::source::acquisition) kind: GitTreeEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::source::acquisition) enum GitTreeEntryKind {
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
pub(in crate::source::acquisition) struct GitBlobBytes {
    pub(in crate::source::acquisition) batch: Arc<Vec<u8>>,
    pub(in crate::source::acquisition) start: usize,
    pub(in crate::source::acquisition) end: usize,
}

impl GitBlobBytes {
    pub(in crate::source::acquisition) fn empty() -> Self {
        Self {
            batch: Arc::new(Vec::new()),
            start: 0,
            end: 0,
        }
    }

    pub(in crate::source::acquisition) fn as_slice(&self) -> &[u8] {
        &self.batch[self.start..self.end]
    }
}

pub(in crate::source::acquisition) fn inspect_git_tree(
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
