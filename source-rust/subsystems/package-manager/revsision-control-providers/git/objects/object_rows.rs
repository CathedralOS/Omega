// In-memory tree entry rows (`GitTreeEntry`, `GitTreeEntryKind`) and shared
// batch-slice payloads (`GitBlobBytes`) used across Git graph authentication
// and snapshot materialization.
//
// `Gitlink` represents an authenticated parent-tree submodule edge that is
// never fetched or materialized. `validate_blob_entry` rejects `Gitlink`
// entries during blob transport, while `validate_source_entry` additionally
// rejects `.gitmodules` manifests when projecting package source trees.

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
    Gitlink,
    File {
        executable: bool,
        bytes: GitBlobBytes,
    },
    Symlink {
        target_bytes: GitBlobBytes,
    },
}

impl GitTreeEntryKind {
    pub(crate) fn carries_blob(&self) -> bool {
        matches!(self, Self::File { .. } | Self::Symlink { .. })
    }
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

impl GitTreeEntry {
    pub(crate) fn validate_blob_entry(&self) -> Result<(), crate::error::SourceResolveError> {
        if let GitTreeEntryKind::Gitlink = self.kind {
            return Err(crate::error::SourceResolveError::GitSubmodulesUnsupported {
                edges: vec![crate::error::GitSubmoduleEdge {
                    path: self.relative_path.clone(),
                    commit: Some(self.oid.clone()),
                }],
            });
        }
        Ok(())
    }

    pub(crate) fn validate_source_entry(&self) -> Result<(), crate::error::SourceResolveError> {
        self.validate_blob_entry()?;
        if self
            .relative_bytes
            .split(|byte| *byte == b'/')
            .any(|component| component.eq_ignore_ascii_case(b".gitmodules"))
        {
            return Err(crate::error::SourceResolveError::GitSubmodulesUnsupported {
                edges: vec![crate::error::GitSubmoduleEdge {
                    path: self.relative_path.clone(),
                    commit: None,
                }],
            });
        }
        Ok(())
    }
}
