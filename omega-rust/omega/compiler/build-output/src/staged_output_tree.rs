//! The staged output tree: the retained entries a build wrote, the
//! canonical commitment over them, the sealed entry views, and the
//! selection of generated sources from retained content.

use crate::capture::{StagedOutputEntry, StagedOutputEntryKind};
use crate::materialization::materialize_retained_tree;
use crate::materialization::validate_retained_tree;
use crate::materialization::verify_materialized_tree;
use diagnostics::Diagnostic;
use sha2::Digest;
use sha2::Sha256;
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

const STAGED_OUTPUT_TREE_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-BUILD-STAGED-OUTPUT-TREE\0";

const STAGED_OUTPUT_TREE_SCHEMA_VERSION: u32 = 2;

const STAGED_OUTPUT_ROOT_TAG: u8 = 1;

const DIRECTORY_MODE: u32 = 0o040000;

const FILE_MODE: u32 = 0o100644;

const EXECUTABLE_FILE_MODE: u32 = 0o100755;

const SYMLINK_MODE: u32 = 0o120000;

pub(crate) const MAX_STAGED_OUTPUT_ENTRIES: usize = 4_096;

pub(crate) const MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES: u64 = 256 * 1024 * 1024;

pub(crate) const MAX_STAGED_OUTPUT_PATH_BYTES: usize = 16 * 1024 * 1024;

/// Compiler-issued identity of the complete canonical staged content tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildStagedOutputTreeCommitment {
    digest: [u8; 32],
    entry_count: u64,
    file_bytes: u64,
}

impl BuildStagedOutputTreeCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub const fn entry_count(self) -> u64 {
        self.entry_count
    }

    pub const fn file_bytes(self) -> u64 {
        self.file_bytes
    }
}

/// Compiler-owned retained content of a successfully captured sponsored
/// staged-output tree.
///
/// The fields are private so package-controlled data cannot construct a tree
/// that impersonates compiler capture. This carrier contains canonical
/// root-relative paths, file bytes, executable/ordinary mode, empty
/// directories, and relative symlink spelling. It deliberately contains no
/// physical root, ambient metadata, inode identity, or public hard-link
/// topology. Materialization reproduces and re-inspects this content; it is
/// not a build-operation transcript, a generated-source handoff, or a receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStagedOutputTree {
    pub(crate) commitment: BuildStagedOutputTreeCommitment,
    pub(crate) entries: Vec<RetainedStagedOutputEntry>,
}

/// Exact regular Omega source retained from one explicit successful
/// `BuildOutput::include_source` handoff. This value is produced only by
/// matching the interpreter's Output-rooted coordinate against sponsored
/// staged-tree custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGeneratedSource {
    relative_path: Vec<u8>,
    bytes: Arc<[u8]>,
    digest: [u8; 32],
}

impl PackageGeneratedSource {
    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

impl BuildStagedOutputTree {
    pub const fn commitment(&self) -> BuildStagedOutputTreeCommitment {
        self.commitment
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.commitment.digest()
    }

    pub const fn entry_count(&self) -> u64 {
        self.commitment.entry_count()
    }

    pub const fn file_bytes(&self) -> u64 {
        self.commitment.file_bytes()
    }

