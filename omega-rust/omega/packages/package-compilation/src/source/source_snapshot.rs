/*
We bind neutral source acquisition (`source::CapturedSourceTree`) and physical package roots to `Build`'s canonical input metadata (`CanonicalFilesystemMetadataIndex`, `CapturedBuildSourceInput`).

We provide three capture and projection routes:
1. `retained_source_input` and `retained_scoped_source_input` derive `CapturedBuildSourceInput` directly from an already-captured in-memory `CapturedSourceTree` or `CapturedBuildSourceInput` without touching the host filesystem, sharing `Arc<[u8]>` file buffers while computing `Build`'s canonical metadata commitment under `OMEGA-CANONICAL-BUILD-SOURCE-CONTENT-V1`.
2. `capture_package_source_input` and `capture` traverse a physical sealed package root in a single pass, enforcing canonical POSIX modes (`0o555` directories, `0o444`/`0o555` files), row/byte ceilings, and drift-checked file reads (`file_read`) so the retained bytes and `CanonicalFilesystemMetadataIndex` cannot disagree.
3. `capture_scoped_source_input` restricts physical capture to an explicit caller-authorized `BuildSourceCaptureRequest` inventory (admitting ancestor directory rows without enumerating unselected siblings) and verifies that every `required_members` source file consumed by compilation under that root is included.
*/

/*
Why `BuildSourceCaptureRequest` rejects nested entry declarations:
A declared file or symlink admits one member, whereas a declared directory admits its entire subtree. If one declared entry nested inside another, the outer directory declaration would silently swallow the inner declaration's boundary and obligation (`Required` vs `Optional`). Rejecting nested declarations upfront forces the caller to state non-overlapping inventory boundaries.
*/

/*
Provenance:
- Drift-checked canonical file reading and hashing: `src/source/source_snapshot/file_read.rs`.
*/

mod file_read;

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
use crate::source::canonical_fs::require_canonical_mode;
use crate::source::canonical_fs::{canonical_file_executable, os_str_bytes};
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

pub use filesystem_authority::{
    SourceCaptureObligation as BuildSourceCaptureObligation,
    SourceCaptureRequest as BuildSourceCaptureRequest,
};

#[derive(Debug)]
struct CapturedSourceMetadataRow {
    kind: CanonicalFilesystemMetadataRowKind,
    content_digest: Option<[u8; 32]>,
}

pub(crate) fn retained_source_input(
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

pub(crate) fn retained_scoped_source_input(
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
    let selected = filesystem_authority::select_source_members(
        request,
        CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
        FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT,
        |path| Ok(available.get(path).copied()),
        |entry| matches!(entry, CapturedSourceEntry::Directory),
        |path| {
            let mut prefix = path.to_vec();
            if !prefix.is_empty() {
                prefix.push(b'/');
            }
            Ok(available
                .range(prefix.clone()..)
                .take_while(|(child, _)| child.starts_with(&prefix))
                .filter(|(child, _)| {
                    child.len() > prefix.len() && !child[prefix.len()..].contains(&b'/')
                })
                .map(|(child, _)| child.clone())
                .collect())
        },
        std::convert::identity,
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

pub(crate) fn capture(root: &Path) -> Result<CanonicalFilesystemMetadataIndex, String> {
    let (index, _) = metadata_rows(root)?;
    Ok(index)
}

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

fn capture_scoped_rows<T>(
    canonical_root: PathBuf,
    request: &BuildSourceCaptureRequest,
    mut row_for: impl FnMut(
        &Path,
        &std::fs::Metadata,
        &mut u64,
    ) -> Result<(CapturedSourceMetadataRow, T), String>,
) -> Result<(CanonicalFilesystemMetadataIndex, Vec<(Vec<u8>, T)>), String> {
    let selected = filesystem_authority::select_source_members(
        request,
        CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
        FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT,
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
        std::convert::identity,
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
        crate::source::canonical_fs::check_metadata_row_ceiling(rows.len(), stack.len())?;
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
        let (target, target_length) = crate::source::canonical_fs::canonical_symlink_target(
            path,
            aggregate_content_bytes,
            CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT,
        )?;
        return Ok(CapturedSourceMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length: target_length,
            },
            content_digest: Some(crate::digest_encoder::content_identity(&target)),
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
        let (target, target_length) = crate::source::canonical_fs::canonical_symlink_target(
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
