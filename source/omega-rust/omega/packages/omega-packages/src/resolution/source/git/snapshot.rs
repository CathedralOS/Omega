//! Authenticated Git tree materialization and immutable snapshot verification.

use super::*;

pub(in crate::resolution::source) fn resolve_git_snapshot(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    mut entries: Vec<GitTreeEntry>,
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    let expected = preflight_git_snapshot(tree, &entries)?;
    let snapshots = repository.open_or_create_snapshots()?;
    let publication = snapshots.path.join(format!("tree-{tree}"));
    if snapshots.publication_exists(&publication)? {
        release_git_blob_payloads(&mut entries);
        let result = verify_git_snapshot(&publication, &expected, &entries, limits);
        return reconcile_git_cache_operation_result(result, snapshots.verify_identity(), None);
    }

    let mut pending = PendingMaterializedSnapshot::create_from_open_parent(
        CacheCustodyKind::Git,
        &snapshots.path,
        &snapshots.directory,
        &format!(".tree-{tree}.stage"),
    )?;
    let source = pending.root.join(GIT_SNAPSHOT_SOURCE);
    pending
        .directory()?
        .create_dir(GIT_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;
    let source_directory = pending
        .directory()?
        .open_dir_nofollow(GIT_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;
    for entry in &entries {
        executor.verify_budget()?;
        checked_git_destination(&source, entry)?;
        match &entry.kind {
            GitTreeEntryKind::Tree => {
                open_or_create_snapshot_directory(
                    CacheCustodyKind::Git,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                )?;
            }
            GitTreeEntryKind::File { executable, bytes } => {
                write_snapshot_file_from_open_root(
                    CacheCustodyKind::Git,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    bytes.as_slice(),
                    *executable,
                )?;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                create_snapshot_symlink_from_open_root(
                    CacheCustodyKind::Git,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    target_bytes.as_slice(),
                )?;
            }
        }
    }

    // The staged source is re-read to bind publication identity. Release the
    // shared batch payload first so that this verification does not retain a
    // second package-sized in-memory copy.
    release_git_blob_payloads(&mut entries);
    let staged = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if staged.file_count != expected.file_count
        || staged.byte_count != expected.byte_count
        || staged.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            &source,
            "materialized snapshot did not preserve the validated Git tree exactly",
        ));
    }
    write_snapshot_file_from_open_root(
        CacheCustodyKind::Git,
        pending.directory()?,
        Path::new(GIT_SNAPSHOT_METADATA),
        &pending.root,
        &git_snapshot_metadata(tree, &staged),
        false,
    )?;
    make_open_snapshot_read_only(CacheCustodyKind::Git, pending.directory()?, &pending.root)?;
    let finalized = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if finalized.file_count != expected.file_count
        || finalized.byte_count != expected.byte_count
        || finalized.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            &source,
            "finalized snapshot did not preserve the authenticated Git tree exactly",
        ));
    }
    pending.publish(&snapshots.path, &publication)?;

    // The returned identity is always calculated from the atomically published tree, never from
    // the staging directory or Git's mutable object-cache state.
    let result = verify_git_snapshot(&publication, &expected, &entries, limits);
    reconcile_git_cache_operation_result(result, snapshots.verify_identity(), None)
}

pub(in crate::resolution::source) fn preflight_git_snapshot(
    tree: &str,
    entries: &[GitTreeEntry],
) -> Result<GitSnapshotMetadata, SourceResolveError> {
    authenticate_git_tree(tree, entries)?;
    verify_git_destination_containment(Path::new("omega-verified-snapshot-root"), entries)?;
    authenticated_git_snapshot_identity(tree, entries)
}

fn authenticated_git_snapshot_identity(
    tree: &str,
    entries: &[GitTreeEntry],
) -> Result<GitSnapshotMetadata, SourceResolveError> {
    let mut identity = SourceIdentityHasher::new(entries.len());
    let mut file_count = 0_usize;
    for entry in entries {
        match &entry.kind {
            GitTreeEntryKind::Tree => {
                identity.add_directory(&entry.relative_bytes, CANONICAL_DIRECTORY_MODE);
            }
            GitTreeEntryKind::File { executable, bytes } => {
                identity.add_file(&entry.relative_bytes, *executable, bytes.as_slice())?;
                file_count += 1;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                identity.add_symlink(&entry.relative_bytes, target_bytes.as_slice());
                file_count += 1;
            }
        }
    }
    let (byte_count, content_identity) = identity.finish();
    Ok(GitSnapshotMetadata {
        tree: tree.to_owned(),
        file_count,
        byte_count,
        content_identity,
    })
}

