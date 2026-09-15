//! Canonical filesystem metadata rows and their index.

use crate::filesystem::FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT;

/// Version of Psi's canonical immutable-source metadata policy.
pub const CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION: u32 = 1;

/// Maximum complete source-tree rows accepted by the canonical metadata
/// carrier. One row is reserved for the source root; the remaining 65,536
/// match the package resolver's source-entry ceiling.
pub const CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT: usize = 65_537;

/// Whether raw bytes are one canonical slash-separated source-tree coordinate.
///
/// Unlike runtime rooted-path evidence, the complete source index deliberately
/// does not require UTF-8. Source custody preserves otherwise valid raw Unix
/// names even when package code cannot express those names through Psi's
/// target-neutral runtime path gate.
pub fn canonical_filesystem_metadata_path_is_canonical(relative: &[u8], allow_empty: bool) -> bool {
    if relative.len() > FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT
        || relative.contains(&0)
        || relative.contains(&b'\\')
    {
        return false;
    }
    if relative.is_empty() {
        return allow_empty;
    }
    if relative[0] == b'/' {
        return false;
    }
    !relative
        .split(|byte| *byte == b'/')
        .any(|component| component.is_empty() || component == b"." || component == b"..")
}

/// Closed source entry shape from which package-visible metadata is derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalFilesystemMetadataRowKind {
    Directory,
    File {
        executable: bool,
        logical_byte_length: u64,
    },
    Symlink {
        target_spelling_logical_byte_length: u64,
    },
}

impl CanonicalFilesystemMetadataRowKind {
    pub const fn logical_byte_length(self) -> u64 {
        match self {
            Self::Directory => 0,
            Self::File {
                logical_byte_length,
                ..
            } => logical_byte_length,
            Self::Symlink {
                target_spelling_logical_byte_length,
            } => target_spelling_logical_byte_length,
        }
    }
}

/// One raw root-relative row in a canonical immutable-source metadata index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFilesystemMetadataRow {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) kind: CanonicalFilesystemMetadataRowKind,
}

impl CanonicalFilesystemMetadataRow {
    pub fn new(
        relative_path: impl Into<Vec<u8>>,
        kind: CanonicalFilesystemMetadataRowKind,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            kind,
        }
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn kind(&self) -> CanonicalFilesystemMetadataRowKind {
        self.kind
    }
}

/// Why compiler-supplied immutable-source metadata is not canonical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalFilesystemMetadataIndexError {
    UnsupportedPolicyVersion(u32),
    RowLimitExceeded { limit: usize, attempted: usize },
    InvalidRelativePath(Vec<u8>),
    DuplicateRelativePath(Vec<u8>),
    AggregatePathBytesLimitExceeded { limit: usize, attempted: usize },
    LogicalByteLengthExceedsI64(Vec<u8>),
    MissingRootDirectory,
    RootIsNotDirectory,
    MissingParentDirectory(Vec<u8>),
    ParentIsNotDirectory(Vec<u8>),
}

impl std::fmt::Display for CanonicalFilesystemMetadataIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPolicyVersion(version) => {
                write!(
                    formatter,
                    "unsupported canonical filesystem metadata policy {version}"
                )
            }
            Self::RowLimitExceeded { limit, attempted } => write!(
                formatter,
                "canonical filesystem metadata rows exceed {limit}: attempted {attempted}"
            ),
            Self::InvalidRelativePath(path) => write!(
                formatter,
                "canonical filesystem metadata contains an invalid relative path: {path:?}"
            ),
            Self::DuplicateRelativePath(path) => write!(
                formatter,
                "canonical filesystem metadata duplicates relative path: {path:?}"
            ),
            Self::AggregatePathBytesLimitExceeded { limit, attempted } => write!(
                formatter,
                "canonical filesystem metadata path bytes exceed {limit}: attempted {attempted}"
            ),
            Self::LogicalByteLengthExceedsI64(path) => write!(
                formatter,
                "canonical filesystem metadata length does not fit i64 at path: {path:?}"
            ),
            Self::MissingRootDirectory => {
                write!(
                    formatter,
                    "canonical filesystem metadata omits the root directory"
                )
            }
            Self::RootIsNotDirectory => {
                write!(
                    formatter,
                    "canonical filesystem metadata root is not a directory"
                )
            }
            Self::MissingParentDirectory(path) => write!(
                formatter,
                "canonical filesystem metadata omits a parent directory for path: {path:?}"
            ),
            Self::ParentIsNotDirectory(path) => write!(
                formatter,
                "canonical filesystem metadata parent is not a directory for path: {path:?}"
            ),
        }
    }
}

