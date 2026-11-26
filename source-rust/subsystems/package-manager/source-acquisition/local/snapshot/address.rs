// Address each local snapshot by the canonical origin and the selected content digest.

use crate::identity::SourceContentDigest;
use crate::identity::digest::format_sha256;
use crate::limits::LOCAL_SNAPSHOT_CUSTODY_POLICY;
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
        format!("{}.lock", self.publication_name())
    }
}
