//! Git object parsing, graph authentication, and bounded blob transfer.

pub(in crate::source) mod authentication;
pub(in crate::source) mod batch;
pub(in crate::source) mod identity;
pub(in crate::source) mod tree;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;

use crate::source::error::SourceResolveError;
use crate::source::git::cache::identity::cache_invalid;
use crate::source::git::cache::repository::VerifiedGitRepository;
use crate::source::git::executable::executor::GitExecutor;
use crate::source::limits::LocalSourceLimits;

use self::batch::read_git_blobs_batch;
use self::identity::is_object_id;
use self::tree::parse_git_tree_entries;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::source) struct GitTreeEntry {
    pub(in crate::source) relative_bytes: Vec<u8>,
    pub(in crate::source) relative_path: PathBuf,
    pub(in crate::source) oid: String,
    pub(in crate::source) size: u64,
    pub(in crate::source) kind: GitTreeEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::source) enum GitTreeEntryKind {
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
pub(in crate::source) struct GitBlobBytes {
    pub(in crate::source) batch: Arc<Vec<u8>>,
    pub(in crate::source) start: usize,
    pub(in crate::source) end: usize,
}

impl GitBlobBytes {
    pub(in crate::source) fn empty() -> Self {
        Self {
            batch: Arc::new(Vec::new()),
            start: 0,
            end: 0,
        }
    }

    pub(in crate::source) fn as_slice(&self) -> &[u8] {
        &self.batch[self.start..self.end]
    }
}

pub(in crate::source) fn inspect_git_tree(
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