fn verify_git_destination_containment(
    source: &Path,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    for entry in entries {
        checked_git_destination(source, entry)?;
    }
    Ok(())
}

fn checked_git_destination(
    source: &Path,
    entry: &GitTreeEntry,
) -> Result<PathBuf, SourceResolveError> {
    if entry
        .relative_path
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "materialization path is not strictly relative",
        ));
    }
    let destination = source.join(&entry.relative_path);
    if !destination.starts_with(source) {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "materialization path escapes the snapshot root",
        ));
    }
    Ok(destination)
}

fn release_git_blob_payloads(entries: &mut [GitTreeEntry]) {
    for entry in entries {
        match &mut entry.kind {
            GitTreeEntryKind::Tree => {}
            GitTreeEntryKind::File { bytes, .. } => *bytes = GitBlobBytes::empty(),
            GitTreeEntryKind::Symlink { target_bytes } => {
                *target_bytes = GitBlobBytes::empty();
            }
        }
    }
}

fn verify_git_snapshot(
    publication: &Path,
    expected: &GitSnapshotMetadata,
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    let source = publication.join(GIT_SNAPSHOT_SOURCE);
    let metadata_path = publication.join(GIT_SNAPSHOT_METADATA);
    let metadata = read_bounded_cache_record(
        CacheCustodyKind::Git,
        publication,
        Path::new(GIT_SNAPSHOT_METADATA),
        1024,
    )?;
    let metadata = parse_git_snapshot_metadata(&metadata, &metadata_path)?;
    if metadata != *expected {
        return Err(cache_invalid(
            &metadata_path,
            "snapshot metadata does not match the authenticated Git tree",
        ));
    }
    let publication_directory = open_absolute_directory_nofollow(publication)
        .map_err(|error| cache_invalid(publication, error.to_string()))?;
    verify_open_snapshot_tree_modes(CacheCustodyKind::Git, &publication_directory, publication)?;
    let source_directory = publication_directory
        .open_dir_nofollow(GIT_SNAPSHOT_SOURCE)
        .map_err(|error| cache_invalid(&source, error.to_string()))?;
    let captured = capture_local_source_from_open_root(
        source.clone(),
        source_directory,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?;
    verify_captured_git_snapshot_shape(&source, &captured.entries, entries)?;
    let local = captured.normalized;
    if local.file_count != expected.file_count
        || local.byte_count != expected.byte_count
        || local.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            publication,
            "published snapshot does not match the authenticated Git tree",
        ));
    }
    Ok((source, local))
}

fn verify_captured_git_snapshot_shape(
    source: &Path,
    captured: &[CapturedLocalEntry],
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    let mut expected_directories = git_directory_paths(entries);
    let mut expected_leaves = entries
        .iter()
        .filter(|entry| !matches!(entry.kind, GitTreeEntryKind::Tree))
        .map(|entry| (entry.relative_bytes.as_slice(), &entry.kind))
        .collect::<BTreeMap<_, _>>();
    for entry in captured {
        let path = source.join(&entry.relative_path);
        match &entry.kind {
            CapturedLocalEntryKind::Directory => {
                if !expected_directories.remove(&entry.relative_bytes) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot contains an undeclared directory",
                    ));
                }
            }
            CapturedLocalEntryKind::File { executable, .. } => {
                let Some(expected) = expected_leaves.remove(entry.relative_bytes.as_slice()) else {
                    return Err(cache_invalid(&path, "snapshot contains an undeclared file"));
                };
                if !matches!(
                    expected,
                    GitTreeEntryKind::File {
                        executable: expected_executable,
                        ..
                    } if expected_executable == executable
                ) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot file kind or executable mode does not match Git",
                    ));
                }
            }
            CapturedLocalEntryKind::Symlink { .. } => {
                let Some(expected) = expected_leaves.remove(entry.relative_bytes.as_slice()) else {
                    return Err(cache_invalid(
                        &path,
                        "snapshot contains an undeclared symlink",
                    ));
                };
                if !matches!(expected, GitTreeEntryKind::Symlink { .. }) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot symlink kind does not match Git",
                    ));
                }
            }
        }
    }
    if !expected_directories.is_empty() || !expected_leaves.is_empty() {
        return Err(cache_invalid(
            source,
            "snapshot paths do not exactly match the validated Git tree",
        ));
    }
    Ok(())
}

