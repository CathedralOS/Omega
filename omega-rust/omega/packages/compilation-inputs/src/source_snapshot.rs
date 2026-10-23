//! Bind neutral source acquisition to Build's canonical input metadata.
//!
//! Acquisition owns immutable source trees independently of Build activation,
//! output staging, and materialization. Retained trees share their file bytes
//! here while Build metadata is derived in its own canonical hash domain;
//! acquisition's content identity never substitutes for that commitment.
//! Physical capture remains an adapter for sealed persisted custody and
//! standalone caller-authorized inventories. Retained input consumption never
//! queries a physical root, and scoped selection never follows captured links.

mod file_read;

/// The package boundary adapts Build inventory storage downward to the neutral
/// source protocol. Neither Psi nor Build-output knows the other's models.
pub(crate) fn captured_source_view(
    input: &build_output::CapturedBuildSourceInput,
) -> source::SourceSnapshot {
    source::SourceSnapshot::new(BuildSourceSnapshotStorage(input.clone()))
}

struct BuildSourceSnapshotStorage(build_output::CapturedBuildSourceInput);

fn snapshot_entry(entry: &build_output::CapturedSourceEntry) -> source::SourceSnapshotEntry<'_> {
    match entry {
        build_output::CapturedSourceEntry::Directory => source::SourceSnapshotEntry::Directory,
        build_output::CapturedSourceEntry::File { bytes, .. } => {
            source::SourceSnapshotEntry::File(bytes)
        }
        build_output::CapturedSourceEntry::Symlink { target, .. } => {
            source::SourceSnapshotEntry::Symlink(target)
        }
    }
}

impl source::SourceSnapshotStorage for BuildSourceSnapshotStorage {
    fn entry(&self, path: &[u8]) -> Option<source::SourceSnapshotEntry<'_>> {
        self.0.retained_entry(path).map(snapshot_entry)
    }

    fn entries(&self) -> Box<dyn Iterator<Item = (&[u8], source::SourceSnapshotEntry<'_>)> + '_> {
        Box::new(
            self.0
                .retained_entries()
                .map(|(path, entry)| (path, snapshot_entry(entry))),
        )
    }
}

#[cfg(unix)]
use crate::canonical_fs::require_canonical_mode;
use crate::canonical_fs::{canonical_file_executable, os_str_bytes};
use build_output::{CapturedBuildSourceInput, CapturedSourceEntry};
use file_read::{hash_canonical_source_file, read_canonical_source_file};
use filesystem_authority::{
    CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION, CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRow,
    CanonicalFilesystemMetadataRowKind, FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const CANONICAL_BUILD_SOURCE_CONTENT_DOMAIN: &[u8] = b"OMEGA-CANONICAL-BUILD-SOURCE-CONTENT-V1\0";
const CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT: u64 = 512 * 1024 * 1024;

/// Whether one declared capture entry must be present when the inventory is
/// taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSourceCaptureObligation {
    /// The member must exist under the captured root; absence fails capture.
    Required,
    /// Absence is itself committed: the inventory simply holds no member at
    /// that path.
    Optional,
}

/// The explicit, caller-authorized source inventory for one standalone build
/// activation: exact canonical relative paths under the captured root.
///
/// A declared file or symlink is one member; a declared directory member
/// admits its whole subtree. Ancestor directories are committed as
/// directory rows so every member keeps canonical parents, but they never
/// widen the inventory — only a declared member opens a subtree. Two
/// entries where one nests inside the other would let one declaration
/// silently admit another's subtree, so the request rejects them instead
/// of guessing at the intended boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSourceCaptureRequest {
    entries: BTreeMap<Vec<u8>, BuildSourceCaptureObligation>,
}

