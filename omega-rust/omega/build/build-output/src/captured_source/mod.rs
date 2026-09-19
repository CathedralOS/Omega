//! Captured immutable build-source input inventory.
//!
//! A [`CapturedBuildSourceInput`] is the complete coherent inventory of one
//! source root captured by the compiler before a build occurrence runs: every
//! directory, every regular file's exact bytes, and every relative symlink's
//! inert captured spelling, bound to the same canonical source metadata index
//! that narrows the build's scoped filesystem reads.
//!
//! The inventory is immutable compiler custody. Package code never receives
//! this value; it executes against a fresh private materialization of the
//! inventory, so a build cannot observe or depend on host path spellings,
//! checkout location, directory iteration order, or post-capture mutation.

use checked_interpreter::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRowKind,
    canonical_filesystem_metadata_path_is_canonical,
};
use diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One compiler-captured immutable source-tree entry.
///
/// File entries retain the exact captured bytes and their content digest so a
/// private materialization can be independently re-inspected. Symlink entries
/// retain only the inert captured target spelling; captured links are never
/// materialized or traversed.
#[derive(Debug, Clone, PartialEq, Eq)]
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
            digest: Sha256::digest(bytes.as_ref()).into(),
            bytes,
            executable,
        }
    }

    /// Retain one inert captured symlink spelling. The bytes are evidence,
    /// never a path the build follows.
    pub fn symlink(target: impl Into<Arc<[u8]>>) -> Self {
        let target = target.into();
        Self::Symlink {
            digest: Sha256::digest(target.as_ref()).into(),
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedBuildSourceInput {
    canonical_source_metadata: CanonicalFilesystemMetadataIndex,
    entries: BTreeMap<Vec<u8>, CapturedSourceEntry>,
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
            canonical_source_metadata,
            entries: retained,
            file_bytes,
        })
    }

    /// The canonical metadata index bound into the build's scoped filesystem
    /// reads. The inventory's content commitment is this index's source
    /// commitment; host path spellings and capture order are unobservable.
    pub const fn canonical_source_metadata(&self) -> &CanonicalFilesystemMetadataIndex {
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

    /// Materialize this inventory into an existing empty concrete directory,
    /// then independently re-inspect the result.
    ///
    /// Only directories and regular files are materialized; captured symlink
    /// spellings remain inert evidence and produce no host link a build could
    /// traverse. Materialized directories are read-only (0o555 on Unix) and
    /// materialized files are read-only (0o444 ordinary, 0o555 executable), so
    /// the private backing can never become a write root.
    pub fn materialize_into(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<(), CapturedSourceMaterializationError> {
        materialize_captured_source(self, destination.as_ref())
    }
}

/// Failure to validate or materialize a captured source inventory into its
/// fresh private backing directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedSourceMaterializationError {
    message: String,
}

impl CapturedSourceMaterializationError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CapturedSourceMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CapturedSourceMaterializationError {}

