//! Captured immutable build-source input inventory.
//!
//! A [`CapturedBuildSourceInput`] is the complete coherent inventory of one
//! source root captured by the compiler before a build occurrence runs: every
//! directory, every regular file's exact bytes, and every relative symlink's
//! inert captured spelling, bound to the same canonical source metadata index
//! that narrows the build's scoped filesystem reads.
//!
//! The inventory is immutable compiler custody. Package code never receives
//! this value; it executes against a strict non-following view of this retained
//! storage. Source reads have no physical spelling or materialized directory.

use diagnostics::Diagnostic;
use filesystem_authority::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRowKind,
    canonical_filesystem_metadata_path_is_canonical,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// One compiler-captured immutable source-tree entry.
///
/// File entries retain the exact captured bytes and their content digest, which
/// is the inventory's identity for this entry. Symlink entries retain only the
/// inert captured target spelling; captured links are never materialized or
/// traversed.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum CapturedSourceEntry {
    Directory,
    File {
        bytes: Arc<[u8]>,
        digest: [u8; 32],
        executable: bool,
    },
    Symlink {
        target: Arc<[u8]>,
        digest: [u8; 32],
    },
}

impl CapturedSourceEntry {
    /// Retain one regular file's exact captured bytes, deriving the content
    /// digest at the same capture boundary so callers cannot substitute a
    /// mismatched digest.
    pub fn file(bytes: impl Into<Arc<[u8]>>, executable: bool) -> Self {
        let bytes = bytes.into();
        Self::File {
            digest: crate::digest_encoder::content_identity(bytes.as_ref()).into(),
            bytes,
            executable,
        }
    }

    /// Retain one inert captured symlink spelling. The bytes are evidence,
    /// never a path the build follows.
    pub fn symlink(target: impl Into<Arc<[u8]>>) -> Self {
        let target = target.into();
        Self::Symlink {
            digest: crate::digest_encoder::content_identity(target.as_ref()).into(),
            target,
        }
    }

    /// The captured content digest for byte-bearing entries.
    pub fn content_digest(&self) -> Option<[u8; 32]> {
        match self {
            Self::File { digest, .. } | Self::Symlink { digest, .. } => Some(*digest),
            Self::Directory => None,
        }
    }

    fn matches_row_kind(&self, kind: CanonicalFilesystemMetadataRowKind) -> bool {
        match (self, kind) {
            (Self::Directory, CanonicalFilesystemMetadataRowKind::Directory) => true,
            (
                Self::File {
                    bytes, executable, ..
                },
                CanonicalFilesystemMetadataRowKind::File {
                    executable: row_executable,
                    logical_byte_length,
                },
            ) => *executable == row_executable && bytes.len() as u64 == logical_byte_length,
            (
                Self::Symlink { target, .. },
                CanonicalFilesystemMetadataRowKind::Symlink {
                    target_spelling_logical_byte_length,
                },
            ) => target.len() as u64 == target_spelling_logical_byte_length,
            _ => false,
        }
    }
}

/// Logical kind-and-length view of one captured source entry, matching the
/// metadata the build snapshot protocol may expose. It deliberately carries
/// no timestamp, inode, host mode, or device identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedSourceEntryKind<'a> {
    Directory,
    File { byte_length: u64, executable: bool },
    Symlink { target: &'a [u8] },
}

/// Borrowed view of one captured source entry and its canonical relative path.
#[derive(Debug, Clone, Copy)]
pub struct CapturedSourceEntryRef<'a> {
    relative_path: &'a [u8],
    kind: CapturedSourceEntryKind<'a>,
}

impl<'a> CapturedSourceEntryRef<'a> {
    pub fn relative_path(&self) -> &'a [u8] {
        self.relative_path
    }

    pub const fn kind(&self) -> CapturedSourceEntryKind<'a> {
        self.kind
    }
}

/// Borrowed regular-file view: exact captured bytes and executable class.
#[derive(Debug, Clone, Copy)]
pub struct CapturedSourceFile<'a> {
    bytes: &'a [u8],
    executable: bool,
}

impl<'a> CapturedSourceFile<'a> {
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub const fn executable(&self) -> bool {
        self.executable
    }
}

/// Complete immutable input inventory captured by the compiler for one build
/// occurrence.
///
/// Construction is restricted to compiler capture authorities
/// (`package-compilation` walks the physical root exactly once and passes the
/// retained rows here); package-controlled data cannot fabricate an inventory
/// that disagrees with the canonical metadata index bound into the build's
/// filesystem grants.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CapturedBuildSourceInput {
    canonical_source_metadata: Arc<CanonicalFilesystemMetadataIndex>,
    entries: Arc<BTreeMap<Vec<u8>, CapturedSourceEntry>>,
    file_bytes: u64,
}