impl BuildSourceCaptureRequest {
    /// The declared inventory keyed by canonical relative path. Every entry
    /// is a nonempty canonical relative spelling; duplicates and
    /// nested-declaration pairs reject.
    pub fn new(
        entries: impl IntoIterator<Item = (Vec<u8>, BuildSourceCaptureObligation)>,
    ) -> Result<Self, String> {
        let mut request = BTreeMap::new();
        for (relative_path, obligation) in entries {
            // Physical package metadata also describes raw host names. A
            // caller-authored logical inventory has the narrower portable
            // protocol: UTF-8 and no drive-qualified spellings on any host.
            let drive_qualified = relative_path.split(|byte| *byte == b'/').any(|component| {
                component.first().is_some_and(u8::is_ascii_alphabetic)
                    && component.get(1) == Some(&b':')
            });
            if std::str::from_utf8(&relative_path).is_err()
                || drive_qualified
                || !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(
                    &relative_path,
                    false,
                )
            {
                return Err(format!(
                    "source capture entry is not a canonical relative path: {relative_path:?}"
                ));
            }
            if request.insert(relative_path.clone(), obligation).is_some() {
                return Err(format!(
                    "source capture entry is declared twice: {relative_path:?}"
                ));
            }
        }
        for relative_path in request.keys() {
            for (end, _) in relative_path
                .iter()
                .enumerate()
                .filter(|(_, byte)| **byte == b'/')
            {
                if request.contains_key(&relative_path[..end]) {
                    return Err(format!(
                        "source capture entry {relative_path:?} nests inside declared entry {:?}",
                        &relative_path[..end]
                    ));
                }
            }
        }
        Ok(Self { entries: request })
    }

    /// The declared entries in canonical path order.
    pub fn entries(&self) -> impl Iterator<Item = (&[u8], BuildSourceCaptureObligation)> {
        self.entries
            .iter()
            .map(|(path, obligation)| (path.as_slice(), *obligation))
    }
}

#[derive(Debug)]
struct CapturedSourceMetadataRow {
    kind: CanonicalFilesystemMetadataRowKind,
    content_digest: Option<[u8; 32]>,
}

/// Derive Build's complete inventory from acquisition's immutable namespace.
/// The implicit root becomes a canonical directory row. File allocations
/// stay shared with the neutral source tree; link targets retain their raw
/// platform spelling as inert bytes, without compiler namespace traversal.
pub(super) fn retained_source_input(
    tree: &source::CapturedSourceTree,
) -> Result<CapturedBuildSourceInput, String> {
    let entries = tree.entries().map(|(path, node)| {
        let relative_path = capture_relative_path_bytes(path)?;
        let entry = match node {
            source::CapturedSourceNode::Directory => CapturedSourceEntry::Directory,
            source::CapturedSourceNode::File { bytes, executable } => {
                CapturedSourceEntry::file(Arc::clone(bytes), *executable)
            }
            source::CapturedSourceNode::Symlink { target } => {
                CapturedSourceEntry::symlink(os_str_bytes(target.as_os_str())?)
            }
        };
        Ok((relative_path, entry))
    });
    retained_entries_input(entries)
}

/// Narrow an authenticated retained inventory using the same membership
/// policy as physical scoped capture. Required members are already resolved
/// root-relative source paths from the invocation's source filesystem.
pub(super) fn retained_scoped_source_input(
    complete: &CapturedBuildSourceInput,
    request: &BuildSourceCaptureRequest,
    required_members: impl IntoIterator<Item = PathBuf>,
) -> Result<CapturedBuildSourceInput, String> {
    let root = CapturedSourceEntry::Directory;
    let mut available = complete
        .retained_entries()
        .map(|(path, entry)| (path.to_vec(), entry))
        .collect::<BTreeMap<_, _>>();
    available.insert(Vec::new(), &root);
    let selected = select_scoped_members(
        request,
        |path| Ok(available.get(path).copied()),
        |entry| matches!(entry, CapturedSourceEntry::Directory),
        |path| {
            let mut prefix = path.to_vec();
            prefix.push(b'/');
            Ok(available
                .range(prefix.clone()..)
                .take_while(|(child, _)| child.starts_with(&prefix))
                .filter(|(child, _)| !child[prefix.len()..].contains(&b'/'))
                .map(|(child, _)| child.clone())
                .collect())
        },
    )?;
    require_source_members(&selected, required_members)?;
    retained_entries_input(
        selected
            .into_iter()
            .filter(|(path, _)| !path.is_empty())
            .map(|(path, entry)| Ok((path, entry.clone()))),
    )
}