fn materialize_captured_source(
    input: &CapturedBuildSourceInput,
    destination: &Path,
) -> Result<(), CapturedSourceMaterializationError> {
    let destination_metadata = std::fs::symlink_metadata(destination).map_err(|error| {
        materialization_error(format!(
            "cannot inspect captured-source backing `{}`: {error}",
            destination.display()
        ))
    })?;
    if !destination_metadata.is_dir() || destination_metadata.file_type().is_symlink() {
        return Err(materialization_error(format!(
            "captured-source backing `{}` is not a concrete directory",
            destination.display()
        )));
    }
    let mut listing = std::fs::read_dir(destination).map_err(|error| {
        materialization_error(format!(
            "cannot enumerate captured-source backing `{}`: {error}",
            destination.display()
        ))
    })?;
    if listing
        .next()
        .transpose()
        .map_err(|error| {
            materialization_error(format!(
                "cannot enumerate captured-source backing `{}`: {error}",
                destination.display()
            ))
        })?
        .is_some()
    {
        return Err(materialization_error(format!(
            "captured-source backing `{}` must be an empty directory",
            destination.display()
        )));
    }

    // Kind validation finishes before any write so a rejected inventory
    // leaves the backing untouched rather than partially materialized.
    for (relative_path, entry) in &input.entries {
        if let CapturedSourceEntry::File { executable, .. } = entry {
            validate_captured_executable_mode(relative_path, *executable)?;
        }
    }

    for (relative_path, entry) in &input.entries {
        let native = captured_native_path(destination, relative_path)?;
        match entry {
            CapturedSourceEntry::Directory => {
                // Directory sealing waits for the second pass: a sealed
                // directory rejects the creation of its later siblings.
                std::fs::create_dir(&native).map_err(|error| {
                    materialization_error(format!(
                        "cannot materialize captured source directory `{relative_path:?}`: {error}"
                    ))
                })?;
            }
            CapturedSourceEntry::File {
                bytes, executable, ..
            } => {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&native)
                    .map_err(|error| {
                        materialization_error(format!(
                            "cannot materialize captured source file `{relative_path:?}`: {error}"
                        ))
                    })?;
                std::io::Write::write_all(&mut file, bytes).map_err(|error| {
                    materialization_error(format!(
                        "cannot write captured source file `{relative_path:?}`: {error}"
                    ))
                })?;
                drop(file);
                set_captured_file_mode(&native, *executable)?;
            }
            CapturedSourceEntry::Symlink { .. } => {
                // Captured links are inert evidence; no host link is created.
            }
        }
    }
    for (relative_path, entry) in &input.entries {
        if let CapturedSourceEntry::Directory = entry {
            set_captured_dir_mode(&captured_native_path(destination, relative_path)?)?;
        }
    }
    verify_materialized_source(input, destination)
}

fn verify_materialized_source(
    input: &CapturedBuildSourceInput,
    destination: &Path,
) -> Result<(), CapturedSourceMaterializationError> {
    let expected = input
        .entries
        .iter()
        .filter(|(_, entry)| !matches!(entry, CapturedSourceEntry::Symlink { .. }))
        .count();
    let mut observed = 0usize;
    let mut stack = vec![(destination.to_path_buf(), Vec::<u8>::new())];
    while let Some((path, relative_path)) = stack.pop() {
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            materialization_error(format!(
                "cannot re-inspect materialized captured source `{relative_path:?}`: {error}"
            ))
        })?;
        if relative_path.is_empty() {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(materialization_error(
                    "materialized captured-source backing is not a concrete directory",
                ));
            }
        } else {
            observed = observed.checked_add(1).ok_or_else(|| {
                materialization_error("materialized captured source count overflow")
            })?;
            match input.entries.get(&relative_path) {
                Some(CapturedSourceEntry::Directory) if metadata.is_dir() => {}
                Some(CapturedSourceEntry::File {
                    digest, executable, ..
                }) if metadata.is_file() => {
                    verify_materialized_captured_file(
                        &path,
                        &relative_path,
                        &metadata,
                        *digest,
                        *executable,
                    )?;
                }
                Some(_) => {
                    return Err(materialization_error(format!(
                        "materialized captured source `{relative_path:?}` has the wrong kind"
                    )));
                }
                None => {
                    return Err(materialization_error(format!(
                        "materialized captured source contains unexpected entry `{relative_path:?}`"
                    )));
                }
            }
        }
        if metadata.is_dir() {
            let children = std::fs::read_dir(&path).map_err(|error| {
                materialization_error(format!(
                    "cannot enumerate materialized captured source `{relative_path:?}`: {error}"
                ))
            })?;
            for child in children {
                let child = child.map_err(|error| {
                    materialization_error(format!(
                        "cannot enumerate materialized captured source `{relative_path:?}`: {error}"
                    ))
                })?;
                let name = captured_os_str_bytes(&child.file_name()).ok_or_else(|| {
                    materialization_error(format!(
                        "materialized captured source name under `{relative_path:?}` is not portable"
                    ))
                })?;
                let mut child_relative = relative_path.clone();
                if !child_relative.is_empty() {
                    child_relative.push(b'/');
                }
                child_relative.extend_from_slice(&name);
                stack.push((child.path(), child_relative));
            }
        }
    }
    if observed != expected {
        return Err(materialization_error(format!(
            "materialized captured source has {observed} entries; expected {expected}"
        )));
    }
    Ok(())
}

