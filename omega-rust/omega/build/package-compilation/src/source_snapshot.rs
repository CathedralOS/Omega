//! Captures and revalidates the complete physical Source root seen by build evaluation.
//! Rows and content commitments are always reconstructed from the filesystem.

mod file_read;

use build_output::{CapturedBuildSourceInput, CapturedSourceEntry};
use checked_interpreter::{
    CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION, CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRow,
    CanonicalFilesystemMetadataRowKind, FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT,
};
use file_read::{hash_canonical_source_file, read_canonical_source_file};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
                || !checked_interpreter::canonical_filesystem_metadata_path_is_canonical(
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

pub(super) fn validate_current(
    root: &Path,
    metadata: &CanonicalFilesystemMetadataIndex,
) -> Result<(), String> {
    let observed = capture(root)?;
    if &observed == metadata {
        Ok(())
    } else {
        Err(
            "canonical Source metadata or content commitment no longer matches the complete physical root"
                .to_owned(),
        )
    }
}

#[derive(Debug)]
struct CapturedPhysicalMetadataRow {
    kind: CanonicalFilesystemMetadataRowKind,
    content_digest: Option<[u8; 32]>,
}

pub(super) fn capture(root: &Path) -> Result<CanonicalFilesystemMetadataIndex, String> {
    let (index, _) = metadata_rows(root)?;
    require_stable_recapture(&index, || metadata_rows(root).map(|(index, _)| index))?;
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

/// Re-traverse the admitted membership and require the second traversal to
/// reproduce the committed index exactly. Per-member reads already reject
/// local drift between inspection and consumption, but a single traversal
/// still pairs each member's row with whatever tree it happened to meet; a
/// member created, removed, renamed, retargeted, or edited between the two
/// traversals produces a different index, so the committed rows cannot
/// describe two different trees at once.
fn require_stable_recapture(
    index: &CanonicalFilesystemMetadataIndex,
    recapture: impl FnOnce() -> Result<CanonicalFilesystemMetadataIndex, String>,
) -> Result<(), String> {
    let revalidated = recapture()?;
    if &revalidated == index {
        Ok(())
    } else {
        Err(
            "canonical Source metadata changed during capture; the complete physical root is not stable"
                .to_owned(),
        )
    }
}

/// Capture one complete compiler-owned immutable build input for this exact
/// physical source root: every directory, every regular file's exact bytes,
/// and every inert relative symlink spelling, bound to the same canonical
/// source metadata index the build's filesystem grants will enforce.
///
/// The retained entries and the index's source-content commitment come from
/// the same traversal, so the retained bytes and granted metadata cannot
/// disagree. A second metadata-only traversal must reproduce the committed
/// index before publication: membership, link, and content changes spanning
/// the traversal reject instead of mixing rows from two different trees.
/// File reads reject detected identity or content drift; these
/// checks do not make a mutable host tree atomic. Callers supply sealed
/// resolver-owned backing and capture before execution; a later replay never
/// rereads the host to rebuild this input.
pub fn capture_package_source_input(
    source_root: &Path,
) -> Result<CapturedBuildSourceInput, String> {
    let canonical_root = crate::package_compilation::canonical_source_root(source_root)?;
    let (index, retained) = capture_rows(canonical_root.clone(), capture_physical_source_row)?;
    require_stable_recapture(&index, || {
        metadata_rows(&canonical_root).map(|(index, _)| index)
    })?;
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
pub fn capture_scoped_source_input(
    source_root: &Path,
    request: &BuildSourceCaptureRequest,
    required_members: impl IntoIterator<Item = PathBuf>,
) -> Result<CapturedBuildSourceInput, String> {
    let canonical_root = crate::package_compilation::canonical_source_root(source_root)?;
    let (index, retained) =
        capture_scoped_rows(canonical_root.clone(), request, capture_physical_source_row)?;
    require_stable_recapture(&index, || {
        capture_scoped_rows(
            canonical_root.clone(),
            request,
            |path, physical, aggregate| {
                capture_physical_metadata_row(path, physical, aggregate).map(|row| (row, ()))
            },
        )
        .map(|(index, _)| index)
    })?;
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
        let mut relative_bytes = Vec::new();
        for component in relative.components() {
            let std::path::Component::Normal(name) = component else {
                return Err(format!(
                    "required source member {} is not a canonical relative path",
                    member.display()
                ));
            };
            if !relative_bytes.is_empty() {
                relative_bytes.push(b'/');
            }
            relative_bytes.extend_from_slice(&os_str_bytes(name)?);
        }
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
    ) -> Result<(CapturedPhysicalMetadataRow, T), String>,
) -> Result<(CanonicalFilesystemMetadataIndex, Vec<(Vec<u8>, T)>), String> {
    let mut stack = vec![(canonical_root, Vec::<u8>::new())];
    let mut rows = BTreeMap::<Vec<u8>, CapturedPhysicalMetadataRow>::new();
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
    ) -> Result<(CapturedPhysicalMetadataRow, T), String>,
) -> Result<(CanonicalFilesystemMetadataIndex, Vec<(Vec<u8>, T)>), String> {
    let mut stack = Vec::<(PathBuf, Vec<u8>)>::new();
    let mut rows = BTreeMap::<Vec<u8>, CapturedPhysicalMetadataRow>::new();
    let mut retained = Vec::<(Vec<u8>, T)>::new();
    let mut aggregate_path_bytes = 0usize;
    let mut aggregate_content_bytes = 0u64;

    let root_physical = std::fs::symlink_metadata(&canonical_root)
        .map_err(|error| format!("could not inspect canonical Source root: {error}"))?;
    if !root_physical.is_dir() {
        return Err("canonical Source root is not a directory".to_owned());
    }
    admit_capture_row(
        &canonical_root,
        &[],
        &root_physical,
        &mut aggregate_content_bytes,
        &mut rows,
        &mut retained,
        &mut row_for,
    )?;

    for (declared, obligation) in request.entries() {
        // Admit each missing ancestor directory row so a committed member
        // always keeps canonical parents. An ancestor that is absent or not
        // a concrete directory leaves the member unreachable: under
        // no-traversal a symlink ancestor never opens a path.
        let mut ancestor_relative = Vec::<u8>::new();
        let mut reachable = true;
        let components: Vec<&[u8]> = declared.split(|byte| *byte == b'/').collect();
        for component in &components[..components.len() - 1] {
            if !ancestor_relative.is_empty() {
                ancestor_relative.push(b'/');
            }
            ancestor_relative.extend_from_slice(component);
            match rows.get(&ancestor_relative) {
                Some(row) if matches!(row.kind, CanonicalFilesystemMetadataRowKind::Directory) => {
                    continue;
                }
                Some(_) => {
                    reachable = false;
                    break;
                }
                None => {}
            }
            let ancestor_path = join_capture_path(&canonical_root, &ancestor_relative)?;
            let physical = match std::fs::symlink_metadata(&ancestor_path) {
                Ok(physical) => physical,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    reachable = false;
                    break;
                }
                Err(error) => {
                    return Err(format!(
                        "could not inspect canonical Source metadata path: {error}"
                    ));
                }
            };
            if !physical.is_dir() {
                reachable = false;
                break;
            }
            charge_capture_path_bytes(&mut aggregate_path_bytes, &ancestor_relative)?;
            admit_capture_row(
                &ancestor_path,
                &ancestor_relative,
                &physical,
                &mut aggregate_content_bytes,
                &mut rows,
                &mut retained,
                &mut row_for,
            )?;
        }
        if !reachable {
            match obligation {
                BuildSourceCaptureObligation::Required => {
                    return Err(format!(
                        "required source capture entry `{}` is absent",
                        String::from_utf8_lossy(declared)
                    ));
                }
                BuildSourceCaptureObligation::Optional => continue,
            }
        }
        let physical_path = join_capture_path(&canonical_root, declared)?;
        match std::fs::symlink_metadata(&physical_path) {
            Ok(_) => {
                charge_capture_path_bytes(&mut aggregate_path_bytes, declared)?;
                stack.push((physical_path, declared.to_vec()));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => match obligation {
                BuildSourceCaptureObligation::Required => {
                    return Err(format!(
                        "required source capture entry `{}` is absent",
                        String::from_utf8_lossy(declared)
                    ));
                }
                BuildSourceCaptureObligation::Optional => {}
            },
            Err(error) => {
                return Err(format!(
                    "could not inspect canonical Source metadata path: {error}"
                ));
            }
        }
    }

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

/// Admit one visited physical member: ceiling, row construction, duplicate
/// detection, and retention of every non-root entry. Returns whether the
/// member is a concrete directory whose children should be enumerated.
fn admit_capture_row<T>(
    path: &Path,
    relative_path: &[u8],
    physical: &std::fs::Metadata,
    aggregate_content_bytes: &mut u64,
    rows: &mut BTreeMap<Vec<u8>, CapturedPhysicalMetadataRow>,
    retained: &mut Vec<(Vec<u8>, T)>,
    row_for: &mut impl FnMut(
        &Path,
        &std::fs::Metadata,
        &mut u64,
    ) -> Result<(CapturedPhysicalMetadataRow, T), String>,
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
    rows: &BTreeMap<Vec<u8>, CapturedPhysicalMetadataRow>,
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
        if rows
            .len()
            .checked_add(stack.len())
            .and_then(|count| count.checked_add(1))
            .is_none_or(|count| count > CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT)
        {
            return Err(format!(
                "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
            ));
        }
        let name = os_str_bytes(&child.file_name())?;
        let mut child_relative = relative_path.to_vec();
        if !child_relative.is_empty() {
            child_relative.push(b'/');
        }
        child_relative.extend_from_slice(&name);
        charge_capture_path_bytes(aggregate_path_bytes, &child_relative)?;
        if !checked_interpreter::canonical_filesystem_metadata_path_is_canonical(
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
    rows: BTreeMap<Vec<u8>, CapturedPhysicalMetadataRow>,
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
) -> Result<CapturedPhysicalMetadataRow, String> {
    if metadata.is_dir() {
        #[cfg(unix)]
        require_canonical_mode(path, metadata, 0o555)?;
        return Ok(CapturedPhysicalMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::Directory,
            content_digest: None,
        });
    }
    if metadata.is_file() {
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode() & 0o777;
            match mode {
                0o444 => false,
                0o555 => true,
                _ => {
                    return Err(format!(
                        "physical Source file {} has noncanonical mode {mode:#o}",
                        path.display()
                    ));
                }
            }
        };
        #[cfg(not(unix))]
        let executable = false;
        charge_canonical_source_content(aggregate_content_bytes, metadata.len())?;
        let content_digest = hash_canonical_source_file(path, metadata)?;
        return Ok(CapturedPhysicalMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::File {
                executable,
                logical_byte_length: metadata.len(),
            },
            content_digest: Some(content_digest),
        });
    }
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(path).map_err(|error| {
            format!(
                "could not read canonical Source symlink {}: {error}",
                path.display()
            )
        })?;
        let target = os_str_bytes(target.as_os_str())?;
        let target_length = u64::try_from(target.len())
            .map_err(|_| "canonical Source symlink target length exceeds u64".to_owned())?;
        charge_canonical_source_content(aggregate_content_bytes, target_length)?;
        return Ok(CapturedPhysicalMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length: target_length,
            },
            content_digest: Some(Sha256::digest(&target).into()),
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
) -> Result<(CapturedPhysicalMetadataRow, CapturedSourceEntry), String> {
    if metadata.is_dir() {
        #[cfg(unix)]
        require_canonical_mode(path, metadata, 0o555)?;
        return Ok((
            CapturedPhysicalMetadataRow {
                kind: CanonicalFilesystemMetadataRowKind::Directory,
                content_digest: None,
            },
            CapturedSourceEntry::Directory,
        ));
    }
    if metadata.is_file() {
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode() & 0o777;
            match mode {
                0o444 => false,
                0o555 => true,
                _ => {
                    return Err(format!(
                        "physical Source file {} has noncanonical mode {mode:#o}",
                        path.display()
                    ));
                }
            }
        };
        #[cfg(not(unix))]
        let executable = false;
        charge_canonical_source_content(aggregate_content_bytes, metadata.len())?;
        let bytes = read_canonical_source_file(path, metadata)?;
        let entry = CapturedSourceEntry::file(bytes, executable);
        return Ok((
            CapturedPhysicalMetadataRow {
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
        let target = std::fs::read_link(path).map_err(|error| {
            format!(
                "could not read canonical Source symlink {}: {error}",
                path.display()
            )
        })?;
        let target = os_str_bytes(target.as_os_str())?;
        let target_length = u64::try_from(target.len())
            .map_err(|_| "canonical Source symlink target length exceeds u64".to_owned())?;
        charge_canonical_source_content(aggregate_content_bytes, target_length)?;
        let entry = CapturedSourceEntry::symlink(target);
        return Ok((
            CapturedPhysicalMetadataRow {
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
    rows: &BTreeMap<Vec<u8>, CapturedPhysicalMetadataRow>,
) -> Result<[u8; 32], String> {
    let mut digest = Sha256::new();
    digest.update(CANONICAL_BUILD_SOURCE_CONTENT_DOMAIN);
    digest.update(CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION.to_le_bytes());
    digest.update(
        u64::try_from(rows.len())
            .map_err(|_| "canonical Source row count exceeds u64".to_owned())?
            .to_le_bytes(),
    );
    for (path, row) in rows {
        hash_framed_bytes(&mut digest, path)?;
        match row.kind {
            CanonicalFilesystemMetadataRowKind::Directory => digest.update([0]),
            CanonicalFilesystemMetadataRowKind::File {
                executable,
                logical_byte_length,
            } => {
                digest.update([1, u8::from(executable)]);
                digest.update(logical_byte_length.to_le_bytes());
                digest.update(
                    row.content_digest
                        .ok_or_else(|| "canonical Source file omits content digest".to_owned())?,
                );
            }
            CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length,
            } => {
                digest.update([2]);
                digest.update(target_spelling_logical_byte_length.to_le_bytes());
                digest.update(
                    row.content_digest.ok_or_else(|| {
                        "canonical Source symlink omits content digest".to_owned()
                    })?,
                );
            }
        }
    }
    Ok(digest.finalize().into())
}

fn hash_framed_bytes(digest: &mut Sha256, bytes: &[u8]) -> Result<(), String> {
    digest.update(
        u64::try_from(bytes.len())
            .map_err(|_| "canonical Source path length exceeds u64".to_owned())?
            .to_le_bytes(),
    );
    digest.update(bytes);
    Ok(())
}

#[cfg(unix)]
fn require_canonical_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected: u32,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode() & 0o777;
    if mode == expected {
        Ok(())
    } else {
        Err(format!(
            "physical Source directory {} has noncanonical mode {mode:#o}; expected {expected:#o}",
            path.display()
        ))
    }
}

#[cfg(unix)]
fn os_str_bytes(value: &std::ffi::OsStr) -> Result<Vec<u8>, String> {
    use std::os::unix::ffi::OsStrExt;
    Ok(value.as_bytes().to_vec())
}

#[cfg(not(unix))]
fn os_str_bytes(value: &std::ffi::OsStr) -> Result<Vec<u8>, String> {
    value
        .to_str()
        .map(|value| value.as_bytes().to_vec())
        .ok_or_else(|| "physical Source path is not portable UTF-8".to_owned())
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

#[cfg(test)]
#[path = "source_snapshot/file_read_tests.rs"]
mod file_read_tests;

#[cfg(test)]
mod capture_coherence_tests {
    use super::{
        BuildSourceCaptureObligation, BuildSourceCaptureRequest, capture,
        capture_physical_metadata_row, capture_scoped_rows, metadata_rows,
        require_stable_recapture,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct CaptureTree(PathBuf);

    impl CaptureTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "omega-source-capture-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create capture fixture root");
            Self(path.canonicalize().expect("canonicalize fixture root"))
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn write(&self, relative: &str, bytes: &[u8]) {
            fs::write(self.0.join(relative), bytes).expect("write fixture member");
        }

        fn mkdir(&self, relative: &str) {
            fs::create_dir(self.0.join(relative)).expect("create fixture directory");
        }

        fn seal(&self) {
            set_tree_permissions(&self.0, true);
        }

        fn unseal(&self) {
            set_tree_permissions(&self.0, false);
        }
    }

    impl Drop for CaptureTree {
        fn drop(&mut self) {
            set_tree_permissions(&self.0, false);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Canonical mode discipline only applies on hosts where the capture
    /// checks it; other platforms keep the fixture writable throughout.
    #[cfg(unix)]
    fn set_tree_permissions(root: &Path, sealed: bool) {
        use std::os::unix::fs::PermissionsExt;
        let dir_mode = if sealed { 0o555 } else { 0o755 };
        let file_mode = if sealed { 0o444 } else { 0o644 };
        let mut stack = vec![root.to_path_buf()];
        while let Some(directory) = stack.pop() {
            for entry in fs::read_dir(&directory).expect("enumerate fixture") {
                let entry = entry.expect("fixture entry");
                let kind = entry.file_type().expect("fixture member kind");
                if kind.is_symlink() {
                    continue;
                }
                if kind.is_dir() {
                    stack.push(entry.path());
                }
                let mode = if kind.is_dir() { dir_mode } else { file_mode };
                fs::set_permissions(entry.path(), fs::Permissions::from_mode(mode))
                    .expect("set fixture permissions");
            }
        }
        fs::set_permissions(root, fs::Permissions::from_mode(dir_mode)).expect("seal fixture root");
    }

    #[cfg(not(unix))]
    fn set_tree_permissions(_root: &Path, _sealed: bool) {}

    fn recapture_index(
        root: &Path,
    ) -> Result<checked_interpreter::CanonicalFilesystemMetadataIndex, String> {
        metadata_rows(root).map(|(index, _)| index)
    }

    fn scoped_recapture_index(
        root: &Path,
        request: &BuildSourceCaptureRequest,
    ) -> Result<checked_interpreter::CanonicalFilesystemMetadataIndex, String> {
        capture_scoped_rows(root.to_path_buf(), request, |path, physical, aggregate| {
            capture_physical_metadata_row(path, physical, aggregate).map(|row| (row, ()))
        })
        .map(|(index, _)| index)
    }

    #[test]
    fn stable_tree_recapture_reproduces_the_committed_index() {
        let tree = CaptureTree::new();
        tree.write("main.omg", b"data Main {}\n");
        tree.mkdir("docs");
        tree.write("docs/readme.md", b"readme\n");
        tree.seal();

        let index = capture(tree.path()).expect("capture stable fixture");
        require_stable_recapture(&index, || recapture_index(tree.path()))
            .expect("an unchanged tree must revalidate");
    }

    #[test]
    fn recapture_rejects_a_member_added_after_the_first_traversal() {
        let tree = CaptureTree::new();
        tree.write("main.omg", b"data Main {}\n");
        tree.mkdir("docs");
        tree.write("docs/readme.md", b"readme\n");
        tree.seal();

        let index = capture(tree.path()).expect("capture stable fixture");
        tree.unseal();
        tree.write("docs/added.omg", b"data Added {}\n");
        tree.seal();

        let error = require_stable_recapture(&index, || recapture_index(tree.path()))
            .expect_err("a member added after traversal cannot inherit the committed index");
        assert!(error.contains("not stable"), "{error}");
    }

    #[test]
    fn recapture_rejects_a_member_removed_after_the_first_traversal() {
        let tree = CaptureTree::new();
        tree.write("main.omg", b"data Main {}\n");
        tree.write("extra.omg", b"data Extra {}\n");
        tree.seal();

        let index = capture(tree.path()).expect("capture stable fixture");
        tree.unseal();
        fs::remove_file(tree.path().join("extra.omg")).expect("remove fixture member");
        tree.seal();

        assert!(
            require_stable_recapture(&index, || recapture_index(tree.path())).is_err(),
            "a member removed after traversal cannot inherit the committed index"
        );
    }

    #[test]
    fn recapture_rejects_same_length_content_replacement() {
        let tree = CaptureTree::new();
        tree.write("main.omg", b"data Main {}\n");
        tree.seal();

        let index = capture(tree.path()).expect("capture stable fixture");
        tree.unseal();
        tree.write("main.omg", b"data Repl {}\n");
        tree.seal();

        assert!(
            require_stable_recapture(&index, || recapture_index(tree.path())).is_err(),
            "edited bytes cannot inherit the earlier commitment"
        );
    }

    #[cfg(unix)]
    #[test]
    fn recapture_rejects_a_retargeted_link_after_the_first_traversal() {
        let tree = CaptureTree::new();
        tree.write("main.omg", b"data Main {}\n");
        tree.mkdir("docs");
        tree.mkdir("other");
        std::os::unix::fs::symlink("docs", tree.path().join("link")).expect("create fixture link");
        tree.seal();

        let index = capture(tree.path()).expect("capture stable fixture");
        tree.unseal();
        fs::remove_file(tree.path().join("link")).expect("unlink fixture link");
        std::os::unix::fs::symlink("other", tree.path().join("link"))
            .expect("retarget fixture link");
        tree.seal();

        assert!(
            require_stable_recapture(&index, || recapture_index(tree.path())).is_err(),
            "a retargeted link cannot inherit the earlier commitment"
        );
    }

    #[test]
    fn scoped_recapture_rejects_inventory_drift_after_the_first_traversal() {
        let tree = CaptureTree::new();
        tree.write("main.omg", b"data Main {}\n");
        tree.mkdir("docs");
        tree.write("docs/readme.md", b"readme\n");
        tree.seal();

        let request = BuildSourceCaptureRequest::new([
            (b"main.omg".to_vec(), BuildSourceCaptureObligation::Required),
            (b"docs".to_vec(), BuildSourceCaptureObligation::Required),
        ])
        .expect("fixture inventory");
        let index = scoped_recapture_index(tree.path(), &request)
            .expect("scoped capture of stable fixture");
        tree.unseal();
        tree.write("docs/added.omg", b"data Added {}\n");
        tree.seal();

        assert!(
            require_stable_recapture(&index, || { scoped_recapture_index(tree.path(), &request) })
                .is_err(),
            "a member added to a declared subtree cannot inherit the committed index"
        );
    }
}

#[cfg(test)]
mod capture_request_tests {
    use super::{BuildSourceCaptureObligation, BuildSourceCaptureRequest};

    #[test]
    fn logical_inventory_rejects_drive_components_and_non_utf8_on_every_host() {
        for path in [
            b"C:/file".as_slice(),
            b"c:relative",
            b"sub/C:relative",
            b"\xff",
        ] {
            assert!(
                BuildSourceCaptureRequest::new([(
                    path.to_vec(),
                    BuildSourceCaptureObligation::Required
                ),])
                .is_err(),
                "{path:?}"
            );
        }
        assert!(
            BuildSourceCaptureRequest::new([(
                "templates/é.txt".as_bytes().to_vec(),
                BuildSourceCaptureObligation::Optional
            ),])
            .is_ok()
        );
    }
}
