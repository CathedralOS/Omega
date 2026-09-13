//! Captures and revalidates the complete physical Source root seen by build evaluation.
//! Rows and content commitments are always reconstructed from the filesystem.

use checked_interpreter::{
    CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION, CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRow,
    CanonicalFilesystemMetadataRowKind, FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

const CANONICAL_BUILD_SOURCE_CONTENT_DOMAIN: &[u8] = b"OMEGA-CANONICAL-BUILD-SOURCE-CONTENT-V1\0";
const CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT: u64 = 512 * 1024 * 1024;

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
    let mut stack = vec![(root.to_path_buf(), Vec::<u8>::new())];
    let mut rows = BTreeMap::<Vec<u8>, CapturedPhysicalMetadataRow>::new();
    let mut aggregate_path_bytes = 0usize;
    let mut aggregate_content_bytes = 0u64;

    while let Some((path, relative_path)) = stack.pop() {
        if rows.len() >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
            return Err(format!(
                "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
            ));
        }
        let physical = std::fs::symlink_metadata(&path).map_err(|error| {
            format!("could not inspect canonical Source metadata path: {error}")
        })?;
        let captured =
            capture_physical_metadata_row(&path, &physical, &mut aggregate_content_bytes)?;
        if rows.insert(relative_path.clone(), captured).is_some() {
            return Err(format!(
                "physical Source traversal duplicated a path: {relative_path:?}"
            ));
        }

        if physical.is_dir() {
            let children = std::fs::read_dir(&path).map_err(|error| {
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
                let mut child_relative = relative_path.clone();
                if !child_relative.is_empty() {
                    child_relative.push(b'/');
                }
                child_relative.extend_from_slice(&name);
                aggregate_path_bytes = aggregate_path_bytes
                    .checked_add(child_relative.len())
                    .filter(|bytes| *bytes <= FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT)
                    .ok_or_else(|| {
                        format!(
                            "canonical Source metadata path bytes exceed {FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT}"
                        )
                    })?;
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
        }
    }

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

fn hash_canonical_source_file(
    path: &Path,
    initial_metadata: &std::fs::Metadata,
) -> Result<[u8; 32], String> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        format!(
            "could not open canonical Source file {}: {error}",
            path.display()
        )
    })?;
    let mut digest = Sha256::new();
    let mut observed = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            format!(
                "could not read canonical Source file {}: {error}",
                path.display()
            )
        })?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(u64::try_from(count).expect("fixed buffer read fits u64"))
            .filter(|observed| *observed <= initial_metadata.len())
            .ok_or_else(|| {
                format!(
                    "canonical Source file {} grew while captured",
                    path.display()
                )
            })?;
        digest.update(&buffer[..count]);
    }
    if observed != initial_metadata.len() {
        return Err(format!(
            "canonical Source file {} changed length while captured",
            path.display()
        ));
    }
    let final_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!(
            "could not recheck canonical Source file {}: {error}",
            path.display()
        )
    })?;
    if !final_metadata.is_file() || final_metadata.len() != initial_metadata.len() {
        return Err(format!(
            "canonical Source file {} changed identity while captured",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if final_metadata.dev() != initial_metadata.dev()
            || final_metadata.ino() != initial_metadata.ino()
            || final_metadata.mode() != initial_metadata.mode()
        {
            return Err(format!(
                "canonical Source file {} changed identity or mode while captured",
                path.display()
            ));
        }
    }
    Ok(digest.finalize().into())
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