fn retained_entries_input(
    entries: impl IntoIterator<Item = Result<(Vec<u8>, CapturedSourceEntry), String>>,
) -> Result<CapturedBuildSourceInput, String> {
    let mut rows = BTreeMap::from([(
        Vec::new(),
        CapturedSourceMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::Directory,
            content_digest: None,
        },
    )]);
    let mut retained = Vec::new();
    let mut aggregate_path_bytes = 0usize;
    let mut aggregate_content_bytes = 0u64;
    for entry in entries {
        if rows.len() >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
            return Err(format!(
                "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
            ));
        }
        let (relative_path, entry) = entry?;
        charge_capture_path_bytes(&mut aggregate_path_bytes, &relative_path)?;
        if !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(
            &relative_path,
            false,
        ) {
            return Err(format!(
                "captured Source path is not canonical metadata: {relative_path:?}"
            ));
        }
        let kind = match &entry {
            CapturedSourceEntry::Directory => CanonicalFilesystemMetadataRowKind::Directory,
            CapturedSourceEntry::File {
                bytes, executable, ..
            } => {
                let logical_byte_length = u64::try_from(bytes.len())
                    .map_err(|_| "canonical Source file length exceeds u64".to_owned())?;
                charge_canonical_source_content(&mut aggregate_content_bytes, logical_byte_length)?;
                CanonicalFilesystemMetadataRowKind::File {
                    executable: *executable,
                    logical_byte_length,
                }
            }
            CapturedSourceEntry::Symlink { target, .. } => {
                let target_spelling_logical_byte_length = u64::try_from(target.len())
                    .map_err(|_| "canonical Source symlink target length exceeds u64".to_owned())?;
                charge_canonical_source_content(
                    &mut aggregate_content_bytes,
                    target_spelling_logical_byte_length,
                )?;
                CanonicalFilesystemMetadataRowKind::Symlink {
                    target_spelling_logical_byte_length,
                }
            }
        };
        let row = CapturedSourceMetadataRow {
            kind,
            content_digest: entry.content_digest(),
        };
        if rows.insert(relative_path.clone(), row).is_some() {
            return Err(format!(
                "captured Source duplicated a path: {relative_path:?}"
            ));
        }
        retained.push((relative_path, entry));
    }
    CapturedBuildSourceInput::from_capture_rows(finish_capture_index(rows)?, retained).map_err(
        |diagnostics| {
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        },
    )
}

pub(crate) fn capture_relative_path_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let mut relative = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(format!(
                "source member {} is not a canonical relative path",
                path.display()
            ));
        };
        if !relative.is_empty() {
            relative.push(b'/');
        }
        relative.extend_from_slice(&os_str_bytes(name)?);
    }
    Ok(relative)
}

