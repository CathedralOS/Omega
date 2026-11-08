//! Opaque final observation for one local-source resolution.

use std::path::Path;

use super::resolution_observations::{LocalSourceResolutionObservation, ResolvedLocalSource};
use super::snapshot::LocalSnapshotAddress;
use crate::identity::SourceContentDigest;
use crate::identity::digest::format_sha256;
use crate::limits::{
    LOCAL_RESOLUTION_OBSERVATION_DOMAIN, LOCAL_RESOLUTION_OBSERVATION_SCHEMA_VERSION,
    LOCAL_SNAPSHOT_CUSTODY_POLICY, LOCAL_SNAPSHOT_POLICY, LocalSourceLimits,
};

pub(super) fn issue_local_source_resolution_observation(
    requested_root: &Path,
    canonical_live_root: &Path,
    publication_root: &Path,
    final_snapshot: &ResolvedLocalSource,
    limits: LocalSourceLimits,
) -> LocalSourceResolutionObservation {
    let content = SourceContentDigest::derive(final_snapshot.content_identity.as_bytes());
    let custody_identity = LocalSnapshotAddress::new(canonical_live_root, &content)
        .custody_identity()
        .to_owned();
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(b"");
    hasher.bytes(LOCAL_RESOLUTION_OBSERVATION_DOMAIN);
    hash_u64(
        &mut hasher,
        u64::from(LOCAL_RESOLUTION_OBSERVATION_SCHEMA_VERSION),
    );
    hasher.bytes(LOCAL_SNAPSHOT_POLICY);
    hasher.bytes(LOCAL_SNAPSHOT_CUSTODY_POLICY);
    hash_path(&mut hasher, requested_root);
    hash_path(&mut hasher, canonical_live_root);
    hash_path(&mut hasher, publication_root);
    hash_path(&mut hasher, &final_snapshot.root);
    hasher.bytes(&final_snapshot.file_count.to_le_bytes());
    hasher.bytes(&final_snapshot.byte_count.to_le_bytes());
    hasher.bytes(final_snapshot.content_identity.as_bytes());
    hasher.bytes(&limits.max_entries.to_le_bytes());
    hasher.bytes(&limits.max_bytes.to_le_bytes());
    hasher.bytes(&limits.max_depth.to_le_bytes());
    hasher.bytes(custody_identity.as_bytes());
    hash_field(&mut hasher, b"final-exact-tree-rehash-complete");
    hash_field(&mut hasher, b"resolved");

    LocalSourceResolutionObservation {
        schema_version: LOCAL_RESOLUTION_OBSERVATION_SCHEMA_VERSION,
        identity: format_sha256(&hasher.finish()),
        custody_identity,
    }
}

fn hash_field(hasher: &mut crate::digest_encoder::DomainDigestEncoder, value: &[u8]) {
    hasher.bytes(value);
}

fn hash_u64(hasher: &mut crate::digest_encoder::DomainDigestEncoder, value: u64) {
    hasher.bytes(&value.to_le_bytes());
}

fn hash_path(hasher: &mut crate::digest_encoder::DomainDigestEncoder, path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hash_field(hasher, b"unix-path");
        hash_field(hasher, path.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        hash_field(hasher, b"windows-path");
        let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
        // Observation identities length-frame this canonical u64 count.
        hash_u64(
            hasher,
            u64::try_from(units.len()).expect("compiler-owned source ceilings fit canonical u64"),
        );
        for unit in units {
            hash_field(hasher, &unit.to_le_bytes());
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        hash_field(hasher, b"platform-path");
        hash_field(hasher, path.as_os_str().as_encoded_bytes());
    }
}