    /// Materialize this retained canonical tree into an existing empty
    /// concrete directory, then independently re-inspect the result.
    ///
    /// Success returns the same compiler-issued commitment. Any invalid
    /// retained shape, unsuitable destination, write failure, or observed
    /// post-materialization drift rejects.
    pub fn materialize_into(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<BuildStagedOutputTreeCommitment, BuildStagedOutputMaterializationError> {
        materialize_retained_tree(self, destination.as_ref())
    }

    /// Verify an existing concrete directory against this retained commitment
    /// without writing or deleting anything. Missing or extra entries, changed
    /// bytes, kinds, modes, or link spellings reject. Success describes the
    /// observed materialization; it does not prevent later host mutation.
    pub fn verify_materialized_at(
        &self,
        destination: &Path,
    ) -> Result<BuildStagedOutputTreeCommitment, BuildStagedOutputMaterializationError> {
        validate_retained_tree(self)?;
        verify_materialized_tree(destination, self)?;
        Ok(self.commitment)
    }

    /// Directly enumerate the retained sealed entries in canonical
    /// unsigned-byte order. A staging tree includes scratch and generated
    /// sources; product owners must select the completed-output roster before
    /// publishing files.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = BuildStagedOutputEntry<'_>> {
        self.entries.iter().map(|entry| {
            let kind = match &entry.kind {
                RetainedStagedOutputEntryKind::Directory => BuildStagedOutputEntryKind::Directory,
                RetainedStagedOutputEntryKind::File { bytes, executable } => {
                    BuildStagedOutputEntryKind::File {
                        bytes: bytes.as_ref(),
                        executable: *executable,
                    }
                }
                RetainedStagedOutputEntryKind::Symlink { target } => {
                    BuildStagedOutputEntryKind::Symlink {
                        target: target.as_slice(),
                    }
                }
            };
            BuildStagedOutputEntry {
                relative_path: entry.relative_path.as_slice(),
                kind,
            }
        })
    }

    /// Look up one sealed entry by its canonical slash-separated relative
    /// path. The lookup is exact: no component is normalized, resolved, or
    /// followed as a link.
    pub fn sealed_entry(&self, relative_path: &[u8]) -> Option<BuildStagedOutputEntry<'_>> {
        self.entries()
            .find(|entry| entry.relative_path == relative_path)
    }

    /// Retain exactly the requested ordinary files and their ancestor directories.
    ///
    /// Paths match exactly without normalization or link traversal. Missing,
    /// duplicate, directory, symbolic-link, and executable file requests reject;
    /// executable admission belongs to a separate product operation. The result
    /// shares immutable file bytes with this tree and has its own canonical
    /// commitment, independent of request order. An empty roster selects nothing.
    pub fn select_files(&self, relative_paths: &[&[u8]]) -> Result<Self, Vec<Diagnostic>> {
        if relative_paths.len() > self.entries.len() {
            return Err(diagnostics(
                "build output file selection exceeds the retained entry count",
            ));
        }
        let mut selected = vec![false; self.entries.len()];
        for relative_path in relative_paths {
            let entry_index = self
                .entries
                .binary_search_by(|entry| entry.relative_path.as_slice().cmp(relative_path))
                .map_err(|_| {
                    diagnostics(format!(
                        "selected build output `{}` is absent from the retained tree",
                        String::from_utf8_lossy(relative_path)
                    ))
                })?;
            if selected[entry_index] {
                return Err(diagnostics(format!(
                    "selected build output `{}` is requested more than once",
                    String::from_utf8_lossy(relative_path)
                )));
            }
            match &self.entries[entry_index].kind {
                RetainedStagedOutputEntryKind::File {
                    executable: false, ..
                } => {}
                _ => {
                    return Err(diagnostics(format!(
                        "selected build output `{}` must be a non-executable regular file",
                        String::from_utf8_lossy(relative_path)
                    )));
                }
            }
            selected[entry_index] = true;
        }
        for relative_path in relative_paths {
            for (separator, byte) in relative_path.iter().enumerate() {
                if *byte != b'/' {
                    continue;
                }
                let parent = &relative_path[..separator];
                let parent_index = self
                    .entries
                    .binary_search_by(|entry| entry.relative_path.as_slice().cmp(parent))
                    .map_err(|_| {
                        diagnostics("selected build output has a missing ancestor directory")
                    })?;
                if !matches!(
                    self.entries[parent_index].kind,
                    RetainedStagedOutputEntryKind::Directory
                ) {
                    return Err(diagnostics(
                        "selected build output has a non-directory ancestor",
                    ));
                }
                selected[parent_index] = true;
            }
        }
        let entries: Vec<_> = self
            .entries
            .iter()
            .zip(selected)
            .filter(|(_, is_selected)| *is_selected)
            .map(|(entry, _)| entry.clone())
            .collect();
        let commitment = commitment_for_retained_entries(&entries).ok_or_else(|| {
            diagnostics("selected build output exceeds the staged-output unique-content ceiling")
        })?;
        Ok(Self {
            commitment,
            entries,
        })
    }
}