fn require_source_members<Member>(
    selected: &BTreeMap<Vec<u8>, Member>,
    required_members: impl IntoIterator<Item = PathBuf>,
) -> Result<(), String> {
    for member in required_members {
        let relative = capture_relative_path_bytes(&member)?;
        if !selected.contains_key(&relative) {
            return Err(format!(
                "scoped source inventory omits required source member {}",
                member.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn capture(root: &Path) -> Result<CanonicalFilesystemMetadataIndex, String> {
    let (index, _) = metadata_rows(root)?;
    Ok(index)
}

/// One metadata-only traversal of the canonical source root: the kind, size,
/// and content commitment of every member, without retaining file bytes.
fn metadata_rows(
    root: &Path,
) -> Result<(CanonicalFilesystemMetadataIndex, Vec<(Vec<u8>, ())>), String> {
    capture_rows(
        root.to_path_buf(),
        |path, physical, aggregate_content_bytes| {
            capture_physical_metadata_row(path, physical, aggregate_content_bytes)
                .map(|row| (row, ()))
        },
    )
}

/// Capture one complete compiler-owned immutable build input for this exact
/// physical source root: every directory, every regular file's exact bytes,
/// and every inert relative symlink spelling, bound to the same canonical
/// source metadata index the build's filesystem grants will enforce.
///
/// The retained entries and the index's source-content commitment come from
/// the same traversal, so the retained bytes and granted metadata cannot
/// disagree. Each member's own read rejects identity or content drift between
/// its inspection and its consumption; nothing re-traverses the root to
/// compare the index against itself, because both sides of that comparison
/// would come from this one call and neither is the tree a later reader meets.
/// Callers supply sealed resolver-owned backing and capture before execution;
/// a later replay never rereads the host to rebuild this input.
pub fn capture_package_source_input(
    source_root: &Path,
) -> Result<CapturedBuildSourceInput, String> {
    let canonical_root = crate::package_compilation::canonical_source_root(source_root)?;
    let (index, retained) = capture_rows(canonical_root.clone(), capture_physical_source_row)?;
    CapturedBuildSourceInput::from_capture_rows(index, retained).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

/// Capture one standalone invocation's explicit source inventory: exactly
/// the members the caller authorized, plus the ancestor directory rows every
/// member's canonical parentage requires. The root row is committed but
/// never enumerated, so members outside the request are never read — a
/// local build does not implicitly expose the working directory.
///
/// `required_members` are the source files the compilation already
/// consumed. Every one located under this root must be a captured member,
/// so a request cannot silently omit a file the assembled program needed;
/// members outside the root (toolchain and injected sources) are not this
/// inventory's members and are skipped.
pub(crate) fn capture_scoped_source_input(
    source_root: &Path,
    request: &BuildSourceCaptureRequest,
    required_members: impl IntoIterator<Item = PathBuf>,
) -> Result<CapturedBuildSourceInput, String> {
    let canonical_root = crate::package_compilation::canonical_source_root(source_root)?;
    let (index, retained) =
        capture_scoped_rows(canonical_root.clone(), request, capture_physical_source_row)?;
    let captured: BTreeSet<&[u8]> = retained.iter().map(|(path, _)| path.as_slice()).collect();
    for member in required_members {
        let relative = match member.canonicalize() {
            Ok(canonical) => canonical
                .strip_prefix(&canonical_root)
                .ok()
                .map(Path::to_path_buf),
            // An unresolvable member still named under the root was deleted
            // after consumption; the membership check below fails it.
            Err(_) => member.strip_prefix(source_root).ok().map(Path::to_path_buf),
        };
        let Some(relative) = relative else { continue };
        let relative_bytes = capture_relative_path_bytes(&relative)?;
        if !captured.contains(relative_bytes.as_slice()) {
            return Err(format!(
                "scoped source inventory omits required source member {}",
                member.display()
            ));
        }
    }
    CapturedBuildSourceInput::from_capture_rows(index, retained).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

/// One traversal of the canonical sealed source root. Every visited
/// path produces both its canonical metadata row and one capture-authority
/// payload, so a caller cannot assemble an index and a retained inventory
/// that disagree with each other.
fn capture_rows<T>(
    canonical_root: PathBuf,
    mut row_for: impl FnMut(
        &Path,
        &std::fs::Metadata,
        &mut u64,
    ) -> Result<(CapturedSourceMetadataRow, T), String>,
) -> Result<(CanonicalFilesystemMetadataIndex, Vec<(Vec<u8>, T)>), String> {
    let mut stack = vec![(canonical_root, Vec::<u8>::new())];
    let mut rows = BTreeMap::<Vec<u8>, CapturedSourceMetadataRow>::new();
    let mut retained = Vec::<(Vec<u8>, T)>::new();
    let mut aggregate_path_bytes = 0usize;
    let mut aggregate_content_bytes = 0u64;

    while let Some((path, relative_path)) = stack.pop() {
        let physical = std::fs::symlink_metadata(&path).map_err(|error| {
            format!("could not inspect canonical Source metadata path: {error}")
        })?;
        if admit_capture_row(
            &path,
            &relative_path,
            &physical,
            &mut aggregate_content_bytes,
            &mut rows,
            &mut retained,
            &mut row_for,
        )? {
            enqueue_capture_children(
                &path,
                &relative_path,
                &rows,
                &mut stack,
                &mut aggregate_path_bytes,
            )?;
        }
    }
    Ok((finish_capture_index(rows)?, retained))
}

/// One traversal restricted to a declared inventory. The root row is
/// admitted but never enumerated; each declared member is admitted and —
/// for directories — opened for subtree enumeration. Ancestor directories
/// are admitted as rows without opening their own subtrees, and a declared
/// required member absent under the root fails capture.
fn capture_scoped_rows<T>(
    canonical_root: PathBuf,
    request: &BuildSourceCaptureRequest,
    mut row_for: impl FnMut(
        &Path,
        &std::fs::Metadata,
        &mut u64,
    ) -> Result<(CapturedSourceMetadataRow, T), String>,
) -> Result<(CanonicalFilesystemMetadataIndex, Vec<(Vec<u8>, T)>), String> {
    let selected = select_scoped_members(
        request,
        |relative| {
            let path = join_capture_path(&canonical_root, relative)?;
            match std::fs::symlink_metadata(path) {
                Ok(metadata) => Ok(Some(metadata)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(format!(
                    "could not inspect canonical Source metadata path: {error}"
                )),
            }
        },
        std::fs::Metadata::is_dir,
        |relative| {
            let path = join_capture_path(&canonical_root, relative)?;
            let children = std::fs::read_dir(path).map_err(|error| {
                format!("could not enumerate canonical Source metadata directory: {error}")
            })?;
            let mut child_paths = Vec::new();
            for child in children {
                if child_paths.len() >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
                    return Err(format!(
                        "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
                    ));
                }
                let child = child.map_err(|error| {
                    format!("could not enumerate canonical Source metadata entry: {error}")
                })?;
                let mut child_relative = relative.to_vec();
                if !child_relative.is_empty() {
                    child_relative.push(b'/');
                }
                child_relative.extend_from_slice(&os_str_bytes(&child.file_name())?);
                child_paths.push(child_relative);
            }
            Ok(child_paths)
        },
    )?;
    let mut rows = BTreeMap::new();
    let mut retained = Vec::new();
    let mut aggregate_content_bytes = 0u64;
    for (relative_path, physical) in selected {
        let path = join_capture_path(&canonical_root, &relative_path)?;
        admit_capture_row(
            &path,
            &relative_path,
            &physical,
            &mut aggregate_content_bytes,
            &mut rows,
            &mut retained,
            &mut row_for,
        )?;
    }
    Ok((finish_capture_index(rows)?, retained))
}

/// Select exact inventory members independently of their backing authority.
/// Ancestors only supply directory rows; declaring a directory opens its
/// subtree, and neither ancestor inspection nor enumeration follows links.
fn select_scoped_members<Member>(
    request: &BuildSourceCaptureRequest,
    mut lookup: impl FnMut(&[u8]) -> Result<Option<Member>, String>,
    is_directory: impl Fn(&Member) -> bool,
    mut children: impl FnMut(&[u8]) -> Result<Vec<Vec<u8>>, String>,
) -> Result<BTreeMap<Vec<u8>, Member>, String> {
    let root = lookup(&[])?
        .filter(&is_directory)
        .ok_or_else(|| "canonical Source root is not a directory".to_owned())?;
    let mut selected = BTreeMap::from([(Vec::new(), root)]);
    let mut pending = Vec::new();
    let mut aggregate_path_bytes = 0usize;
    for (declared, obligation) in request.entries() {
        let mut reachable = true;
        for (end, _) in declared
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'/')
        {
            let ancestor = &declared[..end];
            if let Some(member) = selected.get(ancestor) {
                if !is_directory(member) {
                    reachable = false;
                    break;
                }
                continue;
            }
            let Some(member) = lookup(ancestor)?.filter(&is_directory) else {
                reachable = false;
                break;
            };
            charge_scoped_member(
                &selected,
                pending.len(),
                &mut aggregate_path_bytes,
                ancestor,
            )?;
            selected.insert(ancestor.to_vec(), member);
        }
        let member = if reachable { lookup(declared)? } else { None };
        match member {
            Some(member) => {
                charge_scoped_member(
                    &selected,
                    pending.len(),
                    &mut aggregate_path_bytes,
                    declared,
                )?;
                pending.push((declared.to_vec(), member));
            }
            None if obligation == BuildSourceCaptureObligation::Required => {
                return Err(format!(
                    "required source capture entry `{}` is absent",
                    String::from_utf8_lossy(declared)
                ));
            }
            None => {}
        }
    }
    while let Some((path, member)) = pending.pop() {
        if is_directory(&member) {
            for child in children(&path)? {
                charge_scoped_member(
                    &selected,
                    pending.len() + 1,
                    &mut aggregate_path_bytes,
                    &child,
                )?;
                let member = lookup(&child)?.ok_or_else(|| {
                    format!(
                        "source capture member `{}` disappeared during capture",
                        String::from_utf8_lossy(&child)
                    )
                })?;
                pending.push((child, member));
            }
        }
        if selected.insert(path.clone(), member).is_some() {
            return Err(format!("Source selection duplicated a path: {path:?}"));
        }
    }
    Ok(selected)
}

fn charge_scoped_member<Member>(
    selected: &BTreeMap<Vec<u8>, Member>,
    pending_count: usize,
    aggregate_path_bytes: &mut usize,
    path: &[u8],
) -> Result<(), String> {
    if selected
        .len()
        .checked_add(pending_count)
        .is_none_or(|count| count >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT)
    {
        return Err(format!(
            "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
        ));
    }
    if !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(path, false) {
        return Err(format!("Source path is not canonical metadata: {path:?}"));
    }
    charge_capture_path_bytes(aggregate_path_bytes, path)
}

/// Admit one visited physical member: ceiling, row construction, duplicate
/// detection, and retention of every non-root entry. Returns whether the
/// member is a concrete directory whose children should be enumerated.
fn admit_capture_row<T>(
    path: &Path,
    relative_path: &[u8],
    physical: &std::fs::Metadata,
    aggregate_content_bytes: &mut u64,
    rows: &mut BTreeMap<Vec<u8>, CapturedSourceMetadataRow>,
    retained: &mut Vec<(Vec<u8>, T)>,
    row_for: &mut impl FnMut(
        &Path,
        &std::fs::Metadata,
        &mut u64,
    ) -> Result<(CapturedSourceMetadataRow, T), String>,
) -> Result<bool, String> {
    if rows.len() >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
        return Err(format!(
            "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
        ));
    }
    let (captured, entry) = row_for(path, physical, aggregate_content_bytes)?;
    if rows.insert(relative_path.to_vec(), captured).is_some() {
        return Err(format!(
            "physical Source traversal duplicated a path: {relative_path:?}"
        ));
    }
    if !relative_path.is_empty() {
        retained.push((relative_path.to_vec(), entry));
    }
    Ok(physical.is_dir())
}

/// Push one directory's children onto the traversal stack under the shared
/// row and path-byte ceilings and the canonical-path gate.
fn enqueue_capture_children(
    path: &Path,
    relative_path: &[u8],
    rows: &BTreeMap<Vec<u8>, CapturedSourceMetadataRow>,
    stack: &mut Vec<(PathBuf, Vec<u8>)>,
    aggregate_path_bytes: &mut usize,
) -> Result<(), String> {
    let children = std::fs::read_dir(path).map_err(|error| {
        format!("could not enumerate canonical Source metadata directory: {error}")
    })?;
    for child in children {
        let child = child.map_err(|error| {
            format!("could not enumerate canonical Source metadata entry: {error}")
        })?;
        crate::canonical_fs::check_metadata_row_ceiling(rows.len(), stack.len())?;
        let name = os_str_bytes(&child.file_name())?;
        let mut child_relative = relative_path.to_vec();
        if !child_relative.is_empty() {
            child_relative.push(b'/');
        }
        child_relative.extend_from_slice(&name);
        charge_capture_path_bytes(aggregate_path_bytes, &child_relative)?;
        if !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(
            &child_relative,
            false,
        ) {
            return Err(format!(
                "physical Source path is not canonical metadata: {child_relative:?}"
            ));
        }
        stack.push((child.path(), child_relative));
    }
    Ok(())
}

fn charge_capture_path_bytes(
    aggregate_path_bytes: &mut usize,
    relative_path: &[u8],
) -> Result<(), String> {
    *aggregate_path_bytes = aggregate_path_bytes
        .checked_add(relative_path.len())
        .filter(|bytes| *bytes <= FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT)
        .ok_or_else(|| {
            format!(
                "canonical Source metadata path bytes exceed {FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT}"
            )
        })?;
    Ok(())
}

fn join_capture_path(root: &Path, relative_path: &[u8]) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for component in relative_path.split(|byte| *byte == b'/') {
        path.push(os_str_from_bytes(component)?);
    }
    Ok(path)
}

fn finish_capture_index(
    rows: BTreeMap<Vec<u8>, CapturedSourceMetadataRow>,
) -> Result<CanonicalFilesystemMetadataIndex, String> {
    let commitment = canonical_build_source_content_commitment(&rows)?;
    CanonicalFilesystemMetadataIndex::version_1(
        commitment,
        rows.into_iter()
            .map(|(path, row)| CanonicalFilesystemMetadataRow::new(path, row.kind)),
    )
    .map_err(|error| format!("could not construct canonical Source metadata: {error}"))
}

fn capture_physical_metadata_row(
    path: &Path,
    metadata: &std::fs::Metadata,
    aggregate_content_bytes: &mut u64,
) -> Result<CapturedSourceMetadataRow, String> {
    if metadata.is_dir() {
        #[cfg(unix)]
        require_canonical_mode(path, metadata, 0o555)?;
        return Ok(CapturedSourceMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::Directory,
            content_digest: None,
        });
    }
    if metadata.is_file() {
        let executable = canonical_file_executable(path, metadata)?;
        charge_canonical_source_content(aggregate_content_bytes, metadata.len())?;
        let content_digest = hash_canonical_source_file(path, metadata)?;
        return Ok(CapturedSourceMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::File {
                executable,
                logical_byte_length: metadata.len(),
            },
            content_digest: Some(content_digest),
        });
    }
    if metadata.file_type().is_symlink() {
        let (target, target_length) = crate::canonical_fs::canonical_symlink_target(
            path,
            aggregate_content_bytes,
            CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT,
        )?;
        return Ok(CapturedSourceMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length: target_length,
            },
            content_digest: Some(crate::digest_encoder::content_identity(&target).into()),
        });
    }
    Err(format!(
        "physical Source path {} has an unsupported filesystem kind",
        path.display()
    ))
}