impl CapturedBuildSourceInput {
    /// Assemble a captured inventory from one capture authority's retained
    /// entries. Every non-root canonical metadata row must be covered by an
    /// entry of the same kind and logical extent, and every entry must name a
    /// canonical non-root path present in the index.
    #[doc(hidden)]
    pub fn from_capture_rows(
        canonical_source_metadata: CanonicalFilesystemMetadataIndex,
        entries: impl IntoIterator<Item = (Vec<u8>, CapturedSourceEntry)>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let rows = canonical_source_metadata
            .rows()
            .map(|row| (row.relative_path().to_vec(), row.kind()))
            .collect::<BTreeMap<_, _>>();
        let mut retained = BTreeMap::<Vec<u8>, CapturedSourceEntry>::new();
        let mut file_bytes = 0u64;
        for (relative_path, entry) in entries {
            if relative_path.is_empty()
                || !canonical_filesystem_metadata_path_is_canonical(&relative_path, false)
            {
                return Err(diagnostics(format!(
                    "captured build input contains a noncanonical relative path: {relative_path:?}"
                )));
            }
            let Some(row_kind) = rows.get(&relative_path) else {
                return Err(diagnostics(format!(
                    "captured build input entry `{}` is absent from its canonical source metadata index",
                    String::from_utf8_lossy(&relative_path)
                )));
            };
            if !entry.matches_row_kind(*row_kind) {
                return Err(diagnostics(format!(
                    "captured build input entry `{}` disagrees with its canonical source metadata kind or extent",
                    String::from_utf8_lossy(&relative_path)
                )));
            }
            if let CapturedSourceEntry::File { bytes, .. } = &entry {
                file_bytes = file_bytes
                    .checked_add(bytes.len() as u64)
                    .ok_or_else(|| diagnostics("captured build input file bytes exceed u64"))?;
            }
            if retained.insert(relative_path.clone(), entry).is_some() {
                return Err(diagnostics(format!(
                    "captured build input duplicates relative path: {relative_path:?}"
                )));
            }
        }
        for (relative_path, kind) in rows.iter().filter(|(path, _)| !path.is_empty()) {
            if !retained.contains_key(relative_path) {
                let kind_name = match kind {
                    CanonicalFilesystemMetadataRowKind::Directory => "directory",
                    CanonicalFilesystemMetadataRowKind::File { .. } => "file",
                    CanonicalFilesystemMetadataRowKind::Symlink { .. } => "symlink",
                };
                return Err(diagnostics(format!(
                    "captured build input omits canonical source {kind_name} `{}`",
                    String::from_utf8_lossy(relative_path)
                )));
            }
        }
        Ok(Self {
            canonical_source_metadata: Arc::new(canonical_source_metadata),
            entries: Arc::new(retained),
            file_bytes,
        })
    }

    /// The canonical metadata index bound into the build's scoped filesystem
    /// reads. The inventory's content commitment is this index's source
    /// commitment; host path spellings and capture order are unobservable.
    pub fn canonical_source_metadata(&self) -> &CanonicalFilesystemMetadataIndex {
        &self.canonical_source_metadata
    }

    /// Complete inventory size in canonical rows, including the source root.
    pub fn entry_count(&self) -> u64 {
        self.canonical_source_metadata.rows().len() as u64
    }

    /// Total retained regular-file bytes in the inventory.
    pub const fn file_bytes(&self) -> u64 {
        self.file_bytes
    }

    /// Whether every retained entry is byte-for-byte identical to the same
    /// entry of another captured inventory. Counts, lengths, and matching
    /// metadata kinds alone cannot establish that a narrowed view came from
    /// the package's authenticated source capture.
    pub fn is_subset_of(&self, complete: &Self) -> bool {
        self.entries
            .iter()
            .all(|(path, entry)| complete.entries.get(path) == Some(entry))
    }

    /// List every captured non-root entry in canonical unsigned-byte order.
    /// The listing is deterministic and independent of host directory order.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = CapturedSourceEntryRef<'_>> {
        self.entries.iter().map(|(relative_path, entry)| {
            let kind = match entry {
                CapturedSourceEntry::Directory => CapturedSourceEntryKind::Directory,
                CapturedSourceEntry::File {
                    bytes, executable, ..
                } => CapturedSourceEntryKind::File {
                    byte_length: bytes.len() as u64,
                    executable: *executable,
                },
                CapturedSourceEntry::Symlink { target, .. } => {
                    CapturedSourceEntryKind::Symlink { target }
                }
            };
            CapturedSourceEntryRef {
                relative_path,
                kind,
            }
        })
    }

    /// Borrow an exact retained entry without following links or reopening host data.
    pub fn retained_entry(&self, path: &[u8]) -> Option<&CapturedSourceEntry> {
        self.entries.get(path)
    }

    /// List retained entries without copying inventory models or their payloads.
    pub fn retained_entries(&self) -> impl ExactSizeIterator<Item = (&[u8], &CapturedSourceEntry)> {
        self.entries
            .iter()
            .map(|(relative_path, entry)| (relative_path.as_slice(), entry))
    }

    /// Look up one captured regular file by canonical relative path without
    /// following any link. Returns `None` for absent paths and non-file kinds.
    pub fn file(&self, relative_path: &[u8]) -> Option<CapturedSourceFile<'_>> {
        match self.entries.get(relative_path) {
            Some(CapturedSourceEntry::File {
                bytes, executable, ..
            }) => Some(CapturedSourceFile {
                bytes,
                executable: *executable,
            }),
            _ => None,
        }
    }
}

fn diagnostics(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message.into())]
}

#[cfg(test)]
mod tests;
