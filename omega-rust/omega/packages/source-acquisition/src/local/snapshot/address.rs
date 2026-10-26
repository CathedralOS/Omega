//! One origin-bound publication address from the content pin callers retain.
//! Historical raw-tree keys remain readable through verified legacy recovery;
//! new publication and observation never reinterpret those old addresses.

use crate::identity::SourceContentDigest;
use crate::identity::digest::format_sha256;
use crate::limits::{LEGACY_LOCAL_SNAPSHOT_CUSTODY_POLICY, LOCAL_SNAPSHOT_CUSTODY_POLICY};
use crate::tree::filesystem::raw_os_bytes;
use std::path::Path;

pub(crate) struct LocalSnapshotAddress {
    custody_identity: String,
}

impl LocalSnapshotAddress {
    pub(crate) fn new(canonical_origin: &Path, content: &SourceContentDigest) -> Self {
        let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"");
        hasher.bytes(LOCAL_SNAPSHOT_CUSTODY_POLICY);
        hasher.bytes(raw_os_bytes(canonical_origin.as_os_str()).as_slice());
        hasher.bytes(content.digest());
        Self {
            custody_identity: format_sha256(&hasher.finish()),
        }
    }

    pub(crate) fn custody_identity(&self) -> &str {
        &self.custody_identity
    }

    pub(crate) fn publication_name(&self) -> String {
        format!("source-v2-{}", self.custody_identity)
    }

    pub(crate) fn lock_name(&self) -> String {
        snapshot_lock_name(&self.publication_name())
    }
}

pub(in crate::local) fn snapshot_lock_name(publication_name: &str) -> String {
    format!("{publication_name}.lock")
}

pub(in crate::local) fn legacy_snapshot_publication_name(
    canonical_origin: &Path,
    raw_tree_identity: &str,
) -> String {
    format!(
        "source-{}",
        legacy_local_snapshot_custody_identity(canonical_origin, raw_tree_identity)
    )
}

fn legacy_local_snapshot_custody_identity(
    canonical_origin: &Path,
    raw_tree_identity: &str,
) -> String {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"");
    hasher.bytes(LEGACY_LOCAL_SNAPSHOT_CUSTODY_POLICY);
    hasher.bytes(raw_os_bytes(canonical_origin.as_os_str()).as_slice());
    hasher.bytes(raw_tree_identity.as_bytes());
    format_sha256(&hasher.finish())
}

pub(in crate::local) fn is_legacy_snapshot_name(name: &str) -> bool {
    name.strip_prefix("source-").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