fn capture_physical_source_row(
    path: &Path,
    metadata: &std::fs::Metadata,
    aggregate_content_bytes: &mut u64,
) -> Result<(CapturedSourceMetadataRow, CapturedSourceEntry), String> {
    if metadata.is_dir() {
        #[cfg(unix)]
        require_canonical_mode(path, metadata, 0o555)?;
        return Ok((
            CapturedSourceMetadataRow {
                kind: CanonicalFilesystemMetadataRowKind::Directory,
                content_digest: None,
            },
            CapturedSourceEntry::Directory,
        ));
    }
    if metadata.is_file() {
        let executable = canonical_file_executable(path, metadata)?;
        charge_canonical_source_content(aggregate_content_bytes, metadata.len())?;
        let bytes = read_canonical_source_file(path, metadata)?;
        let entry = CapturedSourceEntry::file(bytes, executable);
        return Ok((
            CapturedSourceMetadataRow {
                kind: CanonicalFilesystemMetadataRowKind::File {
                    executable,
                    logical_byte_length: metadata.len(),
                },
                content_digest: entry.content_digest(),
            },
            entry,
        ));
    }
    if metadata.file_type().is_symlink() {
        let (target, target_length) = crate::canonical_fs::canonical_symlink_target(
            path,
            aggregate_content_bytes,
            CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT,
        )?;
        let entry = CapturedSourceEntry::symlink(target);
        return Ok((
            CapturedSourceMetadataRow {
                kind: CanonicalFilesystemMetadataRowKind::Symlink {
                    target_spelling_logical_byte_length: target_length,
                },
                content_digest: entry.content_digest(),
            },
            entry,
        ));
    }
    Err(format!(
        "physical Source path {} has an unsupported filesystem kind",
        path.display()
    ))
}

