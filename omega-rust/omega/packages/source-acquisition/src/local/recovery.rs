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

use super::snapshot::{
    LocalSnapshotAddress, is_legacy_snapshot_name, legacy_snapshot_publication_name,
    snapshot_lock_name,
};
use crate::limits::LOCAL_SNAPSHOT_LOCK_TIMEOUT;

mod timings;
use timings::RecoveryTimings;

/// Recover an old snapshot for a caller-known canonical local origin.
///
/// The origin routes cache lookup only; it is never opened or read. The result
/// describes verified historical bytes, not current live-source resolution.
/// New publications are addressed by the exact origin and retained content pin.
/// Only a missing direct candidate enters the bounded legacy raw-key lookup.
/// Both paths verify the selected tree; neither creates an archive or index.
pub fn recover_cached_local_source_in_lane(
    canonical_origin: &Path,
    expected: &SourceContentDigest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<Option<ResolvedLocalSource>, SourceResolveError> {
    recover_bounded(
        canonical_origin,
        expected,
        lane,
        limits,
        CACHE_CUSTODY_ENTRY_LIMIT,
    )
}

fn recover_bounded(
    canonical_origin: &Path,
    expected: &SourceContentDigest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    maximum_entries: usize,
) -> Result<Option<ResolvedLocalSource>, SourceResolveError> {
    let mut timings = RecoveryTimings::requested();
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
        let address = LocalSnapshotAddress::new(canonical_origin, expected);
        let publication_name = address.publication_name();
        let publication = collection.join(&publication_name);
        if let Some(timings) = &mut timings {
            timings.direct_lookup_started();
        }
        let direct_exists = match directory.symlink_metadata(&publication_name) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(local_snapshot_invalid(
                    &publication,
                    "direct local snapshot publication is not a concrete directory",
                ));
            }
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(io_error(&publication, error)),
        };
        if direct_exists {
            // A selected direct entry still consumes one lookup slot. Keep
            // the existing independent zero-budget refusal without scanning.
            if maximum_entries == 0 {
                return Err(local_snapshot_invalid(
                    &collection,
                    "cached local source lookup exceeds its entry limit",
                ));
            }
            let identity = read_routing_identity(&publication, &mut timings)?;
            let actual = SourceContentDigest::derive(identity.as_bytes());
            if actual != *expected
                || LocalSnapshotAddress::new(canonical_origin, &actual).publication_name()
                    != publication_name
            {
                return Err(local_snapshot_invalid(
                    &publication,
                    "direct local snapshot metadata does not match its origin and content pin",
                ));
            }
            let source = verify_selected_snapshot(
                lane,
                &directory,
                &publication_name,
                &identity,
                limits,
                &mut timings,
            )?;
            if let Some(timings) = &mut timings {
                timings.direct_hit();
            }
            return Ok(Some(source));
        }
        if let Some(timings) = &mut timings {
            timings.direct_miss();
            timings.legacy_lookup_started();
        }
        for (index, entry) in directory
            .entries()
            .map_err(|error| io_error(&collection, error))?
            .enumerate()
        {
            if let Some(timings) = &mut timings {
                timings.entry_enumerated();
            }
            if index >= maximum_entries {
                return Err(local_snapshot_invalid(
                    &collection,
                    "cached local source lookup exceeds its entry limit",
                ));
            }
            let entry = entry.map_err(|error| io_error(&collection, error))?;
            let name = entry.file_name();
            let Some(name) = name.to_str().filter(|name| is_legacy_snapshot_name(name)) else {
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
            let Ok(identity) = read_routing_identity(&publication, &mut timings) else {
                continue;
            };
            if SourceContentDigest::derive(identity.as_bytes()) != *expected
                || name != legacy_snapshot_publication_name(canonical_origin, &identity)
            {
                continue;
            }
            return verify_selected_snapshot(
                lane,
                &directory,
                name,
                &identity,
                limits,
                &mut timings,
            )
            .map(Some);
        }
        Ok(None)
    })();
    verify_collection(lane, &directory)?;
    result
}

fn read_routing_identity(
    publication: &Path,
    timings: &mut Option<RecoveryTimings>,
) -> Result<String, SourceResolveError> {
    let started = timings.as_mut().map(RecoveryTimings::routing_started);
    let identity = local_snapshot_content_identity(publication);
    if let (Some(timings), Some(started)) = (timings, started) {
        timings.routing_finished(started);
    }
    identity
}

fn verify_selected_snapshot(
    lane: &RetainedStorageLane,
    directory: &Dir,
    publication_name: &str,
    identity: &str,
    limits: LocalSourceLimits,
    timings: &mut Option<RecoveryTimings>,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    if let Some(timings) = &mut *timings {
        timings.candidate_matched();
    }
    let collection = lane.path().join(LOCAL_CACHE_SNAPSHOTS);
    let publication = collection.join(publication_name);
    let lock_name = snapshot_lock_name(publication_name);
    let lock_started = timings.as_ref().map(|_| std::time::Instant::now());
    let lock = CacheEntryLock::acquire_local_from_parent(
        &collection,
        directory,
        OsStr::new(&lock_name),
        LOCAL_SNAPSHOT_LOCK_TIMEOUT,
    );
    if let (Some(timings), Some(started)) = (&mut *timings, lock_started) {
        timings.lock_finished(started);
    }
    let lock = lock?;
    let verification_started = timings.as_ref().map(|_| std::time::Instant::now());
    let source = (|| {
        verify_collection(lane, directory)?;
        let limits = limits.compiler_bounded();
        verify_local_cache_custody(&publication, limits)?;
        let source = verify_local_snapshot(&publication, identity, limits)?;
        lock.verify_path_identity()?;
        verify_collection(lane, directory)?;
        Ok(source)
    })();
    if let (Some(timings), Some(started)) = (timings, verification_started) {
        timings.verification_finished(started);
    }
    source
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