pub(in crate::resolution::source) fn verify_open_snapshot_tree_modes(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    display_root: &Path,
) -> Result<(), SourceResolveError> {
    let root_metadata = root
        .dir_metadata()
        .map_err(|error| io_error(display_root, error))?;
    verify_capability_snapshot_directory_mode(kind, display_root, &root_metadata)?;
    let entries = root
        .entries()
        .map_err(|error| io_error(display_root, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(display_root, error))?;
        let name = entry.file_name();
        let path = display_root.join(&name);
        let metadata = root
            .symlink_metadata(&name)
            .map_err(|error| io_error(&path, error))?;
        if metadata.is_dir() {
            let directory = root.open_dir_nofollow(&name).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot directory changed during verification: {error}"),
                )
            })?;
            let opened = directory
                .dir_metadata()
                .map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot directory changed during verification",
                ));
            }
            verify_open_snapshot_tree_modes(kind, &directory, &path)?;
        } else if metadata.is_file() {
            let mut options = CapabilityOpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = root.open_with(&name, &options).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot file changed during verification: {error}"),
                )
            })?;
            let opened = file.metadata().map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot file changed during verification",
                ));
            }
            verify_capability_snapshot_file_mode(kind, &path, &opened)?;
        } else if !metadata.file_type().is_symlink() {
            return Err(cache_custody_invalid(
                kind,
                &path,
                "snapshot contains an unsupported filesystem entry type",
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn verify_capability_snapshot_directory_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    if metadata.mode() & 0o7777 != u32::from(CANONICAL_DIRECTORY_MODE) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "snapshot directory mode is not canonical 0555",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_capability_snapshot_directory_mode(
    _kind: CacheCustodyKind,
    _path: &Path,
    _metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
fn verify_capability_snapshot_file_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    if !matches!(metadata.mode() & 0o7777, 0o444 | 0o555) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "snapshot file mode is not canonical 0444 or 0555",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_capability_snapshot_file_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    if !metadata.permissions().readonly() {
        return Err(cache_custody_invalid(
            kind,
            path,
            "snapshot file is writable",
        ));
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct LocalSnapshotMetadata {
    file_count: usize,
    byte_count: u64,
    content_identity: String,
}

pub(in crate::resolution::source) fn local_snapshot_metadata(
    local: &ResolvedLocalSource,
) -> Vec<u8> {
    let mut metadata = LOCAL_SNAPSHOT_POLICY.to_vec();
    metadata.extend_from_slice(&(local.file_count as u64).to_le_bytes());
    metadata.extend_from_slice(&local.byte_count.to_le_bytes());
    append_framed_bytes(&mut metadata, local.content_identity.as_bytes());
    metadata
}

fn parse_local_snapshot_metadata(
    bytes: &[u8],
    path: &Path,
) -> Result<LocalSnapshotMetadata, SourceResolveError> {
    let Some(mut remaining) = bytes.strip_prefix(LOCAL_SNAPSHOT_POLICY) else {
        return Err(local_snapshot_invalid(
            path,
            "snapshot metadata policy does not match",
        ));
    };
    let file_count = take_u64(&mut remaining)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| local_snapshot_invalid(path, "snapshot file count is invalid"))?;
    let byte_count = take_u64(&mut remaining)
        .ok_or_else(|| local_snapshot_invalid(path, "snapshot byte count is invalid"))?;
    let content_identity = take_framed_bytes(&mut remaining)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .filter(|identity| {
            identity.len() == 64 && identity.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or_else(|| local_snapshot_invalid(path, "snapshot content identity is invalid"))?
        .to_owned();
    if !remaining.is_empty() {
        return Err(local_snapshot_invalid(
            path,
            "snapshot metadata has trailing bytes",
        ));
    }
    Ok(LocalSnapshotMetadata {
        file_count,
        byte_count,
        content_identity,
    })
}

pub(in crate::resolution::source) fn verify_local_snapshot(
    publication: &Path,
    content_identity: &str,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let source = publication.join(LOCAL_SNAPSHOT_SOURCE);
    let metadata_path = publication.join(LOCAL_SNAPSHOT_METADATA);
    let metadata = read_bounded_cache_record(
        CacheCustodyKind::LocalSnapshot,
        publication,
        Path::new(LOCAL_SNAPSHOT_METADATA),
        512,
    )?;
    let expected = parse_local_snapshot_metadata(&metadata, &metadata_path)?;
    if expected.content_identity != content_identity {
        return Err(local_snapshot_invalid(
            &metadata_path,
            "snapshot content identity does not match its cache key",
        ));
    }
    let publication_directory = open_absolute_directory_nofollow(publication)
        .map_err(|error| local_snapshot_invalid(publication, error.to_string()))?;
    verify_open_snapshot_tree_modes(
        CacheCustodyKind::LocalSnapshot,
        &publication_directory,
        publication,
    )?;
    let source_directory = publication_directory
        .open_dir_nofollow(LOCAL_SNAPSHOT_SOURCE)
        .map_err(|error| local_snapshot_invalid(&source, error.to_string()))?;
    let normalized = capture_local_source_from_open_root(
        source.clone(),
        source_directory,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if normalized.file_count != expected.file_count
        || normalized.byte_count != expected.byte_count
        || normalized.content_identity != expected.content_identity
    {
        return Err(local_snapshot_invalid(
            publication,
            "published snapshot does not match resolver metadata",
        ));
    }
    Ok(normalized)
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::resolution::source) struct GitSnapshotMetadata {
    tree: String,
    file_count: usize,
    byte_count: u64,
    content_identity: String,
}

pub(in crate::resolution::source) fn git_snapshot_metadata(
    tree: &str,
    local: &ResolvedLocalSource,
) -> Vec<u8> {
    let mut metadata = GIT_SNAPSHOT_POLICY.to_vec();
    append_framed_bytes(&mut metadata, tree.as_bytes());
    metadata.extend_from_slice(&(local.file_count as u64).to_le_bytes());
    metadata.extend_from_slice(&local.byte_count.to_le_bytes());
    append_framed_bytes(&mut metadata, local.content_identity.as_bytes());
    metadata
}

fn parse_git_snapshot_metadata(
    bytes: &[u8],
    path: &Path,
) -> Result<GitSnapshotMetadata, SourceResolveError> {
    let Some(mut remaining) = bytes.strip_prefix(GIT_SNAPSHOT_POLICY) else {
        return Err(cache_invalid(
            path,
            "snapshot metadata policy does not match",
        ));
    };
    let tree = take_framed_bytes(&mut remaining)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .filter(|tree| is_object_id(tree))
        .ok_or_else(|| cache_invalid(path, "snapshot metadata tree is invalid"))?
        .to_owned();
    let file_count = take_u64(&mut remaining)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| cache_invalid(path, "snapshot file count is invalid"))?;
    let byte_count = take_u64(&mut remaining)
        .ok_or_else(|| cache_invalid(path, "snapshot byte count is invalid"))?;
    let content_identity = take_framed_bytes(&mut remaining)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .filter(|identity| {
            identity.len() == 64 && identity.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or_else(|| cache_invalid(path, "snapshot content identity is invalid"))?
        .to_owned();
    if !remaining.is_empty() {
        return Err(cache_invalid(path, "snapshot metadata has trailing bytes"));
    }
    Ok(GitSnapshotMetadata {
        tree,
        file_count,
        byte_count,
        content_identity,
    })
}

fn take_u64(bytes: &mut &[u8]) -> Option<u64> {
    let value = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    *bytes = &bytes[8..];
    Some(value)
}

fn take_framed_bytes<'a>(bytes: &mut &'a [u8]) -> Option<&'a [u8]> {
    let length = usize::try_from(take_u64(bytes)?).ok()?;
    let value = bytes.get(..length)?;
    *bytes = &bytes[length..];
    Some(value)
}

pub(in crate::resolution::source) fn open_or_create_snapshot_directory(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    use std::path::Component;

    let mut directory = root
        .try_clone()
        .map_err(|error| io_error(display_root, error))?;
    let mut display_path = display_root.to_path_buf();
    for component in relative_path.components() {
        let Component::Normal(name) = component else {
            return Err(cache_custody_invalid(
                kind,
                &display_path,
                "snapshot materialization received a noncanonical relative directory",
            ));
        };
        display_path.push(name);
        match directory.create_dir(name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(&display_path, error)),
        }
        directory = directory.open_dir_nofollow(name).map_err(|error| {
            cache_custody_invalid(
                kind,
                &display_path,
                format!("snapshot directory is not a stable concrete child: {error}"),
            )
        })?;
    }
    Ok(directory)
}

pub(in crate::resolution::source) fn write_snapshot_file_from_open_root(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
    bytes: &[u8],
    executable: bool,
) -> Result<(), SourceResolveError> {
    let parent_path = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let parent = open_or_create_snapshot_directory(kind, root, parent_path, display_root)?;
    let name = relative_path.file_name().ok_or_else(|| {
        cache_custody_invalid(
            kind,
            &display_root.join(relative_path),
            "snapshot file has no relative name",
        )
    })?;
    let display_path = display_root.join(relative_path);
    let mut options = CapabilityOpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = parent.open_with(name, &options).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            cache_custody_invalid(
                kind,
                &display_path,
                "snapshot file destination already exists",
            )
        } else {
            io_error(&display_path, error)
        }
    })?;
    file.write_all(bytes)
        .map_err(|error| io_error(&display_path, error))?;
    file.sync_all()
        .map_err(|error| io_error(&display_path, error))?;
    set_open_snapshot_file_mode(&file, &display_path, executable)
}

#[cfg(unix)]
pub(in crate::resolution::source) fn create_snapshot_symlink_from_open_root(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
    target: &[u8],
) -> Result<(), SourceResolveError> {
    use std::os::unix::ffi::OsStringExt;

    let parent_path = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let parent = open_or_create_snapshot_directory(kind, root, parent_path, display_root)?;
    let name = relative_path.file_name().ok_or_else(|| {
        cache_custody_invalid(
            kind,
            &display_root.join(relative_path),
            "snapshot symlink has no relative name",
        )
    })?;
    let display_path = display_root.join(relative_path);
    parent
        .symlink_contents(OsString::from_vec(target.to_vec()), name)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                cache_custody_invalid(kind, &display_path, "snapshot symlink already exists")
            } else {
                io_error(&display_path, error)
            }
        })
}