/// Logical kind view of one sealed staged-output entry. File bytes are the
/// retained sealed content; symlink targets are inert captured spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStagedOutputEntryKind<'a> {
    Directory,
    File { bytes: &'a [u8], executable: bool },
    Symlink { target: &'a [u8] },
}

/// Borrowed view of one sealed staged-output entry and its canonical
/// relative path.
#[derive(Debug, Clone, Copy)]
pub struct BuildStagedOutputEntry<'a> {
    relative_path: &'a [u8],
    kind: BuildStagedOutputEntryKind<'a>,
}

impl<'a> BuildStagedOutputEntry<'a> {
    pub fn relative_path(&self) -> &'a [u8] {
        self.relative_path
    }

    pub const fn kind(&self) -> BuildStagedOutputEntryKind<'a> {
        self.kind
    }
}

/// Failure to validate or materialize a retained staged-output tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStagedOutputMaterializationError {
    pub(crate) message: String,
}

impl BuildStagedOutputMaterializationError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for BuildStagedOutputMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BuildStagedOutputMaterializationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetainedStagedOutputEntry {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) kind: RetainedStagedOutputEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RetainedStagedOutputEntryKind {
    Directory,
    File { bytes: Arc<[u8]>, executable: bool },
    Symlink { target: Vec<u8> },
}

pub fn empty() -> BuildStagedOutputTree {
    finish_commitment(Vec::new())
}

pub fn select_included_sources(
    tree: &BuildStagedOutputTree,
    relative_paths: &[Vec<u8>],
) -> Result<Vec<PackageGeneratedSource>, Vec<Diagnostic>> {
    for entry in &tree.entries {
        let RetainedStagedOutputEntryKind::File { .. } = &entry.kind else {
            continue;
        };
        let is_omega_source = entry
            .relative_path
            .rsplit(|byte| *byte == b'/')
            .next()
            .is_some_and(|name| name.ends_with(b".omg") && name.len() > 4);
        if is_omega_source && !relative_paths.contains(&entry.relative_path) {
            return Err(diagnostics(format!(
                "captured staged Omega source `{}` has no explicit include_source handoff",
                String::from_utf8_lossy(&entry.relative_path)
            )));
        }
    }
    let mut selected = Vec::with_capacity(relative_paths.len());
    for relative_path in relative_paths {
        let is_omega_source = relative_path
            .rsplit(|byte| *byte == b'/')
            .next()
            .is_some_and(|name| name.ends_with(b".omg") && name.len() > 4);
        if !is_omega_source {
            return Err(diagnostics(format!(
                "included build source `{}` must name a regular .omg file",
                String::from_utf8_lossy(relative_path)
            )));
        }
        if relative_path
            .rsplit(|byte| *byte == b'/')
            .next()
            .is_some_and(|name| matches!(name, b"build.omg" | b"main.omg"))
        {
            return Err(diagnostics(format!(
                "included build source `{}` uses a reserved source-discovery filename",
                String::from_utf8_lossy(relative_path)
            )));
        }
        let Some(entry) = tree
            .entries
            .iter()
            .find(|entry| entry.relative_path == *relative_path)
        else {
            return Err(diagnostics(format!(
                "included build source `{}` is absent from the captured staged-output tree",
                String::from_utf8_lossy(relative_path)
            )));
        };
        let RetainedStagedOutputEntryKind::File { bytes, executable } = &entry.kind else {
            return Err(diagnostics(format!(
                "included build source `{}` is not a regular file",
                String::from_utf8_lossy(relative_path)
            )));
        };
        if *executable {
            return Err(diagnostics(format!(
                "included build source `{}` must not be executable",
                String::from_utf8_lossy(relative_path)
            )));
        }
        let digest: [u8; 32] = Sha256::digest(bytes.as_ref()).into();
        selected.push(PackageGeneratedSource {
            relative_path: relative_path.clone(),
            bytes: Arc::clone(bytes),
            digest,
        });
    }
    Ok(selected)
}

