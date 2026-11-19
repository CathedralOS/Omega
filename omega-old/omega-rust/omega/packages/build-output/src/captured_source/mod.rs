/*
Captured immutable build-source input inventory (`CapturedBuildSourceInput`).

We retain a complete, coherent snapshot of one source root captured before a
build activation runs: every directory, regular file's exact bytes + content
digest, and relative symlink's inert target spelling, verified against the
`CanonicalFilesystemMetadataIndex` that bounds the build's scoped filesystem
reads.

Why check `is_subset_of` by comparing exact `CapturedSourceEntry` values rather
than counts or lengths? Narrowed views handed to a build activation must prove
that every retained entry came byte-for-byte from the package's authenticated
source capture; matching paths, kinds, and lengths alone would allow substituted
file bytes.
*/

use diagnostics::Diagnostic;
use filesystem_authority::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRowKind,
    canonical_filesystem_metadata_path_is_canonical,
};
use std::collections::BTreeMap;
use std::sync::Arc;

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
    pub fn file(bytes: impl Into<Arc<[u8]>>, executable: bool) -> Self {
        let bytes = bytes.into();
        Self::File {
            digest: crate::digest_encoder::content_identity(bytes.as_ref()),
            bytes,
            executable,
        }
    }

    pub fn symlink(target: impl Into<Arc<[u8]>>) -> Self {
        let target = target.into();
        Self::Symlink {
            digest: crate::digest_encoder::content_identity(target.as_ref()),
            target,
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturedSourceEntryKind<'a> {
    Directory,
    File { byte_length: u64, executable: bool },
    Symlink { target: &'a [u8] },
}

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

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CapturedBuildSourceInput {
    canonical_source_metadata: Arc<CanonicalFilesystemMetadataIndex>,
    entries: Arc<BTreeMap<Vec<u8>, CapturedSourceEntry>>,
    file_bytes: u64,
}

impl CapturedBuildSourceInput {
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

    pub fn canonical_source_metadata(&self) -> &CanonicalFilesystemMetadataIndex {
        &self.canonical_source_metadata
    }

    pub fn entry_count(&self) -> u64 {
        self.canonical_source_metadata.rows().len() as u64
    }

    pub const fn file_bytes(&self) -> u64 {
        self.file_bytes
    }

    pub fn is_subset_of(&self, complete: &Self) -> bool {
        self.entries
            .iter()
            .all(|(path, entry)| complete.entries.get(path) == Some(entry))
    }

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

    pub fn retained_entry(&self, path: &[u8]) -> Option<&CapturedSourceEntry> {
        self.entries.get(path)
    }

    pub fn retained_entries(&self) -> impl ExactSizeIterator<Item = (&[u8], &CapturedSourceEntry)> {
        self.entries
            .iter()
            .map(|(relative_path, entry)| (relative_path.as_slice(), entry))
    }

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