#[cfg(not(unix))]
pub(in crate::resolution::source) fn create_snapshot_symlink_from_open_root(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
    target: &[u8],
) -> Result<(), SourceResolveError> {
    let target = std::str::from_utf8(target).map_err(|_| {
        git_tree_invalid(target, "symlink target cannot be represented on this host")
    })?;
    let parent_path = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let parent = open_or_create_snapshot_directory(kind, root, parent_path, display_root)?;
    let name = relative_path.file_name().ok_or_else(|| {
        cache_custody_invalid(
            kind,
            &display_root.join(relative_path),
            "snapshot symlink has no relative name",
        )
    })?;
    let display_path = display_root.join(relative_path);
    parent.symlink_file(target, name).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            cache_custody_invalid(kind, &display_path, "snapshot symlink already exists")
        } else {
            io_error(&display_path, error)
        }
    })
}

#[cfg(unix)]
fn set_open_snapshot_file_mode(
    file: &cap_std::fs::File,
    path: &Path,
    executable: bool,
) -> Result<(), SourceResolveError> {
    use cap_std::fs::PermissionsExt;

    let mode = if executable { 0o555 } else { 0o444 };
    file.set_permissions(cap_std::fs::Permissions::from_mode(mode))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn set_open_snapshot_file_mode(
    file: &cap_std::fs::File,
    path: &Path,
    _executable: bool,
) -> Result<(), SourceResolveError> {
    let mut permissions = file
        .metadata()
        .map_err(|error| io_error(path, error))?
        .permissions();
    permissions.set_readonly(true);
    file.set_permissions(permissions)
        .map_err(|error| io_error(path, error))
}

pub(in crate::resolution::source) fn make_open_snapshot_read_only(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    display_root: &Path,
) -> Result<(), SourceResolveError> {
    let entries = root
        .entries()
        .map_err(|error| io_error(display_root, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(display_root, error))?;
        let name = entry.file_name();
        let path = display_root.join(&name);
        let metadata = root
            .symlink_metadata(&name)
            .map_err(|error| io_error(&path, error))?;
        if metadata.is_dir() {
            let directory = root.open_dir_nofollow(&name).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot directory changed during finalization: {error}"),
                )
            })?;
            let opened = directory
                .dir_metadata()
                .map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot directory changed during read-only finalization",
                ));
            }
            make_open_snapshot_read_only(kind, &directory, &path)?;
        } else if metadata.is_file() {
            let mut options = CapabilityOpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = root.open_with(&name, &options).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot file changed during finalization: {error}"),
                )
            })?;
            let opened = file.metadata().map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot file changed during read-only finalization",
                ));
            }
            set_open_snapshot_file_mode(&file, &path, capability_is_executable(&metadata))?;
        }
    }
    set_open_snapshot_directory_read_only(root, display_root)
}