fn verify_materialized_captured_file(
    path: &Path,
    relative_path: &[u8],
    metadata: &std::fs::Metadata,
    expected_digest: [u8; 32],
    expected_executable: bool,
) -> Result<(), CapturedSourceMaterializationError> {
    if captured_file_is_executable(metadata) != expected_executable {
        return Err(materialization_error(format!(
            "materialized captured source file `{relative_path:?}` has the wrong executable class"
        )));
    }
    let mut file = std::fs::File::open(path).map_err(|error| {
        materialization_error(format!(
            "cannot re-read materialized captured source file `{relative_path:?}`: {error}"
        ))
    })?;
    let mut digest = Sha256::new();
    let mut observed = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            materialization_error(format!(
                "cannot re-read materialized captured source file `{relative_path:?}`: {error}"
            ))
        })?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(count as u64)
            .filter(|observed| *observed <= metadata.len())
            .ok_or_else(|| {
                materialization_error(format!(
                    "materialized captured source file `{relative_path:?}` grew while verified"
                ))
            })?;
        digest.update(&buffer[..count]);
    }
    if observed != metadata.len() {
        return Err(materialization_error(format!(
            "materialized captured source file `{relative_path:?}` changed length while verified"
        )));
    }
    if <[u8; 32]>::from(digest.finalize()) != expected_digest {
        return Err(materialization_error(format!(
            "materialized captured source file `{relative_path:?}` content drifted"
        )));
    }
    Ok(())
}

/// Remove a materialized captured-source snapshot. Materialization seals
/// directories and files read-only (0o555/0o444 on Unix, the readonly
/// attribute elsewhere), so an ordinary recursive remove cannot unlink the
/// tree; every entry regains a writable mode first. The destination is
/// scratch custody — partial materializations and residue clear the same way.
pub fn discard_materialized_snapshot(
    destination: impl AsRef<Path>,
) -> Result<(), CapturedSourceMaterializationError> {
    let destination = destination.as_ref();
    match std::fs::symlink_metadata(destination) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(materialization_error(format!(
                "cannot inspect materialized captured source snapshot `{}`: {error}",
                destination.display()
            )));
        }
    }
    unseal_materialized_tree(destination)?;
    std::fs::remove_dir_all(destination).map_err(|error| {
        materialization_error(format!(
            "cannot discard materialized captured source snapshot `{}`: {error}",
            destination.display()
        ))
    })
}

