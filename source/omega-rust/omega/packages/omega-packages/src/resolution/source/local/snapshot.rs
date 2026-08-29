//! Local snapshot staging, publication, reuse, and topology checks.

use super::*;

pub(in crate::resolution::source) fn publish_local_snapshot(
    requested_root: PathBuf,
    captured: CapturedLocalTree,
    cache_dir: &Path,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    let canonical_cache_dir =
        validate_local_snapshot_topology(&captured.normalized.root, cache_dir)?;
    std::fs::create_dir_all(&canonical_cache_dir)
        .map_err(|error| io_error(&canonical_cache_dir, error))?;
    let snapshots = canonical_cache_dir.join(LOCAL_CACHE_SNAPSHOTS);
    std::fs::create_dir_all(&snapshots).map_err(|error| io_error(&snapshots, error))?;
    verify_local_cache_root_custody(&canonical_cache_dir)?;
    verify_local_cache_root_custody(&snapshots)?;

    let identity = captured.normalized.content_identity.clone();
    let custody_identity = local_snapshot_custody_identity(
        &captured.normalized.root,
        &captured.normalized.content_identity,
    );
    let publication = snapshots.join(format!("source-{custody_identity}"));
    let lock_path = snapshots.join(format!("source-{custody_identity}.lock"));
    let _entry_lock = CacheEntryLock::acquire_local(&lock_path)?;

    let normalized = if publication.exists() {
        let normalized = verify_local_snapshot(&publication, &identity, limits)?;
        verify_live_source_unchanged(&captured.normalized, limits)?;
        normalized
    } else {
        materialize_local_snapshot(&snapshots, &publication, &captured, limits)?
    };

    verify_local_cache_root_custody(&canonical_cache_dir)?;
    verify_local_cache_root_custody(&snapshots)?;
    verify_local_cache_custody(&publication, limits)?;
    Ok(ResolvedLocalSnapshot {
        requested_root,
        canonical_live_root: captured.normalized.root,
        snapshot_root: normalized.root.clone(),
        normalized,
    })
}

fn validate_local_snapshot_topology(
    canonical_live_root: &Path,
    cache_dir: &Path,
) -> Result<PathBuf, SourceResolveError> {
    let canonical_cache_dir = canonicalize_prospective_path(cache_dir)?;
    let snapshot_collection =
        canonicalize_prospective_path(&canonical_cache_dir.join(LOCAL_CACHE_SNAPSHOTS))?;
    if canonical_cache_dir.starts_with(canonical_live_root)
        || canonical_live_root.starts_with(&snapshot_collection)
    {
        return Err(SourceResolveError::LocalSnapshotCacheOverlapsSource {
            canonical_live_root: canonical_live_root.to_path_buf(),
            canonical_cache_dir,
        });
    }
    Ok(canonical_cache_dir)
}

pub(in crate::resolution::source) fn local_snapshot_custody_identity(
    canonical_live_root: &Path,
    content_identity: &str,
) -> String {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, LOCAL_SNAPSHOT_CUSTODY_POLICY);
    hash_bytes(
        &mut hasher,
        raw_os_bytes(canonical_live_root.as_os_str()).as_slice(),
    );
    hash_bytes(&mut hasher, content_identity.as_bytes());
    format_sha256(&hasher.finalize())
}

fn canonicalize_prospective_path(path: &Path) -> Result<PathBuf, SourceResolveError> {
    let mut existing = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| io_error(Path::new("."), error))?
            .join(path)
    };
    let mut suffix = Vec::<OsString>::new();
    loop {
        match std::fs::symlink_metadata(&existing) {
            Ok(_) => {
                let canonical = existing
                    .canonicalize()
                    .map_err(|error| io_error(&existing, error))?;
                let mut result = canonical;
                for component in suffix.into_iter().rev() {
                    if component == "." {
                        continue;
                    }
                    if component == ".." {
                        result.pop();
                    } else {
                        result.push(component);
                    }
                }
                return Ok(result);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(component) = existing.file_name().map(OsStr::to_os_string) else {
                    return Err(io_error(&existing, error));
                };
                suffix.push(component);
                existing.pop();
            }
            Err(error) => return Err(io_error(&existing, error)),
        }
    }
}

fn materialize_local_snapshot(
    snapshots: &Path,
    publication: &Path,
    captured: &CapturedLocalTree,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let identity = &captured.normalized.content_identity;
    let mut pending = PendingMaterializedSnapshot::create(
        CacheCustodyKind::LocalSnapshot,
        snapshots,
        &format!(".source-{identity}.stage"),
    )?;
    let source = pending.root.join(LOCAL_SNAPSHOT_SOURCE);
    pending
        .directory()?
        .create_dir(LOCAL_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;
    let source_directory = pending
        .directory()?
        .open_dir_nofollow(LOCAL_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;

    for entry in &captured.entries {
        match &entry.kind {
            CapturedLocalEntryKind::Directory => {
                open_or_create_snapshot_directory(
                    CacheCustodyKind::LocalSnapshot,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                )?;
            }
            CapturedLocalEntryKind::File { bytes, executable } => {
                write_snapshot_file_from_open_root(
                    CacheCustodyKind::LocalSnapshot,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    bytes,
                    *executable,
                )?;
            }
            CapturedLocalEntryKind::Symlink { target_bytes } => {
                create_snapshot_symlink_from_open_root(
                    CacheCustodyKind::LocalSnapshot,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    target_bytes,
                )?;
            }
        }
    }

    let staged = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if !same_source_identity(&staged, &captured.normalized) {
        return Err(local_snapshot_invalid(
            &source,
            "staged source does not match the captured local tree",
        ));
    }
    verify_live_source_unchanged(&captured.normalized, limits)?;

    write_snapshot_file_from_open_root(
        CacheCustodyKind::LocalSnapshot,
        pending.directory()?,
        Path::new(LOCAL_SNAPSHOT_METADATA),
        &pending.root,
        &local_snapshot_metadata(&staged),
        false,
    )?;
    make_open_snapshot_read_only(
        CacheCustodyKind::LocalSnapshot,
        pending.directory()?,
        &pending.root,
    )?;
    let finalized = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if !same_source_identity(&finalized, &captured.normalized) {
        return Err(local_snapshot_invalid(
            &source,
            "finalized snapshot does not match the captured local tree",
        ));
    }
    pending.publish(snapshots, publication)?;
    verify_local_snapshot(publication, identity, limits)
}

fn verify_live_source_unchanged(
    captured: &ResolvedLocalSource,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    let current = resolve_local_source(&captured.root, limits).map_err(|_| {
        SourceResolveError::LocalSourceChanged {
            path: captured.root.clone(),
        }
    })?;
    if !same_source_identity(&current, captured) {
        return Err(SourceResolveError::LocalSourceChanged {
            path: captured.root.clone(),
        });
    }
    Ok(())
}

fn same_source_identity(left: &ResolvedLocalSource, right: &ResolvedLocalSource) -> bool {
    left.file_count == right.file_count
        && left.byte_count == right.byte_count
        && left.content_identity == right.content_identity
}