impl std::error::Error for CanonicalFilesystemMetadataIndexError {}

/// Immutable, validated metadata for one complete content-authenticated source.
///
/// The source-content commitment is deliberately opaque to Psi. The package
/// resolver owns its construction and the compiler binds it to the source
/// identity; the interpreter only enforces the closed metadata policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalFilesystemMetadataIndex {
    pub(crate) policy_version: u32,
    pub(crate) source_content_commitment: [u8; 32],
    pub(crate) rows: std::collections::BTreeMap<Vec<u8>, CanonicalFilesystemMetadataRowKind>,
}

impl CanonicalFilesystemMetadataIndex {
    pub fn version_1(
        source_content_commitment: [u8; 32],
        rows: impl IntoIterator<Item = CanonicalFilesystemMetadataRow>,
    ) -> Result<Self, CanonicalFilesystemMetadataIndexError> {
        Self::new(
            CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION,
            source_content_commitment,
            rows,
        )
    }

    pub fn new(
        policy_version: u32,
        source_content_commitment: [u8; 32],
        rows: impl IntoIterator<Item = CanonicalFilesystemMetadataRow>,
    ) -> Result<Self, CanonicalFilesystemMetadataIndexError> {
        if policy_version != CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION {
            return Err(
                CanonicalFilesystemMetadataIndexError::UnsupportedPolicyVersion(policy_version),
            );
        }
        let mut total_path_bytes = 0usize;
        let mut canonical_rows = std::collections::BTreeMap::new();
        for (row_index, row) in rows.into_iter().enumerate() {
            if row_index >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
                return Err(CanonicalFilesystemMetadataIndexError::RowLimitExceeded {
                    limit: CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
                    attempted: row_index.saturating_add(1),
                });
            }
            if !canonical_filesystem_metadata_path_is_canonical(&row.relative_path, true) {
                return Err(CanonicalFilesystemMetadataIndexError::InvalidRelativePath(
                    row.relative_path,
                ));
            }
            total_path_bytes = total_path_bytes
                .checked_add(row.relative_path.len())
                .filter(|total| *total <= FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT)
                .ok_or(
                    CanonicalFilesystemMetadataIndexError::AggregatePathBytesLimitExceeded {
                        limit: FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT,
                        attempted: total_path_bytes.saturating_add(row.relative_path.len()),
                    },
                )?;
            if row.kind.logical_byte_length() > i64::MAX as u64 {
                return Err(
                    CanonicalFilesystemMetadataIndexError::LogicalByteLengthExceedsI64(
                        row.relative_path,
                    ),
                );
            }
            if canonical_rows
                .insert(row.relative_path.clone(), row.kind)
                .is_some()
            {
                return Err(
                    CanonicalFilesystemMetadataIndexError::DuplicateRelativePath(row.relative_path),
                );
            }
        }
        match canonical_rows.get(b"".as_slice()) {
            None => return Err(CanonicalFilesystemMetadataIndexError::MissingRootDirectory),
            Some(CanonicalFilesystemMetadataRowKind::Directory) => {}
            Some(_) => return Err(CanonicalFilesystemMetadataIndexError::RootIsNotDirectory),
        }
        for path in canonical_rows.keys().filter(|path| !path.is_empty()) {
            let parent = path
                .iter()
                .rposition(|byte| *byte == b'/')
                .map_or(b"".as_slice(), |separator| &path[..separator]);
            match canonical_rows.get(parent) {
                None => {
                    return Err(
                        CanonicalFilesystemMetadataIndexError::MissingParentDirectory(path.clone()),
                    );
                }
                Some(CanonicalFilesystemMetadataRowKind::Directory) => {}
                Some(_) => {
                    return Err(CanonicalFilesystemMetadataIndexError::ParentIsNotDirectory(
                        path.clone(),
                    ));
                }
            }
        }
        Ok(Self {
            policy_version,
            source_content_commitment,
            rows: canonical_rows,
        })
    }

    pub const fn policy_version(&self) -> u32 {
        self.policy_version
    }

    pub const fn source_content_commitment(&self) -> &[u8; 32] {
        &self.source_content_commitment
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = CanonicalFilesystemMetadataRow> + '_ {
        self.rows.iter().map(|(relative_path, kind)| {
            CanonicalFilesystemMetadataRow::new(relative_path.clone(), *kind)
        })
    }

    pub(crate) fn row(&self, relative_path: &[u8]) -> Option<CanonicalFilesystemMetadataRowKind> {
        self.rows.get(relative_path).copied()
    }
}
