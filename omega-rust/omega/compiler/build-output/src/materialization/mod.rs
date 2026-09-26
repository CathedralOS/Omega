//! Materializing a retained tree onto the host filesystem and verifying the
//! result against its commitment.

use crate::capture::{is_executable, same_file_observation};
use crate::portable_paths::{
    canonical_relative_path, canonical_symlink_target, validate_portable_component,
};
use crate::staged_output_tree::{
    MAX_STAGED_OUTPUT_ENTRIES, MAX_STAGED_OUTPUT_PATH_BYTES, MAX_STAGED_OUTPUT_UNIQUE_FILE_BYTES,
    RetainedStagedOutputEntryKind, commitment_for_retained_entries,
};
use crate::{
    BuildStagedOutputMaterializationError, BuildStagedOutputTree, BuildStagedOutputTreeCommitment,
};
use diagnostics::Diagnostic;
use sha2::Digest;
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) fn materialize_retained_tree(
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

pub(crate) fn validate_retained_tree(
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

pub(crate) fn verify_materialized_tree(
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

pub(crate) fn retained_native_path(
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
