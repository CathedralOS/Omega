#![forbid(unsafe_code)]

//! Retained build-output custody, canonical identity, and materialization.

use psi_checked_interpreter::{FilesystemSponsor, FilesystemSponsorNamespaceEntryKind};
use psi_diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

const STAGED_OUTPUT_TREE_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-BUILD-STAGED-OUTPUT-TREE\0";
const STAGED_OUTPUT_TREE_SCHEMA_VERSION: u32 = 2;
const STAGED_OUTPUT_ROOT_TAG: u8 = 1;
const DIRECTORY_MODE: u32 = 0o040000;
const FILE_MODE: u32 = 0o100644;
const EXECUTABLE_FILE_MODE: u32 = 0o100755;
const SYMLINK_MODE: u32 = 0o120000;
const MAX_STAGED_OUTPUT_ENTRIES: usize = 4_096;
const MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_STAGED_OUTPUT_PATH_BYTES: usize = 16 * 1024 * 1024;

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
    commitment: BuildStagedOutputTreeCommitment,
    entries: Vec<RetainedStagedOutputEntry>,
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
}

/// Failure to validate or materialize a retained staged-output tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStagedOutputMaterializationError {
    message: String,
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
struct RetainedStagedOutputEntry {
    relative_path: Vec<u8>,
    kind: RetainedStagedOutputEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RetainedStagedOutputEntryKind {
    Directory,
    File { bytes: Arc<[u8]>, executable: bool },
    Symlink { target: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedFileContent {
    length: u64,
    digest: [u8; 32],
    executable: bool,
    identity: HostFileIdentity,
    bytes: Arc<[u8]>,
}

#[derive(Debug)]
enum StagedOutputEntryKind {
    Directory,
    File(CapturedFileContent),
    Symlink { target: Vec<u8> },
}

#[derive(Debug)]
struct StagedOutputEntry {
    relative_path: Vec<u8>,
    kind: StagedOutputEntryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedEntryKind {
    Directory,
    File { group: u64, extent: u64 },
    Symlink { spelling_bytes: u64 },
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(not(unix))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HostFileIdentity;

pub fn empty() -> BuildStagedOutputTree {
    finish_commitment(Vec::new())
}

/// Reconstruct the initial singleton Output grammar's complete staged tree.
pub fn replayed_single_ordinary_file(
    relative_path: &[u8],
    bytes: &[u8],
) -> Result<BuildStagedOutputTree, Vec<Diagnostic>> {
    replayed_ordinary_files(&[(relative_path, bytes)])
}

/// Reconstruct the repeated ordinary-artifact receipt grammar from canonical
/// replay operands. Every file is a distinct direct child created by one exact
/// create/full-write*/close sequence. Directory and other namespace effects remain
/// outside this grammar rather than being inferred from a final digest.
pub fn replayed_ordinary_files(
    files: &[(&[u8], &[u8])],
) -> Result<BuildStagedOutputTree, Vec<Diagnostic>> {
    let files = files
        .iter()
        .map(|(relative_path, bytes)| (*relative_path, *bytes, false))
        .collect::<Vec<_>>();
    replayed_files(&files)
}

/// Reconstruct repeated regular-file outputs with their compiler-derived
/// executable class. The boolean is not package-authored metadata: callers
/// derive it from the retained filesystem operation grammar.
pub fn replayed_files(
    files: &[(&[u8], &[u8], bool)],
) -> Result<BuildStagedOutputTree, Vec<Diagnostic>> {
    if files.is_empty() {
        return Err(diagnostics(
            "receipted regular-file build output requires at least one file",
        ));
    }
    if files.len() > MAX_STAGED_OUTPUT_ENTRIES {
        return Err(diagnostics(format!(
            "receipted build output exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
        )));
    }
    let mut entries = Vec::new();
    entries.try_reserve_exact(files.len()).map_err(|_| {
        diagnostics("receipted build output entry allocation failed on this compiler host")
    })?;
    for (relative_path, bytes, executable) in files {
        let native = retained_native_path(relative_path).map_err(|error| {
            diagnostics(format!(
                "receipted build output path is not canonical: {error}"
            ))
        })?;
        if native
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty())
        {
            return Err(diagnostics(
                "the repeated ordinary-artifact grammar requires direct-child files",
            ));
        }
        let byte_length = u64::try_from(bytes.len()).map_err(|_| {
            diagnostics("receipted build output length cannot be represented canonically")
        })?;
        if byte_length > MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES {
            return Err(diagnostics(format!(
                "receipted build output exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte object ceiling"
            )));
        }
        entries.push(RetainedStagedOutputEntry {
            relative_path: relative_path.to_vec(),
            kind: RetainedStagedOutputEntryKind::File {
                bytes: Arc::from(*bytes),
                executable: *executable,
            },
        });
    }
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if entries
        .windows(2)
        .any(|pair| pair[0].relative_path == pair[1].relative_path)
    {
        return Err(diagnostics(
            "receipted build output contains a duplicate file path",
        ));
    }
    let commitment = commitment_for_retained_entries(&entries).ok_or_else(|| {
        diagnostics("receipted build output exceeds the staged-output unique-content ceiling")
    })?;
    let tree = BuildStagedOutputTree {
        commitment,
        entries,
    };
    validate_retained_tree(&tree).map_err(|error| {
        diagnostics(format!(
            "receipted build output failed canonical tree validation: {error}"
        ))
    })?;
    Ok(tree)
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

pub fn capture(
    root: &Path,
    sponsor: &FilesystemSponsor,
) -> Result<BuildStagedOutputTree, Vec<Diagnostic>> {
    let snapshot = sponsor.namespace_snapshot().map_err(|error| {
        diagnostics(format!(
            "cannot inspect sponsored build staged-output namespace: {error}"
        ))
    })?;
    if snapshot.transaction_prepared() || snapshot.open_descriptors() != 0 {
        return Err(diagnostics(
            "build staged-output capture requires a quiescent sponsor with no prepared transaction or open descriptor",
        ));
    }
    let bound_root = sponsor.bind_path(root).map_err(|error| {
        diagnostics(format!(
            "cannot bind build staged-output root `{}` to its sponsor: {error}",
            root.display()
        ))
    })?;
    let sponsor_root = bound_root.relative();
    let mut root_is_sponsored_directory = false;
    let mut expected = BTreeMap::new();
    for entry in snapshot.entries() {
        if entry.relative_path() == sponsor_root {
            root_is_sponsored_directory =
                entry.kind() == FilesystemSponsorNamespaceEntryKind::Directory;
            continue;
        }
        let Ok(relative) = entry.relative_path().strip_prefix(sponsor_root) else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let kind = match entry.kind() {
            FilesystemSponsorNamespaceEntryKind::Directory => ExpectedEntryKind::Directory,
            FilesystemSponsorNamespaceEntryKind::Symlink { spelling_bytes } => {
                ExpectedEntryKind::Symlink { spelling_bytes }
            }
            FilesystemSponsorNamespaceEntryKind::Object { group, extent } => {
                ExpectedEntryKind::File { group, extent }
            }
        };
        expected.insert(relative.to_path_buf(), kind);
    }
    if !root_is_sponsored_directory {
        return Err(diagnostics(format!(
            "build staged-output root `{}` is not the sponsor's committed directory",
            root.display()
        )));
    }
    if expected.len() > MAX_STAGED_OUTPUT_ENTRIES {
        return Err(diagnostics(format!(
            "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
        )));
    }

    let metadata = std::fs::symlink_metadata(root).map_err(|error| {
        diagnostics(format!(
            "cannot inspect build staged-output root `{}`: {error}",
            root.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(diagnostics(format!(
            "build staged-output root `{}` must be a concrete directory",
            root.display()
        )));
    }

    let mut observed_paths = BTreeSet::new();
    let mut entries = Vec::with_capacity(expected.len());
    let mut pending = vec![root.to_path_buf()];
    let mut total_path_bytes = 0usize;
    let mut total_unique_file_bytes = 0u64;
    let mut file_groups = BTreeMap::<u64, CapturedFileContent>::new();
    while let Some(directory) = pending.pop() {
        let directory_metadata = std::fs::symlink_metadata(&directory).map_err(|error| {
            diagnostics(format!(
                "cannot inspect build staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
            return Err(diagnostics(format!(
                "build staged-output directory `{}` changed kind during capture",
                directory.display()
            )));
        }
        let children = std::fs::read_dir(&directory).map_err(|error| {
            diagnostics(format!(
                "cannot enumerate build staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        let mut bounded_children = Vec::new();
        for child in children {
            if observed_paths.len() + bounded_children.len() == MAX_STAGED_OUTPUT_ENTRIES {
                return Err(diagnostics(format!(
                    "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
                )));
            }
            bounded_children.push(child.map_err(|error| {
                diagnostics(format!(
                    "cannot enumerate build staged-output directory `{}`: {error}",
                    directory.display()
                ))
            })?);
        }
        bounded_children.sort_by(|left, right| left.file_name().cmp(&right.file_name()));

        for child in bounded_children {
            let path = child.path();
            let relative_native = path.strip_prefix(root).map_err(|_| {
                diagnostics(format!(
                    "build staged-output entry `{}` escaped root `{}`",
                    path.display(),
                    root.display()
                ))
            })?;
            let expected_kind = expected.get(relative_native).copied().ok_or_else(|| {
                diagnostics(format!(
                    "build staged-output entry `{}` is absent from sponsor custody",
                    path.display()
                ))
            })?;
            if !observed_paths.insert(relative_native.to_path_buf()) {
                return Err(diagnostics(format!(
                    "build staged-output entry `{}` was observed more than once",
                    path.display()
                )));
            }
            let relative_path = canonical_relative_path(relative_native, &path)?;
            total_path_bytes = reserve_path_bytes(total_path_bytes, relative_path.len())?;
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                diagnostics(format!(
                    "cannot inspect build staged-output entry `{}`: {error}",
                    path.display()
                ))
            })?;
            let file_type = metadata.file_type();
            let kind = match expected_kind {
                ExpectedEntryKind::Directory if file_type.is_dir() => {
                    pending.push(path);
                    StagedOutputEntryKind::Directory
                }
                ExpectedEntryKind::File { group, extent } if file_type.is_file() => {
                    if metadata.len() != extent {
                        return Err(diagnostics(format!(
                            "build staged-output file `{}` disagrees with sponsor extent",
                            path.display()
                        )));
                    }
                    match file_groups.get(&group) {
                        Some(existing) => {
                            validate_hard_link_alias(&path, &metadata, extent, existing)?;
                            StagedOutputEntryKind::File(existing.clone())
                        }
                        None => {
                            total_unique_file_bytes = total_unique_file_bytes
                                .checked_add(extent)
                                .filter(|total| *total <= MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES)
                                .ok_or_else(|| {
                                    diagnostics(format!(
                                        "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte unique-content ceiling"
                                    ))
                                })?;
                            let content = capture_file(&path, &metadata, extent)?;
                            file_groups.insert(group, content.clone());
                            StagedOutputEntryKind::File(content)
                        }
                    }
                }
                ExpectedEntryKind::Symlink { spelling_bytes } if file_type.is_symlink() => {
                    let target = std::fs::read_link(&path).map_err(|error| {
                        diagnostics(format!(
                            "cannot read build staged-output symlink `{}`: {error}",
                            path.display()
                        ))
                    })?;
                    let target = canonical_symlink_target(&target, &relative_path, &path)?;
                    if u64::try_from(target.len()).ok() != Some(spelling_bytes) {
                        return Err(diagnostics(format!(
                            "build staged-output symlink `{}` disagrees with sponsor target length",
                            path.display()
                        )));
                    }
                    total_path_bytes = reserve_path_bytes(total_path_bytes, target.len())?;
                    StagedOutputEntryKind::Symlink { target }
                }
                _ => {
                    return Err(diagnostics(format!(
                        "build staged-output entry `{}` disagrees with sponsor kind",
                        path.display()
                    )));
                }
            };
            entries.push(StagedOutputEntry {
                relative_path,
                kind,
            });
        }
    }
    if observed_paths.len() != expected.len() {
        let missing = expected
            .keys()
            .find(|path| !observed_paths.contains(*path))
            .expect("unequal sponsored and observed counts have one missing path");
        return Err(diagnostics(format!(
            "sponsored build staged-output entry `{}` is missing from the physical tree",
            root.join(missing).display()
        )));
    }
    Ok(finish_commitment(entries))
}

fn finish_commitment(mut entries: Vec<StagedOutputEntry>) -> BuildStagedOutputTree {
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

fn commitment_for_retained_entries(
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

fn materialize_retained_tree(
    tree: &BuildStagedOutputTree,
    destination: &Path,
) -> Result<BuildStagedOutputTreeCommitment, BuildStagedOutputMaterializationError> {
    validate_retained_tree(tree)?;
    validate_empty_destination(destination)?;

    for entry in &tree.entries {
        let relative = retained_native_path(&entry.relative_path)?;
        let path = destination.join(relative);
        match &entry.kind {
            RetainedStagedOutputEntryKind::Directory => {
                std::fs::create_dir(&path).map_err(|error| {
                    materialization_error(format!(
                        "cannot create staged-output directory `{}`: {error}",
                        path.display()
                    ))
                })?;
            }
            RetainedStagedOutputEntryKind::File { bytes, executable } => {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|error| {
                        materialization_error(format!(
                            "cannot create staged-output file `{}`: {error}",
                            path.display()
                        ))
                    })?;
                file.write_all(bytes).map_err(|error| {
                    materialization_error(format!(
                        "cannot write staged-output file `{}`: {error}",
                        path.display()
                    ))
                })?;
                set_materialized_file_mode(&file, &path, *executable)?;
            }
            RetainedStagedOutputEntryKind::Symlink { target } => {
                create_materialized_symlink(target, &path)?;
            }
        }
    }

    verify_materialized_tree(destination, tree)?;
    Ok(tree.commitment)
}

fn validate_retained_tree(
    tree: &BuildStagedOutputTree,
) -> Result<(), BuildStagedOutputMaterializationError> {
    if tree.entries.len() > MAX_STAGED_OUTPUT_ENTRIES {
        return Err(materialization_error(format!(
            "retained staged-output tree exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
        )));
    }
    if tree.commitment.file_bytes() > MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES {
        return Err(materialization_error(format!(
            "retained staged-output tree exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte unique-content ceiling"
        )));
    }

    let mut previous_path: Option<&[u8]> = None;
    let mut directories = BTreeSet::new();
    let mut total_path_bytes = 0usize;
    for entry in &tree.entries {
        if previous_path.is_some_and(|previous| previous >= entry.relative_path.as_slice()) {
            return Err(materialization_error(
                "retained staged-output paths are not in strict canonical order",
            ));
        }
        previous_path = Some(&entry.relative_path);
        let relative = retained_native_path(&entry.relative_path)?;
        total_path_bytes =
            reserve_materialization_path_bytes(total_path_bytes, entry.relative_path.len())?;

        let mut parent = relative.parent();
        while let Some(path) = parent {
            if path.as_os_str().is_empty() {
                break;
            }
            let canonical =
                canonical_relative_path(path, path).map_err(materialization_diagnostics)?;
            if !directories.contains(&canonical) {
                return Err(materialization_error(format!(
                    "retained staged-output entry `{}` has a missing or non-directory parent",
                    String::from_utf8_lossy(&entry.relative_path)
                )));
            }
            parent = path.parent();
        }

        match &entry.kind {
            RetainedStagedOutputEntryKind::Directory => {
                directories.insert(entry.relative_path.clone());
            }
            RetainedStagedOutputEntryKind::File { bytes, executable } => {
                validate_retained_executable_mode(*executable)?;
                let length = u64::try_from(bytes.len()).map_err(|_| {
                    materialization_error("retained staged-output file length exceeds u64")
                })?;
                if length > MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES {
                    return Err(materialization_error(format!(
                        "retained staged-output file exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte object ceiling"
                    )));
                }
            }
            RetainedStagedOutputEntryKind::Symlink { target } => {
                validate_retained_symlink_materialization()?;
                let target_path = retained_symlink_target(target)?;
                let canonical =
                    canonical_symlink_target(&target_path, &entry.relative_path, &relative)
                        .map_err(materialization_diagnostics)?;
                if canonical != *target {
                    return Err(materialization_error(
                        "retained staged-output symlink target is not canonical",
                    ));
                }
                total_path_bytes =
                    reserve_materialization_path_bytes(total_path_bytes, target.len())?;
            }
        }
    }
    let retained_commitment = commitment_for_retained_entries(&tree.entries).ok_or_else(|| {
        materialization_error(format!(
            "retained staged-output tree exceeds its {MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES}-byte unique-content ceiling"
        ))
    })?;
    if retained_commitment != tree.commitment {
        return Err(materialization_error(
            "retained staged-output content disagrees with its commitment",
        ));
    }
    Ok(())
}

fn validate_empty_destination(
    destination: &Path,
) -> Result<(), BuildStagedOutputMaterializationError> {
    let metadata = std::fs::symlink_metadata(destination).map_err(|error| {
        materialization_error(format!(
            "cannot inspect staged-output destination `{}`: {error}",
            destination.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(materialization_error(format!(
            "staged-output destination `{}` must be an existing concrete directory",
            destination.display()
        )));
    }
    let mut children = std::fs::read_dir(destination).map_err(|error| {
        materialization_error(format!(
            "cannot enumerate staged-output destination `{}`: {error}",
            destination.display()
        ))
    })?;
    match children.next() {
        None => Ok(()),
        Some(Ok(_)) => Err(materialization_error(format!(
            "staged-output destination `{}` must be empty",
            destination.display()
        ))),
        Some(Err(error)) => Err(materialization_error(format!(
            "cannot enumerate staged-output destination `{}`: {error}",
            destination.display()
        ))),
    }
}

fn verify_materialized_tree(
    root: &Path,
    expected_tree: &BuildStagedOutputTree,
) -> Result<(), BuildStagedOutputMaterializationError> {
    let metadata = std::fs::symlink_metadata(root).map_err(|error| {
        materialization_error(format!(
            "cannot re-inspect staged-output destination `{}`: {error}",
            root.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(materialization_error(format!(
            "staged-output destination `{}` changed from a concrete directory",
            root.display()
        )));
    }

    let expected = expected_tree
        .entries
        .iter()
        .map(|entry| (entry.relative_path.as_slice(), &entry.kind))
        .collect::<BTreeMap<_, _>>();
    let mut observed_paths = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    let mut total_path_bytes = 0usize;
    while let Some(directory) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&directory).map_err(|error| {
            materialization_error(format!(
                "cannot re-inspect materialized staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(materialization_error(format!(
                "materialized staged-output directory `{}` changed kind",
                directory.display()
            )));
        }
        let children = std::fs::read_dir(&directory).map_err(|error| {
            materialization_error(format!(
                "cannot enumerate materialized staged-output directory `{}`: {error}",
                directory.display()
            ))
        })?;
        let mut bounded_children = Vec::new();
        for child in children {
            if observed_paths.len() + bounded_children.len() == MAX_STAGED_OUTPUT_ENTRIES {
                return Err(materialization_error(format!(
                    "materialized staged-output tree exceeds its {MAX_STAGED_OUTPUT_ENTRIES}-entry ceiling"
                )));
            }
            bounded_children.push(child.map_err(|error| {
                materialization_error(format!(
                    "cannot enumerate materialized staged-output directory `{}`: {error}",
                    directory.display()
                ))
            })?);
        }
        bounded_children.sort_by_key(|entry| entry.file_name());
        for child in bounded_children {
            let path = child.path();
            let relative_native = path.strip_prefix(root).map_err(|_| {
                materialization_error(format!(
                    "materialized staged-output entry `{}` escaped destination `{}`",
                    path.display(),
                    root.display()
                ))
            })?;
            let relative_path = canonical_relative_path(relative_native, &path)
                .map_err(materialization_diagnostics)?;
            let expected_kind = expected.get(relative_path.as_slice()).ok_or_else(|| {
                materialization_error(format!(
                    "materialized staged-output entry `{}` is absent from retained content",
                    path.display()
                ))
            })?;
            if !observed_paths.insert(relative_path.clone()) {
                return Err(materialization_error(format!(
                    "materialized staged-output entry `{}` was observed more than once",
                    path.display()
                )));
            }
            total_path_bytes =
                reserve_materialization_path_bytes(total_path_bytes, relative_path.len())?;
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                materialization_error(format!(
                    "cannot inspect materialized staged-output entry `{}`: {error}",
                    path.display()
                ))
            })?;
            match (*expected_kind, metadata.file_type()) {
                (RetainedStagedOutputEntryKind::Directory, file_type) if file_type.is_dir() => {
                    pending.push(path);
                }
                (RetainedStagedOutputEntryKind::File { bytes, executable }, file_type)
                    if file_type.is_file() =>
                {
                    if metadata.len() != bytes.len() as u64
                        || is_executable(&metadata) != *executable
                    {
                        return Err(materialization_error(format!(
                            "materialized staged-output file `{}` disagrees with retained length or mode",
                            path.display()
                        )));
                    }
                    verify_materialized_file(&path, &metadata, bytes)?;
                }
                (RetainedStagedOutputEntryKind::Symlink { target }, file_type)
                    if file_type.is_symlink() =>
                {
                    let observed_target = std::fs::read_link(&path).map_err(|error| {
                        materialization_error(format!(
                            "cannot read materialized staged-output symlink `{}`: {error}",
                            path.display()
                        ))
                    })?;
                    let observed_target =
                        canonical_symlink_target(&observed_target, &relative_path, &path)
                            .map_err(materialization_diagnostics)?;
                    total_path_bytes = reserve_materialization_path_bytes(
                        total_path_bytes,
                        observed_target.len(),
                    )?;
                    if observed_target != *target {
                        return Err(materialization_error(format!(
                            "materialized staged-output symlink `{}` disagrees with retained spelling",
                            path.display()
                        )));
                    }
                }
                _ => {
                    return Err(materialization_error(format!(
                        "materialized staged-output entry `{}` disagrees with retained kind",
                        path.display()
                    )));
                }
            }
        }
    }
    if observed_paths.len() != expected.len() {
        let missing = expected
            .keys()
            .find(|path| !observed_paths.contains(**path))
            .expect("unequal retained and observed entry counts have one missing path");
        return Err(materialization_error(format!(
            "retained staged-output entry `{}` is missing after materialization",
            String::from_utf8_lossy(missing)
        )));
    }
    if commitment_for_retained_entries(&expected_tree.entries) != Some(expected_tree.commitment) {
        return Err(materialization_error(
            "materialized staged-output tree no longer verifies against its commitment",
        ));
    }
    Ok(())
}

fn verify_materialized_file(
    path: &Path,
    path_metadata: &std::fs::Metadata,
    expected: &[u8],
) -> Result<(), BuildStagedOutputMaterializationError> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        materialization_error(format!(
            "cannot open materialized staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    let before = file.metadata().map_err(|error| {
        materialization_error(format!(
            "cannot inspect opened materialized staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    if !same_file_observation(path_metadata, &before) || before.len() != expected.len() as u64 {
        return Err(materialization_error(format!(
            "materialized staged-output file `{}` changed before verification",
            path.display()
        )));
    }
    let first = verify_materialized_reader(&mut file, expected, path)?;
    file.rewind().map_err(|error| {
        materialization_error(format!(
            "cannot rewind materialized staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    let second = verify_materialized_reader(&mut file, expected, path)?;
    let after = file.metadata().map_err(|error| {
        materialization_error(format!(
            "cannot re-inspect materialized staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    let final_path_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        materialization_error(format!(
            "cannot re-inspect materialized staged-output path `{}`: {error}",
            path.display()
        ))
    })?;
    let expected_digest: [u8; 32] = Sha256::digest(expected).into();
    if first != expected_digest
        || second != expected_digest
        || !same_file_observation(&before, &after)
        || !same_file_observation(&after, &final_path_metadata)
    {
        return Err(materialization_error(format!(
            "materialized staged-output file `{}` drifted during verification",
            path.display()
        )));
    }
    Ok(())
}

fn verify_materialized_reader(
    reader: &mut std::fs::File,
    expected: &[u8],
    path: &Path,
) -> Result<[u8; 32], BuildStagedOutputMaterializationError> {
    let mut digest = Sha256::new();
    let mut offset = 0usize;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|error| {
            materialization_error(format!(
                "cannot read materialized staged-output file `{}`: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            break;
        }
        let end = offset.checked_add(read).ok_or_else(|| {
            materialization_error(format!(
                "materialized staged-output file `{}` length overflowed during verification",
                path.display()
            ))
        })?;
        if expected.get(offset..end) != Some(&buffer[..read]) {
            return Err(materialization_error(format!(
                "materialized staged-output file `{}` disagrees with retained bytes",
                path.display()
            )));
        }
        digest.update(&buffer[..read]);
        offset = end;
    }
    if offset != expected.len() {
        return Err(materialization_error(format!(
            "materialized staged-output file `{}` changed length during verification",
            path.display()
        )));
    }
    Ok(digest.finalize().into())
}

fn retained_native_path(
    relative_path: &[u8],
) -> Result<PathBuf, BuildStagedOutputMaterializationError> {
    let spelling = std::str::from_utf8(relative_path)
        .map_err(|_| materialization_error("retained staged-output path is not canonical UTF-8"))?;
    if spelling.is_empty() || spelling.starts_with('/') || spelling.ends_with('/') {
        return Err(materialization_error(
            "retained staged-output path is not a nonempty relative slash path",
        ));
    }
    let mut native = PathBuf::new();
    for component in spelling.split('/') {
        validate_portable_component(component.as_bytes(), Path::new(spelling))
            .map_err(materialization_diagnostics)?;
        native.push(component);
    }
    let canonical =
        canonical_relative_path(&native, &native).map_err(materialization_diagnostics)?;
    if canonical != relative_path {
        return Err(materialization_error(
            "retained staged-output path is not canonical",
        ));
    }
    Ok(native)
}

fn retained_symlink_target(
    target: &[u8],
) -> Result<PathBuf, BuildStagedOutputMaterializationError> {
    let spelling = std::str::from_utf8(target).map_err(|_| {
        materialization_error("retained staged-output symlink target is not canonical UTF-8")
    })?;
    Ok(PathBuf::from(spelling))
}

fn reserve_materialization_path_bytes(
    current: usize,
    additional: usize,
) -> Result<usize, BuildStagedOutputMaterializationError> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAX_STAGED_OUTPUT_PATH_BYTES)
        .ok_or_else(|| {
            materialization_error(format!(
                "retained staged-output tree exceeds its {MAX_STAGED_OUTPUT_PATH_BYTES}-byte path and symlink-target ceiling"
            ))
        })
}

#[cfg(unix)]
fn validate_retained_executable_mode(
    _executable: bool,
) -> Result<(), BuildStagedOutputMaterializationError> {
    Ok(())
}

#[cfg(not(unix))]
fn validate_retained_executable_mode(
    executable: bool,
) -> Result<(), BuildStagedOutputMaterializationError> {
    if executable {
        Err(materialization_error(
            "this host cannot represent retained executable file mode",
        ))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn set_materialized_file_mode(
    file: &std::fs::File,
    path: &Path,
    executable: bool,
) -> Result<(), BuildStagedOutputMaterializationError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o755 } else { 0o644 };
    file.set_permissions(std::fs::Permissions::from_mode(mode))
        .map_err(|error| {
            materialization_error(format!(
                "cannot set staged-output file mode for `{}`: {error}",
                path.display()
            ))
        })
}

#[cfg(not(unix))]
fn set_materialized_file_mode(
    _file: &std::fs::File,
    _path: &Path,
    executable: bool,
) -> Result<(), BuildStagedOutputMaterializationError> {
    if executable {
        Err(materialization_error(
            "this host cannot represent retained executable file mode",
        ))
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn validate_retained_symlink_materialization() -> Result<(), BuildStagedOutputMaterializationError>
{
    Ok(())
}

#[cfg(not(unix))]
fn validate_retained_symlink_materialization() -> Result<(), BuildStagedOutputMaterializationError>
{
    Err(materialization_error(
        "this host cannot faithfully materialize retained symlink kind",
    ))
}

#[cfg(unix)]
fn create_materialized_symlink(
    target: &[u8],
    path: &Path,
) -> Result<(), BuildStagedOutputMaterializationError> {
    use std::os::unix::fs::symlink;
    let target = retained_symlink_target(target)?;
    symlink(&target, path).map_err(|error| {
        materialization_error(format!(
            "cannot create staged-output symlink `{}`: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn create_materialized_symlink(
    _target: &[u8],
    path: &Path,
) -> Result<(), BuildStagedOutputMaterializationError> {
    Err(materialization_error(format!(
        "this host cannot materialize staged-output symlink `{}`",
        path.display()
    )))
}

fn materialization_diagnostics(
    diagnostics: Vec<Diagnostic>,
) -> BuildStagedOutputMaterializationError {
    let message = diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>()
        .join("; ");
    materialization_error(message)
}

fn materialization_error(message: impl Into<String>) -> BuildStagedOutputMaterializationError {
    BuildStagedOutputMaterializationError {
        message: message.into(),
    }
}

fn capture_file(
    path: &Path,
    path_metadata: &std::fs::Metadata,
    expected_extent: u64,
) -> Result<CapturedFileContent, Vec<Diagnostic>> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        diagnostics(format!(
            "cannot open build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    let before = file.metadata().map_err(|error| {
        diagnostics(format!(
            "cannot inspect opened build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    if !same_file_observation(path_metadata, &before) || before.len() != expected_extent {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed before content capture",
            path.display()
        )));
    }
    let (first, bytes) = capture_reader(&mut file, expected_extent, path)?;
    file.rewind().map_err(|error| {
        diagnostics(format!(
            "cannot rewind build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    let second = hash_reader(&mut file, expected_extent, path)?;
    let after = file.metadata().map_err(|error| {
        diagnostics(format!(
            "cannot re-inspect build staged-output file `{}`: {error}",
            path.display()
        ))
    })?;
    if first != second || !same_file_observation(&before, &after) {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed while content was captured",
            path.display()
        )));
    }
    Ok(CapturedFileContent {
        length: expected_extent,
        digest: first,
        executable: is_executable(&before),
        identity: host_file_identity(&before),
        bytes,
    })
}

fn capture_reader(
    reader: &mut std::fs::File,
    expected_extent: u64,
    path: &Path,
) -> Result<([u8; 32], Arc<[u8]>), Vec<Diagnostic>> {
    let capacity = usize::try_from(expected_extent).map_err(|_| {
        diagnostics(format!(
            "build staged-output file `{}` is too large to retain on this compiler host",
            path.display()
        ))
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut digest = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|error| {
            diagnostics(format!(
                "cannot read build staged-output file `{}`: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            break;
        }
        total = total.checked_add(read as u64).ok_or_else(|| {
            diagnostics(format!(
                "build staged-output file `{}` length overflowed during capture",
                path.display()
            ))
        })?;
        if total > expected_extent {
            return Err(diagnostics(format!(
                "build staged-output file `{}` grew during capture",
                path.display()
            )));
        }
        digest.update(&buffer[..read]);
        bytes.extend_from_slice(&buffer[..read]);
    }
    if total != expected_extent {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed length during capture",
            path.display()
        )));
    }
    Ok((
        digest.finalize().into(),
        Arc::from(bytes.into_boxed_slice()),
    ))
}

#[cfg(unix)]
fn validate_hard_link_alias(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected_extent: u64,
    expected: &CapturedFileContent,
) -> Result<(), Vec<Diagnostic>> {
    if metadata.len() != expected_extent
        || host_file_identity(metadata) != expected.identity
        || is_executable(metadata) != expected.executable
    {
        return Err(diagnostics(format!(
            "build staged-output hard-link group disagrees at `{}`",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_hard_link_alias(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected_extent: u64,
    expected: &CapturedFileContent,
) -> Result<(), Vec<Diagnostic>> {
    let observed = capture_file(path, metadata, expected_extent)?;
    if &observed != expected {
        return Err(diagnostics(format!(
            "build staged-output hard-link group disagrees at `{}`",
            path.display()
        )));
    }
    Ok(())
}

fn hash_reader(
    reader: &mut std::fs::File,
    expected_extent: u64,
    path: &Path,
) -> Result<[u8; 32], Vec<Diagnostic>> {
    let mut digest = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|error| {
            diagnostics(format!(
                "cannot read build staged-output file `{}`: {error}",
                path.display()
            ))
        })?;
        if read == 0 {
            break;
        }
        total = total.checked_add(read as u64).ok_or_else(|| {
            diagnostics(format!(
                "build staged-output file `{}` length overflowed during capture",
                path.display()
            ))
        })?;
        if total > expected_extent {
            return Err(diagnostics(format!(
                "build staged-output file `{}` grew during capture",
                path.display()
            )));
        }
        digest.update(&buffer[..read]);
    }
    if total != expected_extent {
        return Err(diagnostics(format!(
            "build staged-output file `{}` changed length during capture",
            path.display()
        )));
    }
    Ok(digest.finalize().into())
}

fn canonical_relative_path(
    relative: &Path,
    diagnostic_path: &Path,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let mut output = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(diagnostics(format!(
                "build staged-output entry `{}` has a non-canonical relative path",
                diagnostic_path.display()
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            diagnostics(format!(
                "build staged-output entry `{}` has a non-UTF-8 path component",
                diagnostic_path.display()
            ))
        })?;
        validate_portable_component(component.as_bytes(), diagnostic_path)?;
        if !output.is_empty() {
            output.push(b'/');
        }
        output.extend_from_slice(component.as_bytes());
    }
    if output.is_empty() {
        return Err(diagnostics("build staged-output entry has an empty path"));
    }
    Ok(output)
}

fn canonical_symlink_target(
    target: &Path,
    link_relative_path: &[u8],
    diagnostic_path: &Path,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let target = target.to_str().ok_or_else(|| {
        diagnostics(format!(
            "build staged-output symlink `{}` has a non-UTF-8 target",
            diagnostic_path.display()
        ))
    })?;
    let bytes = target.as_bytes();
    if bytes.is_empty() || bytes.starts_with(b"/") || bytes.contains(&b'\\') || bytes.contains(&0) {
        return Err(diagnostics(format!(
            "build staged-output symlink `{}` must have a nonempty relative slash-separated target",
            diagnostic_path.display()
        )));
    }
    let mut resolved_depth = link_relative_path.split(|byte| *byte == b'/').count() - 1;
    for component in bytes.split(|byte| *byte == b'/') {
        if component.is_empty() || component == b"." {
            return Err(diagnostics(format!(
                "build staged-output symlink `{}` has a non-canonical target",
                diagnostic_path.display()
            )));
        }
        if component == b".." {
            if resolved_depth == 0 {
                return Err(diagnostics(format!(
                    "build staged-output symlink `{}` escapes the Output root",
                    diagnostic_path.display()
                )));
            }
            resolved_depth -= 1;
        } else {
            validate_portable_component(component, diagnostic_path)?;
            resolved_depth += 1;
        }
    }
    Ok(bytes.to_vec())
}

fn validate_portable_component(
    component: &[u8],
    diagnostic_path: &Path,
) -> Result<(), Vec<Diagnostic>> {
    if component.is_empty()
        || component == b"."
        || component == b".."
        || component.iter().any(|byte| {
            *byte < 0x20
                || matches!(
                    *byte,
                    b'\\' | b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|'
                )
        })
        || matches!(component.last(), Some(b'.' | b' '))
    {
        return Err(diagnostics(format!(
            "build staged-output entry `{}` has a non-portable path component",
            diagnostic_path.display()
        )));
    }
    let stem = component
        .split(|byte| *byte == b'.')
        .next()
        .unwrap_or(component);
    let reserved_device = [b"CON".as_slice(), b"PRN", b"AUX", b"NUL"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        || (stem.len() == 4
            && (stem[..3].eq_ignore_ascii_case(b"COM") || stem[..3].eq_ignore_ascii_case(b"LPT"))
            && matches!(stem[3], b'1'..=b'9'))
        || stem.eq_ignore_ascii_case(b"CONIN$")
        || stem.eq_ignore_ascii_case(b"CONOUT$");
    if reserved_device {
        return Err(diagnostics(format!(
            "build staged-output entry `{}` uses a reserved portable device name",
            diagnostic_path.display()
        )));
    }
    Ok(())
}

fn reserve_path_bytes(current: usize, additional: usize) -> Result<usize, Vec<Diagnostic>> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAX_STAGED_OUTPUT_PATH_BYTES)
        .ok_or_else(|| {
            diagnostics(format!(
                "build staged-output tree exceeds its {MAX_STAGED_OUTPUT_PATH_BYTES}-byte path and symlink-target ceiling"
            ))
        })
}

#[cfg(unix)]
fn host_file_identity(metadata: &std::fs::Metadata) -> HostFileIdentity {
    use std::os::unix::fs::MetadataExt;
    HostFileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(not(unix))]
fn host_file_identity(_metadata: &std::fs::Metadata) -> HostFileIdentity {
    HostFileIdentity
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn same_file_observation(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.file_type().is_file()
        && right.file_type().is_file()
        && left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mode() == right.mode()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file_observation(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.file_type().is_file()
        && right.file_type().is_file()
        && left.len() == right.len()
        && left.permissions().readonly() == right.permissions().readonly()
        && left.modified().ok() == right.modified().ok()
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("staged-output field length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

fn diagnostics(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn replayed_ordinary_files_are_canonical_and_order_independent() {
        let first = replayed_ordinary_files(&[
            (b"z.bin", b"last"),
            (b"a.bin", b"first"),
            (b"m.bin", b"middle"),
        ])
        .unwrap();
        let reordered = replayed_ordinary_files(&[
            (b"m.bin", b"middle"),
            (b"z.bin", b"last"),
            (b"a.bin", b"first"),
        ])
        .unwrap();
        assert_eq!(first, reordered);
        assert_eq!(first.entry_count(), 3);
        assert_eq!(first.file_bytes(), 15);
    }

    #[test]
    fn replayed_ordinary_files_reject_empty_duplicate_and_nested_shapes() {
        assert!(replayed_ordinary_files(&[]).is_err());
        assert!(
            replayed_ordinary_files(&[(b"same.bin", b"first"), (b"same.bin", b"second")]).is_err()
        );
        assert!(replayed_ordinary_files(&[(b"nested/file.bin", b"bytes")]).is_err());
    }

    #[test]
    fn replayed_file_commitment_binds_compiler_derived_executable_class() {
        let ordinary = replayed_files(&[(b"tool.bin", b"tool", false)]).unwrap();
        let executable = replayed_files(&[(b"tool.bin", b"tool", true)]).unwrap();
        assert_ne!(ordinary.digest(), executable.digest());
        assert_eq!(ordinary.file_bytes(), executable.file_bytes());
    }

    struct Fixture {
        session: PathBuf,
        root: PathBuf,
        sponsor: FilesystemSponsor,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let session = std::env::temp_dir().join(format!(
                "omega-staged-output-{label}-{}-{sequence}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&session);
            std::fs::create_dir(&session).unwrap();
            let session = std::fs::canonicalize(session).unwrap();
            let sponsor = FilesystemSponsor::new(&session).unwrap();
            let root = session.join("output");
            let fixture = Self {
                session,
                root,
                sponsor,
            };
            fixture.create_directory(Path::new(""));
            fixture
        }

        fn bind(&self, relative: &Path) -> psi_checked_interpreter::FilesystemSponsorPath {
            self.sponsor.bind_path(self.root.join(relative)).unwrap()
        }

        fn create_directory(&self, relative: &Path) {
            let path = self.root.join(relative);
            let prepared = self
                .sponsor
                .prepare_create_directory(&self.bind(relative))
                .unwrap();
            std::fs::create_dir(&path).unwrap();
            prepared.commit().unwrap();
        }

        fn create_file(&self, relative: &Path, bytes: &[u8]) {
            let prepared = self
                .sponsor
                .prepare_create_object(&self.bind(relative), bytes.len() as u64)
                .unwrap();
            std::fs::write(self.root.join(relative), bytes).unwrap();
            prepared.commit().unwrap();
        }

        fn empty_destination(&self, label: &str) -> PathBuf {
            let destination = self.session.join(label);
            std::fs::create_dir(&destination).unwrap();
            destination
        }

        #[cfg(unix)]
        fn create_symlink(&self, relative: &Path, target: &str) {
            use std::os::unix::fs::symlink;

            let prepared = self
                .sponsor
                .prepare_create_symlink(&self.bind(relative), target.as_bytes())
                .unwrap();
            symlink(target, self.root.join(relative)).unwrap();
            prepared.commit().unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.session);
        }
    }

    fn populated_fixture(label: &str, bytes: &[u8]) -> Fixture {
        let fixture = Fixture::new(label);
        fixture.create_directory(Path::new("nested"));
        fixture.create_file(Path::new("nested/artifact.bin"), bytes);
        fixture.create_directory(Path::new("empty"));
        fixture
    }

    #[test]
    fn commitment_is_relocation_stable_and_binds_paths_kinds_and_bytes() {
        let first = populated_fixture("first", b"payload");
        let relocated = populated_fixture("relocated", b"payload");
        let changed = populated_fixture("changed", b"changed");
        let first_commitment = capture(&first.root, &first.sponsor).unwrap();
        let relocated_commitment = capture(&relocated.root, &relocated.sponsor).unwrap();
        let changed_commitment = capture(&changed.root, &changed.sponsor).unwrap();
        assert_eq!(first_commitment, relocated_commitment);
        assert_eq!(first_commitment.entry_count(), 3);
        assert_eq!(first_commitment.file_bytes(), 7);
        assert_ne!(first_commitment.digest(), changed_commitment.digest());
    }

    #[test]
    fn included_sources_require_captured_regular_non_executable_omega_files() {
        let fixture = Fixture::new("included-source");
        fixture.create_file(Path::new("generated.omg"), b"data Generated {}\n");
        fixture.create_file(Path::new("artifact.bin"), b"bytes");
        fixture.create_directory(Path::new("directory.omg"));
        let retained = capture(&fixture.root, &fixture.sponsor).unwrap();

        assert!(
            select_included_sources(&retained, &[]).is_err(),
            "an Omega-looking output does not become source implicitly"
        );

        let selected = select_included_sources(&retained, &[b"generated.omg".to_vec()]).unwrap();
        let [selected] = selected.as_slice() else {
            panic!("one explicit handoff retains one source")
        };
        assert_eq!(selected.relative_path(), b"generated.omg");
        assert_eq!(selected.bytes(), b"data Generated {}\n");
        assert_eq!(
            selected.digest(),
            <[u8; 32]>::from(Sha256::digest(b"data Generated {}\n"))
        );

        for rejected in ["missing.omg", "artifact.bin", "directory.omg"] {
            assert!(
                select_included_sources(&retained, &[rejected.as_bytes().to_vec()]).is_err(),
                "{rejected} must not enter generated-source custody"
            );
        }
    }

    #[test]
    fn included_sources_reject_reserved_discovery_filenames() {
        for relative in ["build.omg", "nested/main.omg"] {
            let fixture = Fixture::new("reserved-source-name");
            if relative.contains('/') {
                fixture.create_directory(Path::new("nested"));
            }
            fixture.create_file(Path::new(relative), b"data Generated {}\n");
            let retained = capture(&fixture.root, &fixture.sponsor).unwrap();
            let diagnostics = select_included_sources(&retained, &[relative.as_bytes().to_vec()])
                .expect_err("generated source must not impersonate discovery roots");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("reserved source-discovery filename")),
                "{relative}: {diagnostics:#?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn commitment_normalizes_hard_links_and_binds_executable_mode() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new("hard-links");
        fixture.create_file(Path::new("first"), b"payload");
        let first = fixture.bind(Path::new("first"));
        let second = fixture.bind(Path::new("second"));
        let prepared = fixture.sponsor.prepare_hard_link(&first, &second).unwrap();
        std::fs::hard_link(fixture.root.join("first"), fixture.root.join("second")).unwrap();
        prepared.commit().unwrap();

        let ordinary = capture(&fixture.root, &fixture.sponsor).unwrap();
        assert_eq!(ordinary.entry_count(), 2);
        assert_eq!(ordinary.file_bytes(), 7);

        let duplicated_fixture = Fixture::new("duplicate-files");
        duplicated_fixture.create_file(Path::new("first"), b"payload");
        duplicated_fixture.create_file(Path::new("second"), b"payload");
        let duplicated = capture(&duplicated_fixture.root, &duplicated_fixture.sponsor).unwrap();
        assert_eq!(
            ordinary.commitment(),
            duplicated.commitment(),
            "canonical staged-output identity must not reveal hard-link topology"
        );

        let mut permissions = std::fs::metadata(fixture.root.join("first"))
            .unwrap()
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(fixture.root.join("first"), permissions).unwrap();
        let executable = capture(&fixture.root, &fixture.sponsor).unwrap();
        assert_ne!(ordinary.digest(), executable.digest());
        assert_eq!(executable.file_bytes(), 7);

        let destination = fixture.empty_destination("hard-link-materialization");
        assert_eq!(
            executable.materialize_into(&destination).unwrap(),
            executable.commitment()
        );
        assert_eq!(
            std::fs::read(destination.join("first")).unwrap(),
            b"payload"
        );
        assert_eq!(
            std::fs::read(destination.join("second")).unwrap(),
            b"payload"
        );
    }

    #[cfg(unix)]
    #[test]
    fn retained_tree_materializes_files_empty_directories_and_relative_symlinks() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new("materialize");
        fixture.create_directory(Path::new("bin"));
        fixture.create_directory(Path::new("empty"));
        fixture.create_file(Path::new("ordinary.txt"), b"ordinary\0bytes");
        fixture.create_file(Path::new("bin/tool"), b"#!/omega\n");
        let mut permissions = std::fs::metadata(fixture.root.join("bin/tool"))
            .unwrap()
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(fixture.root.join("bin/tool"), permissions).unwrap();
        fixture.create_symlink(Path::new("bin/ordinary-link"), "../ordinary.txt");

        let retained = capture(&fixture.root, &fixture.sponsor).unwrap();
        let destination = fixture.empty_destination("materialized");
        let materialized_commitment = retained.materialize_into(&destination).unwrap();

        assert_eq!(materialized_commitment, retained.commitment());
        assert_eq!(
            std::fs::read(destination.join("ordinary.txt")).unwrap(),
            b"ordinary\0bytes"
        );
        assert_eq!(
            std::fs::read(destination.join("bin/tool")).unwrap(),
            b"#!/omega\n"
        );
        assert!(destination.join("empty").is_dir());
        assert_eq!(
            std::fs::read_link(destination.join("bin/ordinary-link")).unwrap(),
            PathBuf::from("../ordinary.txt")
        );
        assert_eq!(
            std::fs::metadata(destination.join("ordinary.txt"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
        assert_eq!(
            std::fs::metadata(destination.join("bin/tool"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }

    #[test]
    fn retained_empty_tree_materializes_with_its_explicit_commitment() {
        let fixture = Fixture::new("empty-materialize");
        let retained = empty();
        let destination = fixture.empty_destination("materialized-empty");

        assert_eq!(
            retained.materialize_into(&destination).unwrap(),
            retained.commitment()
        );
        assert_eq!(retained.entry_count(), 0);
        assert_eq!(retained.file_bytes(), 0);
        assert!(std::fs::read_dir(destination).unwrap().next().is_none());
    }

    #[test]
    fn materialization_rejects_nonempty_destination_without_overwriting_it() {
        let fixture = populated_fixture("nonempty-destination", b"payload");
        let retained = capture(&fixture.root, &fixture.sponsor).unwrap();
        let destination = fixture.empty_destination("nonempty");
        std::fs::write(destination.join("sentinel"), b"owned by caller").unwrap();

        let error = retained.materialize_into(&destination).unwrap_err();
        assert!(error.message().contains("must be empty"));
        assert_eq!(
            std::fs::read(destination.join("sentinel")).unwrap(),
            b"owned by caller"
        );
    }

    #[cfg(unix)]
    #[test]
    fn materialization_rejects_symlink_destination() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new("symlink-destination");
        let retained = empty();
        let concrete = fixture.empty_destination("concrete");
        let destination = fixture.session.join("destination-link");
        symlink(&concrete, &destination).unwrap();

        let error = retained.materialize_into(&destination).unwrap_err();
        assert!(error.message().contains("existing concrete directory"));
    }

    #[test]
    fn materialization_rejects_tampered_content_and_invalid_retained_shape() {
        let fixture = populated_fixture("tamper", b"payload");
        let retained = capture(&fixture.root, &fixture.sponsor).unwrap();

        let mut invalid_shape = retained.clone();
        invalid_shape.entries[0].relative_path = b"../escape".to_vec();
        let shape_destination = fixture.empty_destination("invalid-shape");
        assert!(invalid_shape.materialize_into(&shape_destination).is_err());
        assert!(
            std::fs::read_dir(&shape_destination)
                .unwrap()
                .next()
                .is_none(),
            "shape validation must reject before materialization"
        );

        let mut tampered_content = retained;
        let file = tampered_content
            .entries
            .iter_mut()
            .find_map(|entry| match &mut entry.kind {
                RetainedStagedOutputEntryKind::File { bytes, .. } => Some(bytes),
                _ => None,
            })
            .expect("fixture has one retained file");
        *file = Arc::from(b"tampered".as_slice());
        let content_destination = fixture.empty_destination("tampered-content");
        assert!(
            tampered_content
                .materialize_into(&content_destination)
                .is_err()
        );
        assert!(
            std::fs::read_dir(&content_destination)
                .unwrap()
                .next()
                .is_none(),
            "commitment validation must reject before materialization"
        );
    }

    #[test]
    fn rejects_portability_collisions() {
        assert!(validate_portable_component(b"NUL.txt", Path::new("NUL.txt")).is_err());
        assert!(validate_portable_component(b"COM1", Path::new("COM1")).is_err());
        assert!(validate_portable_component(b"trailing.", Path::new("trailing.")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_external_symlink_targets_and_unsponsored_entries() {
        use std::os::unix::fs::symlink;

        let symlink_fixture = Fixture::new("symlink");
        let prepared = symlink_fixture
            .sponsor
            .prepare_create_symlink(&symlink_fixture.bind(Path::new("link")), b"/outside")
            .unwrap();
        symlink("/outside", symlink_fixture.root.join("link")).unwrap();
        prepared.commit().unwrap();
        assert!(capture(&symlink_fixture.root, &symlink_fixture.sponsor).is_err());

        let unsponsored_fixture = Fixture::new("unsponsored");
        std::fs::write(unsponsored_fixture.root.join("extra"), b"bytes").unwrap();
        assert!(capture(&unsponsored_fixture.root, &unsponsored_fixture.sponsor).is_err());
    }
}
