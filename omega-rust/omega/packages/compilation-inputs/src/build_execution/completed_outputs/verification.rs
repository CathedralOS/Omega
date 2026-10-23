//! Check an earlier host export before reusing its completed-set name.
//!
//! The current compile's retained tree remains authoritative. Unlike fresh
//! materialization, an existing directory is untrusted input: the commit marker,
//! exact namespace, file kinds, executable modes and bytes must all match. Capability-relative
//! no-follow opens keep substituted links from selecting unrelated host files.
//! CapabilityMetadata comparisons detect observed replacement during this check; they do
//! not freeze a user-selected export or establish snapshot isolation against a
//! process with the same authority. Consumers retain the manifest-bound bytes,
//! rather than treating this mutable host directory as new source authority.

use build_output::{BuildStagedOutputEntryKind, BuildStagedOutputTree};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{
    Dir as CapabilityDirectory, Metadata as CapabilityMetadata,
    OpenOptions as CapabilityOpenOptions,
};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;

enum ExpectedEntry<'tree> {
    Directory,
    File {
        bytes: &'tree [u8],
        executable: bool,
    },
}

pub(super) fn verify_existing_set(
    build_directory: &Path,
    name: &str,
    tree: &BuildStagedOutputTree,
    manifest: &[u8],
) -> Result<(), String> {
    let check = || -> Result<(), String> {
        let build =
            CapabilityDirectory::open_ambient_dir(build_directory, cap_std::ambient_authority())
                .map_err(|error| error.to_string())?;
        let sets = build
            .open_dir_nofollow("completed")
            .map_err(|error| error.to_string())?;
        let sets_metadata = sets.dir_metadata().map_err(|error| error.to_string())?;
        let directory = sets
            .open_dir_nofollow(name)
            .map_err(|error| error.to_string())?;
        let initial = directory
            .dir_metadata()
            .map_err(|error| error.to_string())?;
        require_same(
            &initial,
            &sets
                .symlink_metadata(name)
                .map_err(|error| error.to_string())?,
        )?;
        let mut expected = BTreeMap::from([
            (b"files".to_vec(), ExpectedEntry::Directory),
            (
                b"manifest.bin".to_vec(),
                ExpectedEntry::File {
                    bytes: manifest,
                    executable: false,
                },
            ),
        ]);
        for entry in tree.entries() {
            let mut path = b"files/".to_vec();
            path.extend_from_slice(entry.relative_path());
            let kind = match entry.kind() {
                BuildStagedOutputEntryKind::Directory => ExpectedEntry::Directory,
                BuildStagedOutputEntryKind::File { bytes, executable } => {
                    ExpectedEntry::File { bytes, executable }
                }
                BuildStagedOutputEntryKind::Symlink { .. } => {
                    return Err("completed file products cannot contain symbolic links".into());
                }
            };
            expected.insert(path, kind);
        }
        verify_directory(&directory, b"", &mut expected)?;
        if let Some((missing, _)) = expected.first_key_value() {
            return Err(format!(
                "missing entry `{}`",
                String::from_utf8_lossy(missing)
            ));
        }
        require_same(
            &initial,
            &sets
                .symlink_metadata(name)
                .map_err(|error| error.to_string())?,
        )?;
        require_same(
            &sets_metadata,
            &build
                .symlink_metadata("completed")
                .map_err(|error| error.to_string())?,
        )
    };
    check().map_err(|reason| format!(
        "existing completed-output set `{}` does not match its retained manifest and files: {reason}",
        build_directory.join("completed").join(name).display()
    ))
}

fn verify_directory(
    directory: &CapabilityDirectory,
    prefix: &[u8],
    expected: &mut BTreeMap<Vec<u8>, ExpectedEntry<'_>>,
) -> Result<(), String> {
    let initial = directory
        .dir_metadata()
        .map_err(|error| error.to_string())?;
    for entry in directory.entries().map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let spelling = name.to_str().ok_or("existing output name is not UTF-8")?;
        let mut path = prefix.to_vec();
        if !path.is_empty() {
            path.push(b'/');
        }
        path.extend_from_slice(spelling.as_bytes());
        let kind = expected
            .remove(&path)
            .ok_or_else(|| format!("unexpected entry `{}`", String::from_utf8_lossy(&path)))?;
        let metadata = directory
            .symlink_metadata(&name)
            .map_err(|error| error.to_string())?;
        match kind {
            ExpectedEntry::Directory => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(format!("entry `{spelling}` is not a concrete directory"));
                }
                let child = directory
                    .open_dir_nofollow(&name)
                    .map_err(|error| error.to_string())?;
                require_same(
                    &metadata,
                    &child.dir_metadata().map_err(|error| error.to_string())?,
                )?;
                verify_directory(&child, &path, expected)?;
                require_same(
                    &metadata,
                    &directory
                        .symlink_metadata(&name)
                        .map_err(|error| error.to_string())?,
                )?;
            }
            ExpectedEntry::File { bytes, executable } => {
                verify_file(directory, &name, &metadata, bytes, executable)?;
            }
        }
    }
    require_same(
        &initial,
        &directory
            .dir_metadata()
            .map_err(|error| error.to_string())?,
    )
}

