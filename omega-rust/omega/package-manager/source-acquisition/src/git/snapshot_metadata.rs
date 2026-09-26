//! Canonical metadata for authenticated Git snapshots.

use crate::SourceResolveError;
use crate::error::cache_invalid;
use crate::git::objects::identity::is_object_id;
use crate::identity::digest::append_framed_bytes;
use crate::limits::GIT_SNAPSHOT_POLICY;
use crate::tree::ResolvedLocalSource;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GitSnapshotMetadata {
    pub(super) tree: String,
    pub(super) file_count: usize,
    pub(super) byte_count: u64,
    pub(super) content_identity: String,
}

pub(crate) fn git_snapshot_metadata(tree: &str, local: &ResolvedLocalSource) -> Vec<u8> {
    let mut metadata = GIT_SNAPSHOT_POLICY.to_vec();
    append_framed_bytes(&mut metadata, tree.as_bytes());
    metadata.extend_from_slice(&(local.file_count as u64).to_le_bytes());
    metadata.extend_from_slice(&local.byte_count.to_le_bytes());
    append_framed_bytes(&mut metadata, local.content_identity.as_bytes());
    metadata
}

pub(crate) fn parse_git_snapshot_metadata(
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