pub(crate) fn finish_commitment(mut entries: Vec<StagedOutputEntry>) -> BuildStagedOutputTree {
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut retained_entries = Vec::with_capacity(entries.len());
    for entry in entries {
        let retained_kind = match entry.kind {
            StagedOutputEntryKind::Directory => RetainedStagedOutputEntryKind::Directory,
            StagedOutputEntryKind::File(content) => {
                debug_assert_eq!(content.length, content.bytes.len() as u64);
                let retained_digest: [u8; 32] = Sha256::digest(&content.bytes).into();
                debug_assert_eq!(content.digest, retained_digest);
                RetainedStagedOutputEntryKind::File {
                    bytes: content.bytes,
                    executable: content.executable,
                }
            }
            StagedOutputEntryKind::Symlink { target } => {
                RetainedStagedOutputEntryKind::Symlink { target }
            }
        };
        retained_entries.push(RetainedStagedOutputEntry {
            relative_path: entry.relative_path,
            kind: retained_kind,
        });
    }
    BuildStagedOutputTree {
        commitment: commitment_for_retained_entries(&retained_entries)
            .expect("captured staged-output content remains within its byte ceiling"),
        entries: retained_entries,
    }
}

pub(crate) fn commitment_for_retained_entries(
    entries: &[RetainedStagedOutputEntry],
) -> Option<BuildStagedOutputTreeCommitment> {
    let entry_count = u64::try_from(entries.len()).expect("staged-output entry ceiling fits u64");
    let mut digest = Sha256::new();
    let mut distinct_content = BTreeSet::new();
    let mut file_bytes = 0u64;
    digest.update(STAGED_OUTPUT_TREE_COMMITMENT_DOMAIN);
    digest.update(STAGED_OUTPUT_TREE_SCHEMA_VERSION.to_le_bytes());
    digest.update([STAGED_OUTPUT_ROOT_TAG]);
    digest.update(entry_count.to_le_bytes());
    for entry in entries {
        hash_field(&mut digest, &entry.relative_path);
        match &entry.kind {
            RetainedStagedOutputEntryKind::Directory => {
                digest.update([0]);
                digest.update(DIRECTORY_MODE.to_le_bytes());
            }
            RetainedStagedOutputEntryKind::File { bytes, executable } => {
                let content_digest: [u8; 32] = Sha256::digest(bytes).into();
                let content_length = u64::try_from(bytes.len()).ok()?;
                if distinct_content.insert((content_digest, content_length)) {
                    file_bytes = file_bytes
                        .checked_add(content_length)
                        .filter(|total| *total <= MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES)?;
                }
                digest.update([1]);
                digest.update(
                    if *executable {
                        EXECUTABLE_FILE_MODE
                    } else {
                        FILE_MODE
                    }
                    .to_le_bytes(),
                );
                digest.update(content_length.to_le_bytes());
                digest.update(content_digest);
            }
            RetainedStagedOutputEntryKind::Symlink { target } => {
                digest.update([2]);
                digest.update(SYMLINK_MODE.to_le_bytes());
                hash_field(&mut digest, target);
            }
        }
    }
    Some(BuildStagedOutputTreeCommitment {
        digest: digest.finalize().into(),
        entry_count,
        file_bytes,
    })
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("staged-output field length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

pub(crate) fn diagnostics(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

#[cfg(test)]
mod tests;
