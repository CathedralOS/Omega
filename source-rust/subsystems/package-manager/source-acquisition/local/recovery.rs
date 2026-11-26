// Recover the snapshot published for this origin and content pin. A package
// review can still read the baseline after the author moves or deletes the
// live directory. The selected cache entry must pass the same custody and
// content checks as a live acquisition.

use crate::SourceResolveError;
use crate::custody::lock::CacheEntryLock;
use crate::custody::platform::same_capability_file_identity;
use crate::custody::tree::verify_local_cache_custody;
use crate::error::local_snapshot_invalid;
use crate::identity::SourceContentDigest;
use crate::limits::{LOCAL_CACHE_SNAPSHOTS, LocalSourceLimits};
use crate::snapshot::metadata::{local_snapshot_content_identity, verify_local_snapshot};
use crate::storage::RetainedStorageLane;
use crate::tree::ResolvedLocalSource;
use crate::tree::filesystem::IoResultAt;
use crate::tree::filesystem::io_error;
use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use std::ffi::OsStr;
use std::path::{Component, Path};

use super::snapshot::LocalSnapshotAddress;
use crate::limits::LOCAL_SNAPSHOT_LOCK_TIMEOUT;

mod timings;
use timings::RecoveryTimings;

pub fn recover_cached_local_source_in_lane(
    canonical_origin: &Path,
    expected: &SourceContentDigest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
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
        match directory.symlink_metadata(&publication_name) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(local_snapshot_invalid(
                    &publication,
                    "direct local snapshot publication is not a concrete directory",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some(timings) = &mut timings {
                    timings.direct_miss();
                }
                return Ok(None);
            }
            Err(error) => return Err(io_error(&publication, error)),
        }
        let identity = read_routing_identity(&publication, &mut timings)?;
        if SourceContentDigest::derive(identity.as_bytes()) != *expected {
            return Err(local_snapshot_invalid(
                &publication,
                "direct local snapshot metadata does not match its origin and content pin",
            ));
        }
        let source =
            verify_selected_snapshot(lane, &directory, &address, &identity, limits, &mut timings)?;
        if let Some(timings) = &mut timings {
            timings.direct_hit();
        }
        Ok(Some(source))
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
    address: &LocalSnapshotAddress,
    identity: &str,
    limits: LocalSourceLimits,
    timings: &mut Option<RecoveryTimings>,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    if let Some(timings) = &mut *timings {
        timings.candidate_matched();
    }
    let collection = lane.path().join(LOCAL_CACHE_SNAPSHOTS);
    let publication = collection.join(address.publication_name());
    let lock_name = address.lock_name();
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
        .at(&path)?;
    let first = retained.dir_metadata().at(&path)?;
    let second = named.dir_metadata().at(&path)?;
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