fn charge_canonical_source_content(total: &mut u64, amount: u64) -> Result<(), String> {
    *total = total
        .checked_add(amount)
        .filter(|total| *total <= CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT)
        .ok_or_else(|| {
            format!(
                "canonical Source content exceeds its {CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT}-byte ceiling"
            )
        })?;
    Ok(())
}

fn canonical_build_source_content_commitment(
    rows: &BTreeMap<Vec<u8>, CapturedSourceMetadataRow>,
) -> Result<[u8; 32], String> {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(CANONICAL_BUILD_SOURCE_CONTENT_DOMAIN);
    digest.u32(CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION);
    digest.u64(
        u64::try_from(rows.len())
            .map_err(|_| "canonical Source row count exceeds u64".to_owned())?,
    );
    for (path, row) in rows {
        digest.bytes(path);
        match row.kind {
            CanonicalFilesystemMetadataRowKind::Directory => digest.byte(0),
            CanonicalFilesystemMetadataRowKind::File {
                executable,
                logical_byte_length,
            } => {
                digest.byte(1);
                digest.byte(u8::from(executable));
                digest.u64(logical_byte_length);
                digest.raw(
                    &row.content_digest
                        .ok_or_else(|| "canonical Source file omits content digest".to_owned())?,
                );
            }
            CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length,
            } => {
                digest.byte(2);
                digest.u64(target_spelling_logical_byte_length);
                digest.raw(
                    &row.content_digest.ok_or_else(|| {
                        "canonical Source symlink omits content digest".to_owned()
                    })?,
                );
            }
        }
    }
    Ok(digest.finish())
}

#[cfg(unix)]
fn os_str_from_bytes(bytes: &[u8]) -> Result<std::ffi::OsString, String> {
    use std::os::unix::ffi::OsStrExt;
    Ok(std::ffi::OsStr::from_bytes(bytes).to_os_string())
}

#[cfg(not(unix))]
fn os_str_from_bytes(bytes: &[u8]) -> Result<std::ffi::OsString, String> {
    std::str::from_utf8(bytes)
        .map(std::ffi::OsString::from)
        .map_err(|_| "physical Source path is not portable UTF-8".to_owned())
}
