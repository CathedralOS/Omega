//! Historical source bytes from the existing local snapshot collection.

use crate::SourceResolveError;
use crate::custody::lock::CacheEntryLock;
use crate::custody::platform::same_capability_file_identity;
use crate::custody::tree::verify_local_cache_custody;
use crate::error::local_snapshot_invalid;
use crate::identity::SourceContentDigest;
use crate::limits::{CACHE_CUSTODY_ENTRY_LIMIT, LOCAL_CACHE_SNAPSHOTS, LocalSourceLimits};
use crate::snapshot::metadata::{local_snapshot_content_identity, verify_local_snapshot};
use crate::storage::RetainedStorageLane;
use crate::tree::ResolvedLocalSource;
use crate::tree::filesystem::io_error;
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use std::ffi::OsStr;
use std::path::{Component, Path};

use super::snapshot::local_snapshot_custody_identity;

/// Recover an old snapshot for a caller-known canonical local origin.
///
/// The origin routes cache lookup only; it is never opened or read. The result
/// describes verified historical bytes, not current live-source resolution.
/// Existing cache keys use the raw tree identity while locks retain a
/// domain-separated digest of it, so lookup scans bounded metadata before
/// verifying exactly one matching tree. It creates no archive or cache index.
pub fn recover_cached_local_source_in_lane(
    canonical_origin: &Path,
    expected: &SourceContentDigest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<Option<ResolvedLocalSource>, SourceResolveError> {
    recover_with_entry_limit(
        canonical_origin,
        expected,
        lane,
        limits,
        CACHE_CUSTODY_ENTRY_LIMIT,
    )
}

fn recover_with_entry_limit(
    canonical_origin: &Path,
    expected: &SourceContentDigest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    maximum_entries: usize,
) -> Result<Option<ResolvedLocalSource>, SourceResolveError> {
    if !canonical_origin.is_absolute()
        || canonical_origin
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(local_snapshot_invalid(
            canonical_origin,
            "cached source origin must be canonical and absolute",
        ));
    }
    lane.verify_path_identity()?;
    let collection = lane.path().join(LOCAL_CACHE_SNAPSHOTS);
    let directory = match lane.directory().open_dir_nofollow(LOCAL_CACHE_SNAPSHOTS) {
        Ok(directory) => directory,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(&collection, error)),
    };
    let result = (|| {
        for (index, entry) in directory
            .entries()
            .map_err(|error| io_error(&collection, error))?
            .enumerate()
        {
            if index >= maximum_entries {
                return Err(local_snapshot_invalid(
                    &collection,
                    "cached local source lookup exceeds its entry limit",
                ));
            }
            let entry = entry.map_err(|error| io_error(&collection, error))?;
            let name = entry.file_name();
            let Some(name) = name.to_str().filter(|name| is_snapshot_name(name)) else {
                continue;
            };
            if !entry
                .file_type()
                .map_err(|error| io_error(&collection, error))?
                .is_dir()
            {
                continue;
            }
            let publication = collection.join(name);
            let Ok(identity) = local_snapshot_content_identity(&publication) else {
                continue;
            };
            if SourceContentDigest::derive(identity.as_bytes()) != *expected
                || name
                    != format!(
                        "source-{}",
                        local_snapshot_custody_identity(canonical_origin, &identity)
                    )
            {
                continue;
            }
            let lock_name = format!("{name}.lock");
            let lock = CacheEntryLock::acquire_local_from_parent(
                &collection,
                &directory,
                OsStr::new(&lock_name),
            )?;
            verify_collection(lane, &directory)?;
            let limits = limits.compiler_bounded();
            verify_local_cache_custody(&publication, limits)?;
            let source = verify_local_snapshot(&publication, &identity, limits)?;
            lock.verify_path_identity()?;
            verify_collection(lane, &directory)?;
            return Ok(Some(source));
        }
        Ok(None)
    })();
    verify_collection(lane, &directory)?;
    result
}

fn is_snapshot_name(name: &str) -> bool {
    name.strip_prefix("source-").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn verify_collection(lane: &RetainedStorageLane, retained: &Dir) -> Result<(), SourceResolveError> {
    lane.verify_path_identity()?;
    let path = lane.path().join(LOCAL_CACHE_SNAPSHOTS);
    let named = lane
        .directory()
        .open_dir_nofollow(LOCAL_CACHE_SNAPSHOTS)
        .map_err(|error| io_error(&path, error))?;
    let first = retained
        .dir_metadata()
        .map_err(|error| io_error(&path, error))?;
    let second = named
        .dir_metadata()
        .map_err(|error| io_error(&path, error))?;
    if !same_capability_file_identity(&first, &second) {
        return Err(local_snapshot_invalid(
            &path,
            "cached snapshot collection changed during lookup",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