#[cfg(unix)]
fn capability_is_executable(metadata: &CapabilityMetadata) -> bool {
    use cap_fs_ext::OsMetadataExt;

    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn capability_is_executable(_metadata: &CapabilityMetadata) -> bool {
    false
}

#[cfg(unix)]
fn set_open_snapshot_directory_read_only(
    directory: &CapabilityDirectory,
    path: &Path,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    directory
        .try_clone()
        .map_err(|error| io_error(path, error))?
        .into_std_file()
        .set_permissions(std::fs::Permissions::from_mode(0o555))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn set_open_snapshot_directory_read_only(
    _directory: &CapabilityDirectory,
    _path: &Path,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(test, unix))]
fn set_snapshot_file_mode(path: &Path, executable: bool) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if executable { 0o555 } else { 0o444 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| io_error(path, error))
}

#[cfg(all(test, not(unix)))]
fn set_snapshot_file_mode(path: &Path, _executable: bool) -> Result<(), SourceResolveError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|error| io_error(path, error))?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).map_err(|error| io_error(path, error))
}

#[cfg(test)]
pub(in crate::resolution::source) fn make_snapshot_read_only(
    root: &Path,
) -> Result<(), SourceResolveError> {
    let mut directories = vec![root.to_path_buf()];
    let mut cursor = 0;
    while cursor < directories.len() {
        let directory = directories[cursor].clone();
        cursor += 1;
        for entry in std::fs::read_dir(&directory).map_err(|error| io_error(&directory, error))? {
            let entry = entry.map_err(|error| io_error(&directory, error))?;
            let path = entry.path();
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
            if metadata.is_dir() {
                directories.push(path);
            } else if metadata.is_file() {
                set_snapshot_file_mode(&path, is_executable(&metadata))?;
            }
        }
    }
    for directory in directories.into_iter().rev() {
        set_snapshot_directory_read_only(&directory)?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
fn set_snapshot_directory_read_only(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555))
        .map_err(|error| io_error(path, error))
}

