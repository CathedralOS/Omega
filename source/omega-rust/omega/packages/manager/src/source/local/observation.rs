//! Opaque final observation for one local-source resolution.

use sha2::{Digest, Sha256};
use std::path::Path;

use super::capture::hash_bytes;
use super::model::{LocalSourceResolutionObservation, ResolvedLocalSource};
use super::snapshot::local_snapshot_custody_identity;
use crate::source::git::format_sha256;
use crate::source::{
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
    let custody_identity =
        local_snapshot_custody_identity(canonical_live_root, &final_snapshot.content_identity);
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, LOCAL_RESOLUTION_OBSERVATION_DOMAIN);
    hash_u64(
        &mut hasher,
        u64::from(LOCAL_RESOLUTION_OBSERVATION_SCHEMA_VERSION),
    );
    hash_field(&mut hasher, LOCAL_SNAPSHOT_POLICY);
    hash_field(&mut hasher, LOCAL_SNAPSHOT_CUSTODY_POLICY);
    hash_path(&mut hasher, requested_root);
    hash_path(&mut hasher, canonical_live_root);
    hash_path(&mut hasher, publication_root);
    hash_path(&mut hasher, &final_snapshot.root);
    hash_usize(&mut hasher, final_snapshot.file_count);
    hash_u64(&mut hasher, final_snapshot.byte_count);
    hash_field(&mut hasher, final_snapshot.content_identity.as_bytes());
    hash_usize(&mut hasher, limits.max_files);
    hash_u64(&mut hasher, limits.max_bytes);
    hash_usize(&mut hasher, limits.max_depth);
    hash_field(&mut hasher, custody_identity.as_bytes());
    hash_field(&mut hasher, b"final-exact-tree-rehash-complete");
    hash_field(&mut hasher, b"resolved-non-admitting");

    LocalSourceResolutionObservation {
        schema_version: LOCAL_RESOLUTION_OBSERVATION_SCHEMA_VERSION,
        identity: format_sha256(&hasher.finalize()),
        custody_identity,
    }
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hash_bytes(hasher, value);
}

fn hash_u64(hasher: &mut Sha256, value: u64) {
    hash_field(hasher, &value.to_le_bytes());
}

fn hash_usize(hasher: &mut Sha256, value: usize) {
    hash_u64(
        hasher,
        u64::try_from(value).expect("compiler-owned source ceilings fit canonical u64"),
    );
}

fn hash_path(hasher: &mut Sha256, path: &Path) {
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
        hash_usize(hasher, units.len());
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
