//! Stable Git cache identities, metadata records, and cache diagnostics.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::source::SourceResolveError;
use crate::source::git::process::identity::format_sha256;
use crate::source::git::request::GitExecutionTransport;
use crate::source::limits::GIT_CACHE_POLICY;
use crate::source::local::capture::hash_bytes;

pub(in crate::source) fn git_cache_identity(
    url: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
) -> String {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, GIT_CACHE_POLICY);
    hash_bytes(&mut hasher, url.as_bytes());
    hash_bytes(&mut hasher, requested_rev.as_bytes());
    hash_bytes(&mut hasher, execution_transport.cache_tag());
    format_sha256(&hasher.finalize())
}

pub(in crate::source) fn git_cache_metadata(
    url: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
) -> Vec<u8> {
    let mut metadata = Vec::new();
    metadata.extend_from_slice(GIT_CACHE_POLICY);
    append_framed_bytes(&mut metadata, url.as_bytes());
    append_framed_bytes(&mut metadata, requested_rev.as_bytes());
    append_framed_bytes(&mut metadata, execution_transport.cache_tag());
    metadata
}

pub(in crate::source) fn append_framed_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(bytes);
}

pub(in crate::source) fn cache_invalid(
    path: &Path,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::GitCacheInvalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

pub(in crate::source) fn local_snapshot_invalid(
    path: &Path,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::LocalSnapshotInvalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}
