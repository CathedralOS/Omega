//! Resolver-owned evidence for accepted retained storage state.

use crate::custody::tree::{CacheCustodyMeasurement, git_cache_custody_byte_limit};
use crate::git::commands::identity::format_sha256;
use crate::limits::{
    CACHE_CUSTODY_DEPTH_LIMIT, CACHE_CUSTODY_ENTRY_LIMIT, GIT_CACHE_POLICY,
    GIT_RETAINED_STORAGE_OBSERVATION_DOMAIN, GIT_RETAINED_STORAGE_OBSERVATION_SCHEMA_VERSION,
    LocalSourceLimits,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Exact post-helper resident storage accepted by the final capability-rooted
/// Git cache traversal.
///
/// This opaque row proves only the bounded state observed after helper
/// completion. It does not claim that the helper was constrained while
/// writing, nor that transient disk use stayed beneath this ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRetainedStorageObservation {
    pub(crate) schema_version: u32,
    pub(crate) identity: String,
    pub(crate) root: PathBuf,
    pub(crate) entry_ceiling: usize,
    pub(crate) byte_ceiling: u64,
    pub(crate) depth_ceiling: usize,
    pub(crate) entry_count: usize,
    pub(crate) logical_bytes: u64,
    pub(crate) maximum_depth: usize,
}

impl GitRetainedStorageObservation {
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub const fn entry_ceiling(&self) -> usize {
        self.entry_ceiling
    }

    pub const fn byte_ceiling(&self) -> u64 {
        self.byte_ceiling
    }

    pub const fn depth_ceiling(&self) -> usize {
        self.depth_ceiling
    }

    pub const fn entry_count(&self) -> usize {
        self.entry_count
    }

    pub const fn logical_bytes(&self) -> u64 {
        self.logical_bytes
    }

    pub const fn maximum_depth(&self) -> usize {
        self.maximum_depth
    }
}

pub(crate) fn issue_git_retained_storage_observation(
    root: &Path,
    limits: LocalSourceLimits,
    measurement: CacheCustodyMeasurement,
) -> GitRetainedStorageObservation {
    let mut observation = GitRetainedStorageObservation {
        schema_version: GIT_RETAINED_STORAGE_OBSERVATION_SCHEMA_VERSION,
        identity: String::new(),
        root: root.to_path_buf(),
        entry_ceiling: CACHE_CUSTODY_ENTRY_LIMIT,
        byte_ceiling: git_cache_custody_byte_limit(limits),
        depth_ceiling: CACHE_CUSTODY_DEPTH_LIMIT,
        entry_count: measurement.entry_count,
        logical_bytes: measurement.logical_bytes,
        maximum_depth: measurement.maximum_depth,
    };
    observation.identity = git_retained_storage_identity(&observation);
    observation
}

pub(crate) fn validate_git_retained_storage_observation(
    observation: &GitRetainedStorageObservation,
    root: &Path,
    limits: LocalSourceLimits,
) -> bool {
    observation.schema_version == GIT_RETAINED_STORAGE_OBSERVATION_SCHEMA_VERSION
        && observation.root == root
        && observation.entry_ceiling == CACHE_CUSTODY_ENTRY_LIMIT
        && observation.byte_ceiling == git_cache_custody_byte_limit(limits)
        && observation.depth_ceiling == CACHE_CUSTODY_DEPTH_LIMIT
        && observation.entry_count > 0
        && observation.entry_count <= observation.entry_ceiling
        && observation.logical_bytes <= observation.byte_ceiling
        && observation.maximum_depth <= observation.depth_ceiling
        && observation.identity == git_retained_storage_identity(observation)
}

fn git_retained_storage_identity(observation: &GitRetainedStorageObservation) -> String {
    let mut hasher = Sha256::new();
    hash_storage_field(&mut hasher, GIT_RETAINED_STORAGE_OBSERVATION_DOMAIN);
    hash_storage_u64(&mut hasher, u64::from(observation.schema_version));
    hash_storage_field(&mut hasher, GIT_CACHE_POLICY);
    hash_storage_path(&mut hasher, &observation.root);
    hash_storage_usize(&mut hasher, observation.entry_ceiling);
    hash_storage_u64(&mut hasher, observation.byte_ceiling);
    hash_storage_usize(&mut hasher, observation.depth_ceiling);
    hash_storage_usize(&mut hasher, observation.entry_count);
    hash_storage_u64(&mut hasher, observation.logical_bytes);
    hash_storage_usize(&mut hasher, observation.maximum_depth);
    format_sha256(&hasher.finalize())
}

fn hash_storage_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(
        u64::try_from(value.len())
            .expect("bounded retained-storage fields fit canonical u64")
            .to_le_bytes(),
    );
    hasher.update(value);
}

fn hash_storage_u64(hasher: &mut Sha256, value: u64) {
    hash_storage_field(hasher, &value.to_le_bytes());
}

fn hash_storage_usize(hasher: &mut Sha256, value: usize) {
    hash_storage_u64(
        hasher,
        u64::try_from(value).expect("compiler-owned retained-storage ceilings fit canonical u64"),
    );
}

fn hash_storage_path(hasher: &mut Sha256, path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hash_storage_field(hasher, b"unix-path");
        hash_storage_field(hasher, path.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        hash_storage_field(hasher, b"windows-path");
        let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
        hash_storage_usize(hasher, units.len());
        for unit in units {
            hash_storage_field(hasher, &unit.to_le_bytes());
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        hash_storage_field(hasher, b"platform-path");
        hash_storage_field(hasher, path.as_os_str().as_encoded_bytes());
    }
}