fn verify_file(
    directory: &CapabilityDirectory,
    name: &OsStr,
    classified: &CapabilityMetadata,
    expected: &[u8],
    executable: bool,
) -> Result<(), String> {
    if classified.file_type().is_symlink() || !classified.is_file() {
        return Err(format!(
            "entry `{}` is not a regular file",
            name.to_string_lossy()
        ));
    }
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = directory
        .open_with(name, &options)
        .map_err(|error| error.to_string())?;
    let initial = file.metadata().map_err(|error| error.to_string())?;
    require_same(classified, &initial)?;
    if initial.len() != expected.len() as u64 || is_executable(&initial) != executable {
        return Err(format!(
            "entry `{}` has different length or executable mode",
            name.to_string_lossy()
        ));
    }
    if !has_single_link(&initial)? {
        return Err("existing completed-output file is a hard-link alias".into());
    }
    let mut chunk = [0u8; 64 * 1024];
    let mut offset = 0usize;
    loop {
        let count = file.read(&mut chunk).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        let end = offset.checked_add(count).ok_or("output length overflow")?;
        if expected.get(offset..end) != Some(&chunk[..count]) {
            return Err(format!(
                "entry `{}` has different bytes",
                name.to_string_lossy()
            ));
        }
        offset = end;
    }
    if offset != expected.len() {
        return Err("existing output was truncated while reading".into());
    }
    require_same(
        &initial,
        &file.metadata().map_err(|error| error.to_string())?,
    )?;
    require_same(
        &initial,
        &directory
            .symlink_metadata(name)
            .map_err(|error| error.to_string())?,
    )
}

fn require_same(initial: &CapabilityMetadata, observed: &CapabilityMetadata) -> Result<(), String> {
    #[cfg(not(windows))]
    let identity_matches = cap_fs_ext::MetadataExt::dev(initial)
        == cap_fs_ext::MetadataExt::dev(observed)
        && cap_fs_ext::MetadataExt::ino(initial) == cap_fs_ext::MetadataExt::ino(observed);
    #[cfg(windows)]
    let identity_matches = {
        use cap_primitives::fs::_WindowsByHandle;
        let (Some(left_volume), Some(left_index), Some(right_volume), Some(right_index)) = (
            initial.volume_serial_number(),
            initial.file_index(),
            observed.volume_serial_number(),
            observed.file_index(),
        ) else {
            return Err("cannot verify completed-output identity: Windows by-handle metadata is unavailable".into());
        };
        left_volume == right_volume && left_index == right_index
    };
    #[cfg(unix)]
    let change_time_matches = {
        use cap_fs_ext::OsMetadataExt;
        initial.ctime() == observed.ctime() && initial.ctime_nsec() == observed.ctime_nsec()
    };
    #[cfg(not(unix))]
    let change_time_matches = true;
    if !identity_matches
        || !change_time_matches
        || initial.file_type() != observed.file_type()
        || initial.len() != observed.len()
        || initial.permissions() != observed.permissions()
        || initial.modified().map_err(|error| error.to_string())?
            != observed.modified().map_err(|error| error.to_string())?
    {
        return Err("existing completed-output entry changed during verification".into());
    }
    Ok(())
}

#[cfg(unix)]
fn is_executable(metadata: &CapabilityMetadata) -> bool {
    use cap_fs_ext::OsMetadataExt;
    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &CapabilityMetadata) -> bool {
    false
}

#[cfg(not(windows))]
fn has_single_link(metadata: &CapabilityMetadata) -> Result<bool, String> {
    Ok(cap_fs_ext::MetadataExt::nlink(metadata) == 1)
}

#[cfg(windows)]
fn has_single_link(metadata: &CapabilityMetadata) -> Result<bool, String> {
    use cap_primitives::fs::_WindowsByHandle;
    metadata
        .number_of_links()
        .map(|count| count == 1)
        .ok_or_else(|| {
            "cannot verify completed-output link count: Windows by-handle metadata is unavailable"
                .into()
        })
}