#[cfg(all(test, not(unix)))]
fn set_snapshot_directory_read_only(_path: &Path) -> Result<(), SourceResolveError> {
    Ok(())
}

pub(in crate::resolution::source) struct PendingMaterializedSnapshot {
    pub(in crate::resolution::source) root: PathBuf,
    pub(in crate::resolution::source) parent: CapabilityDirectory,
    pub(in crate::resolution::source) directory: Option<CapabilityDirectory>,
    pub(in crate::resolution::source) stage_name: OsString,
    pub(in crate::resolution::source) kind: CacheCustodyKind,
    pub(in crate::resolution::source) published: bool,
}

impl PendingMaterializedSnapshot {
    pub(in crate::resolution::source) fn create(
        kind: CacheCustodyKind,
        snapshots: &Path,
        prefix: &str,
    ) -> Result<Self, SourceResolveError> {
        verify_cache_custody_root(snapshots, kind)?;
        let parent = open_absolute_directory_nofollow(snapshots)
            .map_err(|error| cache_custody_invalid(kind, snapshots, error.to_string()))?;
        Self::create_from_open_parent(kind, snapshots, &parent, prefix)
    }

    pub(in crate::resolution::source) fn create_from_open_parent(
        kind: CacheCustodyKind,
        snapshots: &Path,
        retained_parent: &CapabilityDirectory,
        prefix: &str,
    ) -> Result<Self, SourceResolveError> {
        let parent = retained_parent
            .try_clone()
            .map_err(|error| io_error(snapshots, error))?;
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let stage_name = OsString::from(format!("{prefix}-{}-{sequence}", std::process::id()));
            let root = snapshots.join(&stage_name);
            match parent.create_dir(&stage_name) {
                Ok(()) => {
                    let classified = parent
                        .symlink_metadata(&stage_name)
                        .map_err(|error| io_error(&root, error))?;
                    let directory = parent
                        .open_dir_nofollow(&stage_name)
                        .map_err(|error| cache_custody_invalid(kind, &root, error.to_string()))?;
                    let opened = directory
                        .dir_metadata()
                        .map_err(|error| io_error(&root, error))?;
                    if !classified.is_dir() || !same_capability_file_identity(&classified, &opened)
                    {
                        return Err(cache_custody_invalid(
                            kind,
                            &root,
                            "snapshot staging directory changed while being retained",
                        ));
                    }
                    return Ok(Self {
                        root,
                        parent,
                        directory: Some(directory),
                        stage_name,
                        kind,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&root, error)),
            }
        }
        Err(cache_custody_invalid(
            kind,
            snapshots,
            "could not allocate a unique materialized-snapshot staging directory",
        ))
    }