fn unseal_materialized_tree(path: &Path) -> Result<(), CapturedSourceMaterializationError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        materialization_error(format!(
            "cannot inspect materialized captured source `{}`: {error}",
            path.display()
        ))
    })?;
    if metadata.is_dir() {
        unseal_captured_dir(path)?;
        let children = std::fs::read_dir(path).map_err(|error| {
            materialization_error(format!(
                "cannot enumerate materialized captured source `{}`: {error}",
                path.display()
            ))
        })?;
        for child in children {
            let child = child.map_err(|error| {
                materialization_error(format!(
                    "cannot enumerate materialized captured source `{}`: {error}",
                    path.display()
                ))
            })?;
            unseal_materialized_tree(&child.path())?;
        }
    } else {
        unseal_captured_file(path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn unseal_captured_dir(path: &Path) -> Result<(), CapturedSourceMaterializationError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|error| {
        materialization_error(format!(
            "cannot unseal materialized captured source directory `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn unseal_captured_dir(path: &Path) -> Result<(), CapturedSourceMaterializationError> {
    unseal_captured_file(path)
}

#[cfg(unix)]
fn unseal_captured_file(path: &Path) -> Result<(), CapturedSourceMaterializationError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).map_err(|error| {
        materialization_error(format!(
            "cannot unseal materialized captured source file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn unseal_captured_file(path: &Path) -> Result<(), CapturedSourceMaterializationError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|error| {
            materialization_error(format!(
                "cannot inspect materialized captured source file `{}`: {error}",
                path.display()
            ))
        })?
        .permissions();
    // This arm never compiles on Unix: the readonly attribute is the seal
    // here, so clearing it is exactly the unseal this host needs.
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        materialization_error(format!(
            "cannot unseal materialized captured source file `{}`: {error}",
            path.display()
        ))
    })
}

fn captured_native_path(
    destination: &Path,
    relative_path: &[u8],
) -> Result<PathBuf, CapturedSourceMaterializationError> {
    let mut native = destination.to_path_buf();
    for component in relative_path.split(|byte| *byte == b'/') {
        if component.is_empty() || component == b"." || component == b".." {
            return Err(materialization_error(format!(
                "captured source relative path is not canonical: {relative_path:?}"
            )));
        }
        native.push(captured_os_str(component).ok_or_else(|| {
            materialization_error(format!(
                "captured source name is not portable on this host: {relative_path:?}"
            ))
        })?);
    }
    Ok(native)
}

#[cfg(unix)]
fn captured_os_str(component: &[u8]) -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::OsStringExt;
    Some(std::ffi::OsString::from_vec(component.to_vec()))
}

#[cfg(not(unix))]
fn captured_os_str(component: &[u8]) -> Option<std::ffi::OsString> {
    std::str::from_utf8(component)
        .ok()
        .map(std::ffi::OsString::from)
}

#[cfg(unix)]
fn captured_os_str_bytes(name: &std::ffi::OsStr) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    Some(name.as_bytes().to_vec())
}

#[cfg(not(unix))]
fn captured_os_str_bytes(name: &std::ffi::OsStr) -> Option<Vec<u8>> {
    name.to_str().map(|name| name.as_bytes().to_vec())
}

#[cfg(unix)]
fn set_captured_dir_mode(path: &Path) -> Result<(), CapturedSourceMaterializationError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555)).map_err(|error| {
        materialization_error(format!(
            "cannot seal materialized captured source directory `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn set_captured_dir_mode(_path: &Path) -> Result<(), CapturedSourceMaterializationError> {
    Ok(())
}

#[cfg(unix)]
fn set_captured_file_mode(
    path: &Path,
    executable: bool,
) -> Result<(), CapturedSourceMaterializationError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o555 } else { 0o444 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|error| {
        materialization_error(format!(
            "cannot seal materialized captured source file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn set_captured_file_mode(
    path: &Path,
    _executable: bool,
) -> Result<(), CapturedSourceMaterializationError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|error| {
            materialization_error(format!(
                "cannot inspect materialized captured source file `{}`: {error}",
                path.display()
            ))
        })?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        materialization_error(format!(
            "cannot seal materialized captured source file `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(unix)]
fn validate_captured_executable_mode(
    _relative_path: &[u8],
    _executable: bool,
) -> Result<(), CapturedSourceMaterializationError> {
    Ok(())
}

/// A host without an executable file class cannot materialize a captured
/// executable source file faithfully; the inventory refuses before any write
/// rather than produce a snapshot whose re-inspection would disagree with
/// the canonical metadata index the build's grants enforce.
#[cfg(not(unix))]
fn validate_captured_executable_mode(
    relative_path: &[u8],
    executable: bool,
) -> Result<(), CapturedSourceMaterializationError> {
    if executable {
        Err(materialization_error(format!(
            "this host cannot materialize captured source file `{}`'s executable mode",
            String::from_utf8_lossy(relative_path)
        )))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn captured_file_is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn captured_file_is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn materialization_error(message: impl Into<String>) -> CapturedSourceMaterializationError {
    CapturedSourceMaterializationError {
        message: message.into(),
    }
}

fn diagnostics(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message.into())]
}

#[cfg(test)]
mod tests;
