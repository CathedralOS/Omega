//! In-memory rows shared by Git graph authentication and materialization.

use std::path::PathBuf;
use std::sync::Arc;

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