    pub(in crate::resolution::source) fn directory(
        &self,
    ) -> Result<&CapabilityDirectory, SourceResolveError> {
        self.directory.as_ref().ok_or_else(|| {
            cache_custody_invalid(self.kind, &self.root, "snapshot stage handle is absent")
        })
    }

    pub(in crate::resolution::source) fn publish(
        &mut self,
        snapshots: &Path,
        publication: &Path,
    ) -> Result<(), SourceResolveError> {
        let directory = self.directory()?;
        let retained = directory
            .dir_metadata()
            .map_err(|error| io_error(&self.root, error))?;
        let named = self
            .parent
            .symlink_metadata(&self.stage_name)
            .map_err(|error| io_error(&self.root, error))?;
        if !named.is_dir() || !same_capability_file_identity(&retained, &named) {
            return Err(cache_custody_invalid(
                self.kind,
                &self.root,
                "snapshot stage pathname no longer identifies the retained directory",
            ));
        }
        let publication_name = direct_cache_child_name(self.kind, snapshots, publication)?;
        publish_cache_directory_from_open_parent(
            self.kind,
            snapshots,
            &self.parent,
            &self.stage_name,
            publication_name,
            Some(&retained),
        )?;
        self.published = true;
        Ok(())
    }
}

impl Drop for PendingMaterializedSnapshot {
    fn drop(&mut self) {
        if !self.published {
            if let Some(directory) = self.directory.take() {
                make_open_tree_owner_writable(&directory);
                let _ = directory.remove_open_dir_all();
            }
        }
    }
}

pub(in crate::resolution::source) fn make_open_tree_owner_writable(root: &CapabilityDirectory) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Ok(directory) = root.try_clone() {
            let _ = directory
                .into_std_file()
                .set_permissions(std::fs::Permissions::from_mode(0o700));
        }
        if let Ok(entries) = root.entries() {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Ok(metadata) = root.symlink_metadata(&name)
                    && metadata.is_dir()
                    && let Ok(directory) = root.open_dir_nofollow(&name)
                {
                    make_open_tree_owner_writable(&directory);
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        if let Ok(directory) = root.try_clone() {
            let directory = directory.into_std_file();
            if let Ok(metadata) = directory.metadata() {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(false);
                let _ = directory.set_permissions(permissions);
            }
        }
        if let Ok(entries) = root.entries() {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Ok(metadata) = root.symlink_metadata(&name) {
                    if metadata.is_dir() {
                        if let Ok(directory) = root.open_dir_nofollow(&name) {
                            make_open_tree_owner_writable(&directory);
                        }
                    } else if metadata.is_file() {
                        let mut options = CapabilityOpenOptions::new();
                        options.read(true).follow(FollowSymlinks::No);
                        if let Ok(file) = root.open_with(&name, &options)
                            && let Ok(metadata) = file.metadata()
                        {
                            let mut permissions = metadata.permissions();
                            permissions.set_readonly(false);
                            let _ = file.set_permissions(permissions);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
pub(in crate::resolution::source) fn make_tree_owner_writable(root: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Ok(metadata) = std::fs::symlink_metadata(root)
            && metadata.is_dir()
        {
            let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700));
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Ok(metadata) = std::fs::symlink_metadata(&path)
                        && metadata.is_dir()
                    {
                        make_tree_owner_writable(&path);
                    }
                }
            }
        }
    }
}
