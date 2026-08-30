//! Stable Git cache identities, metadata records, and cache diagnostics.

use sha2::{Digest, Sha256};

use crate::git::request::GitExecutionTransport;
use crate::identity::digest::{append_framed_bytes, format_sha256, hash_bytes};
use crate::limits::GIT_CACHE_POLICY;

pub(crate) fn git_cache_identity(
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

pub(crate) fn git_cache_metadata(
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
