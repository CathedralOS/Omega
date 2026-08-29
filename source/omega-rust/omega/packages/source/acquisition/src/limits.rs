//! Compiler-owned source, cache, process, and observation ceilings.

use std::sync::atomic::AtomicU64;
use std::time::Duration;

pub(crate) const GIT_CACHE_POLICY: &[u8] = b"omega-git-cache-v28";
pub(crate) const GIT_CACHE_METADATA: &str = "source.identity";
pub(crate) const GIT_CACHE_REPOSITORY: &str = "repository";
pub(crate) const GIT_CACHE_SNAPSHOTS: &str = "snapshots";
pub(crate) const GIT_SNAPSHOT_METADATA: &str = "snapshot.identity";
pub(crate) const GIT_SNAPSHOT_SOURCE: &str = "source";
pub(crate) const GIT_SNAPSHOT_POLICY: &[u8] = b"omega-git-snapshot-v4";
pub(crate) const LOCAL_CACHE_SNAPSHOTS: &str = "local-snapshots";
pub(crate) const LOCAL_SNAPSHOT_METADATA: &str = "snapshot.identity";
pub(crate) const LOCAL_SNAPSHOT_SOURCE: &str = "source";
pub(crate) const LOCAL_SNAPSHOT_POLICY: &[u8] = b"omega-local-source-snapshot-v2";
pub(crate) const LOCAL_SNAPSHOT_CUSTODY_POLICY: &[u8] = b"omega-local-source-snapshot-custody-v1";
pub(crate) const LOCAL_RESOLUTION_OBSERVATION_SCHEMA_VERSION: u32 = 1;
pub(crate) const LOCAL_RESOLUTION_OBSERVATION_DOMAIN: &[u8] =
    b"omega-local-source-resolution-observation-v1";
pub(crate) const DEFAULT_BUILD_OUTPUT_DIRECTORY: &str = "build";
pub(crate) const CANONICAL_DIRECTORY_MODE: u16 = 0o555;
pub(crate) const GIT_CONFIG_SHA1: &[u8] =
    b"[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = true\n";
pub(crate) const GIT_CONFIG_SHA256: &[u8] = b"[core]\n\trepositoryformatversion = 1\n\tfilemode = false\n\tbare = true\n[extensions]\n\tobjectformat = sha256\n";
pub(crate) const CACHE_CUSTODY_ENTRY_LIMIT: usize = 65_536;
pub(crate) const SOURCE_ENTRY_ABSOLUTE_LIMIT: usize = 65_536;
pub(crate) const SOURCE_BYTE_ABSOLUTE_LIMIT: u64 = 512 * 1024 * 1024;
pub(crate) const SOURCE_DEPTH_ABSOLUTE_LIMIT: usize = 256;
pub(crate) const CACHE_CUSTODY_DEPTH_LIMIT: usize = SOURCE_DEPTH_ABSOLUTE_LIMIT + 4;
pub(crate) const GIT_LOCATOR_BYTE_LIMIT: usize = 4 * 1024;
pub(crate) const GIT_REVISION_BYTE_LIMIT: usize = 1024;
pub(crate) const CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE: u64 = 64 * 1024 * 1024;
pub(crate) const GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT: u64 = 1024 * 1024 * 1024;
pub(crate) const LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT: u64 = 512 * 1024 * 1024;
pub(crate) const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
pub(crate) const GIT_STDOUT_LIMIT: usize = 16 * 1024 * 1024;
pub(crate) const GIT_STDERR_LIMIT: usize = 1024 * 1024;
pub(crate) const GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE: u64 = 64 * 1024 * 1024;
pub(crate) const GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT: u64 = 576 * 1024 * 1024;
pub(crate) const GIT_NETWORK_TRANSFER_FIXED_ALLOWANCE: u64 = 64 * 1024 * 1024;
pub(crate) const GIT_NETWORK_TRANSFER_ABSOLUTE_LIMIT: u64 = 576 * 1024 * 1024;
pub(crate) const GIT_EXECUTABLE_BYTE_LIMIT: u64 = 256 * 1024 * 1024;
pub(crate) const GIT_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
pub(crate) const GIT_FIXED_COMMAND_ALLOWANCE: usize = 64;
pub(crate) const GIT_COMMAND_CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const LOCAL_SNAPSHOT_LOCK_TIMEOUT: Duration = Duration::from_secs(120);
pub(crate) const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(5);
pub(crate) const GIT_RESOLUTION_OBSERVATION_SCHEMA_VERSION: u32 = 5;
pub(crate) const GIT_RESOLUTION_OBSERVATION_DOMAIN: &[u8] = b"omega-git-resolution-observation-v4";
pub(crate) static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalSourceLimits {
    /// Legacy field name: this caps every non-root source identity entry, including directories.
    pub max_files: usize,
    pub max_bytes: u64,
    pub max_depth: usize,
}

impl Default for LocalSourceLimits {
    fn default() -> Self {
        Self {
            max_files: 4096,
            max_bytes: 256 * 1024 * 1024,
            max_depth: 64,
        }
    }
}

impl LocalSourceLimits {
    /// Apply compiler-owned ceilings to caller-selected source limits.
    ///
    /// These are acceptance limits enforced by the resolver. They do not
    /// claim to constrain an unconfined helper while it is writing its
    /// quarantine object store.
    #[doc(hidden)]
    pub fn compiler_bounded(self) -> Self {
        Self {
            max_files: self.max_files.min(SOURCE_ENTRY_ABSOLUTE_LIMIT),
            max_bytes: self.max_bytes.min(SOURCE_BYTE_ABSOLUTE_LIMIT),
            max_depth: self.max_depth.min(SOURCE_DEPTH_ABSOLUTE_LIMIT),
        }
    }
}
