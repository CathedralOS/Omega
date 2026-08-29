//! Git object parsing, graph authentication, and bounded blob transfer.

pub(crate) mod authentication;
pub(crate) mod batch;
pub(crate) mod identity;
pub(crate) mod tree;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Arc;

use crate::error::SourceResolveError;
use crate::git::cache::identity::cache_invalid;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::limits::LocalSourceLimits;

use self::batch::read_git_blobs_batch;
use self::identity::is_object_id;
use self::tree::parse_git_tree_entries;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitTreeEntry {
    pub(crate) relative_bytes: Vec<u8>,
    pub(crate) relative_path: PathBuf,
    pub(crate) oid: String,
    pub(crate) size: u64,
    pub(crate) kind: GitTreeEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitTreeEntryKind {
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
pub(crate) struct GitBlobBytes {
    pub(crate) batch: Arc<Vec<u8>>,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl GitBlobBytes {
    pub(crate) fn empty() -> Self {
        Self {
            batch: Arc::new(Vec::new()),
            start: 0,
            end: 0,
        }
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.batch[self.start..self.end]
    }
}

pub(crate) fn inspect_git_tree(
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
