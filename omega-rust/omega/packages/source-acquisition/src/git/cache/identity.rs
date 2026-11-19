// Domain-separated SHA-256 cache entry identities (`git_cache_identity`) and
// framed metadata records (`git_cache_metadata`) binding `GIT_CACHE_POLICY`,
// canonical locator URL, requested revision, and execution transport tag.

use crate::git::request::GitExecutionTransport;
use crate::identity::digest::{append_framed_bytes, format_sha256};
use crate::limits::GIT_CACHE_POLICY;

pub(crate) fn git_cache_identity(
    url: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
) -> String {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"");
    hasher.bytes(GIT_CACHE_POLICY);
    hasher.bytes(url.as_bytes());
    hasher.bytes(requested_rev.as_bytes());
    hasher.bytes(execution_transport.cache_tag());
    format_sha256(&hasher.finish())
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
