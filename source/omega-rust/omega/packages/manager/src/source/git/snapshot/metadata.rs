//! Snapshot identity records and published-snapshot verification.

use crate::source::{
    CacheCustodyKind, GIT_SNAPSHOT_POLICY, LOCAL_SNAPSHOT_METADATA, LOCAL_SNAPSHOT_POLICY,
    LOCAL_SNAPSHOT_SOURCE, LocalSourceLimits, ResolvedLocalSource, SourceResolveError,
    SourceTreePolicy, append_framed_bytes, cache_invalid, capture_local_source_from_open_root,
    is_object_id, local_snapshot_invalid, open_absolute_directory_nofollow,
    read_bounded_cache_record,
};
use cap_fs_ext::DirExt;
use std::path::Path;

use super::permissions::verify_open_snapshot_tree_modes;

#[derive(Debug, PartialEq, Eq)]
struct LocalSnapshotMetadata {
    file_count: usize,
    byte_count: u64,
    content_identity: String,
}

pub(in crate::source) fn local_snapshot_metadata(local: &ResolvedLocalSource) -> Vec<u8> {
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

pub(in crate::source) fn verify_local_snapshot(
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
pub(in crate::source) struct GitSnapshotMetadata {
    pub(super) tree: String,
    pub(super) file_count: usize,
    pub(super) byte_count: u64,
    pub(super) content_identity: String,
}

pub(in crate::source) fn git_snapshot_metadata(tree: &str, local: &ResolvedLocalSource) -> Vec<u8> {
    let mut metadata = GIT_SNAPSHOT_POLICY.to_vec();
    append_framed_bytes(&mut metadata, tree.as_bytes());
    metadata.extend_from_slice(&(local.file_count as u64).to_le_bytes());
    metadata.extend_from_slice(&local.byte_count.to_le_bytes());
    append_framed_bytes(&mut metadata, local.content_identity.as_bytes());
    metadata
}

pub(super) fn parse_git_snapshot_metadata(
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
