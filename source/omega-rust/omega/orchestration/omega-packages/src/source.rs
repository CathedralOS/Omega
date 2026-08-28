use crate::identity::{
    GitObjectIdAlgorithm, GitTransport, IdentityError, SourceContentDigest, SourceLineage,
};
use crate::record_file::{RecordFileLimits, RecordFileRoot};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::{
    DirBuilderExt as CapabilityDirBuilderExt, OpenOptionsExt as CapabilityOpenOptionsExt,
    PermissionsExt as CapabilityPermissionsExt,
};
use cap_std::{
    ambient_authority,
    fs::{
        Dir as CapabilityDirectory, DirBuilder as CapabilityDirBuilder,
        Metadata as CapabilityMetadata, OpenOptions as CapabilityOpenOptions,
    },
};
use command_group::{CommandGroup, GroupChild};
use sha1_checked::Sha1 as CheckedSha1;
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime};

const GIT_CACHE_POLICY: &[u8] = b"omega-git-cache-v12";
const GIT_CACHE_METADATA: &str = "source.identity";
const GIT_CACHE_REPOSITORY: &str = "repository";
const GIT_CACHE_SNAPSHOTS: &str = "snapshots";
const GIT_SNAPSHOT_METADATA: &str = "snapshot.identity";
const GIT_SNAPSHOT_SOURCE: &str = "source";
const GIT_SNAPSHOT_POLICY: &[u8] = b"omega-git-snapshot-v4";
const LOCAL_CACHE_SNAPSHOTS: &str = "local-snapshots";
const LOCAL_SNAPSHOT_METADATA: &str = "snapshot.identity";
const LOCAL_SNAPSHOT_SOURCE: &str = "source";
const LOCAL_SNAPSHOT_POLICY: &[u8] = b"omega-local-source-snapshot-v2";
const LOCAL_SNAPSHOT_CUSTODY_POLICY: &[u8] = b"omega-local-source-snapshot-custody-v1";
const DEFAULT_BUILD_OUTPUT_DIRECTORY: &str = "build";
const CANONICAL_DIRECTORY_MODE: u16 = 0o555;
const GIT_CONFIG_SHA1: &[u8] =
    b"[core]\n\trepositoryformatversion = 0\n\tfilemode = false\n\tbare = true\n";
const GIT_CONFIG_SHA256: &[u8] = b"[core]\n\trepositoryformatversion = 1\n\tfilemode = false\n\tbare = true\n[extensions]\n\tobjectformat = sha256\n";
const CACHE_CUSTODY_ENTRY_LIMIT: usize = 65_536;
const SOURCE_ENTRY_ABSOLUTE_LIMIT: usize = 65_536;
const SOURCE_BYTE_ABSOLUTE_LIMIT: u64 = 512 * 1024 * 1024;
const SOURCE_DEPTH_ABSOLUTE_LIMIT: usize = 256;
const CACHE_CUSTODY_DEPTH_LIMIT: usize = SOURCE_DEPTH_ABSOLUTE_LIMIT + 4;
const GIT_LOCATOR_BYTE_LIMIT: usize = 4 * 1024;
const GIT_REVISION_BYTE_LIMIT: usize = 1024;
const CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE: u64 = 64 * 1024 * 1024;
const GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT: u64 = 1024 * 1024 * 1024;
const LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT: u64 = 512 * 1024 * 1024;
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const GIT_STDOUT_LIMIT: usize = 16 * 1024 * 1024;
const GIT_STDERR_LIMIT: usize = 1024 * 1024;
const GIT_EXECUTABLE_BYTE_LIMIT: u64 = 256 * 1024 * 1024;
const GIT_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const GIT_FIXED_COMMAND_ALLOWANCE: usize = 64;
const GIT_COMMAND_CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
const LOCAL_SNAPSHOT_LOCK_TIMEOUT: Duration = Duration::from_secs(120);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(5);
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    pub(crate) fn compiler_bounded(self) -> Self {
        Self {
            max_files: self.max_files.min(SOURCE_ENTRY_ABSOLUTE_LIMIT),
            max_bytes: self.max_bytes.min(SOURCE_BYTE_ABSOLUTE_LIMIT),
            max_depth: self.max_depth.min(SOURCE_DEPTH_ABSOLUTE_LIMIT),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocalSource {
    pub root: PathBuf,
    /// Number of file and symlink leaves. Directories participate in identity and limits but are
    /// not reported as files.
    pub file_count: usize,
    pub byte_count: u64,
    pub content_identity: String,
}

/// A resolver-owned immutable copy of a requested local source tree.
///
/// `requested_root` preserves the caller's locator, `canonical_live_root` identifies the mutable
/// tree that was captured, and `snapshot_root` is the only path downstream consumers should use.
/// `normalized` is re-resolved from that published snapshot rather than trusted from the live tree
/// or staging directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocalSnapshot {
    pub requested_root: PathBuf,
    pub canonical_live_root: PathBuf,
    pub snapshot_root: PathBuf,
    pub normalized: ResolvedLocalSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSourceRequest {
    requested_locator: String,
    fetch_locator: String,
    locator_identity: String,
    requested_revision: String,
    lineage: SourceLineage,
    execution_transport: GitExecutionTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitExecutionTransport {
    Https,
    Ssh,
    #[cfg(test)]
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitTransportProfile {
    Https,
    Ssh,
    TestFile,
}

impl GitTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Ssh => "ssh",
            Self::TestFile => "test-file",
        }
    }
}

impl GitExecutionTransport {
    fn from_locator_transport(transport: GitTransport) -> Self {
        match transport {
            GitTransport::Https => Self::Https,
            GitTransport::SshUrl | GitTransport::ScpLike => Self::Ssh,
        }
    }

    fn cache_tag(self) -> &'static [u8] {
        match self {
            Self::Https => b"https",
            Self::Ssh => b"ssh",
            #[cfg(test)]
            Self::File => b"test-file",
        }
    }

    fn allowed_protocol(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Ssh => "ssh",
            #[cfg(test)]
            Self::File => "file",
        }
    }

    fn permits(self, transport: Self) -> &'static str {
        if self == transport { "always" } else { "never" }
    }

    fn profile(self) -> GitTransportProfile {
        match self {
            Self::Https => GitTransportProfile::Https,
            Self::Ssh => GitTransportProfile::Ssh,
            #[cfg(test)]
            Self::File => GitTransportProfile::TestFile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitSourceRequestError {
    EmptyLocator,
    LocatorTooLong { limit: usize },
    InvalidLocator(IdentityError),
    EmptyRevision,
    RevisionTooLong { limit: usize },
    InvalidRevision,
}

impl GitSourceRequest {
    pub fn new(
        locator: impl Into<String>,
        revision: Option<String>,
    ) -> Result<Self, GitSourceRequestError> {
        let locator = locator.into();
        if locator.is_empty() {
            return Err(GitSourceRequestError::EmptyLocator);
        }
        if locator.len() > GIT_LOCATOR_BYTE_LIMIT {
            return Err(GitSourceRequestError::LocatorTooLong {
                limit: GIT_LOCATOR_BYTE_LIMIT,
            });
        }
        if locator.trim() != locator {
            return Err(GitSourceRequestError::InvalidLocator(
                IdentityError::MalformedGitLocator,
            ));
        }
        let (lineage, locator_transport) = SourceLineage::git_with_transport(&locator)
            .map_err(GitSourceRequestError::InvalidLocator)?;
        let requested_revision = revision.unwrap_or_else(|| "HEAD".to_owned());
        validate_git_revision(&requested_revision)?;
        let locator_identity = canonical_git_locator(&lineage);
        Ok(Self {
            requested_locator: locator.clone(),
            fetch_locator: locator,
            locator_identity,
            requested_revision,
            lineage,
            execution_transport: GitExecutionTransport::from_locator_transport(locator_transport),
        })
    }

    pub fn locator_identity(&self) -> &str {
        &self.locator_identity
    }

    /// The exact validated locator spelling supplied by the caller.
    ///
    /// This remains distinct from normalized lineage and locator identity so a
    /// future lock can retain the selector that was actually resolved. Request
    /// validation rejects embedded credentials before this value exists.
    pub fn requested_locator(&self) -> &str {
        &self.requested_locator
    }

    pub fn requested_revision(&self) -> &str {
        &self.requested_revision
    }

    pub fn lineage(&self) -> &SourceLineage {
        &self.lineage
    }

    fn fetch_locator(&self) -> &str {
        &self.fetch_locator
    }

    fn execution_transport(&self) -> GitExecutionTransport {
        self.execution_transport
    }

    pub fn transport_profile(&self) -> GitTransportProfile {
        self.execution_transport.profile()
    }

    #[cfg(test)]
    pub(crate) fn for_local_test_repository(
        repository: &Path,
        revision: Option<String>,
    ) -> Result<Self, GitSourceRequestError> {
        let path_identity = Sha256::digest(repository.as_os_str().to_string_lossy().as_bytes());
        let mut request = Self::new(
            format!(
                "https://local-fixture.invalid/{}.git",
                format_sha256(&path_identity)
            ),
            revision,
        )?;
        request.fetch_locator = repository.display().to_string();
        request.execution_transport = GitExecutionTransport::File;
        Ok(request)
    }

    #[cfg(test)]
    pub(crate) fn for_local_test_repository_with_lineage(
        repository: &Path,
        revision: Option<String>,
        remote_locator: &str,
    ) -> Result<Self, GitSourceRequestError> {
        let mut request = Self::new(remote_locator, revision)?;
        request.fetch_locator = repository.display().to_string();
        request.execution_transport = GitExecutionTransport::File;
        Ok(request)
    }
}

impl fmt::Display for GitSourceRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLocator => formatter.write_str("Git source locator is empty"),
            Self::LocatorTooLong { limit } => {
                write!(formatter, "Git source locator exceeds {limit} bytes")
            }
            Self::InvalidLocator(error) => write!(formatter, "invalid Git source locator: {error}"),
            Self::EmptyRevision => formatter.write_str("Git source revision is empty"),
            Self::RevisionTooLong { limit } => {
                write!(formatter, "Git source revision exceeds {limit} bytes")
            }
            Self::InvalidRevision => formatter.write_str(
                "Git source revision must be one closed selector without refspec syntax",
            ),
        }
    }
}

impl std::error::Error for GitSourceRequestError {}

fn validate_git_revision(revision: &str) -> Result<(), GitSourceRequestError> {
    if revision.is_empty() {
        return Err(GitSourceRequestError::EmptyRevision);
    }
    if revision.len() > GIT_REVISION_BYTE_LIMIT {
        return Err(GitSourceRequestError::RevisionTooLong {
            limit: GIT_REVISION_BYTE_LIMIT,
        });
    }
    if revision.starts_with(['-', '/', '.'])
        || revision.ends_with(['/', '.'])
        || revision.contains("..")
        || revision.contains("//")
        || revision.contains("@{")
        || revision.split('/').any(|component| {
            component.is_empty() || component.starts_with('.') || component.ends_with(".lock")
        })
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        return Err(GitSourceRequestError::InvalidRevision);
    }
    Ok(())
}

fn canonical_git_locator(lineage: &SourceLineage) -> String {
    match lineage {
        SourceLineage::GitHub(lineage) => format!(
            "https://github.com/{}/{}.git",
            lineage.owner(),
            lineage.repository()
        ),
        SourceLineage::GitLab(lineage) => {
            format!("https://gitlab.com/{}.git", lineage.repository_path())
        }
        SourceLineage::Git(lineage) => {
            let user = lineage
                .user()
                .map(|user| format!("{user}@"))
                .unwrap_or_default();
            match lineage.transport() {
                GitTransport::Https => format!(
                    "https://{}{}{}/{}",
                    user,
                    lineage.host(),
                    lineage
                        .port()
                        .map(|port| format!(":{port}"))
                        .unwrap_or_default(),
                    lineage.repository_path()
                ),
                GitTransport::SshUrl => format!(
                    "ssh://{}{}{}/{}",
                    user,
                    lineage.host(),
                    lineage
                        .port()
                        .map(|port| format!(":{port}"))
                        .unwrap_or_default(),
                    lineage.repository_path()
                ),
                GitTransport::ScpLike => {
                    format!("{}{}:{}", user, lineage.host(), lineage.repository_path())
                }
            }
        }
        SourceLineage::Workspace(_) | SourceLineage::ExternalLocal(_) => {
            unreachable!("validated Git requests always carry Git lineage")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGitSource {
    pub requested_locator: String,
    pub locator_identity: String,
    pub transport_profile: GitTransportProfile,
    pub requested_rev: String,
    pub commit: String,
    pub tree: String,
    pub snapshot_root: PathBuf,
    pub local: ResolvedLocalSource,
    /// Absolute parent Git executable identity observed before and after every launch.
    /// This is diagnostic custody, not certification of the executable.
    pub git_executable: GitExecutableIdentity,
    /// Exact transport executable observed for HTTPS or SSH resolution.
    /// The test-only file adapter retains no transport executable here.
    pub transport_executable: Option<GitTransportExecutableIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitExecutableIdentity {
    path: PathBuf,
    content_identity: String,
}

impl GitExecutableIdentity {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn content_identity(&self) -> &str {
        &self.content_identity
    }

    #[cfg(test)]
    pub(crate) fn for_test(path: PathBuf, content_identity: String) -> Self {
        Self {
            path,
            content_identity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitTransportExecutableIdentity {
    invocation_path: PathBuf,
    path: PathBuf,
    content_identity: String,
}

impl GitTransportExecutableIdentity {
    /// Exact path through which Git selects this transport executable.
    ///
    /// HTTPS uses the install-owned `git-remote-https` entry while `path()`
    /// names its canonical executable target. SSH is invoked directly through
    /// the canonical path, so both paths are normally equal.
    pub fn invocation_path(&self) -> &Path {
        &self.invocation_path
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn content_identity(&self) -> &str {
        &self.content_identity
    }
}

#[derive(Debug)]
struct GitExecutor {
    identity: GitExecutableIdentity,
    metadata_identity: GitExecutableMetadataIdentity,
    transport_executable: Option<GitTransportExecutableObservation>,
    execution_transport: GitExecutionTransport,
    started: Instant,
    timeout: Duration,
    launches: Cell<usize>,
    maximum_launches: usize,
}

#[derive(Debug)]
struct GitTransportExecutableObservation {
    identity: GitTransportExecutableIdentity,
    metadata_identity: GitExecutableMetadataIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitExecutableMetadataIdentity {
    length: u64,
    modified: SystemTime,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceResolveError {
    Io {
        path: PathBuf,
        message: String,
    },
    NotDirectory {
        path: PathBuf,
    },
    TooManyFiles {
        limit: usize,
    },
    TooManyBytes {
        limit: u64,
    },
    TooDeep {
        path: PathBuf,
        limit: usize,
    },
    SymlinkEscapesRoot {
        link: PathBuf,
        target: PathBuf,
    },
    SymlinkTargetsExcludedMetadata {
        link: PathBuf,
        target: PathBuf,
    },
    SymlinkTargetsExcludedBuildOutput {
        link: PathBuf,
        target: PathBuf,
    },
    UnsupportedFileType {
        path: PathBuf,
    },
    Git {
        operation: String,
        status: Option<i32>,
        stderr: String,
    },
    GitOutputOverflow {
        operation: String,
        stream: String,
        limit: usize,
    },
    GitTimedOut {
        operation: String,
        timeout_millis: u64,
    },
    GitExecutableUnavailable,
    GitExecutableInvalid {
        path: PathBuf,
        message: String,
    },
    GitExecutableChanged {
        path: PathBuf,
    },
    GitResolutionCommandLimit {
        limit: usize,
    },
    GitResolutionTimedOut {
        timeout_millis: u64,
    },
    GitCleanupFailed {
        operation: String,
        message: String,
    },
    GitSubmodulesUnsupported {
        path: PathBuf,
    },
    GitObjectInvalid {
        oid: String,
        message: String,
    },
    GitTreeInvalid {
        path: Vec<u8>,
        message: String,
    },
    GitCacheInvalid {
        path: PathBuf,
        message: String,
    },
    LocalSnapshotInvalid {
        path: PathBuf,
        message: String,
    },
    LocalSnapshotLockTimedOut {
        path: PathBuf,
        timeout_millis: u64,
    },
    LocalSourceChanged {
        path: PathBuf,
    },
    SourceSnapshotContentMismatch {
        path: PathBuf,
        expected: SourceContentDigest,
        actual: SourceContentDigest,
    },
    LocalSnapshotCacheOverlapsSource {
        canonical_live_root: PathBuf,
        canonical_cache_dir: PathBuf,
    },
}

impl fmt::Display for SourceResolveError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(output, "{}: {message}", path.display()),
            Self::NotDirectory { path } => {
                write!(
                    output,
                    "source root `{}` is not a directory",
                    path.display()
                )
            }
            Self::TooManyFiles { limit } => {
                write!(
                    output,
                    "source root exceeds identity entry limit of {limit}"
                )
            }
            Self::TooManyBytes { limit } => {
                write!(output, "source root exceeds byte limit of {limit}")
            }
            Self::TooDeep { path, limit } => {
                write!(
                    output,
                    "source path `{}` exceeds traversal depth limit of {limit}",
                    path.display()
                )
            }
            Self::SymlinkEscapesRoot { link, target } => write!(
                output,
                "source symlink `{}` resolves outside package root to `{}`",
                link.display(),
                target.display()
            ),
            Self::SymlinkTargetsExcludedMetadata { link, target } => write!(
                output,
                "source symlink `{}` targets excluded repository metadata at `{}`",
                link.display(),
                target.display()
            ),
            Self::SymlinkTargetsExcludedBuildOutput { link, target } => write!(
                output,
                "source symlink `{}` targets excluded root build output at `{}`",
                link.display(),
                target.display()
            ),
            Self::UnsupportedFileType { path } => write!(
                output,
                "source path `{}` has an unsupported filesystem entry type",
                path.display()
            ),
            Self::Git {
                operation,
                status,
                stderr,
            } => write!(
                output,
                "git {operation} failed with status {:?}: {}",
                status,
                stderr.trim()
            ),
            Self::GitOutputOverflow {
                operation,
                stream,
                limit,
            } => write!(
                output,
                "git {operation} exceeded its {stream} capture limit of {limit} bytes"
            ),
            Self::GitTimedOut {
                operation,
                timeout_millis,
            } => write!(
                output,
                "git {operation} exceeded its deadline of {timeout_millis} milliseconds"
            ),
            Self::GitExecutableUnavailable => output.write_str(
                "no supported absolute Git executable is available; the resolver will not search PATH",
            ),
            Self::GitExecutableInvalid { path, message } => write!(
                output,
                "Git resolver executable `{}` is invalid: {message}",
                path.display()
            ),
            Self::GitExecutableChanged { path } => write!(
                output,
                "Git resolver executable `{}` changed during source resolution",
                path.display()
            ),
            Self::GitResolutionCommandLimit { limit } => write!(
                output,
                "Git source resolution exceeded its {limit}-command launch ceiling"
            ),
            Self::GitResolutionTimedOut { timeout_millis } => write!(
                output,
                "Git source resolution exceeded its {timeout_millis}-millisecond whole-operation deadline"
            ),
            Self::GitCleanupFailed { operation, message } => write!(
                output,
                "git {operation} process cleanup failed: {message}"
            ),
            Self::GitSubmodulesUnsupported { path } => write!(
                output,
                "git source `{}` declares submodules; submodules must become explicit package edges before they are supported",
                path.display()
            ),
            Self::GitObjectInvalid { oid, message } => {
                write!(output, "Git object `{oid}` failed authentication: {message}")
            }
            Self::GitTreeInvalid { path, message } => write!(
                output,
                "git tree path `{}` is invalid: {message}",
                String::from_utf8_lossy(path)
            ),
            Self::GitCacheInvalid { path, message } => write!(
                output,
                "git cache entry `{}` is invalid: {message}",
                path.display()
            ),
            Self::LocalSnapshotInvalid { path, message } => write!(
                output,
                "local snapshot cache entry `{}` is invalid: {message}",
                path.display()
            ),
            Self::LocalSnapshotLockTimedOut {
                path,
                timeout_millis,
            } => write!(
                output,
                "local snapshot cache lock `{}` exceeded its {timeout_millis}-millisecond deadline",
                path.display()
            ),
            Self::LocalSourceChanged { path } => write!(
                output,
                "local source `{}` changed while its immutable snapshot was being captured",
                path.display()
            ),
            Self::SourceSnapshotContentMismatch {
                path,
                expected,
                actual,
            } => write!(
                output,
                "source snapshot `{}` no longer matches immutable content {} (found {})",
                path.display(),
                expected.to_hex(),
                actual.to_hex()
            ),
            Self::LocalSnapshotCacheOverlapsSource {
                canonical_live_root,
                canonical_cache_dir,
            } => write!(
                output,
                "local snapshot cache `{}` overlaps live source `{}`",
                canonical_cache_dir.display(),
                canonical_live_root.display()
            ),
        }
    }
}

impl std::error::Error for SourceResolveError {}

pub fn resolve_local_source(
    root: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let limits = limits.compiler_bounded();
    Ok(capture_local_source(root.as_ref(), limits, SourceTreePolicy::LocalPackage)?.normalized)
}

/// Re-hash a published package snapshot under its original resolver limits.
///
/// This is a package-compilation custody check, not a defense against a
/// same-user process that can race both the verification and compiler reads.
pub(crate) fn verify_package_source_snapshot(
    root: &Path,
    expected: &SourceContentDigest,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    capture_verified_package_source_snapshot(root, expected, limits).map(|_| ())
}

/// Capture the exact bytes already covered by package-source custody.
///
/// Review-only consumers use this after transport resolution so they never
/// reopen a live checkout or infer a tree from package-authored ignore rules.
/// The returned paths are the same raw, root-relative bytes used by source
/// identity; every file and symlink payload has already participated in the
/// expected content commitment.
pub(crate) fn capture_verified_package_source_snapshot(
    root: &Path,
    expected: &SourceContentDigest,
    limits: LocalSourceLimits,
) -> Result<Vec<VerifiedPackageSourceEntry>, SourceResolveError> {
    let directory = open_absolute_directory_nofollow(root)
        .map_err(|error| local_snapshot_invalid(root, error.to_string()))?;
    verify_open_snapshot_tree_modes(CacheCustodyKind::LocalSnapshot, &directory, root)?;
    let captured = capture_local_source_from_open_root(
        root.to_path_buf(),
        directory,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?;
    let actual = SourceContentDigest::derive(captured.normalized.content_identity.as_bytes());
    if &actual != expected {
        return Err(SourceResolveError::SourceSnapshotContentMismatch {
            path: root.to_path_buf(),
            expected: expected.clone(),
            actual,
        });
    }
    Ok(captured
        .entries
        .into_iter()
        .map(|entry| VerifiedPackageSourceEntry {
            relative_path: entry.relative_bytes,
            kind: match entry.kind {
                CapturedLocalEntryKind::Directory => VerifiedPackageSourceEntryKind::Directory,
                CapturedLocalEntryKind::File { bytes, executable } => {
                    VerifiedPackageSourceEntryKind::File { bytes, executable }
                }
                CapturedLocalEntryKind::Symlink { target_bytes } => {
                    VerifiedPackageSourceEntryKind::Symlink { target_bytes }
                }
            },
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedPackageSourceEntry {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) kind: VerifiedPackageSourceEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VerifiedPackageSourceEntryKind {
    Directory,
    File { bytes: Vec<u8>, executable: bool },
    Symlink { target_bytes: Vec<u8> },
}

pub fn resolve_local_source_snapshot(
    root: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    let limits = limits.compiler_bounded();
    let requested_root = root.as_ref().to_path_buf();
    let captured = capture_local_source(&requested_root, limits, SourceTreePolicy::LocalPackage)?;
    publish_local_snapshot(requested_root, captured, cache_dir.as_ref(), limits)
}

pub fn resolve_git_source(
    request: &GitSourceRequest,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let limits = limits.compiler_bounded();
    let execution_transport = request.execution_transport();
    let executor = GitExecutor::system(execution_transport)?;
    let result = (|| {
        let requested_rev = request.requested_revision();
        let locator_identity = request.locator_identity();
        let cache_dir = cache_dir.as_ref();
        std::fs::create_dir_all(cache_dir).map_err(|error| io_error(cache_dir, error))?;
        let cache_dir = cache_dir
            .canonicalize()
            .map_err(|error| io_error(cache_dir, error))?;
        verify_git_cache_root_custody(&cache_dir)?;
        let cache_identity =
            git_cache_identity(locator_identity, requested_rev, execution_transport);
        let entry_root = cache_dir.join(format!("git-{cache_identity}"));
        let lock_path = cache_dir.join(format!("git-{cache_identity}.lock"));
        let entry_lock = CacheEntryLock::acquire_with_git_budget(&lock_path, &executor)?;
        let entry_name =
            direct_cache_child_name(CacheCustodyKind::Git, &cache_dir, &entry_root)?.to_os_string();
        let cache_entry_existed = retained_cache_directory_exists(
            CacheCustodyKind::Git,
            entry_lock.parent(),
            &entry_name,
            &entry_root,
        )?;
        entry_lock.verify_path_identity()?;

        if cache_entry_existed {
            let verification_result = VerifiedGitRepository::open(
                entry_lock.parent(),
                &entry_name,
                &entry_root,
                locator_identity,
                requested_rev,
                execution_transport,
                limits,
            );
            let namespace_result = entry_lock.verify_path_identity();
            if verification_result.is_err() || namespace_result.is_err() {
                let invalidation_result = invalidate_git_cache_entry_from_open_parent(
                    &cache_dir,
                    entry_lock.parent(),
                    &entry_name,
                    &entry_root,
                );
                let failure = reconcile_git_cache_operation_result(
                    verification_result,
                    namespace_result,
                    Some(invalidation_result),
                );
                return Err(failure
                    .err()
                    .expect("failed cache verification must retain one failure"));
            }
        } else {
            let creation_result = create_git_cache_entry(
                &executor,
                &cache_dir,
                entry_lock.parent(),
                &entry_root,
                &entry_name,
                &cache_identity,
                locator_identity,
                request.fetch_locator(),
                requested_rev,
                execution_transport,
                limits,
            );
            reconcile_git_cache_operation_result(
                creation_result,
                entry_lock.verify_path_identity(),
                None,
            )?;
        }

        entry_lock.verify_path_identity()?;
        let result = resolve_verified_git_cache_entry(
            &executor,
            entry_lock.parent(),
            &entry_name,
            &entry_root,
            request.requested_locator(),
            locator_identity,
            request.fetch_locator(),
            requested_rev,
            execution_transport,
            limits,
            !cache_entry_existed || !is_object_id(requested_rev),
        );
        let namespace_result = entry_lock.verify_path_identity();
        match result {
            Ok(resolved) => {
                namespace_result?;
                verify_git_cache_root_custody(&cache_dir)?;
                verify_git_cache_custody(&entry_root, limits)?;
                Ok(resolved)
            }
            Err(error) => {
                let invalidation_result = invalidate_git_cache_entry_from_open_parent(
                    &cache_dir,
                    entry_lock.parent(),
                    &entry_name,
                    &entry_root,
                );
                reconcile_git_cache_operation_result(
                    Err(error),
                    namespace_result,
                    Some(invalidation_result),
                )
            }
        }
    })();
    let executable_result = executor.verify_content();
    reconcile_git_command_result(result, executable_result, Ok(()))
}

fn resolve_verified_git_cache_entry(
    executor: &GitExecutor,
    cache_directory: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
    requested_locator: &str,
    locator_identity: &str,
    fetch_locator: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
    limits: LocalSourceLimits,
    fetch_remote: bool,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let repository = VerifiedGitRepository::open(
        cache_directory,
        entry_name,
        entry_root,
        locator_identity,
        requested_rev,
        execution_transport,
        limits,
    )?;

    if fetch_remote {
        let canonical_config = repository.read_canonical_config()?;
        let arguments = bounded_git_fetch_arguments(fetch_locator, requested_rev, limits);
        repository.run_git(executor, arguments.iter())?;
        repository.restore_canonical_config(&canonical_config)?;
    }
    repository.verify_current(limits)?;

    let selected_revision = if fetch_remote {
        "FETCH_HEAD"
    } else {
        requested_rev
    };
    let commit = repository.run_git_stdout(
        executor,
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&format!("{selected_revision}^{{commit}}")),
        ],
    )?;
    let commit = commit.trim().to_owned();
    verify_exact_git_revision(requested_rev, &commit)?;
    let tree = repository.run_git_stdout(
        executor,
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&format!("{commit}^{{tree}}")),
        ],
    )?;
    let tree = tree.trim().to_owned();
    repository.verify_current(limits)?;
    authenticate_git_commit(executor, &repository, &commit, &tree)?;
    let entries = inspect_git_tree(executor, &repository, &tree, limits)?;
    repository.verify_current(limits)?;
    let (snapshot_root, local) =
        resolve_git_snapshot(executor, entry_root, &tree, entries, limits)?;
    repository.verify_current(limits)?;
    executor.verify()?;
    Ok(ResolvedGitSource {
        requested_locator: requested_locator.to_owned(),
        locator_identity: locator_identity.to_owned(),
        transport_profile: execution_transport.profile(),
        requested_rev: requested_rev.to_owned(),
        commit,
        tree,
        snapshot_root,
        local,
        git_executable: executor.identity.clone(),
        transport_executable: executor
            .transport_executable
            .as_ref()
            .map(|executable| executable.identity.clone()),
    })
}

fn bounded_git_fetch_arguments(
    fetch_locator: &str,
    requested_rev: &str,
    limits: LocalSourceLimits,
) -> Vec<OsString> {
    let first_inadmissible_blob_size = limits
        .max_bytes
        .checked_add(1)
        .expect("compiler-owned Git source byte ceiling leaves room for one sentinel byte");
    vec![
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--depth=1"),
        OsString::from("--no-tags"),
        OsString::from("--no-recurse-submodules"),
        OsString::from(format!(
            "--filter=blob:limit={first_inadmissible_blob_size}"
        )),
        OsString::from("--"),
        OsString::from(fetch_locator),
        OsString::from(requested_rev),
    ]
}

fn replace_canonical_git_control_file(
    entry: &CapabilityDirectory,
    repository_name: &OsStr,
    repository_path: &Path,
    canonical_config: &[u8],
) -> Result<(), SourceResolveError> {
    let classified = entry
        .symlink_metadata(repository_name)
        .map_err(|error| io_error(repository_path, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(
            repository_path,
            "Git repository is not a concrete directory",
        ));
    }
    let directory = entry
        .open_dir_nofollow(repository_name)
        .map_err(|error| cache_invalid(repository_path, error.to_string()))?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(repository_path, error))?;
    if !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            repository_path,
            "Git repository changed while opening it for configuration replacement",
        ));
    }
    replace_canonical_git_control_file_from_open_repository(
        &directory,
        repository_path,
        canonical_config,
    )
}

fn replace_canonical_git_control_file_from_open_repository(
    repository: &CapabilityDirectory,
    repository_path: &Path,
    canonical_config: &[u8],
) -> Result<(), SourceResolveError> {
    let config_path = repository_path.join("config");
    let directory = repository
        .try_clone()
        .map_err(|error| io_error(repository_path, error))?;
    let root = RecordFileRoot::from_directory(directory, repository_path.to_path_buf()).map_err(
        |error| {
            cache_invalid(
                repository_path,
                format!("failed to bind Git configuration directory custody: {error:?}"),
            )
        },
    )?;
    root.replace_existing(
        Path::new("config"),
        canonical_config,
        RecordFileLimits {
            maximum_bytes: GIT_CONFIG_SHA256.len(),
        },
    )
    .map_err(|error| {
        cache_invalid(
            &config_path,
            format!("failed to atomically restore canonical Git configuration: {error:?}"),
        )
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitTreeEntry {
    relative_bytes: Vec<u8>,
    relative_path: PathBuf,
    oid: String,
    size: u64,
    kind: GitTreeEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GitTreeEntryKind {
    Tree,
    File {
        executable: bool,
        bytes: GitBlobBytes,
    },
    Symlink {
        target_bytes: GitBlobBytes,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitBlobBytes {
    batch: Arc<Vec<u8>>,
    start: usize,
    end: usize,
}

impl GitBlobBytes {
    fn empty() -> Self {
        Self {
            batch: Arc::new(Vec::new()),
            start: 0,
            end: 0,
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.batch[self.start..self.end]
    }
}

#[derive(Debug)]
enum AuthenticatedGitTreeNode {
    Blob {
        mode: &'static [u8],
        oid: String,
    },
    Tree {
        expected_oid: String,
        directory: AuthenticatedGitDirectory,
    },
}

#[derive(Debug, Default)]
struct AuthenticatedGitDirectory {
    entries: BTreeMap<Vec<u8>, AuthenticatedGitTreeNode>,
}

fn inspect_git_tree(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    limits: LocalSourceLimits,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    if !is_object_id(tree) {
        return Err(cache_invalid(
            repository.path(),
            "Git returned an invalid tree object ID",
        ));
    }
    let listing = repository.run_git_bytes_stdout(
        executor,
        [
            OsStr::new("ls-tree"),
            OsStr::new("--full-tree"),
            OsStr::new("-r"),
            OsStr::new("-t"),
            OsStr::new("-l"),
            OsStr::new("-z"),
            OsStr::new(tree),
        ],
    )?;
    let mut entries = parse_git_tree_entries(&listing, repository.path(), limits)?;
    read_git_blobs_batch(executor, repository, &mut entries, limits)?;
    Ok(entries)
}

fn verify_exact_git_revision(
    requested_rev: &str,
    selected_commit: &str,
) -> Result<(), SourceResolveError> {
    if is_object_id(requested_rev) && !requested_rev.eq_ignore_ascii_case(selected_commit) {
        return Err(git_object_invalid(
            selected_commit,
            "selected commit does not match the exact requested revision",
        ));
    }
    Ok(())
}

fn authenticate_git_commit(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    commit: &str,
    tree: &str,
) -> Result<(), SourceResolveError> {
    let payload = repository.run_git_bytes_stdout(
        executor,
        [
            OsStr::new("cat-file"),
            OsStr::new("commit"),
            OsStr::new(commit),
        ],
    )?;
    authenticate_git_commit_payload(commit, tree, &payload)
}

fn authenticate_git_commit_payload(
    commit: &str,
    reported_tree: &str,
    payload: &[u8],
) -> Result<(), SourceResolveError> {
    let algorithm = git_object_algorithm(commit)?;
    if git_object_algorithm(reported_tree)? != algorithm {
        return Err(git_object_invalid(
            commit,
            "commit and root tree use different object formats",
        ));
    }
    verify_git_object_identity(commit, b"commit", payload, algorithm)?;

    let first_line = payload
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or(payload);
    let Some(commit_tree) = first_line.strip_prefix(b"tree ") else {
        return Err(git_object_invalid(
            commit,
            "commit payload does not begin with one root tree edge",
        ));
    };
    let commit_tree = std::str::from_utf8(commit_tree)
        .map_err(|_| git_object_invalid(commit, "commit tree ID is not ASCII"))?;
    if git_object_algorithm(commit_tree)? != algorithm || commit_tree != reported_tree {
        return Err(git_object_invalid(
            commit,
            "commit root tree edge does not match the selected tree",
        ));
    }
    Ok(())
}

fn authenticate_git_tree(
    expected_tree: &str,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    let algorithm = git_object_algorithm(expected_tree)?;
    let mut root = AuthenticatedGitDirectory::default();
    for entry in entries {
        match &entry.kind {
            GitTreeEntryKind::Tree => {}
            GitTreeEntryKind::File { bytes, .. } => {
                verify_git_object_identity(&entry.oid, b"blob", bytes.as_slice(), algorithm)?;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                verify_git_object_identity(
                    &entry.oid,
                    b"blob",
                    target_bytes.as_slice(),
                    algorithm,
                )?;
            }
        }
        insert_authenticated_git_entry(&mut root, entry)?;
    }
    let actual_tree = authenticate_git_directory(&root, algorithm)?;
    if actual_tree != expected_tree {
        return Err(git_object_invalid(
            expected_tree,
            "authenticated tree graph does not reconstruct the selected root tree",
        ));
    }
    Ok(())
}

fn insert_authenticated_git_entry(
    directory: &mut AuthenticatedGitDirectory,
    entry: &GitTreeEntry,
) -> Result<(), SourceResolveError> {
    let components = entry
        .relative_bytes
        .split(|byte| *byte == b'/')
        .collect::<Vec<_>>();
    insert_authenticated_git_components(directory, &components, entry)
}

fn insert_authenticated_git_components(
    directory: &mut AuthenticatedGitDirectory,
    components: &[&[u8]],
    entry: &GitTreeEntry,
) -> Result<(), SourceResolveError> {
    let Some((name, rest)) = components.split_first() else {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "authenticated tree entry has no path component",
        ));
    };
    if rest.is_empty() {
        let node = match entry.kind {
            GitTreeEntryKind::Tree => AuthenticatedGitTreeNode::Tree {
                expected_oid: entry.oid.clone(),
                directory: AuthenticatedGitDirectory::default(),
            },
            GitTreeEntryKind::File {
                executable: false, ..
            } => AuthenticatedGitTreeNode::Blob {
                mode: b"100644".as_slice(),
                oid: entry.oid.clone(),
            },
            GitTreeEntryKind::File {
                executable: true, ..
            } => AuthenticatedGitTreeNode::Blob {
                mode: b"100755".as_slice(),
                oid: entry.oid.clone(),
            },
            GitTreeEntryKind::Symlink { .. } => AuthenticatedGitTreeNode::Blob {
                mode: b"120000".as_slice(),
                oid: entry.oid.clone(),
            },
        };
        if directory.entries.insert(name.to_vec(), node).is_some() {
            return Err(git_tree_invalid(
                &entry.relative_bytes,
                "authenticated tree contains a duplicate path",
            ));
        }
        return Ok(());
    }

    let Some(node) = directory.entries.get_mut(*name) else {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "authenticated tree path has no declared parent-tree edge",
        ));
    };
    let AuthenticatedGitTreeNode::Tree {
        directory: child, ..
    } = node
    else {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "authenticated tree path traverses a blob",
        ));
    };
    insert_authenticated_git_components(child, rest, entry)
}

fn authenticate_git_directory(
    directory: &AuthenticatedGitDirectory,
    algorithm: GitObjectIdAlgorithm,
) -> Result<String, SourceResolveError> {
    let mut ordered = directory.entries.iter().collect::<Vec<_>>();
    ordered.sort_by(git_tree_entry_order);
    let mut payload = Vec::new();
    for (name, node) in ordered {
        let (mode, oid) = match node {
            AuthenticatedGitTreeNode::Blob { mode, oid } => (*mode, oid.clone()),
            AuthenticatedGitTreeNode::Tree {
                expected_oid,
                directory,
            } => {
                if git_object_algorithm(expected_oid)? != algorithm {
                    return Err(git_object_invalid(
                        expected_oid,
                        "child tree uses a different hash algorithm than its graph",
                    ));
                }
                let actual_oid = authenticate_git_directory(directory, algorithm)?;
                if actual_oid != *expected_oid {
                    return Err(git_object_invalid(
                        expected_oid,
                        "child tree bytes do not match the declared tree edge",
                    ));
                }
                (b"40000".as_slice(), actual_oid)
            }
        };
        payload.extend_from_slice(mode);
        payload.push(b' ');
        payload.extend_from_slice(name);
        payload.push(0);
        payload.extend_from_slice(&decode_git_object_id(&oid, algorithm)?);
    }
    git_object_identity(b"tree", &payload, algorithm)
}

fn git_tree_entry_order(
    left: &(&Vec<u8>, &AuthenticatedGitTreeNode),
    right: &(&Vec<u8>, &AuthenticatedGitTreeNode),
) -> std::cmp::Ordering {
    let common = left.0.len().min(right.0.len());
    let prefix = left.0[..common].cmp(&right.0[..common]);
    if prefix != std::cmp::Ordering::Equal {
        return prefix;
    }
    let left_next = left.0.get(common).copied().unwrap_or({
        if matches!(left.1, AuthenticatedGitTreeNode::Tree { .. }) {
            b'/'
        } else {
            0
        }
    });
    let right_next = right.0.get(common).copied().unwrap_or({
        if matches!(right.1, AuthenticatedGitTreeNode::Tree { .. }) {
            b'/'
        } else {
            0
        }
    });
    left_next.cmp(&right_next)
}

fn verify_git_object_identity(
    expected: &str,
    kind: &[u8],
    payload: &[u8],
    algorithm: GitObjectIdAlgorithm,
) -> Result<(), SourceResolveError> {
    if git_object_algorithm(expected)? != algorithm {
        return Err(git_object_invalid(
            expected,
            "object ID uses a different hash algorithm than its graph",
        ));
    }
    if git_object_identity(kind, payload, algorithm)? != expected {
        return Err(git_object_invalid(
            expected,
            "object bytes do not match the declared object ID",
        ));
    }
    Ok(())
}

fn git_object_identity(
    kind: &[u8],
    payload: &[u8],
    algorithm: GitObjectIdAlgorithm,
) -> Result<String, SourceResolveError> {
    let length = payload.len().to_string();
    match algorithm {
        GitObjectIdAlgorithm::Sha1 => {
            let mut hasher = CheckedSha1::new();
            hasher.update(kind);
            hasher.update(b" ");
            hasher.update(length.as_bytes());
            hasher.update([0]);
            hasher.update(payload);
            finalize_checked_sha1(hasher)
        }
        GitObjectIdAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(kind);
            hasher.update(b" ");
            hasher.update(length.as_bytes());
            hasher.update([0]);
            hasher.update(payload);
            Ok(format_hex(&hasher.finalize()))
        }
    }
}

fn finalize_checked_sha1(hasher: CheckedSha1) -> Result<String, SourceResolveError> {
    let result = hasher.try_finalize();
    if result.has_collision() {
        return Err(git_object_invalid(
            "sha1-collision",
            "Git object bytes match a known SHA-1 collision attack",
        ));
    }
    Ok(format_hex(result.hash()))
}

fn git_object_algorithm(oid: &str) -> Result<GitObjectIdAlgorithm, SourceResolveError> {
    if !is_object_id(oid) {
        return Err(git_object_invalid(oid, "object ID has an invalid spelling"));
    }
    Ok(if oid.len() == 40 {
        GitObjectIdAlgorithm::Sha1
    } else {
        GitObjectIdAlgorithm::Sha256
    })
}

fn decode_git_object_id(
    oid: &str,
    algorithm: GitObjectIdAlgorithm,
) -> Result<Vec<u8>, SourceResolveError> {
    if git_object_algorithm(oid)? != algorithm {
        return Err(git_object_invalid(
            oid,
            "child object uses a different hash algorithm than its tree",
        ));
    }
    let mut bytes = Vec::with_capacity(oid.len() / 2);
    for pair in oid.as_bytes().chunks_exact(2) {
        let high = hex_digit(pair[0])
            .ok_or_else(|| git_object_invalid(oid, "object ID contains a non-hexadecimal digit"))?;
        let low = hex_digit(pair[1])
            .ok_or_else(|| git_object_invalid(oid, "object ID contains a non-hexadecimal digit"))?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn git_object_invalid(oid: impl Into<String>, message: impl Into<String>) -> SourceResolveError {
    SourceResolveError::GitObjectInvalid {
        oid: oid.into(),
        message: message.into(),
    }
}

fn parse_git_tree_entries(
    listing: &[u8],
    repository: &Path,
    limits: LocalSourceLimits,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    let mut entries = Vec::new();
    let mut paths = BTreeMap::new();
    let mut blob_bytes = 0_u64;

    for record in listing.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
            return Err(git_tree_invalid(Vec::new(), "malformed ls-tree record"));
        };
        let header = &record[..tab];
        let path = &record[tab + 1..];
        let fields = header
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(git_tree_invalid(path, "malformed ls-tree header"));
        }
        let mode = fields[0];
        let object_type = fields[1];
        let oid = std::str::from_utf8(fields[2])
            .map_err(|_| git_tree_invalid(path, "object ID is not ASCII"))?;
        if !is_object_id(oid) {
            return Err(git_tree_invalid(path, "object ID has an invalid spelling"));
        }
        if mode == b"160000" || object_type == b"commit" {
            return Err(SourceResolveError::GitSubmodulesUnsupported {
                path: git_path_from_bytes(path).unwrap_or_else(|_| repository.to_path_buf()),
            });
        }
        let relative_path = validate_git_path(path, limits)?;
        if path
            .split(|byte| *byte == b'/')
            .any(|component| component.eq_ignore_ascii_case(b".gitmodules"))
        {
            return Err(SourceResolveError::GitSubmodulesUnsupported {
                path: relative_path,
            });
        }
        let (size, kind) = match (mode, object_type, fields[3]) {
            (b"040000", b"tree", b"-") => (0, GitTreeEntryKind::Tree),
            (b"100644", b"blob", size) => (
                parse_git_blob_size(path, size)?,
                GitTreeEntryKind::File {
                    executable: false,
                    bytes: GitBlobBytes::empty(),
                },
            ),
            (b"100755", b"blob", size) => (
                parse_git_blob_size(path, size)?,
                GitTreeEntryKind::File {
                    executable: true,
                    bytes: GitBlobBytes::empty(),
                },
            ),
            (b"120000", b"blob", size) => (
                parse_git_blob_size(path, size)?,
                GitTreeEntryKind::Symlink {
                    target_bytes: GitBlobBytes::empty(),
                },
            ),
            _ => return Err(git_tree_invalid(path, "unsupported Git tree entry")),
        };
        if paths
            .insert(path.to_vec(), matches!(&kind, GitTreeEntryKind::Tree))
            .is_some()
        {
            return Err(git_tree_invalid(path, "duplicate path"));
        }
        let identity_entry_count =
            entries
                .len()
                .checked_add(1)
                .ok_or(SourceResolveError::TooManyFiles {
                    limit: limits.max_files,
                })?;
        if identity_entry_count > limits.max_files {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }
        if !matches!(&kind, GitTreeEntryKind::Tree) {
            blob_bytes = blob_bytes
                .checked_add(size)
                .ok_or(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                })?;
            if blob_bytes > limits.max_bytes {
                return Err(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                });
            }
        }
        entries.push(GitTreeEntry {
            relative_bytes: path.to_vec(),
            relative_path,
            oid: oid.to_owned(),
            size,
            kind,
        });
    }

    entries.sort_by(|left, right| left.relative_bytes.cmp(&right.relative_bytes));
    for entry in &entries {
        for separator in entry
            .relative_bytes
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'/').then_some(index))
        {
            let parent = &entry.relative_bytes[..separator];
            match paths.get(parent) {
                Some(true) => {}
                Some(false) => {
                    return Err(git_tree_invalid(
                        &entry.relative_bytes,
                        "Git path traverses a blob",
                    ));
                }
                None => {
                    return Err(git_tree_invalid(
                        &entry.relative_bytes,
                        "Git listing omitted a parent-tree edge",
                    ));
                }
            }
        }
    }
    Ok(entries)
}

fn parse_git_blob_size(path: &[u8], size: &[u8]) -> Result<u64, SourceResolveError> {
    std::str::from_utf8(size)
        .ok()
        .and_then(|size| size.parse::<u64>().ok())
        .ok_or_else(|| git_tree_invalid(path, "blob size is missing or invalid"))
}

fn git_directory_paths(entries: &[GitTreeEntry]) -> BTreeSet<Vec<u8>> {
    entries
        .iter()
        .filter(|entry| matches!(&entry.kind, GitTreeEntryKind::Tree))
        .map(|entry| entry.relative_bytes.clone())
        .collect()
}

fn validate_git_path(
    path: &[u8],
    limits: LocalSourceLimits,
) -> Result<PathBuf, SourceResolveError> {
    if path.is_empty() || path.starts_with(b"/") || path.ends_with(b"/") {
        return Err(git_tree_invalid(
            path,
            "path must be a non-empty relative path",
        ));
    }
    if path.contains(&b'\\') {
        return Err(git_tree_invalid(
            path,
            "backslashes are forbidden in portable package paths",
        ));
    }
    let components = path.split(|byte| *byte == b'/').collect::<Vec<_>>();
    for component in &components {
        if component.is_empty() || *component == b"." || *component == b".." {
            return Err(git_tree_invalid(
                path,
                "path contains a traversal component",
            ));
        }
        if component.eq_ignore_ascii_case(b".git") {
            return Err(git_tree_invalid(path, "path enters excluded Git metadata"));
        }
        validate_portable_git_component(path, component)?;
    }
    let depth = components.len().saturating_sub(1);
    if depth > limits.max_depth {
        return Err(SourceResolveError::TooDeep {
            path: git_path_from_bytes(path)?,
            limit: limits.max_depth,
        });
    }
    git_path_from_bytes(path)
}

fn validate_portable_git_component(
    path: &[u8],
    component: &[u8],
) -> Result<(), SourceResolveError> {
    if component
        .iter()
        .any(|byte| *byte < 32 || matches!(*byte, b'<' | b'>' | b':' | b'"' | b'|' | b'?' | b'*'))
    {
        return Err(git_tree_invalid(
            path,
            "path contains a character forbidden by the portable Windows policy",
        ));
    }
    if component
        .last()
        .is_some_and(|byte| matches!(byte, b'.' | b' '))
    {
        return Err(git_tree_invalid(
            path,
            "path component has a Windows-ambiguous trailing dot or space",
        ));
    }
    let stem = component
        .split(|byte| *byte == b'.')
        .next()
        .unwrap_or(component);
    let reserved_device = [b"CON".as_slice(), b"PRN", b"AUX", b"NUL"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        || (stem.len() == 4
            && (stem[..3].eq_ignore_ascii_case(b"COM") || stem[..3].eq_ignore_ascii_case(b"LPT"))
            && matches!(stem[3], b'1'..=b'9'))
        || stem.eq_ignore_ascii_case(b"CONIN$")
        || stem.eq_ignore_ascii_case(b"CONOUT$");
    if reserved_device {
        return Err(git_tree_invalid(
            path,
            "path component uses a reserved Windows device name",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn git_path_from_bytes(path: &[u8]) -> Result<PathBuf, SourceResolveError> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    Ok(PathBuf::from(OsString::from_vec(path.to_vec())))
}

#[cfg(not(unix))]
fn git_path_from_bytes(path: &[u8]) -> Result<PathBuf, SourceResolveError> {
    let text = std::str::from_utf8(path)
        .map_err(|_| git_tree_invalid(path, "path cannot be represented on this host"))?;
    Ok(PathBuf::from(text))
}

fn validate_git_symlink_target(link: &[u8], target: &[u8]) -> Result<(), SourceResolveError> {
    if target.is_empty() || target.starts_with(b"/") || target.contains(&0) {
        return Err(git_tree_invalid(
            link,
            "symlink target must be a non-empty relative path",
        ));
    }
    if target.contains(&b'\\') {
        return Err(git_tree_invalid(
            link,
            "symlink target contains a non-portable path separator",
        ));
    }
    let mut depth = link.split(|byte| *byte == b'/').count().saturating_sub(1);
    for component in target.split(|byte| *byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| git_tree_invalid(link, "symlink target escapes the snapshot"))?;
            }
            component if component.eq_ignore_ascii_case(b".git") => {
                return Err(git_tree_invalid(
                    link,
                    "symlink target enters excluded Git metadata",
                ));
            }
            component => {
                validate_portable_git_component(link, component)?;
                depth += 1;
            }
        }
    }
    Ok(())
}

fn resolve_git_snapshot(
    executor: &GitExecutor,
    entry_root: &Path,
    tree: &str,
    mut entries: Vec<GitTreeEntry>,
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    authenticate_git_tree(tree, &entries)?;
    verify_git_destination_containment(Path::new("omega-verified-snapshot-root"), &entries)?;
    let expected = authenticated_git_snapshot_identity(tree, &entries)?;
    let snapshots = entry_root.join(GIT_CACHE_SNAPSHOTS);
    std::fs::create_dir_all(&snapshots).map_err(|error| io_error(&snapshots, error))?;
    require_real_directory(&snapshots, "snapshot cache is not a real directory")?;
    let publication = snapshots.join(format!("tree-{tree}"));
    if publication.exists() {
        release_git_blob_payloads(&mut entries);
        return verify_git_snapshot(&publication, &expected, &entries, limits);
    }

    let mut pending = PendingMaterializedSnapshot::create(
        CacheCustodyKind::Git,
        &snapshots,
        &format!(".tree-{tree}.stage"),
    )?;
    let source = pending.root.join(GIT_SNAPSHOT_SOURCE);
    pending
        .directory()?
        .create_dir(GIT_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;
    let source_directory = pending
        .directory()?
        .open_dir_nofollow(GIT_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;
    for entry in &entries {
        executor.verify_budget()?;
        checked_git_destination(&source, entry)?;
        match &entry.kind {
            GitTreeEntryKind::Tree => {
                open_or_create_snapshot_directory(
                    CacheCustodyKind::Git,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                )?;
            }
            GitTreeEntryKind::File { executable, bytes } => {
                write_snapshot_file_from_open_root(
                    CacheCustodyKind::Git,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    bytes.as_slice(),
                    *executable,
                )?;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                create_snapshot_symlink_from_open_root(
                    CacheCustodyKind::Git,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    target_bytes.as_slice(),
                )?;
            }
        }
    }

    // The staged source is re-read to bind publication identity. Release the
    // shared batch payload first so that this verification does not retain a
    // second package-sized in-memory copy.
    release_git_blob_payloads(&mut entries);
    let staged = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if staged.file_count != expected.file_count
        || staged.byte_count != expected.byte_count
        || staged.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            &source,
            "materialized snapshot did not preserve the validated Git tree exactly",
        ));
    }
    write_snapshot_file_from_open_root(
        CacheCustodyKind::Git,
        pending.directory()?,
        Path::new(GIT_SNAPSHOT_METADATA),
        &pending.root,
        &git_snapshot_metadata(tree, &staged),
        false,
    )?;
    make_open_snapshot_read_only(CacheCustodyKind::Git, pending.directory()?, &pending.root)?;
    let finalized = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if finalized.file_count != expected.file_count
        || finalized.byte_count != expected.byte_count
        || finalized.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            &source,
            "finalized snapshot did not preserve the authenticated Git tree exactly",
        ));
    }
    pending.publish(&snapshots, &publication)?;

    // The returned identity is always calculated from the atomically published tree, never from
    // the staging directory or Git's mutable object-cache state.
    verify_git_snapshot(&publication, &expected, &entries, limits)
}

fn authenticated_git_snapshot_identity(
    tree: &str,
    entries: &[GitTreeEntry],
) -> Result<GitSnapshotMetadata, SourceResolveError> {
    let mut identity = SourceIdentityHasher::new(entries.len());
    let mut file_count = 0_usize;
    for entry in entries {
        match &entry.kind {
            GitTreeEntryKind::Tree => {
                identity.add_directory(&entry.relative_bytes, CANONICAL_DIRECTORY_MODE);
            }
            GitTreeEntryKind::File { executable, bytes } => {
                identity.add_file(&entry.relative_bytes, *executable, bytes.as_slice())?;
                file_count += 1;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                identity.add_symlink(&entry.relative_bytes, target_bytes.as_slice());
                file_count += 1;
            }
        }
    }
    let (byte_count, content_identity) = identity.finish();
    Ok(GitSnapshotMetadata {
        tree: tree.to_owned(),
        file_count,
        byte_count,
        content_identity,
    })
}

fn verify_git_destination_containment(
    source: &Path,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    for entry in entries {
        checked_git_destination(source, entry)?;
    }
    Ok(())
}

fn checked_git_destination(
    source: &Path,
    entry: &GitTreeEntry,
) -> Result<PathBuf, SourceResolveError> {
    if entry
        .relative_path
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "materialization path is not strictly relative",
        ));
    }
    let destination = source.join(&entry.relative_path);
    if !destination.starts_with(source) {
        return Err(git_tree_invalid(
            &entry.relative_bytes,
            "materialization path escapes the snapshot root",
        ));
    }
    Ok(destination)
}

fn release_git_blob_payloads(entries: &mut [GitTreeEntry]) {
    for entry in entries {
        match &mut entry.kind {
            GitTreeEntryKind::Tree => {}
            GitTreeEntryKind::File { bytes, .. } => *bytes = GitBlobBytes::empty(),
            GitTreeEntryKind::Symlink { target_bytes } => {
                *target_bytes = GitBlobBytes::empty();
            }
        }
    }
}

fn verify_git_snapshot(
    publication: &Path,
    expected: &GitSnapshotMetadata,
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    let source = publication.join(GIT_SNAPSHOT_SOURCE);
    let metadata_path = publication.join(GIT_SNAPSHOT_METADATA);
    let metadata = read_bounded_cache_record(
        CacheCustodyKind::Git,
        publication,
        Path::new(GIT_SNAPSHOT_METADATA),
        1024,
    )?;
    let metadata = parse_git_snapshot_metadata(&metadata, &metadata_path)?;
    if metadata != *expected {
        return Err(cache_invalid(
            &metadata_path,
            "snapshot metadata does not match the authenticated Git tree",
        ));
    }
    let publication_directory = open_absolute_directory_nofollow(publication)
        .map_err(|error| cache_invalid(publication, error.to_string()))?;
    verify_open_snapshot_tree_modes(CacheCustodyKind::Git, &publication_directory, publication)?;
    let source_directory = publication_directory
        .open_dir_nofollow(GIT_SNAPSHOT_SOURCE)
        .map_err(|error| cache_invalid(&source, error.to_string()))?;
    let captured = capture_local_source_from_open_root(
        source.clone(),
        source_directory,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?;
    verify_captured_git_snapshot_shape(&source, &captured.entries, entries)?;
    let local = captured.normalized;
    if local.file_count != expected.file_count
        || local.byte_count != expected.byte_count
        || local.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            publication,
            "published snapshot does not match the authenticated Git tree",
        ));
    }
    Ok((source, local))
}

fn verify_captured_git_snapshot_shape(
    source: &Path,
    captured: &[CapturedLocalEntry],
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    let mut expected_directories = git_directory_paths(entries);
    let mut expected_leaves = entries
        .iter()
        .filter(|entry| !matches!(entry.kind, GitTreeEntryKind::Tree))
        .map(|entry| (entry.relative_bytes.as_slice(), &entry.kind))
        .collect::<BTreeMap<_, _>>();
    for entry in captured {
        let path = source.join(&entry.relative_path);
        match &entry.kind {
            CapturedLocalEntryKind::Directory => {
                if !expected_directories.remove(&entry.relative_bytes) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot contains an undeclared directory",
                    ));
                }
            }
            CapturedLocalEntryKind::File { executable, .. } => {
                let Some(expected) = expected_leaves.remove(entry.relative_bytes.as_slice()) else {
                    return Err(cache_invalid(&path, "snapshot contains an undeclared file"));
                };
                if !matches!(
                    expected,
                    GitTreeEntryKind::File {
                        executable: expected_executable,
                        ..
                    } if expected_executable == executable
                ) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot file kind or executable mode does not match Git",
                    ));
                }
            }
            CapturedLocalEntryKind::Symlink { .. } => {
                let Some(expected) = expected_leaves.remove(entry.relative_bytes.as_slice()) else {
                    return Err(cache_invalid(
                        &path,
                        "snapshot contains an undeclared symlink",
                    ));
                };
                if !matches!(expected, GitTreeEntryKind::Symlink { .. }) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot symlink kind does not match Git",
                    ));
                }
            }
        }
    }
    if !expected_directories.is_empty() || !expected_leaves.is_empty() {
        return Err(cache_invalid(
            source,
            "snapshot paths do not exactly match the validated Git tree",
        ));
    }
    Ok(())
}

fn verify_open_snapshot_tree_modes(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    display_root: &Path,
) -> Result<(), SourceResolveError> {
    let root_metadata = root
        .dir_metadata()
        .map_err(|error| io_error(display_root, error))?;
    verify_capability_snapshot_directory_mode(kind, display_root, &root_metadata)?;
    let entries = root
        .entries()
        .map_err(|error| io_error(display_root, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(display_root, error))?;
        let name = entry.file_name();
        let path = display_root.join(&name);
        let metadata = root
            .symlink_metadata(&name)
            .map_err(|error| io_error(&path, error))?;
        if metadata.is_dir() {
            let directory = root.open_dir_nofollow(&name).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot directory changed during verification: {error}"),
                )
            })?;
            let opened = directory
                .dir_metadata()
                .map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot directory changed during verification",
                ));
            }
            verify_open_snapshot_tree_modes(kind, &directory, &path)?;
        } else if metadata.is_file() {
            let mut options = CapabilityOpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = root.open_with(&name, &options).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot file changed during verification: {error}"),
                )
            })?;
            let opened = file.metadata().map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot file changed during verification",
                ));
            }
            verify_capability_snapshot_file_mode(kind, &path, &opened)?;
        } else if !metadata.file_type().is_symlink() {
            return Err(cache_custody_invalid(
                kind,
                &path,
                "snapshot contains an unsupported filesystem entry type",
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn verify_capability_snapshot_directory_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    if metadata.mode() & 0o7777 != u32::from(CANONICAL_DIRECTORY_MODE) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "snapshot directory mode is not canonical 0555",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_capability_snapshot_directory_mode(
    _kind: CacheCustodyKind,
    _path: &Path,
    _metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
fn verify_capability_snapshot_file_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    if !matches!(metadata.mode() & 0o7777, 0o444 | 0o555) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "snapshot file mode is not canonical 0444 or 0555",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_capability_snapshot_file_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    if !metadata.permissions().readonly() {
        return Err(cache_custody_invalid(
            kind,
            path,
            "snapshot file is writable",
        ));
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct LocalSnapshotMetadata {
    file_count: usize,
    byte_count: u64,
    content_identity: String,
}

fn local_snapshot_metadata(local: &ResolvedLocalSource) -> Vec<u8> {
    let mut metadata = LOCAL_SNAPSHOT_POLICY.to_vec();
    metadata.extend_from_slice(&(local.file_count as u64).to_le_bytes());
    metadata.extend_from_slice(&local.byte_count.to_le_bytes());
    append_framed_bytes(&mut metadata, local.content_identity.as_bytes());
    metadata
}

fn parse_local_snapshot_metadata(
    bytes: &[u8],
    path: &Path,
) -> Result<LocalSnapshotMetadata, SourceResolveError> {
    let Some(mut remaining) = bytes.strip_prefix(LOCAL_SNAPSHOT_POLICY) else {
        return Err(local_snapshot_invalid(
            path,
            "snapshot metadata policy does not match",
        ));
    };
    let file_count = take_u64(&mut remaining)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| local_snapshot_invalid(path, "snapshot file count is invalid"))?;
    let byte_count = take_u64(&mut remaining)
        .ok_or_else(|| local_snapshot_invalid(path, "snapshot byte count is invalid"))?;
    let content_identity = take_framed_bytes(&mut remaining)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .filter(|identity| {
            identity.len() == 64 && identity.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or_else(|| local_snapshot_invalid(path, "snapshot content identity is invalid"))?
        .to_owned();
    if !remaining.is_empty() {
        return Err(local_snapshot_invalid(
            path,
            "snapshot metadata has trailing bytes",
        ));
    }
    Ok(LocalSnapshotMetadata {
        file_count,
        byte_count,
        content_identity,
    })
}

fn verify_local_snapshot(
    publication: &Path,
    content_identity: &str,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let source = publication.join(LOCAL_SNAPSHOT_SOURCE);
    let metadata_path = publication.join(LOCAL_SNAPSHOT_METADATA);
    let metadata = read_bounded_cache_record(
        CacheCustodyKind::LocalSnapshot,
        publication,
        Path::new(LOCAL_SNAPSHOT_METADATA),
        512,
    )?;
    let expected = parse_local_snapshot_metadata(&metadata, &metadata_path)?;
    if expected.content_identity != content_identity {
        return Err(local_snapshot_invalid(
            &metadata_path,
            "snapshot content identity does not match its cache key",
        ));
    }
    let publication_directory = open_absolute_directory_nofollow(publication)
        .map_err(|error| local_snapshot_invalid(publication, error.to_string()))?;
    verify_open_snapshot_tree_modes(
        CacheCustodyKind::LocalSnapshot,
        &publication_directory,
        publication,
    )?;
    let source_directory = publication_directory
        .open_dir_nofollow(LOCAL_SNAPSHOT_SOURCE)
        .map_err(|error| local_snapshot_invalid(&source, error.to_string()))?;
    let normalized = capture_local_source_from_open_root(
        source.clone(),
        source_directory,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if normalized.file_count != expected.file_count
        || normalized.byte_count != expected.byte_count
        || normalized.content_identity != expected.content_identity
    {
        return Err(local_snapshot_invalid(
            publication,
            "published snapshot does not match resolver metadata",
        ));
    }
    Ok(normalized)
}

#[derive(Debug, PartialEq, Eq)]
struct GitSnapshotMetadata {
    tree: String,
    file_count: usize,
    byte_count: u64,
    content_identity: String,
}

fn git_snapshot_metadata(tree: &str, local: &ResolvedLocalSource) -> Vec<u8> {
    let mut metadata = GIT_SNAPSHOT_POLICY.to_vec();
    append_framed_bytes(&mut metadata, tree.as_bytes());
    metadata.extend_from_slice(&(local.file_count as u64).to_le_bytes());
    metadata.extend_from_slice(&local.byte_count.to_le_bytes());
    append_framed_bytes(&mut metadata, local.content_identity.as_bytes());
    metadata
}

fn parse_git_snapshot_metadata(
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

fn is_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn git_tree_invalid(path: impl AsRef<[u8]>, message: impl Into<String>) -> SourceResolveError {
    SourceResolveError::GitTreeInvalid {
        path: path.as_ref().to_vec(),
        message: message.into(),
    }
}

fn read_git_blobs_batch(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    entries: &mut [GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    repository.verify_identity()?;
    let result = read_git_blobs_batch_from_path(executor, repository.path(), entries, limits);
    reconcile_git_cache_operation_result(result, repository.verify_identity(), None)
}

fn read_git_blobs_batch_from_path(
    executor: &GitExecutor,
    repository: &Path,
    entries: &mut [GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    executor.verify_budget()?;
    if entries
        .iter()
        .all(|entry| matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        return Ok(());
    }
    let stdout_limit = git_batch_output_limit(entries, limits)?;
    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_path = repository
        .parent()
        .expect("validated bare repository has an entry root")
        .join(format!(
            ".omega-cat-file-batch.{}.{}",
            std::process::id(),
            sequence
        ));
    let request_guard = TemporaryFileGuard {
        path: request_path.clone(),
    };
    let mut request = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&request_path)
        .map_err(|error| io_error(&request_path, error))?;
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        request
            .write_all(entry.oid.as_bytes())
            .and_then(|_| request.write_all(b"\n"))
            .map_err(|error| io_error(&request_path, error))?;
    }
    request
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error(&request_path, error))?;

    let mut command = sealed_git_command(executor, repository)?;
    let command_timeout = executor.begin_launch()?;
    command.args([OsStr::new("cat-file"), OsStr::new("--batch")]);
    let result = run_command_bounded_with_stdin(
        &mut command,
        Stdio::from(request),
        "cat-file --batch",
        stdout_limit,
        GIT_STDERR_LIMIT,
        command_timeout,
    );
    let output = reconcile_git_command_result(result, executor.verify(), executor.verify_budget())?;
    drop(request_guard);
    if !output.status.success() {
        return Err(SourceResolveError::Git {
            operation: "cat-file --batch".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    assign_git_batch_output(entries, output.stdout)?;
    executor.verify_budget()
}

fn git_batch_output_limit(
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<usize, SourceResolveError> {
    let mut payload_bytes = 0_u64;
    let mut output_bytes = 0_usize;
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        payload_bytes =
            payload_bytes
                .checked_add(entry.size)
                .ok_or(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                })?;
        if payload_bytes > limits.max_bytes {
            return Err(SourceResolveError::TooManyBytes {
                limit: limits.max_bytes,
            });
        }
        let size = usize::try_from(entry.size).map_err(|_| {
            git_tree_invalid(entry.oid.as_bytes(), "blob cannot fit in host memory")
        })?;
        output_bytes = output_bytes
            .checked_add(entry.oid.len())
            .and_then(|value| value.checked_add(b" blob ".len()))
            .and_then(|value| value.checked_add(decimal_digit_count(entry.size)))
            .and_then(|value| value.checked_add(1))
            .and_then(|value| value.checked_add(size))
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| {
                git_tree_invalid(
                    entry.oid.as_bytes(),
                    "batch output cannot fit in host memory",
                )
            })?;
    }
    Ok(output_bytes)
}

fn decimal_digit_count(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn assign_git_batch_output(
    entries: &mut [GitTreeEntry],
    output: Vec<u8>,
) -> Result<(), SourceResolveError> {
    let mut remaining = output.as_slice();
    let mut offset = 0_usize;
    let mut ranges = Vec::with_capacity(entries.len());
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        let Some(header_end) = remaining.iter().position(|byte| *byte == b'\n') else {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "truncated cat-file batch header",
            ));
        };
        let header = &remaining[..=header_end];
        let expected_header = format!("{} blob {}\n", entry.oid, entry.size);
        if header != expected_header.as_bytes() {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "cat-file batch header did not match the exact requested blob",
            ));
        }
        remaining = &remaining[header_end + 1..];
        offset = offset
            .checked_add(header_end + 1)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
        let size = usize::try_from(entry.size).map_err(|_| {
            git_tree_invalid(entry.oid.as_bytes(), "blob cannot fit in host memory")
        })?;
        let Some(bytes) = remaining.get(..size) else {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "truncated cat-file batch blob",
            ));
        };
        if remaining.get(size) != Some(&b'\n') {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "cat-file batch blob lacks its separator",
            ));
        }
        if matches!(&entry.kind, GitTreeEntryKind::Symlink { .. }) {
            validate_git_symlink_target(&entry.relative_bytes, bytes)?;
        }
        let end = offset
            .checked_add(size)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
        ranges.push(offset..end);
        remaining = &remaining[size + 1..];
        offset = end
            .checked_add(1)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
    }
    if !remaining.is_empty() {
        return Err(git_tree_invalid(
            Vec::new(),
            "cat-file batch returned an unexpected trailing response",
        ));
    }
    let batch = Arc::new(output);
    for (entry, range) in entries
        .iter_mut()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
        .zip(ranges)
    {
        match &mut entry.kind {
            GitTreeEntryKind::Tree => unreachable!("tree rows are excluded from blob assignment"),
            GitTreeEntryKind::File { bytes, .. } => {
                *bytes = GitBlobBytes {
                    batch: Arc::clone(&batch),
                    start: range.start,
                    end: range.end,
                };
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                *target_bytes = GitBlobBytes {
                    batch: Arc::clone(&batch),
                    start: range.start,
                    end: range.end,
                };
            }
        }
    }
    Ok(())
}

struct TemporaryFileGuard {
    path: PathBuf,
}

impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn open_or_create_snapshot_directory(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    use std::path::Component;

    let mut directory = root
        .try_clone()
        .map_err(|error| io_error(display_root, error))?;
    let mut display_path = display_root.to_path_buf();
    for component in relative_path.components() {
        let Component::Normal(name) = component else {
            return Err(cache_custody_invalid(
                kind,
                &display_path,
                "snapshot materialization received a noncanonical relative directory",
            ));
        };
        display_path.push(name);
        match directory.create_dir(name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(&display_path, error)),
        }
        directory = directory.open_dir_nofollow(name).map_err(|error| {
            cache_custody_invalid(
                kind,
                &display_path,
                format!("snapshot directory is not a stable concrete child: {error}"),
            )
        })?;
    }
    Ok(directory)
}

fn write_snapshot_file_from_open_root(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
    bytes: &[u8],
    executable: bool,
) -> Result<(), SourceResolveError> {
    let parent_path = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let parent = open_or_create_snapshot_directory(kind, root, parent_path, display_root)?;
    let name = relative_path.file_name().ok_or_else(|| {
        cache_custody_invalid(
            kind,
            &display_root.join(relative_path),
            "snapshot file has no relative name",
        )
    })?;
    let display_path = display_root.join(relative_path);
    let mut options = CapabilityOpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = parent.open_with(name, &options).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            cache_custody_invalid(
                kind,
                &display_path,
                "snapshot file destination already exists",
            )
        } else {
            io_error(&display_path, error)
        }
    })?;
    file.write_all(bytes)
        .map_err(|error| io_error(&display_path, error))?;
    file.sync_all()
        .map_err(|error| io_error(&display_path, error))?;
    set_open_snapshot_file_mode(&file, &display_path, executable)
}

#[cfg(unix)]
fn create_snapshot_symlink_from_open_root(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
    target: &[u8],
) -> Result<(), SourceResolveError> {
    use std::os::unix::ffi::OsStringExt;

    let parent_path = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let parent = open_or_create_snapshot_directory(kind, root, parent_path, display_root)?;
    let name = relative_path.file_name().ok_or_else(|| {
        cache_custody_invalid(
            kind,
            &display_root.join(relative_path),
            "snapshot symlink has no relative name",
        )
    })?;
    let display_path = display_root.join(relative_path);
    parent
        .symlink_contents(OsString::from_vec(target.to_vec()), name)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                cache_custody_invalid(kind, &display_path, "snapshot symlink already exists")
            } else {
                io_error(&display_path, error)
            }
        })
}

#[cfg(not(unix))]
fn create_snapshot_symlink_from_open_root(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
    target: &[u8],
) -> Result<(), SourceResolveError> {
    let target = std::str::from_utf8(target).map_err(|_| {
        git_tree_invalid(target, "symlink target cannot be represented on this host")
    })?;
    let parent_path = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let parent = open_or_create_snapshot_directory(kind, root, parent_path, display_root)?;
    let name = relative_path.file_name().ok_or_else(|| {
        cache_custody_invalid(
            kind,
            &display_root.join(relative_path),
            "snapshot symlink has no relative name",
        )
    })?;
    let display_path = display_root.join(relative_path);
    parent.symlink_file(target, name).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            cache_custody_invalid(kind, &display_path, "snapshot symlink already exists")
        } else {
            io_error(&display_path, error)
        }
    })
}

#[cfg(unix)]
fn set_open_snapshot_file_mode(
    file: &cap_std::fs::File,
    path: &Path,
    executable: bool,
) -> Result<(), SourceResolveError> {
    use cap_std::fs::PermissionsExt;

    let mode = if executable { 0o555 } else { 0o444 };
    file.set_permissions(cap_std::fs::Permissions::from_mode(mode))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn set_open_snapshot_file_mode(
    file: &cap_std::fs::File,
    path: &Path,
    _executable: bool,
) -> Result<(), SourceResolveError> {
    let mut permissions = file
        .metadata()
        .map_err(|error| io_error(path, error))?
        .permissions();
    permissions.set_readonly(true);
    file.set_permissions(permissions)
        .map_err(|error| io_error(path, error))
}

fn make_open_snapshot_read_only(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    display_root: &Path,
) -> Result<(), SourceResolveError> {
    let entries = root
        .entries()
        .map_err(|error| io_error(display_root, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(display_root, error))?;
        let name = entry.file_name();
        let path = display_root.join(&name);
        let metadata = root
            .symlink_metadata(&name)
            .map_err(|error| io_error(&path, error))?;
        if metadata.is_dir() {
            let directory = root.open_dir_nofollow(&name).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot directory changed during finalization: {error}"),
                )
            })?;
            let opened = directory
                .dir_metadata()
                .map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot directory changed during read-only finalization",
                ));
            }
            make_open_snapshot_read_only(kind, &directory, &path)?;
        } else if metadata.is_file() {
            let mut options = CapabilityOpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = root.open_with(&name, &options).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot file changed during finalization: {error}"),
                )
            })?;
            let opened = file.metadata().map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot file changed during read-only finalization",
                ));
            }
            set_open_snapshot_file_mode(&file, &path, capability_is_executable(&metadata))?;
        }
    }
    set_open_snapshot_directory_read_only(root, display_root)
}

#[cfg(unix)]
fn capability_is_executable(metadata: &CapabilityMetadata) -> bool {
    use cap_fs_ext::OsMetadataExt;

    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn capability_is_executable(_metadata: &CapabilityMetadata) -> bool {
    false
}

#[cfg(unix)]
fn set_open_snapshot_directory_read_only(
    directory: &CapabilityDirectory,
    path: &Path,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    directory
        .try_clone()
        .map_err(|error| io_error(path, error))?
        .into_std_file()
        .set_permissions(std::fs::Permissions::from_mode(0o555))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn set_open_snapshot_directory_read_only(
    _directory: &CapabilityDirectory,
    _path: &Path,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(test, unix))]
fn set_snapshot_file_mode(path: &Path, executable: bool) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if executable { 0o555 } else { 0o444 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| io_error(path, error))
}

#[cfg(all(test, not(unix)))]
fn set_snapshot_file_mode(path: &Path, _executable: bool) -> Result<(), SourceResolveError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|error| io_error(path, error))?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).map_err(|error| io_error(path, error))
}

#[cfg(test)]
fn make_snapshot_read_only(root: &Path) -> Result<(), SourceResolveError> {
    let mut directories = vec![root.to_path_buf()];
    let mut cursor = 0;
    while cursor < directories.len() {
        let directory = directories[cursor].clone();
        cursor += 1;
        for entry in std::fs::read_dir(&directory).map_err(|error| io_error(&directory, error))? {
            let entry = entry.map_err(|error| io_error(&directory, error))?;
            let path = entry.path();
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
            if metadata.is_dir() {
                directories.push(path);
            } else if metadata.is_file() {
                set_snapshot_file_mode(&path, is_executable(&metadata))?;
            }
        }
    }
    for directory in directories.into_iter().rev() {
        set_snapshot_directory_read_only(&directory)?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
fn set_snapshot_directory_read_only(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555))
        .map_err(|error| io_error(path, error))
}

#[cfg(all(test, not(unix)))]
fn set_snapshot_directory_read_only(_path: &Path) -> Result<(), SourceResolveError> {
    Ok(())
}

struct PendingMaterializedSnapshot {
    root: PathBuf,
    parent: CapabilityDirectory,
    directory: Option<CapabilityDirectory>,
    stage_name: OsString,
    kind: CacheCustodyKind,
    published: bool,
}

impl PendingMaterializedSnapshot {
    fn create(
        kind: CacheCustodyKind,
        snapshots: &Path,
        prefix: &str,
    ) -> Result<Self, SourceResolveError> {
        verify_cache_custody_root(snapshots, kind)?;
        let parent = open_absolute_directory_nofollow(snapshots)
            .map_err(|error| cache_custody_invalid(kind, snapshots, error.to_string()))?;
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let stage_name = OsString::from(format!("{prefix}-{}-{sequence}", std::process::id()));
            let root = snapshots.join(&stage_name);
            match parent.create_dir(&stage_name) {
                Ok(()) => {
                    let classified = parent
                        .symlink_metadata(&stage_name)
                        .map_err(|error| io_error(&root, error))?;
                    let directory = parent
                        .open_dir_nofollow(&stage_name)
                        .map_err(|error| cache_custody_invalid(kind, &root, error.to_string()))?;
                    let opened = directory
                        .dir_metadata()
                        .map_err(|error| io_error(&root, error))?;
                    if !classified.is_dir() || !same_capability_file_identity(&classified, &opened)
                    {
                        return Err(cache_custody_invalid(
                            kind,
                            &root,
                            "snapshot staging directory changed while being retained",
                        ));
                    }
                    return Ok(Self {
                        root,
                        parent,
                        directory: Some(directory),
                        stage_name,
                        kind,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&root, error)),
            }
        }
        Err(cache_custody_invalid(
            kind,
            snapshots,
            "could not allocate a unique materialized-snapshot staging directory",
        ))
    }

    fn directory(&self) -> Result<&CapabilityDirectory, SourceResolveError> {
        self.directory.as_ref().ok_or_else(|| {
            cache_custody_invalid(self.kind, &self.root, "snapshot stage handle is absent")
        })
    }

    fn publish(&mut self, snapshots: &Path, publication: &Path) -> Result<(), SourceResolveError> {
        let directory = self.directory()?;
        let retained = directory
            .dir_metadata()
            .map_err(|error| io_error(&self.root, error))?;
        let named = self
            .parent
            .symlink_metadata(&self.stage_name)
            .map_err(|error| io_error(&self.root, error))?;
        if !named.is_dir() || !same_capability_file_identity(&retained, &named) {
            return Err(cache_custody_invalid(
                self.kind,
                &self.root,
                "snapshot stage pathname no longer identifies the retained directory",
            ));
        }
        let publication_name = direct_cache_child_name(self.kind, snapshots, publication)?;
        publish_cache_directory_from_open_parent(
            self.kind,
            snapshots,
            &self.parent,
            &self.stage_name,
            publication_name,
            Some(&retained),
        )?;
        self.published = true;
        Ok(())
    }
}

impl Drop for PendingMaterializedSnapshot {
    fn drop(&mut self) {
        if !self.published {
            if let Some(directory) = self.directory.take() {
                make_open_tree_owner_writable(&directory);
                let _ = directory.remove_open_dir_all();
            }
        }
    }
}

fn make_open_tree_owner_writable(root: &CapabilityDirectory) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Ok(directory) = root.try_clone() {
            let _ = directory
                .into_std_file()
                .set_permissions(std::fs::Permissions::from_mode(0o700));
        }
        if let Ok(entries) = root.entries() {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Ok(metadata) = root.symlink_metadata(&name)
                    && metadata.is_dir()
                    && let Ok(directory) = root.open_dir_nofollow(&name)
                {
                    make_open_tree_owner_writable(&directory);
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        if let Ok(directory) = root.try_clone() {
            let directory = directory.into_std_file();
            if let Ok(metadata) = directory.metadata() {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(false);
                let _ = directory.set_permissions(permissions);
            }
        }
        if let Ok(entries) = root.entries() {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Ok(metadata) = root.symlink_metadata(&name) {
                    if metadata.is_dir() {
                        if let Ok(directory) = root.open_dir_nofollow(&name) {
                            make_open_tree_owner_writable(&directory);
                        }
                    } else if metadata.is_file() {
                        let mut options = CapabilityOpenOptions::new();
                        options.read(true).follow(FollowSymlinks::No);
                        if let Ok(file) = root.open_with(&name, &options)
                            && let Ok(metadata) = file.metadata()
                        {
                            let mut permissions = metadata.permissions();
                            permissions.set_readonly(false);
                            let _ = file.set_permissions(permissions);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
fn make_tree_owner_writable(root: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Ok(metadata) = std::fs::symlink_metadata(root)
            && metadata.is_dir()
        {
            let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700));
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Ok(metadata) = std::fs::symlink_metadata(&path)
                        && metadata.is_dir()
                    {
                        make_tree_owner_writable(&path);
                    }
                }
            }
        }
    }
}

fn create_git_cache_entry(
    executor: &GitExecutor,
    cache_dir: &Path,
    cache_directory: &CapabilityDirectory,
    entry_root: &Path,
    entry_name: &OsStr,
    cache_identity: &str,
    locator_identity: &str,
    fetch_locator: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    let mut pending = PendingCacheEntry::create(cache_dir, cache_directory, cache_identity)?;
    let repository = pending.root.join(GIT_CACHE_REPOSITORY);
    let empty_template = pending.root.join("empty-template");
    pending.create_private_directory("empty-template", &empty_template)?;
    pending.verify_ambient_path_identity(cache_dir)?;
    let object_format_result =
        discover_git_object_format(executor, &pending.root, fetch_locator, requested_rev);
    let object_format = reconcile_git_cache_operation_result(
        object_format_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    let mut init_arguments = vec![
        OsString::from("init"),
        OsString::from("--quiet"),
        OsString::from("--bare"),
    ];
    if object_format == GitObjectIdAlgorithm::Sha256 {
        init_arguments.push(OsString::from("--object-format=sha256"));
    }
    init_arguments.push(OsString::from("--template"));
    init_arguments.push(empty_template.as_os_str().to_owned());
    init_arguments.push(repository.as_os_str().to_owned());
    pending.verify_ambient_path_identity(cache_dir)?;
    let init_result = run_git(executor, &pending.root, init_arguments.iter());
    reconcile_git_cache_operation_result(
        init_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    let canonical_config = match object_format {
        GitObjectIdAlgorithm::Sha1 => GIT_CONFIG_SHA1,
        GitObjectIdAlgorithm::Sha256 => GIT_CONFIG_SHA256,
    };
    pending.verify_ambient_path_identity(cache_dir)?;
    let config_result = replace_canonical_git_control_file(
        pending.directory()?,
        OsStr::new(GIT_CACHE_REPOSITORY),
        &repository,
        canonical_config,
    );
    reconcile_git_cache_operation_result(
        config_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    pending
        .directory()?
        .remove_dir("empty-template")
        .map_err(|error| io_error(&empty_template, error))?;

    let metadata_path = pending.root.join(GIT_CACHE_METADATA);
    let mut metadata_options = CapabilityOpenOptions::new();
    metadata_options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    metadata_options.mode(0o600);
    let mut metadata = pending
        .directory()?
        .open_with(GIT_CACHE_METADATA, &metadata_options)
        .map_err(|error| io_error(&metadata_path, error))?;
    #[cfg(unix)]
    {
        let mut permissions = metadata
            .metadata()
            .map_err(|error| io_error(&metadata_path, error))?
            .permissions();
        permissions.set_mode(0o600);
        metadata
            .set_permissions(permissions)
            .map_err(|error| io_error(&metadata_path, error))?;
    }
    metadata
        .write_all(&git_cache_metadata(
            locator_identity,
            requested_rev,
            execution_transport,
        ))
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .sync_all()
        .map_err(|error| io_error(&metadata_path, error))?;
    let metadata_custody = metadata
        .metadata()
        .map_err(|error| io_error(&metadata_path, error))?;
    verify_capability_cache_node_owner_and_mode(
        CacheCustodyKind::Git,
        &metadata_path,
        &metadata_custody,
    )?;
    #[cfg(unix)]
    {
        use cap_fs_ext::OsMetadataExt;

        if metadata_custody.mode() & 0o777 != 0o600 {
            return Err(cache_invalid(
                &metadata_path,
                "resolver metadata does not have exact private mode 0600",
            ));
        }
    }

    pending.verify_ambient_path_identity(cache_dir)?;
    let verification_result = VerifiedGitRepository::open(
        &pending.parent,
        &pending.stage_name,
        &pending.root,
        locator_identity,
        requested_rev,
        execution_transport,
        limits,
    );
    reconcile_git_cache_operation_result(
        verification_result,
        pending.verify_ambient_path_identity(cache_dir),
        None,
    )?;
    pending.publish(cache_dir, entry_root, entry_name)?;
    Ok(())
}

fn discover_git_object_format(
    executor: &GitExecutor,
    working_directory: &Path,
    url: &str,
    requested_rev: &str,
) -> Result<GitObjectIdAlgorithm, SourceResolveError> {
    if is_object_id(requested_rev) {
        return git_object_algorithm(requested_rev);
    }
    let output = run_git_bytes_stdout(
        executor,
        working_directory,
        [
            OsStr::new("ls-remote"),
            OsStr::new("--symref"),
            OsStr::new("--"),
            OsStr::new(url),
            OsStr::new("HEAD"),
            OsStr::new(requested_rev),
        ],
    )?;
    parse_git_remote_object_format(&output, working_directory)
}

fn parse_git_remote_object_format(
    output: &[u8],
    working_directory: &Path,
) -> Result<GitObjectIdAlgorithm, SourceResolveError> {
    let mut selected = None;
    for line in output.split(|byte| *byte == b'\n') {
        if line.is_empty() || line.starts_with(b"ref: ") {
            continue;
        }
        let Some(separator) = line.iter().position(|byte| *byte == b'\t') else {
            return Err(cache_invalid(
                working_directory,
                "Git object-format discovery returned a malformed row",
            ));
        };
        let oid = std::str::from_utf8(&line[..separator]).map_err(|_| {
            cache_invalid(
                working_directory,
                "Git object-format discovery returned a non-ASCII object ID",
            )
        })?;
        let algorithm = git_object_algorithm(oid)?;
        if selected.is_some_and(|selected| selected != algorithm) {
            return Err(cache_invalid(
                working_directory,
                "Git object-format discovery returned mixed hash algorithms",
            ));
        }
        selected = Some(algorithm);
    }
    selected.ok_or_else(|| {
        cache_invalid(
            working_directory,
            "Git object-format discovery returned no selected object ID",
        )
    })
}

struct VerifiedGitRepository {
    entry_root: PathBuf,
    repository_path: PathBuf,
    entry_name: OsString,
    expected_metadata: Vec<u8>,
    cache_parent: CapabilityDirectory,
    entry: CapabilityDirectory,
    repository: CapabilityDirectory,
    objects: CapabilityDirectory,
    entry_identity: CapabilityMetadata,
    repository_identity: CapabilityMetadata,
    objects_identity: CapabilityMetadata,
}

impl VerifiedGitRepository {
    fn open(
        cache_parent: &CapabilityDirectory,
        entry_name: &OsStr,
        entry_root: &Path,
        url: &str,
        requested_rev: &str,
        execution_transport: GitExecutionTransport,
        limits: LocalSourceLimits,
    ) -> Result<Self, SourceResolveError> {
        let (entry, entry_identity) = open_retained_git_directory(
            cache_parent,
            entry_name,
            entry_root,
            "cache entry root is not a concrete directory",
        )?;
        let repository_path = entry_root.join(GIT_CACHE_REPOSITORY);
        let (repository, repository_identity) = open_retained_git_directory(
            &entry,
            OsStr::new(GIT_CACHE_REPOSITORY),
            &repository_path,
            "repository is not a concrete directory",
        )?;
        let objects_path = repository_path.join("objects");
        let (objects, objects_identity) = open_retained_git_directory(
            &repository,
            OsStr::new("objects"),
            &objects_path,
            "Git object directory is not a concrete directory",
        )?;
        let verified = Self {
            entry_root: entry_root.to_path_buf(),
            repository_path,
            entry_name: entry_name.to_os_string(),
            expected_metadata: git_cache_metadata(url, requested_rev, execution_transport),
            cache_parent: cache_parent
                .try_clone()
                .map_err(|error| io_error(entry_root, error))?,
            entry,
            repository,
            objects,
            entry_identity,
            repository_identity,
            objects_identity,
        };
        verified.verify_current(limits)?;
        Ok(verified)
    }

    fn path(&self) -> &Path {
        &self.repository_path
    }

    fn verify_identity(&self) -> Result<(), SourceResolveError> {
        let cache_root = self.entry_root.parent().ok_or_else(|| {
            cache_invalid(&self.entry_root, "Git cache entry has no retained parent")
        })?;
        verify_retained_cache_parent_path(CacheCustodyKind::Git, cache_root, &self.cache_parent)?;
        verify_retained_git_directory_identity(
            &self.cache_parent,
            &self.entry_name,
            &self.entry,
            &self.entry_identity,
            &self.entry_root,
            "cache entry root no longer identifies the retained directory",
        )?;
        verify_retained_git_directory_identity(
            &self.entry,
            OsStr::new(GIT_CACHE_REPOSITORY),
            &self.repository,
            &self.repository_identity,
            &self.repository_path,
            "repository no longer identifies the retained directory",
        )?;
        verify_retained_git_directory_identity(
            &self.repository,
            OsStr::new("objects"),
            &self.objects,
            &self.objects_identity,
            &self.repository_path.join("objects"),
            "Git object directory no longer identifies the retained directory",
        )
    }

    fn verify_current(&self, limits: LocalSourceLimits) -> Result<(), SourceResolveError> {
        self.verify_identity()?;
        verify_cache_custody_from_open_root(
            &self.entry_root,
            self.entry
                .try_clone()
                .map_err(|error| io_error(&self.entry_root, error))?,
            CacheCustodyKind::Git,
            git_cache_custody_byte_limit(limits),
        )?;
        let actual_metadata = read_bounded_cache_record_from_open_directory(
            CacheCustodyKind::Git,
            &self.entry,
            &self.entry_root,
            Path::new(GIT_CACHE_METADATA),
            self.expected_metadata.len(),
        )?;
        if actual_metadata != self.expected_metadata {
            return Err(cache_invalid(
                &self.entry_root,
                "resolver metadata does not match the exact source locator and revision",
            ));
        }
        verify_git_repository_tree_from_open_root(&self.repository, &self.repository_path)?;
        reject_retained_git_path(
            &self.objects,
            &self.repository_path.join("objects"),
            &["info", "alternates"],
        )?;
        reject_retained_git_path(
            &self.objects,
            &self.repository_path.join("objects"),
            &["info", "http-alternates"],
        )?;
        reject_retained_git_path(&self.repository, &self.repository_path, &["commondir"])?;
        self.read_canonical_config()?;
        self.verify_identity()
    }

    fn read_canonical_config(&self) -> Result<Vec<u8>, SourceResolveError> {
        let config_path = self.repository_path.join("config");
        let config = read_bounded_cache_record_from_open_directory(
            CacheCustodyKind::Git,
            &self.repository,
            &self.repository_path,
            Path::new("config"),
            GIT_CONFIG_SHA256.len(),
        )?;
        if config.as_slice() != GIT_CONFIG_SHA1 && config.as_slice() != GIT_CONFIG_SHA256 {
            return Err(cache_invalid(
                &config_path,
                "local Git configuration is not the exact resolver-owned canonical file",
            ));
        }
        Ok(config)
    }

    fn restore_canonical_config(&self, canonical_config: &[u8]) -> Result<(), SourceResolveError> {
        debug_assert!(canonical_config == GIT_CONFIG_SHA1 || canonical_config == GIT_CONFIG_SHA256);
        self.verify_identity()?;
        let result = replace_canonical_git_control_file_from_open_repository(
            &self.repository,
            &self.repository_path,
            canonical_config,
        );
        reconcile_git_cache_operation_result(result, self.verify_identity(), None)
    }

    fn run_git<I, S>(&self, executor: &GitExecutor, args: I) -> Result<(), SourceResolveError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.verify_identity()?;
        let result = run_git(executor, &self.repository_path, args);
        reconcile_git_cache_operation_result(result, self.verify_identity(), None)
    }

    fn run_git_stdout<I, S>(
        &self,
        executor: &GitExecutor,
        args: I,
    ) -> Result<String, SourceResolveError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.verify_identity()?;
        let result = run_git_stdout(executor, &self.repository_path, args);
        reconcile_git_cache_operation_result(result, self.verify_identity(), None)
    }

    fn run_git_bytes_stdout<I, S>(
        &self,
        executor: &GitExecutor,
        args: I,
    ) -> Result<Vec<u8>, SourceResolveError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.verify_identity()?;
        let result = run_git_bytes_stdout(executor, &self.repository_path, args);
        reconcile_git_cache_operation_result(result, self.verify_identity(), None)
    }
}

fn open_retained_git_directory(
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
    message: &str,
) -> Result<(CapabilityDirectory, CapabilityMetadata), SourceResolveError> {
    let classified = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(path, message));
    }
    let directory = parent
        .open_dir_nofollow(name)
        .map_err(|error| cache_invalid(path, error.to_string()))?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !opened.is_dir() || !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            path,
            "Git directory changed between classification and no-follow open",
        ));
    }
    Ok((directory, opened))
}

fn verify_retained_git_directory_identity(
    parent: &CapabilityDirectory,
    name: &OsStr,
    retained: &CapabilityDirectory,
    expected: &CapabilityMetadata,
    path: &Path,
    message: &str,
) -> Result<(), SourceResolveError> {
    let named = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    let opened = retained
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if named.file_type().is_symlink()
        || !named.is_dir()
        || !opened.is_dir()
        || !same_capability_file_identity(expected, &named)
        || !same_capability_file_identity(expected, &opened)
    {
        return Err(cache_invalid(path, message));
    }
    Ok(())
}

fn verify_git_repository_tree_from_open_root(
    repository: &CapabilityDirectory,
    repository_path: &Path,
) -> Result<(), SourceResolveError> {
    let root_metadata = repository
        .dir_metadata()
        .map_err(|error| io_error(repository_path, error))?;
    let mut pending = vec![(
        PathBuf::new(),
        repository_path.to_path_buf(),
        root_metadata,
        0usize,
    )];
    let mut observed = 0usize;
    while let Some((relative_path, path, classified, depth)) = pending.pop() {
        observed = observed
            .checked_add(1)
            .ok_or_else(|| cache_invalid(&path, "Git repository entry count overflowed"))?;
        if observed > CACHE_CUSTODY_ENTRY_LIMIT {
            return Err(cache_invalid(
                repository_path,
                format!("Git repository exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry ceiling"),
            ));
        }
        let directory = open_cache_custody_directory(
            repository,
            &relative_path,
            &path,
            &classified,
            CacheCustodyKind::Git,
        )?;
        for child in directory
            .entries()
            .map_err(|error| io_error(&path, error))?
        {
            let child = child.map_err(|error| io_error(&path, error))?;
            let name = child.file_name();
            let child_path = path.join(&name);
            let metadata = directory
                .symlink_metadata(&name)
                .map_err(|error| io_error(&child_path, error))?;
            if metadata.file_type().is_symlink() {
                return Err(cache_invalid(
                    &child_path,
                    "symlinks are forbidden in the native Git repository",
                ));
            }
            if metadata.is_file() {
                verify_retained_git_regular_file(&directory, &name, &child_path, &metadata)?;
                observed = observed.checked_add(1).ok_or_else(|| {
                    cache_invalid(&child_path, "Git repository entry count overflowed")
                })?;
            } else if metadata.is_dir() {
                let child_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| cache_invalid(&child_path, "Git repository depth overflowed"))?;
                if child_depth > CACHE_CUSTODY_DEPTH_LIMIT {
                    return Err(cache_invalid(
                        &child_path,
                        format!(
                            "Git repository exceeds its {CACHE_CUSTODY_DEPTH_LIMIT}-level depth ceiling"
                        ),
                    ));
                }
                pending.push((relative_path.join(&name), child_path, metadata, child_depth));
            } else {
                return Err(cache_invalid(
                    &child_path,
                    "native Git repository contains an unsupported filesystem entry kind",
                ));
            }
            if observed
                .checked_add(pending.len())
                .is_none_or(|total| total > CACHE_CUSTODY_ENTRY_LIMIT)
            {
                return Err(cache_invalid(
                    repository_path,
                    format!("Git repository exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry ceiling"),
                ));
            }
        }
    }
    Ok(())
}

fn verify_retained_git_regular_file(
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
    classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent
        .open_with(name, &options)
        .map_err(|error| cache_invalid(path, error.to_string()))?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if !opened.is_file() || !same_capability_file_identity(classified, &opened) {
        return Err(cache_invalid(
            path,
            "Git repository file changed between classification and no-follow open",
        ));
    }
    verify_git_regular_file_link_count(path, &opened)
}

#[cfg(unix)]
fn verify_git_regular_file_link_count(
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    if metadata.nlink() != 1 {
        return Err(cache_invalid(
            path,
            "multiply-linked files are forbidden in the native Git repository",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_git_regular_file_link_count(
    _path: &Path,
    _metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

fn reject_retained_git_path(
    root: &CapabilityDirectory,
    root_path: &Path,
    components: &[&str],
) -> Result<(), SourceResolveError> {
    let Some((leaf, parents)) = components.split_last() else {
        return Err(cache_invalid(root_path, "forbidden Git path is empty"));
    };
    let mut directory = root
        .try_clone()
        .map_err(|error| io_error(root_path, error))?;
    let mut path = root_path.to_path_buf();
    for parent in parents {
        path.push(parent);
        let metadata = match directory.symlink_metadata(parent) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(&path, error)),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(cache_invalid(
                &path,
                "cannot prove forbidden Git path absent beneath a non-directory",
            ));
        }
        let opened = directory
            .open_dir_nofollow(parent)
            .map_err(|error| cache_invalid(&path, error.to_string()))?;
        let opened_metadata = opened
            .dir_metadata()
            .map_err(|error| io_error(&path, error))?;
        if !same_capability_file_identity(&metadata, &opened_metadata) {
            return Err(cache_invalid(
                &path,
                "Git directory changed while checking forbidden indirection",
            ));
        }
        directory = opened;
    }
    path.push(leaf);
    match directory.symlink_metadata(leaf) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(&path, error)),
        Ok(_) => Err(cache_invalid(
            &path,
            "external Git object or directory indirection is forbidden",
        )),
    }
}

#[cfg(test)]
fn invalidate_git_cache_entry_from_retained_parent(
    entry_root: &Path,
) -> Result<(), SourceResolveError> {
    let cache_root = entry_root
        .parent()
        .ok_or_else(|| cache_invalid(entry_root, "Git cache entry has no cache parent"))?;
    verify_git_cache_root_custody(cache_root)?;
    let cache_directory = open_absolute_directory_nofollow(cache_root)
        .map_err(|error| cache_invalid(cache_root, error.to_string()))?;
    let entry_name = direct_cache_child_name(CacheCustodyKind::Git, cache_root, entry_root)?;
    invalidate_git_cache_entry_from_open_parent(
        cache_root,
        &cache_directory,
        entry_name,
        entry_root,
    )
}

fn invalidate_git_cache_entry_from_open_parent(
    cache_root: &Path,
    cache_directory: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
) -> Result<(), SourceResolveError> {
    let classified = cache_directory
        .symlink_metadata(entry_name)
        .map_err(|error| io_error(entry_root, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(
            entry_root,
            "Git cache invalidation target is not a concrete directory",
        ));
    }
    let entry_directory = cache_directory
        .open_dir_nofollow(entry_name)
        .map_err(|error| cache_invalid(entry_root, error.to_string()))?;
    let opened = entry_directory
        .dir_metadata()
        .map_err(|error| io_error(entry_root, error))?;
    if !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            entry_root,
            "Git cache entry changed while opening it for invalidation",
        ));
    }
    entry_directory
        .remove_file(GIT_CACHE_METADATA)
        .map_err(|error| io_error(&entry_root.join(GIT_CACHE_METADATA), error))?;
    cache_directory
        .try_clone()
        .map_err(|error| io_error(cache_root, error))?
        .into_std_file()
        .sync_all()
        .map_err(|error| io_error(cache_root, error))
}

fn git_cache_identity(
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

fn git_cache_metadata(
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

fn append_framed_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn cache_invalid(path: &Path, message: impl Into<String>) -> SourceResolveError {
    SourceResolveError::GitCacheInvalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

fn local_snapshot_invalid(path: &Path, message: impl Into<String>) -> SourceResolveError {
    SourceResolveError::LocalSnapshotInvalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

fn require_real_directory(path: &Path, message: &str) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(cache_invalid(path, message));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum CacheCustodyKind {
    Git,
    LocalSnapshot,
}

fn read_bounded_cache_record(
    kind: CacheCustodyKind,
    root: &Path,
    relative_path: &Path,
    maximum_bytes: usize,
) -> Result<Vec<u8>, SourceResolveError> {
    verify_cache_custody_root(root, kind)?;
    let directory = open_absolute_directory_nofollow(root)
        .map_err(|error| cache_custody_invalid(kind, root, error.to_string()))?;
    read_bounded_cache_record_from_open_directory(
        kind,
        &directory,
        root,
        relative_path,
        maximum_bytes,
    )
}

fn read_bounded_cache_record_from_open_directory(
    kind: CacheCustodyKind,
    directory: &CapabilityDirectory,
    root: &Path,
    relative_path: &Path,
    maximum_bytes: usize,
) -> Result<Vec<u8>, SourceResolveError> {
    let directory = directory
        .try_clone()
        .map_err(|error| io_error(root, error))?;
    let record_root =
        RecordFileRoot::from_directory(directory, root.to_path_buf()).map_err(|error| {
            cache_custody_invalid(
                kind,
                root,
                format!("failed to retain cache record directory: {error:?}"),
            )
        })?;
    let record = record_root
        .read(relative_path, RecordFileLimits { maximum_bytes })
        .map_err(|error| {
            cache_custody_invalid(
                kind,
                &root.join(relative_path),
                format!("failed to read bounded cache record: {error:?}"),
            )
        })?;
    Ok(record.bytes().to_vec())
}

fn verify_git_cache_custody(
    root: &Path,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    verify_cache_custody(
        root,
        CacheCustodyKind::Git,
        git_cache_custody_byte_limit(limits),
    )
}

fn verify_git_cache_root_custody(root: &Path) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(root, CacheCustodyKind::Git)
}

fn verify_local_cache_custody(
    root: &Path,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    verify_cache_custody(
        root,
        CacheCustodyKind::LocalSnapshot,
        local_cache_custody_byte_limit(limits),
    )
}

fn verify_local_cache_root_custody(root: &Path) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(root, CacheCustodyKind::LocalSnapshot)
}

fn verify_cache_custody_root(
    root: &Path,
    kind: CacheCustodyKind,
) -> Result<(), SourceResolveError> {
    verify_cache_ancestry(kind, root)?;
    let metadata = std::fs::symlink_metadata(root).map_err(|error| io_error(root, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(cache_custody_invalid(
            kind,
            root,
            "cache custody root is not a concrete directory",
        ));
    }
    verify_cache_node_owner_and_mode(kind, root, &metadata)?;
    verify_macos_open_cache_directory_acl_custody(kind, root, &metadata)
}

#[cfg(unix)]
fn verify_cache_ancestry(kind: CacheCustodyKind, root: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    for ancestor in root.ancestors() {
        let metadata =
            std::fs::symlink_metadata(ancestor).map_err(|error| io_error(ancestor, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(cache_custody_invalid(
                kind,
                ancestor,
                "cache custody ancestry contains a non-directory or symlink",
            ));
        }
        if metadata.uid() != effective_user && metadata.uid() != 0 {
            return Err(cache_custody_invalid(
                kind,
                ancestor,
                "cache custody ancestry is owned by an unrelated user",
            ));
        }
        let mode = metadata.mode();
        if mode & 0o022 != 0 && mode & 0o1000 == 0 {
            return Err(cache_custody_invalid(
                kind,
                ancestor,
                "cache custody ancestry is externally writable without sticky-entry protection",
            ));
        }
        verify_macos_open_cache_directory_acl_custody(kind, ancestor, &metadata)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_cache_ancestry(_kind: CacheCustodyKind, _root: &Path) -> Result<(), SourceResolveError> {
    Ok(())
}

fn git_cache_custody_byte_limit(limits: LocalSourceLimits) -> u64 {
    limits
        .max_bytes
        .saturating_mul(3)
        .saturating_add(CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE)
        .min(GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT)
}

fn local_cache_custody_byte_limit(limits: LocalSourceLimits) -> u64 {
    limits
        .max_bytes
        .saturating_add(CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE)
        .min(LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT)
}

fn verify_cache_custody(
    root: &Path,
    kind: CacheCustodyKind,
    byte_limit: u64,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(root, kind)?;
    let root_directory = open_absolute_directory_nofollow(root)
        .map_err(|error| cache_custody_invalid(kind, root, error.to_string()))?;
    verify_cache_custody_from_open_root(root, root_directory, kind, byte_limit)
}

fn verify_cache_custody_from_open_root(
    root: &Path,
    root_directory: CapabilityDirectory,
    kind: CacheCustodyKind,
    byte_limit: u64,
) -> Result<(), SourceResolveError> {
    let root_metadata = root_directory
        .dir_metadata()
        .map_err(|error| io_error(root, error))?;
    let mut pending = vec![(PathBuf::new(), root.to_path_buf(), root_metadata, 0usize)];
    let mut observed = 0usize;
    let mut logical_bytes = 0u64;
    while let Some((relative_path, path, classified, depth)) = pending.pop() {
        observed = observed.checked_add(1).ok_or_else(|| {
            cache_custody_invalid(kind, &path, "cache custody entry count overflowed")
        })?;
        if observed > CACHE_CUSTODY_ENTRY_LIMIT {
            return Err(cache_custody_invalid(
                kind,
                root,
                format!(
                    "cache custody tree exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry metadata ceiling"
                ),
            ));
        }
        let directory = open_cache_custody_directory(
            &root_directory,
            &relative_path,
            &path,
            &classified,
            kind,
        )?;
        let metadata = directory
            .dir_metadata()
            .map_err(|error| io_error(&path, error))?;
        verify_capability_cache_node_owner_and_mode(kind, &path, &metadata)?;
        let directory_file = directory
            .try_clone()
            .map_err(|error| io_error(&path, error))?
            .into_std_file();
        verify_macos_open_cache_extended_acl_custody(kind, &path, &directory_file)?;

        let children = directory
            .entries()
            .map_err(|error| io_error(&path, error))?;
        for child in children {
            let child = child.map_err(|error| io_error(&path, error))?;
            let name = child.file_name();
            let child_path = path.join(&name);
            if !cache_custody_has_capacity(observed, pending.len()) {
                return Err(cache_custody_invalid(
                    kind,
                    root,
                    format!(
                        "cache custody tree exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry metadata ceiling"
                    ),
                ));
            }
            let metadata = directory
                .symlink_metadata(&name)
                .map_err(|error| io_error(&child_path, error))?;
            verify_capability_cache_node_owner_and_mode(kind, &child_path, &metadata)?;
            let file_type = metadata.file_type();
            if file_type.is_file() {
                verify_macos_open_cache_regular_file_acl_custody(
                    kind,
                    &child_path,
                    &directory,
                    &name,
                    &metadata,
                )?;
            } else if file_type.is_symlink() {
                verify_macos_cache_link_extended_acl_custody(kind, &child_path)?;
            }
            if file_type.is_file() || file_type.is_symlink() {
                logical_bytes = logical_bytes
                    .checked_add(metadata.len())
                    .filter(|bytes| *bytes <= byte_limit)
                    .ok_or_else(|| {
                        cache_custody_invalid(
                            kind,
                            root,
                            format!(
                                "cache custody tree exceeds its {byte_limit}-byte logical resident ceiling"
                            ),
                        )
                    })?;
                observed = observed.checked_add(1).ok_or_else(|| {
                    cache_custody_invalid(kind, &child_path, "cache custody entry count overflowed")
                })?;
            } else if file_type.is_dir() {
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    cache_custody_invalid(kind, &child_path, "cache custody depth overflowed")
                })?;
                if child_depth > CACHE_CUSTODY_DEPTH_LIMIT {
                    return Err(cache_custody_invalid(
                        kind,
                        &child_path,
                        format!(
                            "cache custody tree exceeds its {CACHE_CUSTODY_DEPTH_LIMIT}-level depth ceiling"
                        ),
                    ));
                }
                pending.push((relative_path.join(&name), child_path, metadata, child_depth));
            } else {
                return Err(cache_custody_invalid(
                    kind,
                    &child_path,
                    "cache custody contains an unsupported filesystem entry kind",
                ));
            }
            if observed > CACHE_CUSTODY_ENTRY_LIMIT {
                // The retained-entry check above should make this unreachable, but keep the
                // ceiling explicit if traversal accounting changes.
                return Err(cache_custody_invalid(
                    kind,
                    root,
                    format!(
                        "cache custody tree exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry metadata ceiling"
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn cache_custody_has_capacity(observed: usize, pending: usize) -> bool {
    observed
        .checked_add(pending)
        .is_some_and(|retained| retained < CACHE_CUSTODY_ENTRY_LIMIT)
}

#[cfg(test)]
fn publish_cache_directory(
    kind: CacheCustodyKind,
    parent: &Path,
    staged: &Path,
    publication: &Path,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(parent, kind)?;
    let directory = open_absolute_directory_nofollow(parent)
        .map_err(|error| cache_custody_invalid(kind, parent, error.to_string()))?;
    let staged_name = direct_cache_child_name(kind, parent, staged)?;
    let publication_name = direct_cache_child_name(kind, parent, publication)?;
    publish_cache_directory_from_open_parent(
        kind,
        parent,
        &directory,
        staged_name,
        publication_name,
        None,
    )
}

fn direct_cache_child_name<'a>(
    kind: CacheCustodyKind,
    parent: &Path,
    child: &'a Path,
) -> Result<&'a OsStr, SourceResolveError> {
    let relative = child.strip_prefix(parent).map_err(|_| {
        cache_custody_invalid(
            kind,
            child,
            "cache publication is outside its retained parent",
        )
    })?;
    let mut components = relative.components();
    let name = match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(name)), None) => name,
        _ => {
            return Err(cache_custody_invalid(
                kind,
                child,
                "cache publication is not a direct child of its retained parent",
            ));
        }
    };
    Ok(name)
}

fn retained_cache_directory_exists(
    kind: CacheCustodyKind,
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
) -> Result<bool, SourceResolveError> {
    match parent.symlink_metadata(name) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(
            cache_custody_invalid(kind, path, "cache entry is not a concrete directory"),
        ),
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

fn publish_cache_directory_from_open_parent(
    kind: CacheCustodyKind,
    parent: &Path,
    directory: &CapabilityDirectory,
    staged_name: &OsStr,
    publication_name: &OsStr,
    expected_staged: Option<&CapabilityMetadata>,
) -> Result<(), SourceResolveError> {
    let staged_path = parent.join(staged_name);
    let publication_path = parent.join(publication_name);
    let staged_metadata = directory
        .symlink_metadata(staged_name)
        .map_err(|error| io_error(&staged_path, error))?;
    if staged_metadata.file_type().is_symlink() || !staged_metadata.is_dir() {
        return Err(cache_custody_invalid(
            kind,
            &staged_path,
            "cache publication stage is not a concrete directory",
        ));
    }
    if expected_staged
        .is_some_and(|expected| !same_capability_file_identity(expected, &staged_metadata))
    {
        return Err(cache_custody_invalid(
            kind,
            &staged_path,
            "cache publication stage no longer identifies the retained directory",
        ));
    }
    match directory.symlink_metadata(publication_name) {
        Ok(_) => {
            return Err(cache_custody_invalid(
                kind,
                &publication_path,
                "cache publication destination already exists",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&publication_path, error)),
    }

    directory
        .rename(staged_name, directory, publication_name)
        .map_err(|error| io_error(&publication_path, error))?;
    let published_metadata = directory
        .symlink_metadata(publication_name)
        .map_err(|error| io_error(&publication_path, error))?;
    if !published_metadata.is_dir()
        || !same_capability_file_identity(&staged_metadata, &published_metadata)
    {
        return Err(cache_custody_invalid(
            kind,
            &publication_path,
            "published cache directory does not identify the staged directory",
        ));
    }
    directory
        .try_clone()
        .map_err(|error| io_error(parent, error))?
        .into_std_file()
        .sync_all()
        .map_err(|error| io_error(parent, error))?;
    Ok(())
}

fn open_cache_custody_directory(
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_path: &Path,
    classified: &CapabilityMetadata,
    kind: CacheCustodyKind,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let mut directory = root
        .try_clone()
        .map_err(|error| io_error(display_path, error))?;
    for component in relative_path.components() {
        use std::path::Component;

        let Component::Normal(name) = component else {
            return Err(cache_custody_invalid(
                kind,
                display_path,
                "cache custody queued a noncanonical relative directory path",
            ));
        };
        directory = directory
            .open_dir_nofollow(name)
            .map_err(|error| cache_custody_invalid(kind, display_path, error.to_string()))?;
    }
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(display_path, error))?;
    if !opened.is_dir() || !same_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            display_path,
            "cache directory changed between classification and no-follow open",
        ));
    }
    Ok(directory)
}

fn same_capability_file_identity(left: &CapabilityMetadata, right: &CapabilityMetadata) -> bool {
    use cap_fs_ext::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(unix)]
fn verify_capability_cache_node_owner_and_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    if metadata.uid() != effective_user {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache entry is not owned by the resolver's effective user",
        ));
    }
    if !metadata.file_type().is_symlink() && metadata.mode() & 0o022 != 0 {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache entry is writable by group or other users",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_capability_cache_node_owner_and_mode(
    _kind: CacheCustodyKind,
    _path: &Path,
    _metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
fn verify_cache_node_owner_and_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    if metadata.uid() != effective_user {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache entry is not owned by the resolver's effective user",
        ));
    }
    if !metadata.file_type().is_symlink() && metadata.mode() & 0o022 != 0 {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache entry is writable by group or other users",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_cache_node_owner_and_mode(
    _kind: CacheCustodyKind,
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    // Windows ownership/DACL enforcement belongs to the native isolation
    // backend. The portable floor still checks concrete kinds and bounded
    // topology instead of asking Git to describe its own cache.
    Ok(())
}

fn cache_custody_invalid(
    kind: CacheCustodyKind,
    path: &Path,
    message: impl Into<String>,
) -> SourceResolveError {
    match kind {
        CacheCustodyKind::Git => cache_invalid(path, message),
        CacheCustodyKind::LocalSnapshot => local_snapshot_invalid(path, message),
    }
}

#[cfg(target_os = "macos")]
fn verify_macos_cache_link_extended_acl_custody(
    kind: CacheCustodyKind,
    path: &Path,
) -> Result<(), SourceResolveError> {
    let has_allow_entry = omega_platform_custody::extended_acl_has_allow_entry(
        path,
        omega_platform_custody::SymbolicLinkBehavior::InspectLink,
    )
    .map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not inspect cache symbolic-link extended ACL custody: {error}"),
        )
    })?;
    if has_allow_entry {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache custody contains an extended ACL allow entry",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_macos_open_cache_extended_acl_custody(
    kind: CacheCustodyKind,
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    let has_allow_entry = omega_platform_custody::open_file_extended_acl_has_allow_entry(file)
        .map_err(|error| {
            cache_custody_invalid(
                kind,
                path,
                format!("could not inspect retained cache extended ACL custody: {error}"),
            )
        })?;
    if has_allow_entry {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache custody contains an extended ACL allow entry",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_macos_open_cache_directory_acl_custody(
    kind: CacheCustodyKind,
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let directory = open_absolute_directory_nofollow(path).map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not retain cache custody directory: {error}"),
        )
    })?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !opened.is_dir() || !same_std_and_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache custody directory changed between classification and no-follow open",
        ));
    }
    verify_macos_open_cache_extended_acl_custody(
        kind,
        path,
        &directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file(),
    )
}

#[cfg(not(target_os = "macos"))]
fn verify_macos_open_cache_directory_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn verify_macos_open_cache_extended_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _file: &File,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_macos_open_cache_regular_file_acl_custody(
    kind: CacheCustodyKind,
    path: &Path,
    parent: &CapabilityDirectory,
    name: &OsStr,
    classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options).map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not open cache file without following links: {error}"),
        )
    })?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if !opened.is_file() || !same_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache file changed between classification and no-follow open",
        ));
    }
    verify_macos_open_cache_extended_acl_custody(kind, path, &file.into_std())
}

#[cfg(not(target_os = "macos"))]
fn verify_macos_open_cache_regular_file_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _parent: &CapabilityDirectory,
    _name: &OsStr,
    _classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn verify_macos_cache_link_extended_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
) -> Result<(), SourceResolveError> {
    Ok(())
}

struct CacheEntryLock {
    file: File,
    parent: CapabilityDirectory,
    kind: CacheCustodyKind,
    path: PathBuf,
    lock_name: OsString,
}

impl CacheEntryLock {
    fn open_retained(
        kind: CacheCustodyKind,
        path: &Path,
    ) -> Result<(File, CapabilityDirectory, OsString), SourceResolveError> {
        let parent_path = path.parent().ok_or_else(|| {
            cache_custody_invalid(kind, path, "cache lock has no publication parent")
        })?;
        verify_cache_custody_root(parent_path, kind)?;
        let parent = open_absolute_directory_nofollow(parent_path)
            .map_err(|error| cache_custody_invalid(kind, parent_path, error.to_string()))?;
        let lock_name = direct_cache_child_name(kind, parent_path, path)?.to_os_string();
        let mut options = CapabilityOpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .follow(FollowSymlinks::No);
        #[cfg(unix)]
        options.mode(0o600);
        let capability_file = parent.open_with(&lock_name, &options).map_err(|error| {
            cache_custody_invalid(
                kind,
                path,
                format!("could not open cache lock without following links: {error}"),
            )
        })?;
        let handle_metadata = capability_file
            .metadata()
            .map_err(|error| io_error(path, error))?;
        let path_metadata = parent
            .symlink_metadata(&lock_name)
            .map_err(|error| io_error(path, error))?;
        if !handle_metadata.is_file()
            || path_metadata.file_type().is_symlink()
            || !path_metadata.is_file()
            || !same_capability_file_identity(&handle_metadata, &path_metadata)
        {
            return Err(cache_custody_invalid(
                kind,
                path,
                "cache lock is not a stable regular file beneath its retained parent",
            ));
        }
        verify_capability_cache_node_owner_and_mode(kind, path, &path_metadata)?;
        let file = capability_file.into_std();
        verify_macos_open_cache_extended_acl_custody(kind, path, &file)?;
        Ok((file, parent, lock_name))
    }

    #[cfg(test)]
    fn open_git(path: &Path) -> Result<File, SourceResolveError> {
        let (file, _, _) = Self::open_retained(CacheCustodyKind::Git, path)?;
        Ok(file)
    }

    fn acquire_with_git_budget(
        path: &Path,
        executor: &GitExecutor,
    ) -> Result<Self, SourceResolveError> {
        let (file, parent, lock_name) = Self::open_retained(CacheCustodyKind::Git, path)?;
        loop {
            executor.verify_budget()?;
            match file.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) => {
                    let remaining = executor.remaining_time()?;
                    std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(io_error(path, error));
                }
            }
        }
        if let Err(error) = executor.verify_budget() {
            let _ = file.unlock();
            return Err(error);
        }
        verify_cache_lock_path_identity(CacheCustodyKind::Git, path, &parent, &lock_name, &file)?;
        Ok(Self {
            file,
            parent,
            kind: CacheCustodyKind::Git,
            path: path.to_path_buf(),
            lock_name,
        })
    }

    #[cfg(test)]
    fn acquire(path: &Path) -> Result<Self, SourceResolveError> {
        let (file, parent, lock_name) = Self::open_retained(CacheCustodyKind::Git, path)?;
        file.lock().map_err(|error| io_error(path, error))?;
        verify_cache_lock_path_identity(CacheCustodyKind::Git, path, &parent, &lock_name, &file)?;
        Ok(Self {
            file,
            parent,
            kind: CacheCustodyKind::Git,
            path: path.to_path_buf(),
            lock_name,
        })
    }

    fn acquire_local(path: &Path) -> Result<Self, SourceResolveError> {
        Self::acquire_local_with_timeout(path, LOCAL_SNAPSHOT_LOCK_TIMEOUT)
    }

    fn acquire_local_with_timeout(
        path: &Path,
        timeout: Duration,
    ) -> Result<Self, SourceResolveError> {
        let (file, parent, lock_name) = Self::open_retained(CacheCustodyKind::LocalSnapshot, path)?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) => {
                    let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                        return Err(local_snapshot_lock_timed_out(path, timeout));
                    };
                    if remaining.is_zero() {
                        return Err(local_snapshot_lock_timed_out(path, timeout));
                    }
                    std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(io_error(path, error));
                }
            }
        }
        if started.elapsed() >= timeout {
            let _ = file.unlock();
            return Err(local_snapshot_lock_timed_out(path, timeout));
        }
        verify_cache_lock_path_identity(
            CacheCustodyKind::LocalSnapshot,
            path,
            &parent,
            &lock_name,
            &file,
        )?;
        Ok(Self {
            file,
            parent,
            kind: CacheCustodyKind::LocalSnapshot,
            path: path.to_path_buf(),
            lock_name,
        })
    }

    fn parent(&self) -> &CapabilityDirectory {
        &self.parent
    }

    fn verify_path_identity(&self) -> Result<(), SourceResolveError> {
        verify_cache_lock_path_identity(
            self.kind,
            &self.path,
            &self.parent,
            &self.lock_name,
            &self.file,
        )
    }
}

fn local_snapshot_lock_timed_out(path: &Path, timeout: Duration) -> SourceResolveError {
    SourceResolveError::LocalSnapshotLockTimedOut {
        path: path.to_path_buf(),
        timeout_millis: u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
    }
}

fn verify_cache_lock_path_identity(
    kind: CacheCustodyKind,
    path: &Path,
    parent: &CapabilityDirectory,
    lock_name: &OsStr,
    file: &File,
) -> Result<(), SourceResolveError> {
    let path_metadata = parent
        .symlink_metadata(lock_name)
        .map_err(|error| io_error(path, error))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache lock was replaced while being acquired",
        ));
    }
    let handle_metadata = file.metadata().map_err(|error| io_error(path, error))?;
    if !handle_metadata.is_file()
        || !same_std_and_capability_file_identity(&handle_metadata, &path_metadata)
    {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache lock path does not identify the locked file",
        ));
    }
    verify_capability_cache_node_owner_and_mode(kind, path, &path_metadata)?;
    verify_macos_open_cache_extended_acl_custody(kind, path, file)?;

    let parent_path = path
        .parent()
        .ok_or_else(|| cache_custody_invalid(kind, path, "cache lock has no publication parent"))?;
    verify_retained_cache_parent_path(kind, parent_path, parent)
}

fn verify_retained_cache_parent_path(
    kind: CacheCustodyKind,
    parent_path: &Path,
    retained_parent: &CapabilityDirectory,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(parent_path, kind)?;
    let current_parent = open_absolute_directory_nofollow(parent_path)
        .map_err(|error| cache_custody_invalid(kind, parent_path, error.to_string()))?;
    let retained_metadata = retained_parent
        .dir_metadata()
        .map_err(|error| io_error(parent_path, error))?;
    let current_metadata = current_parent
        .dir_metadata()
        .map_err(|error| io_error(parent_path, error))?;
    if !same_capability_file_identity(&retained_metadata, &current_metadata) {
        return Err(cache_custody_invalid(
            kind,
            parent_path,
            "cache parent pathname no longer identifies the retained directory",
        ));
    }
    Ok(())
}

fn same_std_and_capability_file_identity(
    left: &std::fs::Metadata,
    right: &CapabilityMetadata,
) -> bool {
    use cap_fs_ext::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(test)]
fn verify_cache_lock_path_identity_for_test(
    kind: CacheCustodyKind,
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    let parent_path = path.parent().expect("test cache lock has a parent");
    let canonical_parent = parent_path
        .canonicalize()
        .map_err(|error| io_error(parent_path, error))?;
    let lock_name = path.file_name().expect("test cache lock has a name");
    let canonical_path = canonical_parent.join(lock_name);
    let parent = open_absolute_directory_nofollow(&canonical_parent)
        .map_err(|error| io_error(&canonical_parent, error))?;
    verify_cache_lock_path_identity(kind, &canonical_path, &parent, lock_name, file)
}

impl Drop for CacheEntryLock {
    fn drop(&mut self) {
        // Keep the inode in place: unlinking a lock file lets a waiter lock the old inode while a
        // newcomer locks a replacement. Closing this handle releases the advisory lock safely.
        let _ = self.file.unlock();
    }
}

struct PendingCacheEntry {
    root: PathBuf,
    parent: CapabilityDirectory,
    directory: Option<CapabilityDirectory>,
    stage_name: OsString,
    published: bool,
}

impl PendingCacheEntry {
    fn create(
        cache_dir: &Path,
        cache_directory: &CapabilityDirectory,
        cache_identity: &str,
    ) -> Result<Self, SourceResolveError> {
        let parent = cache_directory
            .try_clone()
            .map_err(|error| io_error(cache_dir, error))?;
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let stage_name = OsString::from(format!(
                ".git-{cache_identity}.stage-{}-{sequence}",
                std::process::id()
            ));
            let root = cache_dir.join(&stage_name);
            match create_private_cache_directory(&parent, &stage_name) {
                Ok(()) => {
                    let provisional = ProvisionalCacheDirectory::new(&parent, &stage_name);
                    let directory = retain_private_cache_directory(
                        CacheCustodyKind::Git,
                        &parent,
                        &stage_name,
                        &root,
                    )?;
                    provisional.disarm();
                    return Ok(Self {
                        root,
                        parent,
                        directory: Some(directory),
                        stage_name,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&root, error)),
            }
        }
        Err(cache_invalid(
            cache_dir,
            "could not allocate a unique Git cache staging directory",
        ))
    }

    fn directory(&self) -> Result<&CapabilityDirectory, SourceResolveError> {
        self.directory
            .as_ref()
            .ok_or_else(|| cache_invalid(&self.root, "Git cache stage handle is absent"))
    }

    fn create_private_directory(&self, name: &str, path: &Path) -> Result<(), SourceResolveError> {
        let directory = self.directory()?;
        create_private_cache_directory(directory, name).map_err(|error| io_error(path, error))?;
        let provisional = ProvisionalCacheDirectory::new(directory, OsStr::new(name));
        retain_private_cache_directory(CacheCustodyKind::Git, directory, OsStr::new(name), path)?;
        provisional.disarm();
        Ok(())
    }

    fn verify_path_identity(&self) -> Result<CapabilityMetadata, SourceResolveError> {
        let retained = self
            .directory()?
            .dir_metadata()
            .map_err(|error| io_error(&self.root, error))?;
        let named = self
            .parent
            .symlink_metadata(&self.stage_name)
            .map_err(|error| io_error(&self.root, error))?;
        if !named.is_dir() || !same_capability_file_identity(&retained, &named) {
            return Err(cache_invalid(
                &self.root,
                "Git cache stage pathname no longer identifies the retained directory",
            ));
        }
        Ok(retained)
    }

    fn verify_parent_path_identity(&self, cache_dir: &Path) -> Result<(), SourceResolveError> {
        verify_retained_cache_parent_path(CacheCustodyKind::Git, cache_dir, &self.parent)
    }

    fn verify_ambient_path_identity(&self, cache_dir: &Path) -> Result<(), SourceResolveError> {
        self.verify_parent_path_identity(cache_dir)?;
        self.verify_path_identity().map(|_| ())
    }

    fn publish(
        &mut self,
        cache_dir: &Path,
        entry_root: &Path,
        entry_name: &OsStr,
    ) -> Result<(), SourceResolveError> {
        let retained = self.verify_path_identity()?;
        publish_cache_directory_from_open_parent(
            CacheCustodyKind::Git,
            cache_dir,
            &self.parent,
            &self.stage_name,
            entry_name,
            Some(&retained),
        )?;
        let published = self
            .parent
            .symlink_metadata(entry_name)
            .map_err(|error| io_error(entry_root, error))?;
        if !same_capability_file_identity(&retained, &published) {
            return Err(cache_invalid(
                entry_root,
                "published Git cache entry does not identify the retained stage",
            ));
        }
        self.published = true;
        Ok(())
    }
}

struct ProvisionalCacheDirectory<'a> {
    parent: &'a CapabilityDirectory,
    name: &'a OsStr,
    armed: bool,
}

impl<'a> ProvisionalCacheDirectory<'a> {
    fn new(parent: &'a CapabilityDirectory, name: &'a OsStr) -> Self {
        Self {
            parent,
            name,
            armed: true,
        }
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for ProvisionalCacheDirectory<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.parent.remove_dir_all(self.name);
        }
    }
}

impl Drop for PendingCacheEntry {
    fn drop(&mut self) {
        if !self.published
            && let Some(directory) = self.directory.take()
        {
            make_open_tree_owner_writable(&directory);
            let _ = directory.remove_open_dir_all();
        }
    }
}

fn create_private_cache_directory(
    parent: &CapabilityDirectory,
    name: impl AsRef<Path>,
) -> std::io::Result<()> {
    #[cfg(not(target_os = "wasi"))]
    {
        let mut builder = CapabilityDirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        parent.create_dir_with(name, &builder)
    }
    #[cfg(target_os = "wasi")]
    {
        parent.create_dir(name)
    }
}

fn retain_private_cache_directory(
    kind: CacheCustodyKind,
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let classified = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    let directory = parent
        .open_dir_nofollow(name)
        .map_err(|error| cache_custody_invalid(kind, path, error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file()
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .map_err(|error| io_error(path, error))?;
    }
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !classified.is_dir() || !same_capability_file_identity(&classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "private cache directory changed while being retained",
        ));
    }
    verify_capability_cache_node_owner_and_mode(kind, path, &opened)?;
    #[cfg(unix)]
    {
        use cap_fs_ext::OsMetadataExt;

        if opened.mode() & 0o777 != 0o700 {
            return Err(cache_custody_invalid(
                kind,
                path,
                "private cache directory does not have exact mode 0700",
            ));
        }
    }
    verify_macos_open_cache_extended_acl_custody(
        kind,
        path,
        &directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file(),
    )?;
    Ok(directory)
}

#[derive(Debug)]
struct SourceEntry {
    relative_bytes: Vec<u8>,
    relative_path: PathBuf,
    kind: SourceEntryKind,
}

#[derive(Debug)]
enum SourceEntryKind {
    Directory,
    File { bytes: Vec<u8>, executable: bool },
    Symlink { target_bytes: Vec<u8> },
}

#[derive(Debug)]
struct CapturedLocalTree {
    normalized: ResolvedLocalSource,
    entries: Vec<CapturedLocalEntry>,
}

#[derive(Debug)]
struct CapturedLocalEntry {
    relative_path: PathBuf,
    relative_bytes: Vec<u8>,
    kind: CapturedLocalEntryKind,
}

#[derive(Debug)]
enum CapturedLocalEntryKind {
    Directory,
    File { bytes: Vec<u8>, executable: bool },
    Symlink { target_bytes: Vec<u8> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceTreePolicy {
    /// Mutable local package roots omit only paths reserved for resolver or compiler output.
    LocalPackage,
    /// Resolver-owned materializations must be hashed exactly as published.
    ExactMaterialized,
}

#[cfg(test)]
fn resolve_materialized_source(
    root: &Path,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    Ok(capture_local_source(root, limits, SourceTreePolicy::ExactMaterialized)?.normalized)
}

fn capture_local_source(
    requested_root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
) -> Result<CapturedLocalTree, SourceResolveError> {
    let root = requested_root
        .canonicalize()
        .map_err(|error| io_error(requested_root, error))?;
    if !root.is_dir() {
        return Err(SourceResolveError::NotDirectory { path: root });
    }

    let root_directory = open_canonical_source_root(&root)?;
    capture_local_source_from_open_root(root, root_directory, limits, policy)
}

fn open_canonical_source_root(
    canonical_root: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let directory = open_absolute_directory_nofollow(canonical_root)
        .map_err(|error| io_error(canonical_root, error))?;
    let metadata = directory
        .dir_metadata()
        .map_err(|error| io_error(canonical_root, error))?;
    if !metadata.is_dir() {
        return Err(SourceResolveError::NotDirectory {
            path: canonical_root.to_path_buf(),
        });
    }
    Ok(directory)
}

fn open_absolute_directory_nofollow(
    canonical_root: &Path,
) -> Result<CapabilityDirectory, std::io::Error> {
    use std::path::Component;

    let mut anchor = PathBuf::new();
    let mut relative_components = Vec::new();
    for component in canonical_root.components() {
        match component {
            Component::Prefix(prefix) => anchor.push(prefix.as_os_str()),
            Component::RootDir => anchor.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(name) => relative_components.push(name.to_os_string()),
            Component::ParentDir => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "canonical directory contains a parent component",
                ));
            }
        }
    }
    if anchor.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "canonical directory is not absolute",
        ));
    }

    let mut directory = CapabilityDirectory::open_ambient_dir(&anchor, ambient_authority())?;
    for component in relative_components {
        directory = directory.open_dir_nofollow(&component)?;
    }
    let metadata = directory.dir_metadata()?;
    if !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "opened path is not a directory",
        ));
    }
    Ok(directory)
}

fn capture_local_source_from_open_root(
    root: PathBuf,
    root_directory: CapabilityDirectory,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
) -> Result<CapturedLocalTree, SourceResolveError> {
    let mut source_entries = Vec::new();
    let mut captured_file_bytes = 0_u64;
    visit_directory(
        &root_directory,
        &root_directory,
        &root,
        PathBuf::new(),
        0,
        &root,
        limits,
        policy,
        &mut captured_file_bytes,
        &mut source_entries,
    )?;
    source_entries.sort_by(|left, right| left.relative_bytes.cmp(&right.relative_bytes));

    let mut identity = SourceIdentityHasher::new(source_entries.len());
    let mut file_count = 0;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(source_entries.len())
        .map_err(|_| SourceResolveError::TooManyFiles {
            limit: limits.max_files,
        })?;
    for entry in source_entries {
        let kind = match entry.kind {
            SourceEntryKind::Directory => {
                identity.add_directory(&entry.relative_bytes, CANONICAL_DIRECTORY_MODE);
                CapturedLocalEntryKind::Directory
            }
            SourceEntryKind::File { bytes, executable } => {
                identity.add_file(&entry.relative_bytes, executable, &bytes)?;
                file_count += 1;
                CapturedLocalEntryKind::File { bytes, executable }
            }
            SourceEntryKind::Symlink { target_bytes } => {
                identity.add_symlink(&entry.relative_bytes, &target_bytes);
                file_count += 1;
                CapturedLocalEntryKind::Symlink { target_bytes }
            }
        };
        entries.push(CapturedLocalEntry {
            relative_path: entry.relative_path,
            relative_bytes: entry.relative_bytes,
            kind,
        });
    }
    let (byte_count, content_identity) = identity.finish();
    Ok(CapturedLocalTree {
        normalized: ResolvedLocalSource {
            root,
            file_count,
            byte_count,
            content_identity,
        },
        entries,
    })
}

fn publish_local_snapshot(
    requested_root: PathBuf,
    captured: CapturedLocalTree,
    cache_dir: &Path,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSnapshot, SourceResolveError> {
    let canonical_cache_dir =
        validate_local_snapshot_topology(&captured.normalized.root, cache_dir)?;
    std::fs::create_dir_all(&canonical_cache_dir)
        .map_err(|error| io_error(&canonical_cache_dir, error))?;
    let snapshots = canonical_cache_dir.join(LOCAL_CACHE_SNAPSHOTS);
    std::fs::create_dir_all(&snapshots).map_err(|error| io_error(&snapshots, error))?;
    verify_local_cache_root_custody(&canonical_cache_dir)?;
    verify_local_cache_root_custody(&snapshots)?;

    let identity = captured.normalized.content_identity.clone();
    let custody_identity = local_snapshot_custody_identity(
        &captured.normalized.root,
        &captured.normalized.content_identity,
    );
    let publication = snapshots.join(format!("source-{custody_identity}"));
    let lock_path = snapshots.join(format!("source-{custody_identity}.lock"));
    let _entry_lock = CacheEntryLock::acquire_local(&lock_path)?;

    let normalized = if publication.exists() {
        let normalized = verify_local_snapshot(&publication, &identity, limits)?;
        verify_live_source_unchanged(&captured.normalized, limits)?;
        normalized
    } else {
        materialize_local_snapshot(&snapshots, &publication, &captured, limits)?
    };

    verify_local_cache_root_custody(&canonical_cache_dir)?;
    verify_local_cache_root_custody(&snapshots)?;
    verify_local_cache_custody(&publication, limits)?;
    Ok(ResolvedLocalSnapshot {
        requested_root,
        canonical_live_root: captured.normalized.root,
        snapshot_root: normalized.root.clone(),
        normalized,
    })
}

fn validate_local_snapshot_topology(
    canonical_live_root: &Path,
    cache_dir: &Path,
) -> Result<PathBuf, SourceResolveError> {
    let canonical_cache_dir = canonicalize_prospective_path(cache_dir)?;
    let snapshot_collection =
        canonicalize_prospective_path(&canonical_cache_dir.join(LOCAL_CACHE_SNAPSHOTS))?;
    if canonical_cache_dir.starts_with(canonical_live_root)
        || canonical_live_root.starts_with(&snapshot_collection)
    {
        return Err(SourceResolveError::LocalSnapshotCacheOverlapsSource {
            canonical_live_root: canonical_live_root.to_path_buf(),
            canonical_cache_dir,
        });
    }
    Ok(canonical_cache_dir)
}

fn local_snapshot_custody_identity(canonical_live_root: &Path, content_identity: &str) -> String {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, LOCAL_SNAPSHOT_CUSTODY_POLICY);
    hash_bytes(
        &mut hasher,
        raw_os_bytes(canonical_live_root.as_os_str()).as_slice(),
    );
    hash_bytes(&mut hasher, content_identity.as_bytes());
    format_sha256(&hasher.finalize())
}

fn canonicalize_prospective_path(path: &Path) -> Result<PathBuf, SourceResolveError> {
    let mut existing = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| io_error(Path::new("."), error))?
            .join(path)
    };
    let mut suffix = Vec::<OsString>::new();
    loop {
        match std::fs::symlink_metadata(&existing) {
            Ok(_) => {
                let canonical = existing
                    .canonicalize()
                    .map_err(|error| io_error(&existing, error))?;
                let mut result = canonical;
                for component in suffix.into_iter().rev() {
                    if component == "." {
                        continue;
                    }
                    if component == ".." {
                        result.pop();
                    } else {
                        result.push(component);
                    }
                }
                return Ok(result);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(component) = existing.file_name().map(OsStr::to_os_string) else {
                    return Err(io_error(&existing, error));
                };
                suffix.push(component);
                existing.pop();
            }
            Err(error) => return Err(io_error(&existing, error)),
        }
    }
}

fn materialize_local_snapshot(
    snapshots: &Path,
    publication: &Path,
    captured: &CapturedLocalTree,
    limits: LocalSourceLimits,
) -> Result<ResolvedLocalSource, SourceResolveError> {
    let identity = &captured.normalized.content_identity;
    let mut pending = PendingMaterializedSnapshot::create(
        CacheCustodyKind::LocalSnapshot,
        snapshots,
        &format!(".source-{identity}.stage"),
    )?;
    let source = pending.root.join(LOCAL_SNAPSHOT_SOURCE);
    pending
        .directory()?
        .create_dir(LOCAL_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;
    let source_directory = pending
        .directory()?
        .open_dir_nofollow(LOCAL_SNAPSHOT_SOURCE)
        .map_err(|error| io_error(&source, error))?;

    for entry in &captured.entries {
        match &entry.kind {
            CapturedLocalEntryKind::Directory => {
                open_or_create_snapshot_directory(
                    CacheCustodyKind::LocalSnapshot,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                )?;
            }
            CapturedLocalEntryKind::File { bytes, executable } => {
                write_snapshot_file_from_open_root(
                    CacheCustodyKind::LocalSnapshot,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    bytes,
                    *executable,
                )?;
            }
            CapturedLocalEntryKind::Symlink { target_bytes } => {
                create_snapshot_symlink_from_open_root(
                    CacheCustodyKind::LocalSnapshot,
                    &source_directory,
                    &entry.relative_path,
                    &source,
                    target_bytes,
                )?;
            }
        }
    }

    let staged = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if !same_source_identity(&staged, &captured.normalized) {
        return Err(local_snapshot_invalid(
            &source,
            "staged source does not match the captured local tree",
        ));
    }
    verify_live_source_unchanged(&captured.normalized, limits)?;

    write_snapshot_file_from_open_root(
        CacheCustodyKind::LocalSnapshot,
        pending.directory()?,
        Path::new(LOCAL_SNAPSHOT_METADATA),
        &pending.root,
        &local_snapshot_metadata(&staged),
        false,
    )?;
    make_open_snapshot_read_only(
        CacheCustodyKind::LocalSnapshot,
        pending.directory()?,
        &pending.root,
    )?;
    let finalized = capture_local_source_from_open_root(
        source.clone(),
        source_directory
            .try_clone()
            .map_err(|error| io_error(&source, error))?,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if !same_source_identity(&finalized, &captured.normalized) {
        return Err(local_snapshot_invalid(
            &source,
            "finalized snapshot does not match the captured local tree",
        ));
    }
    pending.publish(snapshots, publication)?;
    verify_local_snapshot(publication, identity, limits)
}

fn verify_live_source_unchanged(
    captured: &ResolvedLocalSource,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    let current = resolve_local_source(&captured.root, limits).map_err(|_| {
        SourceResolveError::LocalSourceChanged {
            path: captured.root.clone(),
        }
    })?;
    if !same_source_identity(&current, captured) {
        return Err(SourceResolveError::LocalSourceChanged {
            path: captured.root.clone(),
        });
    }
    Ok(())
}

fn same_source_identity(left: &ResolvedLocalSource, right: &ResolvedLocalSource) -> bool {
    left.file_count == right.file_count
        && left.byte_count == right.byte_count
        && left.content_identity == right.content_identity
}

fn visit_directory(
    root_directory: &CapabilityDirectory,
    directory: &CapabilityDirectory,
    display_dir: &Path,
    logical_dir: PathBuf,
    depth: usize,
    root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    captured_file_bytes: &mut u64,
    entries: &mut Vec<SourceEntry>,
) -> Result<(), SourceResolveError> {
    if depth > limits.max_depth {
        return Err(SourceResolveError::TooDeep {
            path: display_dir.to_path_buf(),
            limit: limits.max_depth,
        });
    }

    let remaining_entries = limits.max_files.saturating_sub(entries.len());
    let excluded_entry_allowance = match policy {
        SourceTreePolicy::ExactMaterialized => 0,
        SourceTreePolicy::LocalPackage if logical_dir.as_os_str().is_empty() => 2,
        SourceTreePolicy::LocalPackage => 1,
    };
    let directory_listing_limit = remaining_entries.saturating_add(excluded_entry_allowance);
    let mut entry_names = Vec::new();
    for entry in directory
        .entries()
        .map_err(|error| io_error(display_dir, error))?
    {
        if entry_names.len() >= directory_listing_limit {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }
        entry_names.push(
            entry
                .map_err(|error| io_error(display_dir, error))?
                .file_name(),
        );
    }
    entry_names.sort();

    for name in entry_names {
        if policy == SourceTreePolicy::LocalPackage
            && (name == ".git"
                || (logical_dir.as_os_str().is_empty() && name == DEFAULT_BUILD_OUTPUT_DIRECTORY))
        {
            continue;
        }
        if entries.len() >= limits.max_files {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }
        let display_path = display_dir.join(&name);
        let logical_path = logical_dir.join(&name);
        let metadata = directory
            .symlink_metadata(&name)
            .map_err(|error| io_error(&display_path, error))?;
        if metadata.file_type().is_symlink() {
            let raw_target = read_and_validate_symlink_target(
                root_directory,
                root,
                directory,
                &logical_dir,
                &name,
                &display_path,
                policy,
            )?;
            push_entry(
                entries,
                logical_path,
                SourceEntryKind::Symlink {
                    target_bytes: raw_os_bytes(raw_target.as_os_str()),
                },
                limits,
            )?;
        } else if metadata.is_dir() {
            let child = open_captured_directory(directory, &name, &display_path)?;
            push_entry(
                entries,
                logical_path.clone(),
                SourceEntryKind::Directory,
                limits,
            )?;
            visit_directory(
                root_directory,
                &child,
                &display_path,
                logical_path,
                depth + 1,
                root,
                limits,
                policy,
                captured_file_bytes,
                entries,
            )?;
        } else if metadata.is_file() {
            let remaining = limits.max_bytes.checked_sub(*captured_file_bytes).ok_or(
                SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                },
            )?;
            let (bytes, executable) = read_capability_file_bounded(
                directory,
                &name,
                &display_path,
                remaining,
                limits.max_bytes,
            )?;
            *captured_file_bytes = captured_file_bytes.checked_add(bytes.len() as u64).ok_or(
                SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                },
            )?;
            push_entry(
                entries,
                logical_path,
                SourceEntryKind::File { bytes, executable },
                limits,
            )?;
        } else {
            return Err(SourceResolveError::UnsupportedFileType { path: display_path });
        }
    }
    Ok(())
}

fn read_and_validate_symlink_target(
    root_directory: &CapabilityDirectory,
    root: &Path,
    directory: &CapabilityDirectory,
    logical_directory: &Path,
    name: &OsStr,
    link: &Path,
    policy: SourceTreePolicy,
) -> Result<PathBuf, SourceResolveError> {
    // Package-local policy hashes link spelling, requires an existing canonical target inside this
    // root, and rejects targets under paths excluded from that package view. Exact resolver-owned
    // materializations have no exclusions. Target contents are visited independently through the
    // ordinary tree walk rather than dereferenced through the link.
    let raw_target = directory
        .read_link_contents(name)
        .map_err(|error| io_error(link, error))?;
    if raw_target.is_absolute() {
        return Err(SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target: raw_target,
        });
    }
    let target_request = logical_directory.join(&raw_target);
    let target_display = root.join(&target_request);
    let relative_target = root_directory.canonicalize(&target_request).map_err(|_| {
        SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target: target_display,
        }
    })?;
    if policy == SourceTreePolicy::LocalPackage
        && relative_target
            .components()
            .any(|component| component.as_os_str() == ".git")
    {
        return Err(SourceResolveError::SymlinkTargetsExcludedMetadata {
            link: link.to_path_buf(),
            target: root.join(&relative_target),
        });
    }
    if policy == SourceTreePolicy::LocalPackage
        && relative_target
            .components()
            .next()
            .is_some_and(|component| component.as_os_str() == DEFAULT_BUILD_OUTPUT_DIRECTORY)
    {
        return Err(SourceResolveError::SymlinkTargetsExcludedBuildOutput {
            link: link.to_path_buf(),
            target: root.join(&relative_target),
        });
    }
    Ok(raw_target)
}

fn push_entry(
    entries: &mut Vec<SourceEntry>,
    relative: PathBuf,
    kind: SourceEntryKind,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    if entries.len() >= limits.max_files {
        return Err(SourceResolveError::TooManyFiles {
            limit: limits.max_files,
        });
    }
    entries.push(SourceEntry {
        relative_bytes: raw_os_bytes(relative.as_os_str()),
        relative_path: relative,
        kind,
    });
    Ok(())
}

fn open_captured_directory(
    directory: &CapabilityDirectory,
    name: &OsStr,
    display_path: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let child = directory
        .open_dir_nofollow(name)
        .map_err(|error| io_error(display_path, error))?;
    let metadata = child
        .dir_metadata()
        .map_err(|error| io_error(display_path, error))?;
    if !metadata.is_dir() {
        return Err(SourceResolveError::UnsupportedFileType {
            path: display_path.to_path_buf(),
        });
    }
    Ok(child)
}

fn read_capability_file_bounded(
    directory: &CapabilityDirectory,
    name: &OsStr,
    display_path: &Path,
    remaining: u64,
    limit: u64,
) -> Result<(Vec<u8>, bool), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = directory
        .open_with(name, &options)
        .map_err(|error| io_error(display_path, error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error(display_path, error))?;
    if !metadata.is_file() {
        return Err(SourceResolveError::UnsupportedFileType {
            path: display_path.to_path_buf(),
        });
    }
    if metadata.len() > remaining {
        return Err(SourceResolveError::TooManyBytes { limit });
    }

    let initial_capacity =
        usize::try_from(metadata.len()).map_err(|_| SourceResolveError::TooManyBytes { limit })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(initial_capacity)
        .map_err(|_| SourceResolveError::TooManyBytes { limit })?;
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|error| io_error(display_path, error))?;
        if count == 0 {
            break;
        }
        let next_len = (bytes.len() as u64)
            .checked_add(count as u64)
            .ok_or(SourceResolveError::TooManyBytes { limit })?;
        if next_len > remaining {
            return Err(SourceResolveError::TooManyBytes { limit });
        }
        bytes.extend_from_slice(&chunk[..count]);
    }

    Ok((bytes, capability_metadata_is_executable(&metadata)))
}

#[cfg(unix)]
fn capability_metadata_is_executable(metadata: &cap_std::fs::Metadata) -> bool {
    use cap_fs_ext::OsMetadataExt;

    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn capability_metadata_is_executable(_metadata: &cap_std::fs::Metadata) -> bool {
    false
}

#[cfg(all(test, unix))]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn raw_os_bytes(value: &OsStr) -> Vec<u8> {
    value.as_encoded_bytes().to_vec()
}

struct SourceIdentityHasher {
    hasher: Sha256,
    byte_count: u64,
}

impl SourceIdentityHasher {
    fn new(entry_count: usize) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"omega-source-tree-v4\0");
        hash_length(&mut hasher, entry_count as u64);
        Self {
            hasher,
            byte_count: 0,
        }
    }

    fn add_directory(&mut self, relative_bytes: &[u8], normalized_mode: u16) {
        self.add_path(relative_bytes);
        self.hasher.update(b"directory");
        self.hasher.update(normalized_mode.to_le_bytes());
    }

    fn add_file(
        &mut self,
        relative_bytes: &[u8],
        executable: bool,
        bytes: &[u8],
    ) -> Result<(), SourceResolveError> {
        self.add_path(relative_bytes);
        self.hasher.update(b"file");
        self.hasher.update([u8::from(executable)]);
        hash_bytes(&mut self.hasher, bytes);
        self.byte_count = self
            .byte_count
            .checked_add(bytes.len() as u64)
            .ok_or(SourceResolveError::TooManyBytes { limit: u64::MAX })?;
        Ok(())
    }

    fn add_symlink(&mut self, relative_bytes: &[u8], target_bytes: &[u8]) {
        self.add_path(relative_bytes);
        self.hasher.update(b"symlink");
        hash_bytes(&mut self.hasher, target_bytes);
    }

    fn add_path(&mut self, relative_bytes: &[u8]) {
        self.hasher.update(b"entry");
        hash_bytes(&mut self.hasher, relative_bytes);
    }

    fn finish(self) -> (u64, String) {
        (self.byte_count, format_sha256(&self.hasher.finalize()))
    }
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hash_length(hasher, bytes.len() as u64);
    hasher.update(bytes);
}

fn hash_length(hasher: &mut Sha256, length: u64) {
    hasher.update(length.to_le_bytes());
}

fn io_error(path: &Path, error: std::io::Error) -> SourceResolveError {
    SourceResolveError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

impl GitExecutor {
    fn system(execution_transport: GitExecutionTransport) -> Result<Self, SourceResolveError> {
        for candidate in system_git_candidates() {
            let path = Path::new(candidate);
            if path.is_file() {
                return Self::open_with_budget_for_transport(
                    path,
                    GIT_FIXED_COMMAND_ALLOWANCE,
                    GIT_RESOLUTION_TIMEOUT,
                    execution_transport,
                );
            }
        }
        Err(SourceResolveError::GitExecutableUnavailable)
    }

    #[cfg(test)]
    fn open(path: &Path) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            GIT_FIXED_COMMAND_ALLOWANCE,
            GIT_RESOLUTION_TIMEOUT,
            GitExecutionTransport::File,
        )
    }

    #[cfg(test)]
    fn open_with_budget(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
    ) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            maximum_launches,
            timeout,
            GitExecutionTransport::File,
        )
    }

    fn open_with_budget_for_transport(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
        execution_transport: GitExecutionTransport,
    ) -> Result<Self, SourceResolveError> {
        let started = Instant::now();
        if !path.is_absolute() {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: "path is not absolute".to_owned(),
            });
        }
        let canonical =
            path.canonicalize()
                .map_err(|error| SourceResolveError::GitExecutableInvalid {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
        verify_git_executable_custody(&canonical)?;
        let metadata_identity = observe_git_executable_metadata(&canonical)?;
        let content_identity = hash_git_executable(&canonical)?;
        if observe_git_executable_metadata(&canonical)? != metadata_identity {
            return Err(SourceResolveError::GitExecutableChanged { path: canonical });
        }
        let transport_executable = match execution_transport {
            GitExecutionTransport::Ssh => Some(open_ssh_transport_executable(&canonical)?),
            GitExecutionTransport::Https => Some(open_https_transport_executable(&canonical)?),
            #[cfg(test)]
            GitExecutionTransport::File => None,
        };
        Ok(Self {
            identity: GitExecutableIdentity {
                path: canonical,
                content_identity,
            },
            metadata_identity,
            transport_executable,
            execution_transport,
            started,
            timeout,
            launches: Cell::new(0),
            maximum_launches,
        })
    }

    fn verify(&self) -> Result<(), SourceResolveError> {
        if observe_git_executable_metadata(&self.identity.path)? != self.metadata_identity {
            return Err(SourceResolveError::GitExecutableChanged {
                path: self.identity.path.clone(),
            });
        }
        verify_git_executable_custody(&self.identity.path)?;
        if let Some(transport_executable) = &self.transport_executable {
            verify_git_transport_executable(transport_executable)?;
        }
        Ok(())
    }

    fn verify_content(&self) -> Result<(), SourceResolveError> {
        self.verify()?;
        if hash_git_executable(&self.identity.path)? != self.identity.content_identity {
            return Err(SourceResolveError::GitExecutableChanged {
                path: self.identity.path.clone(),
            });
        }
        if let Some(transport_executable) = &self.transport_executable
            && hash_git_executable(&transport_executable.identity.path)?
                != transport_executable.identity.content_identity
        {
            return Err(SourceResolveError::GitExecutableChanged {
                path: transport_executable.identity.path.clone(),
            });
        }
        self.verify()?;
        self.verify_budget()
    }

    fn begin_launch(&self) -> Result<Duration, SourceResolveError> {
        self.verify_budget()?;
        let launches = self.launches.get();
        if launches >= self.maximum_launches {
            return Err(SourceResolveError::GitResolutionCommandLimit {
                limit: self.maximum_launches,
            });
        }
        self.launches.set(launches + 1);
        Ok(GIT_COMMAND_TIMEOUT.min(self.remaining_time()?))
    }

    fn verify_budget(&self) -> Result<(), SourceResolveError> {
        self.remaining_time().map(|_| ())
    }

    fn remaining_time(&self) -> Result<Duration, SourceResolveError> {
        let elapsed = self.started.elapsed();
        if elapsed >= self.timeout {
            Err(SourceResolveError::GitResolutionTimedOut {
                timeout_millis: duration_millis(self.timeout),
            })
        } else {
            Ok(self.timeout - elapsed)
        }
    }
}

fn open_ssh_transport_executable(
    git_executable: &Path,
) -> Result<GitTransportExecutableObservation, SourceResolveError> {
    let requested_path = ssh_transport_executable_path(git_executable);
    let mut observation = open_git_transport_executable(&requested_path)?;
    // SSH is supplied through `GIT_SSH_COMMAND`, so invoke the already
    // authenticated canonical target directly rather than retaining an alias.
    observation.identity.invocation_path = observation.identity.path.clone();
    Ok(observation)
}

fn open_https_transport_executable(
    git_executable: &Path,
) -> Result<GitTransportExecutableObservation, SourceResolveError> {
    let candidates = https_transport_executable_candidates(git_executable);
    for requested_path in &candidates {
        match std::fs::symlink_metadata(requested_path) {
            Ok(_) => return open_git_transport_executable(requested_path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(SourceResolveError::GitExecutableInvalid {
                    path: requested_path.clone(),
                    message: format!("HTTPS transport executable is unavailable: {error}"),
                });
            }
        }
    }
    Err(SourceResolveError::GitExecutableInvalid {
        path: git_executable.to_path_buf(),
        message: format!(
            "HTTPS transport executable is unavailable at the closed install-relative candidates: {}",
            candidates
                .iter()
                .map(|candidate| candidate.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    })
}

fn open_git_transport_executable(
    requested_path: &Path,
) -> Result<GitTransportExecutableObservation, SourceResolveError> {
    if !requested_path.is_absolute() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: requested_path.to_path_buf(),
            message: "transport executable path is not absolute".to_owned(),
        });
    }
    let canonical = requested_path.canonicalize().map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: requested_path.to_path_buf(),
            message: format!("transport executable is unavailable: {error}"),
        }
    })?;
    verify_git_transport_invocation_path(requested_path, &canonical)?;
    verify_git_executable_custody(&canonical)?;
    let metadata_identity = observe_git_executable_metadata(&canonical)?;
    let content_identity = hash_git_executable(&canonical)?;
    if observe_git_executable_metadata(&canonical)? != metadata_identity {
        return Err(SourceResolveError::GitExecutableChanged { path: canonical });
    }
    Ok(GitTransportExecutableObservation {
        identity: GitTransportExecutableIdentity {
            invocation_path: requested_path.to_path_buf(),
            path: canonical,
            content_identity,
        },
        metadata_identity,
    })
}

fn verify_git_transport_executable(
    executable: &GitTransportExecutableObservation,
) -> Result<(), SourceResolveError> {
    verify_git_transport_invocation_path(
        &executable.identity.invocation_path,
        &executable.identity.path,
    )?;
    if observe_git_executable_metadata(&executable.identity.path)? != executable.metadata_identity {
        return Err(SourceResolveError::GitExecutableChanged {
            path: executable.identity.path.clone(),
        });
    }
    verify_git_executable_custody(&executable.identity.path)
}

fn verify_git_transport_invocation_path(
    invocation_path: &Path,
    expected_canonical: &Path,
) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(invocation_path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: invocation_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if !metadata.is_file() && !metadata.file_type().is_symlink() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: invocation_path.to_path_buf(),
            message: "transport invocation path is not a regular file or symbolic link".to_owned(),
        });
    }
    verify_git_transport_invocation_node_custody(invocation_path, &metadata)?;
    let canonical = invocation_path.canonicalize().map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: invocation_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if canonical != expected_canonical {
        return Err(SourceResolveError::GitExecutableChanged {
            path: invocation_path.to_path_buf(),
        });
    }
    verify_git_executable_ancestry(invocation_path)
}

#[cfg(unix)]
fn verify_git_transport_invocation_node_custody(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    if metadata.uid() != 0 && metadata.uid() != effective_user {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "transport invocation entry is owned by an unrelated user".to_owned(),
        });
    }
    if metadata.file_type().is_symlink() {
        verify_macos_path_extended_acl_custody(path, false)?;
    } else {
        verify_macos_open_executable_acl_custody(path, metadata)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_git_transport_invocation_node_custody(
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
fn verify_git_executable_custody(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "canonical resolver executable is not a concrete regular file".to_owned(),
        });
    }
    if metadata.uid() != 0 && metadata.uid() != effective_user {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message:
                "resolver executable is owned by neither root nor the resolver's effective user"
                    .to_owned(),
        });
    }
    let mode = metadata.mode();
    if mode & 0o022 != 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable is writable by group or other users".to_owned(),
        });
    }
    if mode & 0o6000 != 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable must not carry set-user-ID or set-group-ID authority"
                .to_owned(),
        });
    }
    if mode & 0o111 == 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no executable mode bit".to_owned(),
        });
    }
    verify_macos_open_executable_acl_custody(path, &metadata)?;

    verify_git_executable_ancestry(path)
}

#[cfg(unix)]
fn verify_git_executable_ancestry(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();

    let parent = path
        .parent()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no absolute custody ancestry".to_owned(),
        })?;
    for ancestor in parent.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor).map_err(|error| {
            SourceResolveError::GitExecutableInvalid {
                path: ancestor.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: ancestor.to_path_buf(),
                message: "resolver executable ancestry contains a non-directory or symlink"
                    .to_owned(),
            });
        }
        if metadata.uid() != 0 && metadata.uid() != effective_user {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: ancestor.to_path_buf(),
                message: "resolver executable ancestry is owned by an unrelated user".to_owned(),
            });
        }
        let mode = metadata.mode();
        if mode & 0o022 != 0 && mode & 0o1000 == 0 {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: ancestor.to_path_buf(),
                message:
                    "resolver executable ancestry is externally writable without sticky-entry protection"
                        .to_owned(),
            });
        }
        verify_macos_open_executable_ancestry_acl_custody(ancestor, &metadata)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_macos_path_extended_acl_custody(
    path: &Path,
    follow_symbolic_link: bool,
) -> Result<(), SourceResolveError> {
    let symbolic_link_behavior = if follow_symbolic_link {
        omega_platform_custody::SymbolicLinkBehavior::Follow
    } else {
        omega_platform_custody::SymbolicLinkBehavior::InspectLink
    };
    let has_allow_entry =
        omega_platform_custody::extended_acl_has_allow_entry(path, symbolic_link_behavior)
            .map_err(|error| SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: format!(
                    "could not inspect resolver executable extended ACL custody: {error}"
                ),
            })?;
    if has_allow_entry {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable custody contains an extended ACL allow entry".to_owned(),
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_macos_open_executable_acl_custody(
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let parent_path = path
        .parent()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no absolute custody parent".to_owned(),
        })?;
    let name = path
        .file_name()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no concrete filename".to_owned(),
        })?;
    let parent = open_absolute_directory_nofollow(parent_path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: parent_path.to_path_buf(),
            message: format!("could not retain resolver executable parent: {error}"),
        }
    })?;
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!("could not open resolver executable without following links: {error}"),
        }
    })?;
    let opened = file
        .metadata()
        .map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!("could not inspect retained resolver executable: {error}"),
        })?;
    if !opened.is_file() || !same_std_and_capability_file_identity(classified, &opened) {
        return Err(SourceResolveError::GitExecutableChanged {
            path: path.to_path_buf(),
        });
    }
    verify_macos_open_executable_extended_acl_custody(path, &file.into_std())
}

#[cfg(target_os = "macos")]
fn verify_macos_open_executable_ancestry_acl_custody(
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let directory = open_absolute_directory_nofollow(path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!("could not retain resolver executable ancestry: {error}"),
        }
    })?;
    let opened =
        directory
            .dir_metadata()
            .map_err(|error| SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: format!(
                    "could not inspect retained resolver executable ancestry: {error}"
                ),
            })?;
    if !opened.is_dir() || !same_std_and_capability_file_identity(classified, &opened) {
        return Err(SourceResolveError::GitExecutableChanged {
            path: path.to_path_buf(),
        });
    }
    verify_macos_open_executable_extended_acl_custody(
        path,
        &directory
            .try_clone()
            .map_err(|error| SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: format!("could not clone retained executable ancestry: {error}"),
            })?
            .into_std_file(),
    )
}

#[cfg(target_os = "macos")]
fn verify_macos_open_executable_extended_acl_custody(
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    let has_allow_entry = omega_platform_custody::open_file_extended_acl_has_allow_entry(file)
        .map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!(
                "could not inspect retained resolver executable extended ACL custody: {error}"
            ),
        })?;
    if has_allow_entry {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable custody contains an extended ACL allow entry".to_owned(),
        });
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn verify_macos_path_extended_acl_custody(
    _path: &Path,
    _follow_symbolic_link: bool,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn verify_macos_open_executable_acl_custody(
    _path: &Path,
    _classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn verify_macos_open_executable_ancestry_acl_custody(
    _path: &Path,
    _classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(not(unix))]
fn verify_git_executable_custody(_path: &Path) -> Result<(), SourceResolveError> {
    // Windows ownership and DACL enforcement belongs to the native isolation
    // backend. The portable floor still commits the concrete file identity.
    Ok(())
}

#[cfg(not(unix))]
fn verify_git_executable_ancestry(path: &Path) -> Result<(), SourceResolveError> {
    if path.parent().is_none() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no absolute custody ancestry".to_owned(),
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn system_git_candidates() -> &'static [&'static str] {
    &[
        "/Library/Developer/CommandLineTools/usr/bin/git",
        "/Applications/Xcode.app/Contents/Developer/usr/bin/git",
        "/usr/local/bin/git",
        "/opt/homebrew/bin/git",
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn system_git_candidates() -> &'static [&'static str] {
    &["/usr/bin/git", "/usr/local/bin/git"]
}

#[cfg(windows)]
fn system_git_candidates() -> &'static [&'static str] {
    &[
        r"C:\Program Files\Git\cmd\git.exe",
        r"C:\Program Files\Git\bin\git.exe",
        r"C:\Program Files (x86)\Git\cmd\git.exe",
    ]
}

#[cfg(unix)]
fn https_transport_executable_candidates(git_executable: &Path) -> Vec<PathBuf> {
    let Some(installation_root) = git_executable.parent().and_then(Path::parent) else {
        return Vec::new();
    };
    vec![
        installation_root.join("libexec/git-core/git-remote-https"),
        installation_root.join("lib/git-core/git-remote-https"),
    ]
}

#[cfg(windows)]
fn https_transport_executable_candidates(git_executable: &Path) -> Vec<PathBuf> {
    let Some(installation_root) = git_executable.parent().and_then(Path::parent) else {
        return Vec::new();
    };
    vec![installation_root.join("mingw64/libexec/git-core/git-remote-https.exe")]
}

#[cfg(unix)]
fn ssh_transport_executable_path(_git_executable: &Path) -> PathBuf {
    PathBuf::from("/usr/bin/ssh")
}

#[cfg(windows)]
fn ssh_transport_executable_path(git_executable: &Path) -> PathBuf {
    git_executable
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("usr/bin/ssh.exe"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Git\usr\bin\ssh.exe"))
}

fn hash_git_executable(path: &Path) -> Result<String, SourceResolveError> {
    let metadata =
        std::fs::metadata(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "path is not a regular file".to_owned(),
        });
    }
    if metadata.len() > GIT_EXECUTABLE_BYTE_LIMIT {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!(
                "file exceeds the {GIT_EXECUTABLE_BYTE_LIMIT}-byte executable ceiling"
            ),
        });
    }
    let mut file = File::open(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count =
            file.read(&mut buffer)
                .map_err(|error| SourceResolveError::GitExecutableInvalid {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
        if count == 0 {
            break;
        }
        observed = observed.saturating_add(count as u64);
        if observed > GIT_EXECUTABLE_BYTE_LIMIT {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: format!(
                    "file exceeds the {GIT_EXECUTABLE_BYTE_LIMIT}-byte executable ceiling"
                ),
            });
        }
        hasher.update(&buffer[..count]);
    }
    if observed != metadata.len() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "file length changed while it was hashed".to_owned(),
        });
    }
    Ok(format_sha256(&hasher.finalize()))
}

fn observe_git_executable_metadata(
    path: &Path,
) -> Result<GitExecutableMetadataIdentity, SourceResolveError> {
    let metadata =
        std::fs::metadata(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "path is not a regular file".to_owned(),
        });
    }
    let modified =
        metadata
            .modified()
            .map_err(|error| SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        Ok(GitExecutableMetadataIdentity {
            length: metadata.len(),
            modified,
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
        })
    }
    #[cfg(windows)]
    {
        Ok(GitExecutableMetadataIdentity {
            length: metadata.len(),
            modified,
        })
    }
}

fn run_git<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    args: I,
) -> Result<(), SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(executor, working_directory, args)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn run_git_stdout<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    args: I,
) -> Result<String, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(executor, working_directory, args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn run_git_bytes_stdout<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    args: I,
) -> Result<Vec<u8>, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(executor, working_directory, args)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn run_git_output<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    args: I,
) -> Result<BoundedCommandOutput, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = sealed_git_command(executor, working_directory)?;
    let command_timeout = executor.begin_launch()?;
    command.args(args);
    let result = run_command_bounded(
        &mut command,
        "command",
        GIT_STDOUT_LIMIT,
        GIT_STDERR_LIMIT,
        command_timeout,
    );
    reconcile_git_command_result(result, executor.verify(), executor.verify_budget())
}

fn reconcile_git_command_result<T>(
    result: Result<T, SourceResolveError>,
    executable_result: Result<(), SourceResolveError>,
    budget_result: Result<(), SourceResolveError>,
) -> Result<T, SourceResolveError> {
    match (result, executable_result, budget_result) {
        (Err(error @ SourceResolveError::GitCleanupFailed { .. }), _, _) => Err(error),
        (_, Err(error), _) => Err(error),
        (_, _, Err(error)) => Err(error),
        (result, Ok(()), Ok(())) => result,
    }
}

fn reconcile_git_cache_operation_result<T>(
    operation_result: Result<T, SourceResolveError>,
    namespace_result: Result<(), SourceResolveError>,
    invalidation_result: Option<Result<(), SourceResolveError>>,
) -> Result<T, SourceResolveError> {
    if let Err(error) = namespace_result {
        return Err(error);
    }
    if let Some(Err(error)) = invalidation_result {
        return Err(error);
    }
    operation_result
}

#[derive(Debug)]
struct BoundedCommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturedStream {
    Stdout,
    Stderr,
}

impl CapturedStream {
    fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

enum StreamCaptureResult {
    Complete(Vec<u8>),
    Overflow,
    Failed(String),
}

struct StreamCapture {
    stream: CapturedStream,
    result: StreamCaptureResult,
}

fn run_command_bounded(
    command: &mut Command,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
) -> Result<BoundedCommandOutput, SourceResolveError> {
    run_command_bounded_with_stdin(
        command,
        Stdio::null(),
        operation,
        stdout_limit,
        stderr_limit,
        timeout,
    )
}

fn run_command_bounded_with_stdin(
    command: &mut Command,
    stdin: Stdio,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
) -> Result<BoundedCommandOutput, SourceResolveError> {
    let started = Instant::now();
    let mut child = command
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .group_spawn()
        .map_err(|error| SourceResolveError::Git {
            operation: format!("{operation} spawn"),
            status: None,
            stderr: error.to_string(),
        })?;
    let stdout = child
        .inner()
        .stdout
        .take()
        .expect("command stdout was piped");
    let stderr = child
        .inner()
        .stderr
        .take()
        .expect("command stderr was piped");
    let (sender, receiver) = mpsc::channel();
    if let Err(error) = spawn_stream_capture(stdout, CapturedStream::Stdout, stdout_limit, &sender)
    {
        return fail_after_cleanup(
            &mut child,
            operation,
            SourceResolveError::Git {
                operation: format!("{operation} stdout capture"),
                status: None,
                stderr: error.to_string(),
            },
        );
    }
    if let Err(error) = spawn_stream_capture(stderr, CapturedStream::Stderr, stderr_limit, &sender)
    {
        return fail_after_cleanup(
            &mut child,
            operation,
            SourceResolveError::Git {
                operation: format!("{operation} stderr capture"),
                status: None,
                stderr: error.to_string(),
            },
        );
    }
    drop(sender);

    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if status.is_none() {
            status = match child.try_wait() {
                Ok(Some(status)) => {
                    terminate_child_bounded(&mut child, operation)?;
                    Some(status)
                }
                Ok(None) => None,
                Err(error) => {
                    return fail_after_cleanup(
                        &mut child,
                        operation,
                        SourceResolveError::Git {
                            operation: format!("{operation} wait"),
                            status: None,
                            stderr: error.to_string(),
                        },
                    );
                }
            };
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            return Ok(BoundedCommandOutput {
                status: status.expect("status was checked"),
                stdout: stdout.expect("stdout was checked"),
                stderr: stderr.expect("stderr was checked"),
            });
        }

        let elapsed = started.elapsed();
        if elapsed >= timeout {
            return fail_after_cleanup(
                &mut child,
                operation,
                SourceResolveError::GitTimedOut {
                    operation: operation.to_owned(),
                    timeout_millis: duration_millis(timeout),
                },
            );
        }
        let wait = PROCESS_POLL_INTERVAL.min(timeout.saturating_sub(elapsed));
        match receiver.recv_timeout(wait) {
            Ok(capture) => {
                let bytes = match capture.result {
                    StreamCaptureResult::Complete(bytes) => bytes,
                    StreamCaptureResult::Overflow => {
                        return fail_after_cleanup(
                            &mut child,
                            operation,
                            SourceResolveError::GitOutputOverflow {
                                operation: operation.to_owned(),
                                stream: capture.stream.name().to_owned(),
                                limit: match capture.stream {
                                    CapturedStream::Stdout => stdout_limit,
                                    CapturedStream::Stderr => stderr_limit,
                                },
                            },
                        );
                    }
                    StreamCaptureResult::Failed(message) => {
                        return fail_after_cleanup(
                            &mut child,
                            operation,
                            SourceResolveError::Git {
                                operation: format!("{operation} {} capture", capture.stream.name()),
                                status: None,
                                stderr: message,
                            },
                        );
                    }
                };
                match capture.stream {
                    CapturedStream::Stdout => stdout = Some(bytes),
                    CapturedStream::Stderr => stderr = Some(bytes),
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if stdout.is_none() || stderr.is_none() {
                    return fail_after_cleanup(
                        &mut child,
                        operation,
                        SourceResolveError::Git {
                            operation: format!("{operation} capture"),
                            status: None,
                            stderr: "output capture ended before both streams completed".to_owned(),
                        },
                    );
                }
                std::thread::sleep(wait);
            }
        }
    }
}

fn spawn_stream_capture<R>(
    reader: R,
    stream: CapturedStream,
    limit: usize,
    sender: &mpsc::Sender<StreamCapture>,
) -> std::io::Result<()>
where
    R: Read + Send + 'static,
{
    let sender = sender.clone();
    std::thread::Builder::new()
        .name(format!("omega-git-{}", stream.name()))
        .spawn(move || {
            let result = capture_stream_bounded(reader, limit);
            let _ = sender.send(StreamCapture { stream, result });
        })?;
    Ok(())
}

fn capture_stream_bounded<R>(mut reader: R, limit: usize) -> StreamCaptureResult
where
    R: Read,
{
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = match reader.read(&mut chunk) {
            Ok(0) => return StreamCaptureResult::Complete(bytes),
            Ok(count) => count,
            Err(error) => return StreamCaptureResult::Failed(error.to_string()),
        };
        let Some(next_length) = bytes.len().checked_add(count) else {
            return StreamCaptureResult::Overflow;
        };
        if next_length > limit {
            return StreamCaptureResult::Overflow;
        }
        if bytes.try_reserve(count).is_err() {
            return StreamCaptureResult::Failed("output capture allocation failed".to_owned());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}

fn fail_after_cleanup<T>(
    child: &mut GroupChild,
    operation: &str,
    original: SourceResolveError,
) -> Result<T, SourceResolveError> {
    match terminate_child_bounded(child, operation) {
        Ok(()) => Err(original),
        Err(cleanup) => Err(cleanup),
    }
}

fn terminate_child_bounded(
    child: &mut GroupChild,
    operation: &str,
) -> Result<(), SourceResolveError> {
    let kill_error = child.kill().err();
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                if let Some(error) = kill_error
                    .as_ref()
                    .filter(|error| !process_group_already_absent(error))
                {
                    return Err(SourceResolveError::GitCleanupFailed {
                        operation: operation.to_owned(),
                        message: format!("could not terminate the process group: {error}"),
                    });
                }
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                return Err(SourceResolveError::GitCleanupFailed {
                    operation: operation.to_owned(),
                    message: format!("could not reap the process: {error}"),
                });
            }
        }
        if started.elapsed() >= GIT_COMMAND_CLEANUP_TIMEOUT {
            let message = match &kill_error {
                Some(error) => format!(
                    "could not terminate the process group ({error}) or reap it within {} milliseconds",
                    duration_millis(GIT_COMMAND_CLEANUP_TIMEOUT)
                ),
                None => format!(
                    "could not reap the terminated process within {} milliseconds",
                    duration_millis(GIT_COMMAND_CLEANUP_TIMEOUT)
                ),
            };
            return Err(SourceResolveError::GitCleanupFailed {
                operation: operation.to_owned(),
                message,
            });
        }
        std::thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn process_group_already_absent(error: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        // POSIX ESRCH alone proves that no process group exists. EPERM proves
        // the opposite: a group exists but this resolver cannot signal it.
        error.raw_os_error() == Some(3)
    }
    #[cfg(not(unix))]
    {
        error.kind() == std::io::ErrorKind::InvalidInput
    }
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn sealed_git_command(
    executor: &GitExecutor,
    working_directory: &Path,
) -> Result<Command, SourceResolveError> {
    executor.verify()?;
    if !working_directory.is_absolute() {
        return Err(SourceResolveError::Git {
            operation: "command configuration".to_owned(),
            status: None,
            stderr: format!(
                "working directory `{}` is not absolute",
                working_directory.display()
            ),
        });
    }
    let metadata =
        std::fs::metadata(working_directory).map_err(|error| io_error(working_directory, error))?;
    if !metadata.is_dir() {
        return Err(SourceResolveError::NotDirectory {
            path: working_directory.to_path_buf(),
        });
    }

    let mut command = Command::new(&executor.identity.path);
    command
        .env_clear()
        .current_dir(working_directory)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PATH", git_helper_path(executor))
        .env(
            "GIT_ALLOW_PROTOCOL",
            executor.execution_transport.allowed_protocol(),
        )
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("--no-replace-objects")
        .arg("-c")
        .arg(format!("core.hooksPath={}", null_device()))
        .args(["-c", "protocol.allow=never"])
        .arg("-c")
        .arg(format!("protocol.file.allow={}", {
            #[cfg(test)]
            {
                executor
                    .execution_transport
                    .permits(GitExecutionTransport::File)
            }
            #[cfg(not(test))]
            {
                "never"
            }
        }))
        .args(["-c", "protocol.http.allow=never"])
        .arg("-c")
        .arg(format!(
            "protocol.https.allow={}",
            executor
                .execution_transport
                .permits(GitExecutionTransport::Https)
        ))
        .arg("-c")
        .arg(format!(
            "protocol.ssh.allow={}",
            executor
                .execution_transport
                .permits(GitExecutionTransport::Ssh)
        ))
        .args([
            "-c",
            "protocol.git.allow=never",
            "-c",
            "protocol.ext.allow=never",
            "-c",
            "http.followRedirects=false",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.autocrlf=false",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "-c",
            "fetch.fsckObjects=true",
            "-c",
            "transfer.fsckObjects=true",
            "-c",
            "fetch.recurseSubmodules=false",
            "-c",
            "submodule.recurse=false",
            "-c",
            "filter.lfs.smudge=",
            "-c",
            "filter.lfs.required=false",
        ]);
    if executor.execution_transport == GitExecutionTransport::Https {
        let helper = executor
            .transport_executable
            .as_ref()
            .expect("validated HTTPS executor retains its transport helper");
        let helper_directory = helper
            .identity
            .invocation_path
            .parent()
            .expect("validated HTTPS helper has an absolute parent");
        command.env("GIT_EXEC_PATH", helper_directory);
    }
    if let Some(transport_executable) = &executor.transport_executable {
        if executor.execution_transport == GitExecutionTransport::Ssh {
            command
                .env(
                    "GIT_SSH_COMMAND",
                    sealed_ssh_command(&transport_executable.identity.path),
                )
                .env("GIT_SSH_VARIANT", "ssh");
        }
    }
    Ok(command)
}

#[cfg(unix)]
fn git_helper_path(executor: &GitExecutor) -> OsString {
    if executor.execution_transport == GitExecutionTransport::Https {
        return executor
            .transport_executable
            .as_ref()
            .and_then(|helper| helper.identity.invocation_path.parent())
            .map(Path::as_os_str)
            .map(OsStr::to_os_string)
            .unwrap_or_default();
    }
    OsString::from("/usr/bin:/bin")
}

#[cfg(unix)]
fn sealed_ssh_command(ssh_executable: &Path) -> OsString {
    OsString::from(format!(
        "{} -F /dev/null -oBatchMode=yes -oPasswordAuthentication=no -oKbdInteractiveAuthentication=no -oNumberOfPasswordPrompts=0 -oStrictHostKeyChecking=yes",
        ssh_executable.display()
    ))
}

#[cfg(windows)]
fn git_helper_path(executor: &GitExecutor) -> OsString {
    if executor.execution_transport == GitExecutionTransport::Https {
        return executor
            .transport_executable
            .as_ref()
            .and_then(|helper| helper.identity.invocation_path.parent())
            .map(Path::as_os_str)
            .map(OsStr::to_os_string)
            .unwrap_or_default();
    }
    let mut directories = Vec::new();
    if let Some(parent) = executor.identity.path.parent() {
        directories.push(parent.to_path_buf());
        if let Some(root) = parent.parent() {
            directories.push(root.join("bin"));
            directories.push(root.join("usr/bin"));
        }
    }
    std::env::join_paths(directories).unwrap_or_default()
}

#[cfg(windows)]
fn sealed_ssh_command(ssh_executable: &Path) -> OsString {
    OsString::from(format!(
        "\"{}\" -F NUL -oBatchMode=yes -oPasswordAuthentication=no -oKbdInteractiveAuthentication=no -oNumberOfPasswordPrompts=0 -oStrictHostKeyChecking=yes",
        ssh_executable.display()
    ))
}

#[cfg(unix)]
fn null_device() -> &'static str {
    "/dev/null"
}

#[cfg(windows)]
fn null_device() -> &'static str {
    "NUL"
}

fn format_sha256(bytes: &[u8]) -> String {
    format_hex(bytes)
}

fn format_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::PackageName;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PACKAGE_FIXTURES: &[&str] = &[
        "arithmetic-kernels",
        "generated-table",
        "file-journal",
        "network-overreach",
        "remote-journal",
        "axiom-ledger",
        "opaque-carrier",
        "provider-switchboard",
        "capability-vault",
        "graph-workbench",
    ];

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let temporary_directory = std::env::temp_dir()
            .canonicalize()
            .expect("canonicalize test temporary directory");
        temporary_directory.join(format!(
            "omega-packages-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[cfg(target_os = "macos")]
    fn change_macos_acl(path: &Path, arguments: &[&str]) {
        let status = Command::new("/bin/chmod")
            .args(arguments)
            .arg(path)
            .status()
            .expect("run the concrete macOS ACL editor");
        assert!(
            status.success(),
            "macOS ACL edit failed for {}",
            path.display()
        );
    }

    fn local_git_request(repository: &Path, revision: &str) -> GitSourceRequest {
        GitSourceRequest::for_local_test_repository(repository, Some(revision.to_owned()))
            .expect("local Git fixture request")
    }

    #[test]
    fn git_request_validates_transport_and_emits_sanitized_identity() {
        let https = GitSourceRequest::new(
            "https://GitHub.com/CathedralOS/Arithmetic-Kernels",
            Some("refs/tags/v1.0.0".to_owned()),
        )
        .expect("valid HTTPS request");
        let ssh = GitSourceRequest::new("git@github.com:CathedralOS/Arithmetic-Kernels.git", None)
            .expect("valid SSH request");

        assert_eq!(
            https.locator_identity(),
            "https://github.com/cathedralos/arithmetic-kernels.git"
        );
        assert_eq!(https.locator_identity(), ssh.locator_identity());
        assert_eq!(ssh.requested_revision(), "HEAD");
        assert_eq!(https.lineage(), ssh.lineage());
        assert_eq!(https.execution_transport(), GitExecutionTransport::Https);
        assert_eq!(ssh.execution_transport(), GitExecutionTransport::Ssh);
        assert_ne!(
            git_cache_identity(
                https.locator_identity(),
                https.requested_revision(),
                https.execution_transport(),
            ),
            git_cache_identity(
                ssh.locator_identity(),
                ssh.requested_revision(),
                ssh.execution_transport(),
            )
        );
    }

    #[test]
    fn git_request_rejects_insecure_secret_bearing_and_local_forms() {
        for locator in [
            "http://github.com/CathedralOS/tool.git",
            "https://token@github.com/CathedralOS/tool.git",
            "ssh://git:secret@github.com/CathedralOS/tool.git",
            "git://github.com/CathedralOS/tool.git",
            "file:///tmp/tool.git",
            "/tmp/tool.git",
        ] {
            assert!(
                matches!(
                    GitSourceRequest::new(locator, None),
                    Err(GitSourceRequestError::InvalidLocator(_))
                ),
                "accepted {locator:?}"
            );
        }
    }

    #[test]
    fn git_request_rejects_unbounded_or_refspec_shaped_inputs() {
        assert_eq!(
            GitSourceRequest::new("x".repeat(GIT_LOCATOR_BYTE_LIMIT + 1), None),
            Err(GitSourceRequestError::LocatorTooLong {
                limit: GIT_LOCATOR_BYTE_LIMIT
            })
        );
        assert_eq!(
            GitSourceRequest::new(
                "https://example.com/group/tool.git",
                Some("x".repeat(GIT_REVISION_BYTE_LIMIT + 1)),
            ),
            Err(GitSourceRequestError::RevisionTooLong {
                limit: GIT_REVISION_BYTE_LIMIT
            })
        );
        for revision in ["", "--upload-pack=tool", "main:refs/heads/owned", "a..b"] {
            assert!(
                matches!(
                    GitSourceRequest::new(
                        "https://example.com/group/tool.git",
                        Some(revision.to_owned())
                    ),
                    Err(GitSourceRequestError::EmptyRevision)
                        | Err(GitSourceRequestError::InvalidRevision)
                ),
                "accepted {revision:?}"
            );
        }
    }

    #[test]
    fn compiler_owned_source_ceilings_bound_caller_limits() {
        assert_eq!(
            LocalSourceLimits {
                max_files: usize::MAX,
                max_bytes: u64::MAX,
                max_depth: usize::MAX,
            }
            .compiler_bounded(),
            LocalSourceLimits {
                max_files: SOURCE_ENTRY_ABSOLUTE_LIMIT,
                max_bytes: SOURCE_BYTE_ABSOLUTE_LIMIT,
                max_depth: SOURCE_DEPTH_ABSOLUTE_LIMIT,
            }
        );
    }

    fn run_test_git<I, S>(directory: &Path, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn run_test_git_with_input<I, S>(directory: &Path, args: I, input: &[u8]) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("git")
            .current_dir(directory)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn git");
        command
            .stdin
            .take()
            .expect("capture git stdin")
            .write_all(input)
            .expect("write git input");
        let output = command.wait_with_output().expect("wait for git");
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("Git test output is UTF-8")
            .trim()
            .to_owned()
    }

    fn create_git_source(name: &str) -> (PathBuf, String) {
        create_git_source_with_format(name, None)
    }

    fn create_git_source_with_format(name: &str, object_format: Option<&str>) -> (PathBuf, String) {
        let root = temp_root(name);
        std::fs::create_dir_all(&root).expect("create git source");
        let mut init_arguments = vec!["init", "--quiet"];
        let object_format_argument =
            object_format.map(|format| format!("--object-format={format}"));
        if let Some(argument) = object_format_argument.as_deref() {
            init_arguments.push(argument);
        }
        run_test_git(&root, init_arguments);
        run_test_git(&root, ["config", "user.email", "omega@example.invalid"]);
        run_test_git(&root, ["config", "user.name", "Omega Tests"]);
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        run_test_git(&root, ["add", "main.omg"]);
        run_test_git(&root, ["commit", "--quiet", "-m", "initial"]);
        let output = Command::new("git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("read head");
        assert!(output.status.success());
        let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (root, commit)
    }

    fn add_empty_tree_commit(repository: &Path) -> String {
        let empty_tree =
            run_test_git_with_input(repository, ["hash-object", "-t", "tree", "--stdin"], b"");
        let main_blob = run_test_git_with_input(repository, ["rev-parse", "HEAD:main.omg"], b"");
        let root_tree_input =
            format!("100644 blob {main_blob}\tmain.omg\n040000 tree {empty_tree}\tempty\n");
        let root_tree = run_test_git_with_input(repository, ["mktree"], root_tree_input.as_bytes());
        let commit = run_test_git_with_input(
            repository,
            [
                "commit-tree",
                &root_tree,
                "-p",
                "HEAD",
                "-m",
                "add empty tree",
            ],
            b"",
        );
        run_test_git(repository, ["update-ref", "HEAD", &commit]);
        commit
    }

    fn package_fixtures_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../tests/fixtures/packages")
    }

    fn git_cache_entry_root(cache: &Path, request: &GitSourceRequest) -> PathBuf {
        cache.join(format!(
            "git-{}",
            git_cache_identity(
                request.locator_identity(),
                request.requested_revision(),
                request.execution_transport(),
            )
        ))
    }

    fn open_verified_git_repository(
        cache: &Path,
        request: &GitSourceRequest,
    ) -> VerifiedGitRepository {
        let canonical_cache = cache.canonicalize().expect("canonicalize Git cache");
        let cache_directory =
            open_absolute_directory_nofollow(&canonical_cache).expect("retain Git cache parent");
        let entry_root = git_cache_entry_root(&canonical_cache, request);
        let entry_name = entry_root.file_name().expect("Git cache entry has a name");
        VerifiedGitRepository::open(
            &cache_directory,
            entry_name,
            &entry_root,
            request.locator_identity(),
            request.requested_revision(),
            request.execution_transport(),
            LocalSourceLimits::default(),
        )
        .expect("retain verified Git repository")
    }

    #[cfg(unix)]
    fn first_regular_descendant(root: &Path) -> PathBuf {
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            for entry in std::fs::read_dir(&directory).expect("read test directory") {
                let path = entry.expect("read test entry").path();
                let metadata = std::fs::symlink_metadata(&path).expect("classify test entry");
                if metadata.is_file() {
                    return path;
                }
                if metadata.is_dir() {
                    pending.push(path);
                }
            }
        }
        panic!(
            "test tree contains no regular file beneath {}",
            root.display()
        );
    }

    #[cfg(unix)]
    fn shell_command(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", script]);
        command
    }

    #[cfg(unix)]
    #[test]
    fn bounded_command_uses_null_stdin_and_drains_both_streams() {
        let mut null_stdin =
            shell_command("if IFS= read -r value; then printf input; else printf eof; fi");
        let output = run_command_bounded(
            &mut null_stdin,
            "test-null-stdin",
            16,
            16,
            Duration::from_secs(2),
        )
        .expect("null stdin must reach EOF without blocking");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"eof");

        let mut both_streams = shell_command(
            "dd if=/dev/zero bs=65536 count=2 1>&2 2>/dev/null; \
             dd if=/dev/zero bs=65536 count=2 2>/dev/null",
        );
        let output = run_command_bounded(
            &mut both_streams,
            "test-both-streams",
            128 * 1024,
            128 * 1024,
            Duration::from_secs(2),
        )
        .expect("stdout and stderr must be drained concurrently");
        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 128 * 1024);
        assert_eq!(output.stderr.len(), 128 * 1024);
    }

    #[cfg(unix)]
    #[test]
    fn bounded_command_rejects_stdout_and_stderr_overflow() {
        assert!(matches!(
            capture_stream_bounded(std::io::Cursor::new(vec![0_u8; 1025]), 1024),
            StreamCaptureResult::Overflow
        ));
        for (stream, redirect) in [("stdout", ""), ("stderr", "1>&2")] {
            let script = format!(
                "i=0; while [ $i -lt 4096 ]; do printf x {redirect}; i=$((i + 1)); done; while :; do :; done"
            );
            let mut command = shell_command(&script);
            let error = run_command_bounded(
                &mut command,
                "test-overflow",
                1024,
                1024,
                Duration::from_secs(2),
            )
            .expect_err("capture overflow must fail closed");
            let exact_overflow = matches!(
                &error,
                SourceResolveError::GitOutputOverflow {
                    stream: actual,
                    limit: 1024,
                    ..
                } if actual == stream
            );
            let fail_closed_macos_cleanup = cfg!(target_os = "macos")
                && matches!(&error, SourceResolveError::GitCleanupFailed { .. });
            assert!(
                exact_overflow || fail_closed_macos_cleanup,
                "unexpected overflow error: {error:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn bounded_command_terminates_on_deadline() {
        let mut command = shell_command("exec sleep 10");
        let started = Instant::now();
        let error = run_command_bounded(
            &mut command,
            "test-timeout",
            1024,
            1024,
            Duration::from_millis(50),
        )
        .expect_err("deadline must fail closed");
        assert!(matches!(
            error,
            SourceResolveError::GitTimedOut {
                operation,
                timeout_millis: 50,
            } if operation == "test-timeout"
        ));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timed out subprocess was not terminated promptly"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bounded_command_terminates_descendants_on_deadline() {
        let root = temp_root("bounded-descendant-timeout");
        std::fs::create_dir_all(&root).expect("create descendant test root");
        let marker = root.join("survived");
        let mut command = shell_command(
            "(sleep 0.25; printf survived > \"$OMEGA_DESCENDANT_MARKER\") & exec sleep 10",
        );
        command.env("OMEGA_DESCENDANT_MARKER", &marker);

        let error = run_command_bounded(
            &mut command,
            "test-descendant-timeout",
            1024,
            1024,
            Duration::from_millis(50),
        )
        .expect_err("deadline must fail closed and terminate descendants");
        assert!(matches!(error, SourceResolveError::GitTimedOut { .. }));

        std::thread::sleep(Duration::from_millis(400));
        assert!(
            !marker.exists(),
            "a descendant survived the bounded command deadline"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn ssh_transport_executable_reuses_resolver_executable_custody() {
        use std::os::unix::fs::PermissionsExt;

        let temporary_root = temp_root("ssh-transport-executable");
        std::fs::create_dir_all(&temporary_root).expect("create SSH executable custody root");
        let root = temporary_root
            .canonicalize()
            .expect("canonicalize SSH executable custody root");
        let fake_ssh = root.join("ssh");
        std::fs::write(&fake_ssh, b"#!/bin/sh\nexit 0\n").expect("write fake SSH executable");
        std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700))
            .expect("make fake SSH executable");

        let executable =
            open_git_transport_executable(&fake_ssh).expect("capture SSH executable identity");
        assert!(executable.identity.path.is_absolute());
        assert_eq!(executable.identity.content_identity.len(), 64);
        verify_git_transport_executable(&executable).expect("verify unchanged SSH executable");

        std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o777))
            .expect("make SSH executable unsafe");
        assert!(matches!(
            verify_git_transport_executable(&executable),
            Err(SourceResolveError::GitExecutableChanged { .. })
                | Err(SourceResolveError::GitExecutableInvalid { .. })
        ));

        std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn https_transport_executable_binds_invocation_alias_and_canonical_target() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temporary_root = temp_root("https-transport-executable");
        std::fs::create_dir_all(&temporary_root).expect("create HTTPS helper custody root");
        let root = temporary_root
            .canonicalize()
            .expect("canonicalize HTTPS helper custody root");
        let bin = root.join("bin");
        let helpers = root.join("libexec/git-core");
        std::fs::create_dir_all(&bin).expect("create fake Git bin directory");
        std::fs::create_dir_all(&helpers).expect("create fake Git helper directory");

        let fake_git = bin.join("git");
        let helper_target = helpers.join("git-remote-http");
        let helper_alias = helpers.join("git-remote-https");
        std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
        std::fs::write(&helper_target, b"#!/bin/sh\nexit 0\n")
            .expect("write fake HTTPS helper target");
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
            .expect("make fake Git executable");
        std::fs::set_permissions(&helper_target, std::fs::Permissions::from_mode(0o700))
            .expect("make fake HTTPS helper target executable");
        symlink("git-remote-http", &helper_alias).expect("create HTTPS helper alias");

        let executable = open_https_transport_executable(&fake_git)
            .expect("capture HTTPS helper alias and target identity");
        assert_eq!(executable.identity.invocation_path(), helper_alias);
        assert_eq!(
            executable.identity.path(),
            helper_target
                .canonicalize()
                .expect("canonicalize HTTPS helper target")
        );
        assert_eq!(executable.identity.content_identity().len(), 64);
        verify_git_transport_executable(&executable).expect("verify unchanged HTTPS helper");

        let replacement = helpers.join("replacement");
        std::fs::write(&replacement, b"#!/bin/sh\nexit 1\n")
            .expect("write replacement HTTPS helper");
        std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o700))
            .expect("make replacement HTTPS helper executable");
        std::fs::remove_file(&helper_alias).expect("remove original HTTPS helper alias");
        symlink("replacement", &helper_alias).expect("replace HTTPS helper alias");
        assert!(matches!(
            verify_git_transport_executable(&executable),
            Err(SourceResolveError::GitExecutableChanged { .. })
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn bounded_command_cleans_up_descendants_after_parent_exit() {
        let mut command = shell_command("(sleep 10) &");
        let started = Instant::now();
        let output = run_command_bounded(
            &mut command,
            "test-descendant-cleanup",
            1024,
            1024,
            Duration::from_secs(2),
        )
        .expect("a completed parent must not wait on descendant-held capture pipes");
        assert!(output.status.success());
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "descendant cleanup did not close inherited capture pipes promptly"
        );
    }

    #[test]
    fn git_blob_batch_uses_one_bounded_launch_for_many_files() {
        let (repo, _) = create_git_source("batched-blobs");
        for index in 0..32 {
            std::fs::write(
                repo.join(format!("source-{index}.omg")),
                format!("// {index}\n"),
            )
            .expect("write batched source");
        }
        run_test_git(&repo, ["add", "."]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "add batched sources"]);
        let executor =
            GitExecutor::system(GitExecutionTransport::Https).expect("system Git executor");
        let tree = run_git_stdout(
            &executor,
            &repo,
            [OsStr::new("rev-parse"), OsStr::new("HEAD^{tree}")],
        )
        .expect("resolve tree");
        let listing = run_git_bytes_stdout(
            &executor,
            &repo,
            [
                OsStr::new("ls-tree"),
                OsStr::new("--full-tree"),
                OsStr::new("-r"),
                OsStr::new("-t"),
                OsStr::new("-l"),
                OsStr::new("-z"),
                OsStr::new(tree.trim()),
            ],
        )
        .expect("list tree");
        let mut entries = parse_git_tree_entries(
            &listing,
            &repo,
            LocalSourceLimits {
                max_files: 10_000,
                ..LocalSourceLimits::default()
            },
        )
        .expect("parse tree");
        let launches_before = executor.launches.get();
        read_git_blobs_batch_from_path(
            &executor,
            &repo,
            &mut entries,
            LocalSourceLimits::default(),
        )
        .expect("read all blobs in one batch");

        assert_eq!(entries.len(), 33);
        assert_eq!(executor.launches.get() - launches_before, 1);
        assert_eq!(executor.maximum_launches, GIT_FIXED_COMMAND_ALLOWANCE);
        assert!(entries.iter().any(|entry| {
            entry.relative_bytes == b"main.omg"
                && matches!(
                    &entry.kind,
                    GitTreeEntryKind::File { bytes, .. }
                        if bytes.as_slice() == b"machine Main::main() {}\n"
                )
        }));

        let oversized = GitTreeEntry {
            relative_bytes: b"oversized.omg".to_vec(),
            relative_path: PathBuf::from("oversized.omg"),
            oid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            size: 2,
            kind: GitTreeEntryKind::File {
                executable: false,
                bytes: GitBlobBytes::empty(),
            },
        };
        let error = git_batch_output_limit(
            &[oversized],
            LocalSourceLimits {
                max_bytes: 1,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("aggregate batch payload must honor the source byte ceiling");
        assert!(matches!(
            error,
            SourceResolveError::TooManyBytes { limit: 1 }
        ));

        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn git_blob_batch_parser_binds_order_type_size_and_framing() {
        fn entry(oid: char, path: &str, size: u64, symlink: bool) -> GitTreeEntry {
            GitTreeEntry {
                relative_bytes: path.as_bytes().to_vec(),
                relative_path: PathBuf::from(path),
                oid: std::iter::repeat_n(oid, 40).collect(),
                size,
                kind: if symlink {
                    GitTreeEntryKind::Symlink {
                        target_bytes: GitBlobBytes::empty(),
                    }
                } else {
                    GitTreeEntryKind::File {
                        executable: false,
                        bytes: GitBlobBytes::empty(),
                    }
                },
            }
        }

        let first_oid = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let second_oid = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let mut entries = vec![
            entry('a', "binary.omg", 3, false),
            entry('b', "link", 6, true),
        ];
        let mut valid = format!("{first_oid} blob 3\n").into_bytes();
        valid.extend_from_slice(&[0, 255, b'\n']);
        valid.push(b'\n');
        valid.extend_from_slice(format!("{second_oid} blob 6\n").as_bytes());
        valid.extend_from_slice(b"target\n");
        assign_git_batch_output(&mut entries, valid).expect("parse exact batch response");
        assert!(matches!(
            &entries[0].kind,
            GitTreeEntryKind::File { bytes, .. } if bytes.as_slice() == &[0, 255, b'\n']
        ));
        assert!(matches!(
            &entries[1].kind,
            GitTreeEntryKind::Symlink { target_bytes } if target_bytes.as_slice() == b"target"
        ));

        for malformed in [
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab blob 3\nabc\n".as_slice(),
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa tree 3\nabc\n".as_slice(),
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 4\nabc\n".as_slice(),
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 3".as_slice(),
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 3\nab".as_slice(),
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 3\nabc".as_slice(),
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa blob 3\nabc\nextra".as_slice(),
        ] {
            let mut one = vec![entry('a', "file.omg", 3, false)];
            assert!(matches!(
                assign_git_batch_output(&mut one, malformed.to_vec()),
                Err(SourceResolveError::GitTreeInvalid { .. })
            ));
        }

        let mut escaping_link = vec![entry('a', "link", 2, true)];
        let response = format!("{first_oid} blob 2\n..\n");
        assert!(matches!(
            assign_git_batch_output(&mut escaping_link, response.into_bytes()),
            Err(SourceResolveError::GitTreeInvalid { .. })
        ));
    }

    fn authenticated_file_entry(oid: &str, path: &str, payload: &[u8]) -> GitTreeEntry {
        GitTreeEntry {
            relative_bytes: path.as_bytes().to_vec(),
            relative_path: PathBuf::from(path),
            oid: oid.to_owned(),
            size: payload.len() as u64,
            kind: GitTreeEntryKind::File {
                executable: false,
                bytes: GitBlobBytes {
                    batch: Arc::new(payload.to_vec()),
                    start: 0,
                    end: payload.len(),
                },
            },
        }
    }

    #[test]
    fn git_object_authentication_accepts_fixed_sha1_and_sha256_graphs() {
        for (algorithm, blob, tree, commit) in [
            (
                GitObjectIdAlgorithm::Sha1,
                "ce013625030ba8dba906f756967f9e9ca394464a",
                "6e3b5fe3c2f6b56c4d150929f0df706a5356004a",
                "63338d8e114523a7087c391b234d776baae7af51",
            ),
            (
                GitObjectIdAlgorithm::Sha256,
                "2cf8d83d9ee29543b34a87727421fdecb7e3f3a183d337639025de576db9ebb4",
                "2ff2fdf5e33d610f8013a2eba140fd1660dac0491d9cac96ac024c5789c44e07",
                "5145c89465c4d7f1ab705bb9e032ef1a9ac879a5e137733bdab3b1d6cd354ff7",
            ),
        ] {
            assert_eq!(
                git_object_identity(b"blob", b"hello\n", algorithm).expect("hash fixed Git object"),
                blob
            );
            authenticate_git_tree(
                tree,
                &[authenticated_file_entry(blob, "main.omg", b"hello\n")],
            )
            .expect("fixed authenticated tree graph");
            let commit_payload = format!("tree {tree}\n\n");
            authenticate_git_commit_payload(commit, tree, commit_payload.as_bytes())
                .expect("fixed authenticated commit graph");
        }
    }

    #[test]
    fn git_object_authentication_rejects_mismatched_bytes_and_edges() {
        let blob = "ce013625030ba8dba906f756967f9e9ca394464a";
        let tree = "6e3b5fe3c2f6b56c4d150929f0df706a5356004a";
        let commit = "63338d8e114523a7087c391b234d776baae7af51";
        assert!(matches!(
            verify_git_object_identity(blob, b"blob", b"tampered\n", GitObjectIdAlgorithm::Sha1),
            Err(SourceResolveError::GitObjectInvalid { .. })
        ));

        let commit_payload = format!("tree {tree}\n\n");
        assert!(matches!(
            authenticate_git_commit_payload(
                "0000000000000000000000000000000000000000",
                tree,
                commit_payload.as_bytes()
            ),
            Err(SourceResolveError::GitObjectInvalid { .. })
        ));
        assert!(matches!(
            authenticate_git_commit_payload(
                commit,
                "1111111111111111111111111111111111111111",
                commit_payload.as_bytes()
            ),
            Err(SourceResolveError::GitObjectInvalid { .. })
        ));

        let replacement = b"replacement\n";
        let replacement_oid = git_object_identity(b"blob", replacement, GitObjectIdAlgorithm::Sha1)
            .expect("hash replacement Git object");
        assert!(matches!(
            authenticate_git_tree(
                tree,
                &[authenticated_file_entry(
                    &replacement_oid,
                    "main.omg",
                    replacement
                )]
            ),
            Err(SourceResolveError::GitObjectInvalid { .. })
        ));

        let false_empty_tree = GitTreeEntry {
            relative_bytes: b"empty".to_vec(),
            relative_path: PathBuf::from("empty"),
            oid: "0000000000000000000000000000000000000000".to_owned(),
            size: 0,
            kind: GitTreeEntryKind::Tree,
        };
        assert!(matches!(
            authenticate_git_tree(tree, &[false_empty_tree]),
            Err(SourceResolveError::GitObjectInvalid { .. })
        ));
    }

    #[test]
    fn exact_git_revision_must_equal_the_selected_commit() {
        let revision = "0123456789abcdef0123456789abcdef01234567";
        verify_exact_git_revision(revision, &revision.to_ascii_uppercase())
            .expect("hexadecimal case does not change an object identity");
        verify_exact_git_revision("refs/heads/main", revision)
            .expect("symbolic selectors are bound by ordinary fetch resolution");
        assert!(matches!(
            verify_exact_git_revision(revision, "1123456789abcdef0123456789abcdef01234567"),
            Err(SourceResolveError::GitObjectInvalid { .. })
        ));
    }

    #[test]
    fn checked_sha1_rejects_a_known_collision_attack() {
        // SHA-MBLES collision-detection vector, distributed by sha1-checked under MIT/Apache-2.0.
        let encoded = "99040d047fe81780012000ff4b65792069732070617274206f66206120636f6c6c6973696f6e212049742773206120747261702179c61af0afcc054515d9274e7307624b1dc7fb23988bb8de8b575dba7b9eab31c1674b6d974378a827732ff5851c76a2e60772b5a47ce1eac40bb993c12d8c70e24a4f8d5fcdedc1b32c9cf19e31af2429759d42e4dfdb31719f587623ee552939b6dcdc459fca53553b70f87ede30a247ea3af6c759a2f20b320d760db64ff479084fd3ccb3cdd48362d96a9c430617caff6c36c637e53fde28417f626fec54ed7943a46e5f5730f2bb38fb1df6e0090010d00e24ad78bf92641993608e8d158a789f34c46fe1e6027f35a4cbfb827076c50eca0e8b7cca69bb2c2b790259f9bf9570dd8d4437a3115faff7c3cac09ad25266055c27104755178eaeff825a2caa2acfb5de64ce7641dc59a541a9fc9c756756e2e23dc713c8c24c9790aa6b0e38a7f55f14452a1ca2850ddd9562fd9a18ad42496aa97008f74672f68ef461eb88b09933d626b4f918749cc027fddd6c425fc4216835d0134d15285bab2cb784a4f7cbb4fb514d4bf0f6237cf00a9e9f132b9a066e6fd17f6c42987478586ff651af96747fb426b9872b9a88e4063f59bb334cc00650f83a80c42751b71974d300fc2819a2e8f1e32c1b51cb18e6bfc4db9baef675d4aaf5b1574a047f8f6dd2ec153a93412293974d928f88ced9363cfef97ce2e742bf34c96b8ef3875676fea5cca8e5f7dea0bab2413d4de00ee71ee01f162bdb6d1eafd925e6aebaae6a354ef17cf205a404fbdb12fc454d41fdd95cf2459664a2ad032d1da60a73264075d7f1e0d6c1403ae7a0d861df3fe5707188dd5e07d1589b9f8b6630553f8fc352b3e0c27da80bddba4c64020d";
        let collision = encoded
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| (hex_digit(pair[0]).unwrap() << 4) | hex_digit(pair[1]).unwrap())
            .collect::<Vec<_>>();
        let mut hasher = CheckedSha1::new();
        hasher.update(&collision);

        assert!(matches!(
            finalize_checked_sha1(hasher),
            Err(SourceResolveError::GitObjectInvalid { .. })
        ));
    }

    #[test]
    fn git_object_rejection_precedes_snapshot_staging() {
        let entry_root = temp_root("git-object-rejection-before-stage");
        let executor =
            GitExecutor::system(GitExecutionTransport::Https).expect("system Git executor");
        let error = resolve_git_snapshot(
            &executor,
            &entry_root,
            "6e3b5fe3c2f6b56c4d150929f0df706a5356004a",
            vec![authenticated_file_entry(
                "ce013625030ba8dba906f756967f9e9ca394464a",
                "main.omg",
                b"tampered\n",
            )],
            LocalSourceLimits::default(),
        )
        .expect_err("mismatched object bytes must reject before staging");
        assert!(matches!(error, SourceResolveError::GitObjectInvalid { .. }));
        assert!(
            !entry_root.exists(),
            "object authentication failure must not create a cache or snapshot path"
        );

        let mut escaping = authenticated_file_entry(
            "ce013625030ba8dba906f756967f9e9ca394464a",
            "main.omg",
            b"hello\n",
        );
        escaping.relative_path = std::env::temp_dir().join("omega-escaped-snapshot.omg");
        let error = resolve_git_snapshot(
            &executor,
            &entry_root,
            "6e3b5fe3c2f6b56c4d150929f0df706a5356004a",
            vec![escaping],
            LocalSourceLimits::default(),
        )
        .expect_err("destination escape must reject before staging");
        assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
        assert!(
            !entry_root.exists(),
            "destination preflight failure must not create a cache or snapshot path"
        );
    }

    #[test]
    fn package_fixtures_resolve_as_distinct_local_sources() {
        let fixtures_root = package_fixtures_root();
        let mut identities = BTreeSet::new();
        for package in PACKAGE_FIXTURES {
            PackageName::parse(*package).expect("fixture package names must be kebab-case");
            let root = fixtures_root.join(package);
            assert!(root.join("build.omg").is_file());
            assert!(root.join("main.omg").is_file());

            let resolved =
                resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve fixture");
            assert!(resolved.file_count >= 3);
            assert!(identities.insert(resolved.content_identity));
        }
        assert_eq!(identities.len(), PACKAGE_FIXTURES.len());
    }

    #[test]
    fn local_source_identity_is_order_independent_and_ignores_git_dir() {
        let root = temp_root("identity");
        std::fs::create_dir_all(root.join("src")).expect("create source tree");
        std::fs::create_dir_all(root.join(".git")).expect("create git dir");
        std::fs::write(root.join("src/lib.omg"), "machine Lib::id() {}\n").expect("write source");
        std::fs::write(root.join("README.md"), "package\n").expect("write readme");
        std::fs::write(root.join(".git/index"), "ignored").expect("write ignored git data");

        let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");
        std::fs::write(root.join(".git/index"), "ignored but changed")
            .expect("change ignored git data");
        let second = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

        assert_eq!(first.file_count, 2);
        assert_eq!(first.content_identity, second.content_identity);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_package_identity_excludes_only_root_build_output() {
        let root = temp_root("root-build-output");
        std::fs::create_dir_all(root.join("build")).expect("create root build output");
        std::fs::create_dir_all(root.join("src/build")).expect("create nested source directory");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n")
            .expect("write package source");
        std::fs::write(
            root.join("build/00_pipeline.html"),
            "first generated report",
        )
        .expect("write generated report");
        std::fs::write(
            root.join("src/build/rules.omg"),
            "machine Rules::apply() {}\n",
        )
        .expect("write nested source");

        let first = resolve_local_source(&root, LocalSourceLimits::default())
            .expect("resolve local package");
        std::fs::write(
            root.join("build/00_pipeline.html"),
            "changed generated report",
        )
        .expect("change generated report");
        let changed_output = resolve_local_source(&root, LocalSourceLimits::default())
            .expect("resolve package after output change");
        assert_eq!(first.file_count, 2);
        assert_eq!(first.content_identity, changed_output.content_identity);

        std::fs::write(
            root.join("src/build/rules.omg"),
            "machine Rules::replace() {}\n",
        )
        .expect("change nested source");
        let changed_source = resolve_local_source(&root, LocalSourceLimits::default())
            .expect("resolve package after source change");
        assert_ne!(
            changed_output.content_identity,
            changed_source.content_identity
        );

        let exact = resolve_materialized_source(&root, LocalSourceLimits::default())
            .expect("resolve exact materialized tree");
        assert_eq!(exact.file_count, 3);
        assert_ne!(changed_source.content_identity, exact.content_identity);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_source_identity_changes_when_source_bytes_change() {
        let root = temp_root("bytes");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::write(root.join("main.omg"), "machine Main::a() {}\n").expect("write source");
        let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

        std::fs::write(root.join("main.omg"), "machine Main::b() {}\n").expect("rewrite source");
        let second = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

        assert_ne!(first.content_identity, second.content_identity);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_capture_does_not_follow_replaced_regular_leaf() {
        let root = temp_root("nofollow-replaced-file");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::write(root.join("source.omg"), "classified bytes")
            .expect("write classified source");
        std::fs::write(root.join("replacement.omg"), "replacement bytes")
            .expect("write replacement source");
        let canonical_root = root.canonicalize().expect("canonicalize source root");
        let directory = CapabilityDirectory::open_ambient_dir(&canonical_root, ambient_authority())
            .expect("open source root capability");
        assert!(
            directory
                .symlink_metadata("source.omg")
                .expect("classify source leaf")
                .is_file()
        );

        std::fs::remove_file(root.join("source.omg")).expect("remove classified source");
        std::os::unix::fs::symlink("replacement.omg", root.join("source.omg"))
            .expect("replace source with symlink");
        let _error = read_capability_file_bounded(
            &directory,
            OsStr::new("source.omg"),
            &canonical_root.join("source.omg"),
            LocalSourceLimits::default().max_bytes,
            LocalSourceLimits::default().max_bytes,
        )
        .expect_err("capture must not follow a replacement symlink");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_capture_does_not_follow_replaced_directory_leaf() {
        let root = temp_root("nofollow-replaced-directory");
        std::fs::create_dir_all(root.join("source")).expect("create classified directory");
        std::fs::create_dir_all(root.join("replacement")).expect("create replacement directory");
        let canonical_root = root.canonicalize().expect("canonicalize source root");
        let directory = CapabilityDirectory::open_ambient_dir(&canonical_root, ambient_authority())
            .expect("open source root capability");
        assert!(
            directory
                .symlink_metadata("source")
                .expect("classify source directory")
                .is_dir()
        );

        std::fs::remove_dir(root.join("source")).expect("remove classified directory");
        std::os::unix::fs::symlink("replacement", root.join("source"))
            .expect("replace directory with symlink");
        let _error = open_captured_directory(
            &directory,
            OsStr::new("source"),
            &canonical_root.join("source"),
        )
        .expect_err("capture must not follow a replacement directory symlink");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_capture_does_not_follow_replaced_root_leaf() {
        let root = temp_root("nofollow-replaced-root");
        let retained = root.with_extension("retained");
        let replacement = root.with_extension("replacement");
        std::fs::create_dir_all(&root).expect("create classified source root");
        std::fs::create_dir_all(&replacement).expect("create replacement source root");
        let canonical_root = root.canonicalize().expect("canonicalize source root");

        std::fs::rename(&root, &retained).expect("relocate classified source root");
        std::os::unix::fs::symlink(&replacement, &root).expect("replace source root with symlink");
        let _error = open_canonical_source_root(&canonical_root)
            .expect_err("root acquisition must not follow a replacement symlink");

        std::fs::remove_file(&root).expect("remove replacement root symlink");
        let _ = std::fs::remove_dir_all(&retained);
        let _ = std::fs::remove_dir_all(&replacement);
    }

    #[cfg(unix)]
    #[test]
    fn local_capture_remains_bound_to_open_root_after_path_replacement() {
        let root = temp_root("open-root-replacement");
        let retained = root.with_extension("retained");
        std::fs::create_dir_all(&root).expect("create source root");
        std::fs::write(root.join("main.omg"), "retained bytes").expect("write retained source");
        let canonical_root = root.canonicalize().expect("canonicalize source root");
        let directory = CapabilityDirectory::open_ambient_dir(&canonical_root, ambient_authority())
            .expect("open source root capability");

        std::fs::rename(&root, &retained).expect("relocate opened source root");
        std::fs::create_dir_all(&root).expect("create replacement root");
        std::fs::write(root.join("main.omg"), "replacement bytes")
            .expect("write replacement source");

        let captured = capture_local_source_from_open_root(
            canonical_root,
            directory,
            LocalSourceLimits::default(),
            SourceTreePolicy::LocalPackage,
        )
        .expect("capture through retained root capability");
        let retained_identity = resolve_local_source(&retained, LocalSourceLimits::default())
            .expect("resolve retained source");
        let replacement_identity = resolve_local_source(&root, LocalSourceLimits::default())
            .expect("resolve replacement source");
        assert_eq!(
            captured.normalized.content_identity,
            retained_identity.content_identity
        );
        assert_ne!(
            captured.normalized.content_identity,
            replacement_identity.content_identity
        );

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&retained);
    }

    #[test]
    fn local_source_identity_includes_empty_directory_paths() {
        let root = temp_root("empty-directory-identity");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        let without_empty =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");

        std::fs::create_dir(root.join("generated")).expect("create empty directory");
        let with_empty =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");
        assert_eq!(without_empty.file_count, with_empty.file_count);
        assert_ne!(without_empty.content_identity, with_empty.content_identity);

        std::fs::remove_dir(root.join("generated")).expect("remove empty directory");
        let removed =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");
        assert_eq!(without_empty.content_identity, removed.content_identity);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_identity_canonicalizes_live_directory_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("directory-mode-identity");
        let directory = root.join("generated");
        std::fs::create_dir_all(&directory).expect("create source tree");
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
            .expect("set writable directory mode");
        let writable =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");

        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555))
            .expect("set read-only directory mode");
        let read_only =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve source");

        assert_eq!(writable.file_count, 0);
        assert_eq!(writable.content_identity, read_only.content_identity);

        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
            .expect("restore directory mode");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_path_encoding_preserves_non_utf8_bytes() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let first = OsString::from_vec(b"source-\x80.omg".to_vec());
        let second = OsString::from_vec(b"source-\x81.omg".to_vec());

        assert_eq!(raw_os_bytes(&first), b"source-\x80.omg");
        assert_eq!(raw_os_bytes(&second), b"source-\x81.omg");
        assert_ne!(raw_os_bytes(&first), raw_os_bytes(&second));
    }

    #[cfg(all(unix, not(target_vendor = "apple")))]
    #[test]
    fn local_source_identity_distinguishes_non_utf8_paths() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let first_root = temp_root("non-utf8-first");
        let second_root = temp_root("non-utf8-second");
        std::fs::create_dir_all(&first_root).expect("create first source tree");
        std::fs::create_dir_all(&second_root).expect("create second source tree");
        let first_name = OsString::from_vec(b"source-\x80.omg".to_vec());
        let second_name = OsString::from_vec(b"source-\x81.omg".to_vec());
        std::fs::write(first_root.join(first_name), "same bytes").expect("write first source");
        std::fs::write(second_root.join(second_name), "same bytes").expect("write second source");

        let first =
            resolve_local_source(&first_root, LocalSourceLimits::default()).expect("resolve first");
        let second = resolve_local_source(&second_root, LocalSourceLimits::default())
            .expect("resolve second");

        assert_ne!(first.content_identity, second.content_identity);

        let _ = std::fs::remove_dir_all(&first_root);
        let _ = std::fs::remove_dir_all(&second_root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_rejects_symlinks_into_excluded_git_metadata() {
        let root = temp_root("symlink-git-metadata");
        std::fs::create_dir_all(root.join(".git")).expect("create ignored target directory");
        let target = root.join(".git/target.omg");
        let link = root.join("linked.omg");
        std::fs::write(&target, "first target bytes").expect("write target");
        std::os::unix::fs::symlink(".git/target.omg", &link).expect("create symlink");

        let error = resolve_local_source(&root, LocalSourceLimits::default())
            .expect_err("excluded metadata target must reject");
        assert!(matches!(
            error,
            SourceResolveError::SymlinkTargetsExcludedMetadata { .. }
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_rejects_symlinks_into_excluded_root_build_output() {
        let root = temp_root("symlink-build-output");
        std::fs::create_dir_all(root.join("build")).expect("create excluded build output");
        std::fs::write(root.join("build/generated.omg"), "generated")
            .expect("write generated output");
        std::os::unix::fs::symlink("build/generated.omg", root.join("linked.omg"))
            .expect("create source symlink");

        let error = resolve_local_source(&root, LocalSourceLimits::default())
            .expect_err("excluded build-output target must reject");
        assert!(matches!(
            error,
            SourceResolveError::SymlinkTargetsExcludedBuildOutput { .. }
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_rejects_absolute_symlink_targets_inside_the_live_root() {
        let root = temp_root("absolute-symlink-target");
        std::fs::create_dir_all(&root).expect("create source tree");
        let target = root.join("target.omg");
        std::fs::write(&target, "target bytes").expect("write target");
        std::os::unix::fs::symlink(&target, root.join("linked.omg"))
            .expect("create absolute source symlink");

        let error = resolve_local_source(&root, LocalSourceLimits::default())
            .expect_err("absolute spelling cannot remain snapshot-rooted after publication");
        assert!(matches!(
            error,
            SourceResolveError::SymlinkEscapesRoot { .. }
        ));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_identity_hashes_internal_symlink_spelling_and_reachable_target() {
        let root = temp_root("symlink-identity");
        std::fs::create_dir_all(&root).expect("create source tree");
        let target = root.join("target.omg");
        let link = root.join("linked.omg");
        std::fs::write(&target, "first target bytes").expect("write target");
        std::os::unix::fs::symlink("target.omg", &link).expect("create symlink");

        let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");
        std::fs::write(&target, "different target bytes").expect("rewrite target");
        let changed_target =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve target");
        assert_ne!(first.content_identity, changed_target.content_identity);

        std::fs::remove_file(&link).expect("remove symlink");
        std::os::unix::fs::symlink("./target.omg", &link).expect("recreate symlink");
        let changed_spelling = resolve_local_source(&root, LocalSourceLimits::default())
            .expect("resolve spelling change");
        assert_ne!(
            changed_target.content_identity,
            changed_spelling.content_identity
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_identity_distinguishes_executable_mode() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("executable-mode");
        std::fs::create_dir_all(&root).expect("create source tree");
        let source = root.join("generate");
        std::fs::write(&source, "same bytes").expect("write source");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
            .expect("make source non-executable");
        let non_executable =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve mode");

        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
            .expect("make source executable");
        let executable =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve mode");

        assert_ne!(non_executable.content_identity, executable.content_identity);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_rejects_special_file_kind() {
        use std::os::unix::net::UnixListener;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = PathBuf::from("/tmp").join(format!(
            "omega-source-socket-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create source tree");
        let socket_path = root.join("source.sock");
        let listener = UnixListener::bind(&socket_path).expect("create Unix socket");
        let expected_path = root
            .canonicalize()
            .expect("canonicalize source tree")
            .join("source.sock");

        let error = resolve_local_source(&root, LocalSourceLimits::default())
            .expect_err("special file should reject");

        assert_eq!(
            error,
            SourceResolveError::UnsupportedFileType {
                path: expected_path
            }
        );

        drop(listener);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_source_limits_reject_too_many_files() {
        let root = temp_root("files");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::write(root.join("a.omg"), "").expect("write source");

        let error = resolve_local_source(
            &root,
            LocalSourceLimits {
                max_files: 0,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("file limit should reject");

        assert_eq!(error, SourceResolveError::TooManyFiles { limit: 0 });

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_entry_limit_rejects_excess_before_classification_or_read() {
        let root = temp_root("entry-limit-before-read");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::write(root.join("a.omg"), "accepted entry").expect("write accepted source");
        std::os::unix::fs::symlink("/outside-source-root", root.join("b.omg"))
            .expect("create excess escaping link");

        let error = resolve_local_source(
            &root,
            LocalSourceLimits {
                max_files: 1,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("entry limit must reject before classifying the excess leaf");
        assert_eq!(error, SourceResolveError::TooManyFiles { limit: 1 });

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_directory_collection_is_bounded_without_counting_reserved_exclusions() {
        let root = temp_root("bounded-directory-listing");
        std::fs::create_dir_all(root.join(".git")).expect("create excluded metadata");
        std::fs::create_dir_all(root.join("build")).expect("create excluded build output");
        std::fs::write(root.join("first.omg"), "").expect("write first source");
        std::fs::write(root.join("second.omg"), "").expect("write second source");
        let limits = LocalSourceLimits {
            max_files: 2,
            ..LocalSourceLimits::default()
        };

        let accepted = resolve_local_source(&root, limits)
            .expect("the two reserved exclusions must not consume source identity entries");
        assert_eq!(accepted.file_count, 2);

        std::fs::write(root.join("third.omg"), "").expect("write excess source");
        assert_eq!(
            resolve_local_source(&root, limits)
                .expect_err("directory collection must stop at its bounded allowance"),
            SourceResolveError::TooManyFiles { limit: 2 }
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_source_limits_count_directories_and_report_identity_entries() {
        let root = temp_root("directory-entry-limit");
        std::fs::create_dir_all(root.join("nested")).expect("create source tree");
        std::fs::write(root.join("nested/main.omg"), "").expect("write source");

        let error = resolve_local_source(
            &root,
            LocalSourceLimits {
                max_files: 1,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("directory and file must consume separate identity entries");

        assert_eq!(error, SourceResolveError::TooManyFiles { limit: 1 });
        assert_eq!(
            error.to_string(),
            "source root exceeds identity entry limit of 1"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_source_limits_reject_file_before_reading_past_byte_limit() {
        let root = temp_root("bytes-limit");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::write(root.join("source.omg"), "four").expect("write source");

        let error = resolve_local_source(
            &root,
            LocalSourceLimits {
                max_bytes: 3,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("byte limit should reject");

        assert_eq!(error, SourceResolveError::TooManyBytes { limit: 3 });

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn local_source_rejects_symlink_escape() {
        let root = temp_root("symlink");
        let outside = temp_root("outside");
        std::fs::create_dir_all(&root).expect("create source tree");
        std::fs::create_dir_all(&outside).expect("create outside tree");
        std::fs::write(outside.join("secret.omg"), "secret").expect("write outside source");
        std::os::unix::fs::symlink(outside.join("secret.omg"), root.join("secret.omg"))
            .expect("create escaping symlink");

        let error =
            resolve_local_source(&root, LocalSourceLimits::default()).expect_err("escape rejects");

        assert!(matches!(
            error,
            SourceResolveError::SymlinkEscapesRoot { .. }
        ));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn local_snapshot_preserves_empty_directories_and_uses_published_identity() {
        let root = temp_root("local-snapshot-empty-directory");
        let cache = temp_root("local-snapshot-empty-directory-cache");
        std::fs::create_dir_all(root.join("generated/empty")).expect("create empty directory");
        std::fs::create_dir_all(root.join(".git")).expect("create excluded metadata");
        std::fs::create_dir_all(root.join("build")).expect("create excluded build output");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        std::fs::write(root.join(".git/index"), "excluded").expect("write Git metadata");
        std::fs::write(root.join("build/omega-program"), "excluded").expect("write build output");
        let live = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve live");

        let resolved = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
            .expect("snapshot local source");

        assert_eq!(resolved.requested_root, root);
        assert_eq!(resolved.canonical_live_root, live.root);
        assert_ne!(resolved.snapshot_root, resolved.canonical_live_root);
        assert_eq!(resolved.normalized.root, resolved.snapshot_root);
        assert!(resolved.snapshot_root.join("generated/empty").is_dir());
        assert!(!resolved.snapshot_root.join(".git").exists());
        assert!(!resolved.snapshot_root.join("build").exists());
        assert_eq!(resolved.normalized.file_count, 1);
        assert_eq!(resolved.normalized.byte_count, live.byte_count);
        assert_eq!(resolved.normalized.content_identity, live.content_identity);
        assert!(
            resolved
                .snapshot_root
                .parent()
                .expect("publication root")
                .join(LOCAL_SNAPSHOT_METADATA)
                .is_file()
        );
        assert!(
            !resolved
                .snapshot_root
                .join(LOCAL_SNAPSHOT_METADATA)
                .exists()
        );

        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn local_snapshot_detects_live_mutation_and_removes_staging_tree() {
        let root = temp_root("local-snapshot-mutation");
        let cache = temp_root("local-snapshot-mutation-cache");
        std::fs::create_dir_all(&root).expect("create source");
        std::fs::write(root.join("main.omg"), "machine Before::main() {}\n")
            .expect("write initial source");
        let captured = capture_local_source(
            &root,
            LocalSourceLimits::default(),
            SourceTreePolicy::LocalPackage,
        )
        .expect("capture source");
        let captured_custody_identity = local_snapshot_custody_identity(
            &captured.normalized.root,
            &captured.normalized.content_identity,
        );
        std::fs::write(root.join("main.omg"), "machine After::main() {}\n")
            .expect("mutate live source");

        let error =
            publish_local_snapshot(root.clone(), captured, &cache, LocalSourceLimits::default())
                .expect_err("concurrent mutation must reject");
        assert!(matches!(
            error,
            SourceResolveError::LocalSourceChanged { .. }
        ));
        let snapshots = cache.join(LOCAL_CACHE_SNAPSHOTS);
        assert!(
            !snapshots
                .join(format!("source-{captured_custody_identity}"))
                .exists()
        );
        assert!(
            std::fs::read_dir(&snapshots)
                .expect("read snapshot collection")
                .all(|entry| !entry
                    .expect("snapshot collection entry")
                    .file_name()
                    .to_string_lossy()
                    .contains(".stage-"))
        );

        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn byte_identical_local_sources_retain_distinct_custody_roots() {
        let first_root = temp_root("local-snapshot-first-lineage");
        let second_root = temp_root("local-snapshot-second-lineage");
        let cache = temp_root("local-snapshot-lineage-cache");
        for root in [&first_root, &second_root] {
            std::fs::create_dir_all(root).expect("create source root");
            std::fs::write(
                root.join("main.omg"),
                "pub machine identity() -> u64 { 1 }\n",
            )
            .expect("write identical source");
        }

        let first =
            resolve_local_source_snapshot(&first_root, &cache, LocalSourceLimits::default())
                .expect("publish first lineage snapshot");
        let second =
            resolve_local_source_snapshot(&second_root, &cache, LocalSourceLimits::default())
                .expect("publish second lineage snapshot");

        assert_eq!(
            first.normalized.content_identity, second.normalized.content_identity,
            "content identity must remain independent of source lineage"
        );
        assert_ne!(first.canonical_live_root, second.canonical_live_root);
        assert_ne!(
            first.snapshot_root, second.snapshot_root,
            "distinct lineages need distinct physical custody roots for compiler attribution"
        );
        assert_eq!(
            resolve_local_source_snapshot(&first_root, &cache, LocalSourceLimits::default())
                .expect("reuse first lineage snapshot"),
            first
        );
        assert_eq!(
            resolve_local_source_snapshot(&second_root, &cache, LocalSourceLimits::default())
                .expect("reuse second lineage snapshot"),
            second
        );

        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&first_root);
        let _ = std::fs::remove_dir_all(&second_root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn local_snapshot_rejects_cache_inside_source_before_creating_it() {
        let root = temp_root("local-snapshot-overlap");
        let cache = root.join("target/omega-cache");
        std::fs::create_dir_all(&root).expect("create source");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

        let error = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
            .expect_err("overlapping cache must reject");
        assert!(matches!(
            error,
            SourceResolveError::LocalSnapshotCacheOverlapsSource { .. }
        ));
        assert!(!cache.exists());
        assert!(!root.join("target").exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_snapshot_rejects_live_source_beneath_snapshot_collection() {
        let cache = temp_root("local-snapshot-containing-cache");
        let root = cache.join(LOCAL_CACHE_SNAPSHOTS).join("imported/source");
        std::fs::create_dir_all(&root).expect("create nested source");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

        let error = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
            .expect_err("resolver-owned collection source must reject");
        assert!(matches!(
            error,
            SourceResolveError::LocalSnapshotCacheOverlapsSource { .. }
        ));

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn local_snapshot_canonicalizes_permissions_and_preserves_symlink_spelling() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("local-snapshot-modes-symlink");
        let cache = temp_root("local-snapshot-modes-symlink-cache");
        std::fs::create_dir_all(root.join("tools")).expect("create tools");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");
        std::fs::write(root.join("tools/generate"), "generator\n").expect("write executable");
        std::fs::set_permissions(root.join("tools"), std::fs::Permissions::from_mode(0o700))
            .expect("set live directory mode");
        std::fs::set_permissions(
            root.join("tools/generate"),
            std::fs::Permissions::from_mode(0o711),
        )
        .expect("set executable mode");
        std::os::unix::fs::symlink("generate", root.join("tools/current"))
            .expect("create relative symlink");

        let resolved = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
            .expect("snapshot local source");
        let mode = |path: &Path| {
            std::fs::symlink_metadata(path)
                .expect("snapshot metadata")
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode(&resolved.snapshot_root), 0o555);
        assert_eq!(mode(&resolved.snapshot_root.join("tools")), 0o555);
        assert_eq!(mode(&resolved.snapshot_root.join("main.omg")), 0o444);
        assert_eq!(mode(&resolved.snapshot_root.join("tools/generate")), 0o555);
        assert_eq!(
            std::fs::read_link(resolved.snapshot_root.join("tools/current"))
                .expect("read snapshot symlink"),
            PathBuf::from("generate")
        );
        assert_eq!(
            std::fs::read(resolved.snapshot_root.join("tools/current"))
                .expect("follow snapshot symlink"),
            b"generator\n"
        );

        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn local_snapshot_reuse_rehashes_and_rejects_tampering() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("local-snapshot-reuse");
        let cache = temp_root("local-snapshot-reuse-cache");
        std::fs::create_dir_all(&root).expect("create source");
        std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

        let first = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
            .expect("publish snapshot");
        let second = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
            .expect("reuse snapshot");
        assert_eq!(first, second);

        std::fs::set_permissions(&first.snapshot_root, std::fs::Permissions::from_mode(0o755))
            .expect("make snapshot root writable");
        let source = first.snapshot_root.join("main.omg");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
            .expect("make snapshot file writable");
        std::fs::write(&source, "machine Tampered::main() {}\n").expect("tamper snapshot");

        let error = resolve_local_source_snapshot(&root, &cache, LocalSourceLimits::default())
            .expect_err("tampered snapshot must reject");
        assert!(matches!(
            error,
            SourceResolveError::LocalSnapshotInvalid { .. }
        ));

        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_source_resolves_exact_commit_and_local_identity() {
        let (repo, commit) = create_git_source("git");
        let cache = temp_root("git-cache");

        let resolved = resolve_git_source(
            &local_git_request(&repo, &commit),
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("resolve git source");

        assert_eq!(resolved.commit, commit);
        assert_eq!(resolved.local.file_count, 1);
        assert!(!resolved.tree.is_empty());

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_source_authenticates_and_materializes_empty_subtrees() {
        let (repo, _) = create_git_source("git-empty-subtree");
        let commit = add_empty_tree_commit(&repo);
        let cache = temp_root("git-empty-subtree-cache");

        let resolved = resolve_git_source(
            &local_git_request(&repo, &commit),
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("resolve Git source containing an explicit empty subtree");

        assert_eq!(resolved.commit, commit);
        assert_eq!(resolved.local.file_count, 1);
        assert!(resolved.snapshot_root.join("empty").is_dir());

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_source_authenticates_sha256_object_graph() {
        let (repo, commit) = create_git_source_with_format("git-sha256", Some("sha256"));
        let cache = temp_root("git-sha256-cache");

        let resolved = resolve_git_source(
            &local_git_request(&repo, &commit),
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("resolve SHA-256 git source");

        assert_eq!(resolved.commit, commit);
        assert_eq!(resolved.commit.len(), 64);
        assert_eq!(resolved.tree.len(), 64);
        assert_eq!(resolved.local.file_count, 1);

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_source_discovers_sha256_for_symbolic_revision() {
        let (repo, commit) = create_git_source_with_format("git-sha256-symbolic", Some("sha256"));
        let cache = temp_root("git-sha256-symbolic-cache");

        let resolved = resolve_git_source(
            &local_git_request(&repo, "HEAD"),
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("discover and resolve symbolic SHA-256 git source");

        assert_eq!(resolved.commit, commit);
        assert_eq!(resolved.commit.len(), 64);
        assert_eq!(resolved.tree.len(), 64);

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_remote_object_format_parser_rejects_absent_malformed_and_mixed_ids() {
        let root = temp_root("git-object-format-parser");
        let sha1 = "11".repeat(20);
        let sha256 = "22".repeat(32);
        assert_eq!(
            parse_git_remote_object_format(
                format!("ref: refs/heads/main\tHEAD\n{sha1}\tHEAD\n").as_bytes(),
                &root,
            )
            .expect("parse SHA-1 remote advertisement"),
            GitObjectIdAlgorithm::Sha1
        );
        assert_eq!(
            parse_git_remote_object_format(
                format!("ref: refs/heads/main\tHEAD\n{sha256}\tHEAD\n").as_bytes(),
                &root,
            )
            .expect("parse SHA-256 remote advertisement"),
            GitObjectIdAlgorithm::Sha256
        );
        for invalid in [
            b"ref: refs/heads/main\tHEAD\n".to_vec(),
            b"not-a-row\n".to_vec(),
            format!("{sha1}\tHEAD\n{sha256}\trefs/heads/main\n").into_bytes(),
        ] {
            assert!(matches!(
                parse_git_remote_object_format(&invalid, &root),
                Err(SourceResolveError::GitCacheInvalid { .. })
                    | Err(SourceResolveError::GitObjectInvalid { .. })
            ));
        }
        assert!(!root.exists());
    }

    #[test]
    fn git_tree_authentication_matches_git_prefix_ordering() {
        let (repo, _) = create_git_source("git-prefix-ordering");
        std::fs::create_dir(repo.join("name")).expect("create prefix directory");
        std::fs::write(repo.join("name/child.omg"), "// child\n").expect("write child");
        std::fs::write(repo.join("name.ext"), "// sibling\n").expect("write sibling");
        run_test_git(&repo, ["add", "."]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "exercise tree ordering"]);
        let cache = temp_root("git-prefix-ordering-cache");

        let resolved = resolve_git_source(
            &local_git_request(&repo, "HEAD"),
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("Git tree reconstruction must use canonical directory ordering");

        assert!(resolved.snapshot_root.join("name/child.omg").is_file());
        assert!(resolved.snapshot_root.join("name.ext").is_file());
        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_source_fetches_only_the_selected_revision_depth() {
        let (repo, _) = create_git_source("git-shallow");
        std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n")
            .expect("change source");
        run_test_git(&repo, ["add", "main.omg"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "second"]);
        let cache = temp_root("git-shallow-cache");
        let request = local_git_request(&repo, "HEAD");

        resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("resolve a shallow exact revision");

        let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
        let output = Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["rev-list", "--count", "FETCH_HEAD"])
            .output()
            .expect("count fetched history");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");
        assert!(repository.join("shallow").is_file());

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_fetch_request_is_depth_one_and_omits_individually_inadmissible_blobs() {
        let arguments = bounded_git_fetch_arguments(
            "https://example.invalid/package.git",
            "0123456789012345678901234567890123456789",
            LocalSourceLimits {
                max_bytes: 4096,
                ..LocalSourceLimits::default()
            },
        );
        let arguments = arguments
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            arguments,
            [
                "fetch",
                "--quiet",
                "--depth=1",
                "--no-tags",
                "--no-recurse-submodules",
                "--filter=blob:limit=4097",
                "--",
                "https://example.invalid/package.git",
                "0123456789012345678901234567890123456789",
            ]
        );
    }

    #[test]
    fn git_fetch_omits_a_blob_above_the_source_byte_ceiling_and_rejects() {
        let (repo, _) = create_git_source("git-filtered-oversized-blob");
        std::fs::write(repo.join("oversized.bin"), vec![0x5a; 4096]).expect("write oversized blob");
        run_test_git(&repo, ["add", "oversized.bin"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "add oversized blob"]);
        run_test_git(&repo, ["config", "uploadpack.allowFilter", "true"]);
        let commit = run_test_git_with_input(&repo, ["rev-parse", "HEAD"], b"");
        let oversized_blob =
            run_test_git_with_input(&repo, ["rev-parse", "HEAD:oversized.bin"], b"");
        let cache = temp_root("git-filtered-oversized-blob-cache");
        let mut request = local_git_request(&repo, &commit);
        request.fetch_locator = format!("file://{}", repo.display());

        let limits = LocalSourceLimits {
            max_bytes: 1024,
            ..LocalSourceLimits::default()
        }
        .compiler_bounded();
        std::fs::create_dir_all(&cache).expect("create resolver cache");
        let canonical_cache = cache.canonicalize().expect("canonical resolver cache");
        verify_git_cache_root_custody(&canonical_cache).expect("verify resolver cache custody");
        let execution_transport = request.execution_transport();
        let executor = GitExecutor::system(execution_transport).expect("select test Git executor");
        let cache_identity = git_cache_identity(
            request.locator_identity(),
            request.requested_revision(),
            execution_transport,
        );
        let entry_root = canonical_cache.join(format!("git-{cache_identity}"));
        let cache_directory =
            open_absolute_directory_nofollow(&canonical_cache).expect("retain resolver cache");
        let entry_name = entry_root.file_name().expect("cache entry has a name");
        create_git_cache_entry(
            &executor,
            &canonical_cache,
            &cache_directory,
            &entry_root,
            entry_name,
            &cache_identity,
            request.locator_identity(),
            request.fetch_locator(),
            request.requested_revision(),
            execution_transport,
            limits,
        )
        .expect("create quarantined Git cache entry");
        let error = resolve_verified_git_cache_entry(
            &executor,
            &cache_directory,
            entry_name,
            &entry_root,
            request.requested_locator(),
            request.locator_identity(),
            request.fetch_locator(),
            request.requested_revision(),
            execution_transport,
            limits,
            true,
        )
        .expect_err("a required blob above the source ceiling must not be acquired");

        assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
        let repository = entry_root.join(GIT_CACHE_REPOSITORY);
        let output = Command::new("git")
            .env("GIT_NO_LAZY_FETCH", "1")
            .arg("-C")
            .arg(&repository)
            .args(["cat-file", "-e", &oversized_blob])
            .output()
            .expect("inspect quarantined object store");
        assert!(
            !output.status.success(),
            "the inadmissible blob must remain absent from resolver custody"
        );
        assert!(entry_root.join(GIT_CACHE_METADATA).exists());
        assert!(!entry_root.join(GIT_CACHE_SNAPSHOTS).exists());

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn exact_git_revision_reuses_authenticated_objects_without_transport() {
        let (repo, commit) = create_git_source("git-exact-offline-reuse");
        let cache = temp_root("git-exact-offline-reuse-cache");
        let request = local_git_request(&repo, &commit);
        let first = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("resolve exact revision");
        let offline_repo = repo.with_extension("offline");
        std::fs::rename(&repo, &offline_repo).expect("make source transport unavailable");

        let second = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("reuse exact resolver custody without transport");

        assert_eq!(second.commit, first.commit);
        assert_eq!(second.tree, first.tree);
        assert_eq!(second.snapshot_root, first.snapshot_root);
        assert_eq!(second.local, first.local);

        let _ = std::fs::remove_dir_all(&offline_repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn exact_git_revision_offline_reuse_still_enforces_source_limits() {
        let (repo, commit) = create_git_source("git-exact-offline-limits");
        let cache = temp_root("git-exact-offline-limits-cache");
        let request = local_git_request(&repo, &commit);
        resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("resolve exact revision");
        let offline_repo = repo.with_extension("offline");
        std::fs::rename(&repo, &offline_repo).expect("make source transport unavailable");

        let error = resolve_git_source(
            &request,
            &cache,
            LocalSourceLimits {
                max_bytes: 0,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("cached exact source must remain subject to current limits");

        assert_eq!(error, SourceResolveError::TooManyBytes { limit: 0 });

        let _ = std::fs::remove_dir_all(&offline_repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn symbolic_git_revision_still_refetches_and_observes_movement() {
        let (repo, first_commit) = create_git_source("git-symbolic-refresh");
        let cache = temp_root("git-symbolic-refresh-cache");
        let request = local_git_request(&repo, "HEAD");
        let first = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("resolve initial symbolic revision");
        assert_eq!(first.commit, first_commit);

        std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n")
            .expect("change source");
        run_test_git(&repo, ["add", "main.omg"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "move symbolic revision"]);
        let second_commit = run_test_git_with_input(&repo, ["rev-parse", "HEAD"], b"");

        let second = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("refresh symbolic revision");

        assert_eq!(second.commit, second_commit);
        assert_ne!(second.commit, first.commit);
        assert_eq!(
            std::fs::read(second.snapshot_root.join("main.omg")).expect("read refreshed source"),
            b"machine Main::changed() {}\n"
        );

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_tree_rejects_traversal_metadata_and_nonportable_paths_before_materialization() {
        let repository = temp_root("git-tree-path-validation");
        let oid = "0123456789012345678901234567890123456789";
        for path in [
            b"../escape.omg".as_slice(),
            b"nested/../../escape.omg".as_slice(),
            b"/absolute.omg".as_slice(),
            b"nested\\ambiguous.omg".as_slice(),
            b"nested/.git/config".as_slice(),
            b"C:/drive-prefixed.omg".as_slice(),
            b"nested/NUL.txt".as_slice(),
            b"aux.omg".as_slice(),
            b"name:stream.omg".as_slice(),
            b"trailing.".as_slice(),
            b"trailing ".as_slice(),
            b"question?.omg".as_slice(),
        ] {
            let mut listing = format!("100644 blob {oid} 1\t").into_bytes();
            listing.extend_from_slice(path);
            listing.push(0);
            let error = parse_git_tree_entries(&listing, &repository, LocalSourceLimits::default())
                .expect_err("unsafe Git path must reject");
            assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
        }
        assert!(
            !repository.exists(),
            "validation must not create a staging path"
        );
    }

    #[test]
    fn git_symlink_targets_reject_windows_ambiguous_spellings() {
        for target in [
            b"C:/escape.omg".as_slice(),
            b"NUL".as_slice(),
            b"nested/COM1.log".as_slice(),
            b"name:stream".as_slice(),
            b"trailing.".as_slice(),
        ] {
            assert!(matches!(
                validate_git_symlink_target(b"link", target),
                Err(SourceResolveError::GitTreeInvalid { .. })
            ));
        }
    }

    #[test]
    fn git_tree_enforces_declared_limits_before_reading_blobs() {
        let repository = temp_root("git-tree-limit-validation");
        let oid = "0123456789012345678901234567890123456789";
        let listing = format!("100644 blob {oid} 4\tmain.omg\0");

        let error = parse_git_tree_entries(
            listing.as_bytes(),
            &repository,
            LocalSourceLimits {
                max_bytes: 3,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("oversized tree must reject from metadata");

        assert_eq!(error, SourceResolveError::TooManyBytes { limit: 3 });
        assert!(
            !repository.exists(),
            "limit rejection must not inspect an object"
        );
    }

    #[test]
    fn git_tree_entry_limit_counts_declared_directories() {
        let repository = temp_root("git-tree-directory-limit");
        let oid = "0123456789012345678901234567890123456789";
        let listing =
            format!("040000 tree {oid} -\tnested\0100644 blob {oid} 0\tnested/main.omg\0");

        let error = parse_git_tree_entries(
            listing.as_bytes(),
            &repository,
            LocalSourceLimits {
                max_files: 1,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("directory and blob must consume separate identity entries");

        assert_eq!(error, SourceResolveError::TooManyFiles { limit: 1 });
        assert!(!repository.exists());
    }

    #[test]
    fn git_tree_rejects_gitlinks_before_materialization() {
        let repository = temp_root("gitlink-validation");
        let oid = "0123456789012345678901234567890123456789";
        let listing = format!("160000 commit {oid} -\tdependency\0");

        let error = parse_git_tree_entries(
            listing.as_bytes(),
            &repository,
            LocalSourceLimits::default(),
        )
        .expect_err("gitlink must reject");

        assert!(matches!(
            error,
            SourceResolveError::GitSubmodulesUnsupported { .. }
        ));
        assert!(!repository.exists());
    }

    #[cfg(unix)]
    #[test]
    fn git_snapshot_preserves_paths_executable_modes_and_symlink_spelling() {
        use std::os::unix::fs::PermissionsExt;

        let (repo, _) = create_git_source("git-snapshot-kinds");
        let script = repo.join("tools/generate");
        std::fs::create_dir_all(script.parent().expect("script parent")).expect("create tools");
        std::fs::write(&script, "#!/bin/sh\n").expect("write script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("mark script executable");
        std::os::unix::fs::symlink("generate", repo.join("tools/current"))
            .expect("create source symlink");
        run_test_git(&repo, ["add", "tools/generate", "tools/current"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "add exact entry kinds"]);
        let cache = temp_root("git-snapshot-kinds-cache");
        let request = local_git_request(&repo, "HEAD");

        let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("resolve kinds");
        let published_script = resolved.snapshot_root.join("tools/generate");
        let published_link = resolved.snapshot_root.join("tools/current");

        assert_eq!(
            std::fs::read(&published_script).expect("read script"),
            b"#!/bin/sh\n"
        );
        assert_ne!(
            std::fs::metadata(&published_script)
                .expect("script metadata")
                .permissions()
                .mode()
                & 0o111,
            0
        );
        assert_eq!(
            raw_os_bytes(
                std::fs::read_link(&published_link)
                    .expect("read published symlink")
                    .as_os_str()
            ),
            b"generate"
        );
        assert_eq!(resolved.local.file_count, 3);
        assert_eq!(
            std::fs::metadata(resolved.snapshot_root.join("tools"))
                .expect("nested directory metadata")
                .permissions()
                .mode()
                & 0o7777,
            u32::from(CANONICAL_DIRECTORY_MODE)
        );
        let verified = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("verify nested snapshot reuse");
        assert_eq!(resolved.local, verified.local);

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_snapshot_uses_blob_bytes_not_checkout_attribute_conversions() {
        let (repo, _) = create_git_source("git-snapshot-attributes");
        std::fs::write(repo.join(".gitattributes"), "*.omg eol=crlf\n")
            .expect("write checkout conversion attribute");
        run_test_git(&repo, ["add", ".gitattributes"]);
        run_test_git(
            &repo,
            ["commit", "--quiet", "-m", "add checkout conversion"],
        );
        let cache = temp_root("git-snapshot-attributes-cache");
        let request = local_git_request(&repo, "HEAD");

        let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("materialize object bytes");

        assert_eq!(
            std::fs::read(resolved.snapshot_root.join("main.omg")).expect("read snapshot blob"),
            b"machine Main::main() {}\n"
        );
        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn git_snapshot_reuse_rejects_content_with_forged_matching_metadata() {
        use std::os::unix::fs::PermissionsExt;

        let (repo, _) = create_git_source("git-snapshot-reuse");
        let cache = temp_root("git-snapshot-reuse-cache");
        let request = local_git_request(&repo, "HEAD");
        let first = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("first resolve");
        let second = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("reuse snapshot");
        assert_eq!(first.snapshot_root, second.snapshot_root);
        assert_eq!(first.local, second.local);

        std::fs::set_permissions(&first.snapshot_root, std::fs::Permissions::from_mode(0o755))
            .expect("make source root writable for tamper simulation");
        let source = first.snapshot_root.join("main.omg");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
            .expect("make source writable for tamper simulation");
        std::fs::write(&source, "machine Tampered::main() {}\n").expect("tamper snapshot");
        let publication = first
            .snapshot_root
            .parent()
            .expect("snapshot source has a publication parent");
        let metadata_path = publication.join(GIT_SNAPSHOT_METADATA);
        std::fs::set_permissions(&metadata_path, std::fs::Permissions::from_mode(0o644))
            .expect("make snapshot metadata writable for tamper simulation");
        let forged =
            resolve_materialized_source(&first.snapshot_root, LocalSourceLimits::default())
                .expect("derive the public identity an attacker could recompute");
        std::fs::write(&metadata_path, git_snapshot_metadata(&first.tree, &forged))
            .expect("forge matching snapshot metadata");
        make_snapshot_read_only(publication).expect("restore canonical snapshot modes");

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("tampered snapshot and matching forged metadata must reject");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        let entry = git_cache_entry_root(&cache, &request);
        assert!(!entry.join(GIT_CACHE_METADATA).exists());

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_batch_failure_precedes_snapshot_staging() {
        let (repo, _) = create_git_source("git-snapshot-cleanup");
        let cache = temp_root("git-snapshot-cleanup-cache");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry_root = git_cache_entry_root(&cache, &request);
        let repository = entry_root.join(GIT_CACHE_REPOSITORY);
        let missing_oid = "0000000000000000000000000000000000000000";
        let executor =
            GitExecutor::system(GitExecutionTransport::Https).expect("system Git executor");
        let mut entries = vec![GitTreeEntry {
            relative_bytes: b"missing.omg".to_vec(),
            relative_path: PathBuf::from("missing.omg"),
            oid: missing_oid.to_owned(),
            size: 1,
            kind: GitTreeEntryKind::File {
                executable: false,
                bytes: GitBlobBytes::empty(),
            },
        }];
        let error = read_git_blobs_batch_from_path(
            &executor,
            &repository,
            &mut entries,
            LocalSourceLimits::default(),
        )
        .expect_err("missing object must fail before staged materialization");
        assert!(matches!(error, SourceResolveError::GitTreeInvalid { .. }));
        let snapshots = entry_root.join(GIT_CACHE_SNAPSHOTS);
        assert!(
            std::fs::read_dir(&snapshots)
                .expect("read snapshots")
                .all(|entry| !entry
                    .expect("snapshot entry")
                    .file_name()
                    .to_string_lossy()
                    .contains(".stage-")),
            "failed materialization must leave no staging directory"
        );
        assert!(
            !snapshots
                .join("tree-1111111111111111111111111111111111111111")
                .exists()
        );

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_snapshot_excludes_untracked_source_worktree_state() {
        let (repo, _) = create_git_source("git-untracked-source");
        let cache = temp_root("git-untracked-cache");
        std::fs::write(repo.join("injected.omg"), "machine Injected::main() {}\n")
            .expect("write untracked source state");
        let request = local_git_request(&repo, "HEAD");
        let resolved = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("prime cache");

        assert!(!resolved.snapshot_root.join("injected.omg").exists());
        assert_eq!(resolved.local.file_count, 1);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_identity_is_full_policy_versioned_and_injectively_framed() {
        let first = git_cache_identity("a\0b", "c", GitExecutionTransport::Https);
        let second = git_cache_identity("a", "b\0c", GitExecutionTransport::Https);

        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|character| character.is_ascii_hexdigit()));
        assert_ne!(first, second);
        assert_ne!(
            first,
            git_cache_identity("a\0b", "C", GitExecutionTransport::Https)
        );
        assert_ne!(
            first,
            git_cache_identity("a\0b", "c", GitExecutionTransport::Ssh)
        );
    }

    #[test]
    fn git_cache_serializes_access_without_unlinking_its_lock() {
        use std::sync::mpsc;
        use std::time::Duration;

        let cache = temp_root("git-lock");
        std::fs::create_dir_all(&cache).expect("create cache");
        let lock_path = cache.join("entry.lock");
        let first = CacheEntryLock::acquire(&lock_path).expect("acquire first lock");
        let thread_lock_path = lock_path.clone();
        let (sender, receiver) = mpsc::channel();
        let waiter = std::thread::spawn(move || {
            let second =
                CacheEntryLock::acquire(&thread_lock_path).expect("acquire serialized lock");
            sender.send(()).expect("report lock acquisition");
            drop(second);
        });

        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("waiter should acquire released lock");
        waiter.join().expect("join lock waiter");
        assert!(lock_path.is_file());

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_rejects_a_replaced_locked_path() {
        let cache = temp_root("git-lock-replaced");
        std::fs::create_dir_all(&cache).expect("create cache");
        let lock_path = cache.join("entry.lock");
        let displaced_path = cache.join("entry.lock.displaced");
        let file = CacheEntryLock::open_git(&lock_path).expect("open cache lock");
        file.lock().expect("lock cache entry");
        std::fs::rename(&lock_path, &displaced_path).expect("displace locked path");
        std::fs::write(&lock_path, []).expect("replace lock path");

        assert!(matches!(
            verify_cache_lock_path_identity_for_test(CacheCustodyKind::Git, &lock_path, &file),
            Err(SourceResolveError::GitCacheInvalid { .. })
        ));

        file.unlock().expect("unlock displaced cache entry");
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn local_cache_rejects_a_replaced_locked_path() {
        let cache = temp_root("local-lock-replaced");
        std::fs::create_dir_all(&cache).expect("create cache");
        let lock_path = cache.join("entry.lock");
        let displaced_path = cache.join("entry.lock.displaced");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .expect("open local cache lock");
        file.lock().expect("lock local cache entry");
        std::fs::rename(&lock_path, &displaced_path).expect("displace locked path");
        std::fs::write(&lock_path, []).expect("replace lock path");

        assert!(matches!(
            verify_cache_lock_path_identity_for_test(
                CacheCustodyKind::LocalSnapshot,
                &lock_path,
                &file,
            ),
            Err(SourceResolveError::LocalSnapshotInvalid { .. })
        ));

        file.unlock().expect("unlock displaced cache entry");
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn cache_lock_open_does_not_follow_a_preexisting_symlink() {
        let root = temp_root("cache-lock-symlink");
        std::fs::create_dir_all(&root).expect("create cache lock root");
        let target = root.join("target");
        std::fs::write(&target, b"untouched").expect("create symlink target");

        for (name, kind) in [
            ("git.lock", CacheCustodyKind::Git),
            ("local.lock", CacheCustodyKind::LocalSnapshot),
        ] {
            let lock_path = root.join(name);
            std::os::unix::fs::symlink(&target, &lock_path).expect("create cache lock symlink");
            let error = CacheEntryLock::open_retained(kind, &lock_path)
                .expect_err("cache lock open must not follow a symlink");
            assert!(matches!(
                (kind, error),
                (
                    CacheCustodyKind::Git,
                    SourceResolveError::GitCacheInvalid { .. }
                ) | (
                    CacheCustodyKind::LocalSnapshot,
                    SourceResolveError::LocalSnapshotInvalid { .. }
                )
            ));
            std::fs::remove_file(&lock_path).expect("remove cache lock symlink");
        }
        assert_eq!(
            std::fs::read(&target).expect("read symlink target"),
            b"untouched"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cache_lock_identity_rejects_a_replaced_parent_path() {
        for (name, kind) in [
            ("git", CacheCustodyKind::Git),
            ("local", CacheCustodyKind::LocalSnapshot),
        ] {
            let root = temp_root(&format!("cache-lock-parent-replaced-{name}"));
            let cache = root.join("cache");
            let retained = root.join("retained");
            std::fs::create_dir_all(&cache).expect("create cache lock parent");
            let lock_path = cache.join("entry.lock");
            let (file, parent, lock_name) = CacheEntryLock::open_retained(kind, &lock_path)
                .expect("open lock through retained parent");
            file.lock().expect("lock retained cache entry");

            std::fs::rename(&cache, &retained).expect("replace cache lock parent path");
            std::fs::create_dir(&cache).expect("create replacement cache lock parent");
            std::fs::write(cache.join("entry.lock"), []).expect("create replacement lock leaf");
            let error =
                verify_cache_lock_path_identity(kind, &lock_path, &parent, &lock_name, &file)
                    .expect_err("replaced cache lock parent must reject");
            assert!(matches!(
                (kind, error),
                (
                    CacheCustodyKind::Git,
                    SourceResolveError::GitCacheInvalid { .. }
                ) | (
                    CacheCustodyKind::LocalSnapshot,
                    SourceResolveError::LocalSnapshotInvalid { .. }
                )
            ));

            file.unlock().expect("unlock retained cache entry");
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[cfg(unix)]
    #[test]
    fn local_cache_lock_wait_has_a_fail_closed_deadline() {
        let root = temp_root("local-lock-budget");
        std::fs::create_dir_all(&root).expect("create lock budget root");
        let lock_path = root.join("entry.lock");
        let held = CacheEntryLock::acquire_local_with_timeout(&lock_path, Duration::from_secs(1))
            .expect("hold local cache lock");
        let timeout = Duration::from_millis(30);
        let started = Instant::now();

        let result = CacheEntryLock::acquire_local_with_timeout(&lock_path, timeout);

        assert!(matches!(
            result,
            Err(SourceResolveError::LocalSnapshotLockTimedOut {
                ref path,
                timeout_millis: 30,
            }) if path == &lock_path
        ));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "bounded local cache lock acquisition must not become an indefinite wait"
        );
        drop(held);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn local_cache_lock_acquires_after_the_competing_handle_releases() {
        let root = temp_root("local-lock-release");
        std::fs::create_dir_all(&root).expect("create lock release root");
        let lock_path = root.join("entry.lock");
        let held = CacheEntryLock::acquire_local_with_timeout(&lock_path, Duration::from_secs(1))
            .expect("hold local cache lock");
        drop(held);

        CacheEntryLock::acquire_local_with_timeout(&lock_path, Duration::from_secs(1))
            .expect("released local cache lock must become available");

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn git_cache_lock_wait_obeys_the_whole_resolution_budget() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("git-lock-budget");
        std::fs::create_dir_all(&root).expect("create lock budget root");
        let lock_path = root.join("entry.lock");
        let held = CacheEntryLock::acquire(&lock_path).expect("hold cache lock");
        let fake_git = root.join("git");
        std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git");
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
            .expect("make fake Git executable");
        let executor = GitExecutor::open_with_budget(&fake_git, 1, Duration::from_millis(30))
            .expect("capture time-bounded Git");

        assert!(matches!(
            CacheEntryLock::acquire_with_git_budget(&lock_path, &executor),
            Err(SourceResolveError::GitResolutionTimedOut { .. })
        ));

        drop(held);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn only_esrch_proves_a_process_group_is_absent() {
        assert!(process_group_already_absent(
            &std::io::Error::from_raw_os_error(3)
        ));
        assert!(!process_group_already_absent(
            &std::io::Error::from_raw_os_error(1)
        ));
    }

    #[test]
    fn cleanup_failure_outranks_whole_resolution_expiry() {
        let result: Result<(), _> = Err(SourceResolveError::GitCleanupFailed {
            operation: "test".to_owned(),
            message: "process group may remain".to_owned(),
        });
        let budget = Err(SourceResolveError::GitResolutionTimedOut { timeout_millis: 1 });

        assert!(matches!(
            reconcile_git_command_result(result, Ok(()), budget),
            Err(SourceResolveError::GitCleanupFailed { .. })
        ));
    }

    #[test]
    fn cache_namespace_and_invalidation_failures_outrank_operation_failure() {
        let operation = Err::<(), _>(SourceResolveError::Git {
            operation: "test".to_owned(),
            status: Some(1),
            stderr: "operation failed".to_owned(),
        });
        let namespace = Err(cache_invalid(
            Path::new("cache"),
            "namespace reconciliation failed",
        ));
        let error = reconcile_git_cache_operation_result(operation, namespace, None)
            .expect_err("namespace custody must outrank operation failure");
        assert!(matches!(
            error,
            SourceResolveError::GitCacheInvalid { message, .. }
                if message.contains("namespace reconciliation")
        ));

        let operation = Err::<(), _>(SourceResolveError::Git {
            operation: "test".to_owned(),
            status: Some(1),
            stderr: "operation failed".to_owned(),
        });
        let invalidation = Err(cache_invalid(
            Path::new("cache"),
            "invalidation synchronization failed",
        ));
        let error = reconcile_git_cache_operation_result(operation, Ok(()), Some(invalidation))
            .expect_err("invalidation custody must outrank operation failure");
        assert!(matches!(
            error,
            SourceResolveError::GitCacheInvalid { message, .. }
                if message.contains("invalidation synchronization")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn failed_operation_still_reconciles_the_retained_lock_parent() {
        let cache = temp_root("git-failed-operation-parent-reconciliation");
        let retained = cache.with_extension("retained");
        std::fs::create_dir_all(&cache).expect("create cache parent");
        let cache = cache.canonicalize().expect("canonicalize cache parent");
        let lock_path = cache.join("entry.lock");
        let lock = CacheEntryLock::acquire(&lock_path).expect("acquire retained cache lock");

        std::fs::rename(&cache, &retained).expect("replace retained cache parent path");
        std::fs::create_dir(&cache).expect("create replacement cache parent");
        let operation = Err::<(), _>(SourceResolveError::Git {
            operation: "test".to_owned(),
            status: Some(1),
            stderr: "native operation failed".to_owned(),
        });
        let error =
            reconcile_git_cache_operation_result(operation, lock.verify_path_identity(), None)
                .expect_err("post-operation namespace reconciliation must still run");

        assert!(matches!(
            error,
            SourceResolveError::GitCacheInvalid { path, message }
                if path == cache && message.contains("retained directory")
        ));
        drop(lock);
        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::remove_dir_all(&retained);
    }

    #[test]
    fn provisional_git_cache_directory_is_cleaned_if_retention_fails() {
        let cache = temp_root("git-provisional-stage-cleanup");
        std::fs::create_dir_all(&cache).expect("create provisional cache parent");
        let cache = cache.canonicalize().expect("canonicalize cache parent");
        let parent = open_absolute_directory_nofollow(&cache).expect("retain cache parent");
        create_private_cache_directory(&parent, "provisional")
            .expect("create provisional cache directory");
        {
            let _provisional = ProvisionalCacheDirectory::new(&parent, OsStr::new("provisional"));
            // Returning from a failed retention path drops this guard while it
            // still owns the just-created parent-relative name.
        }
        assert!(!cache.join("provisional").exists());
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_rejects_resolver_metadata_substitution() {
        let (repo, _) = create_git_source("git-metadata-source");
        let (substitute, _) = create_git_source("git-metadata-substitute");
        let cache = temp_root("git-metadata-cache");
        let substitute_url = substitute.display().to_string();
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry = git_cache_entry_root(&cache, &request);
        std::fs::write(
            entry.join(GIT_CACHE_METADATA),
            git_cache_metadata(&substitute_url, "HEAD", GitExecutionTransport::File),
        )
        .expect("substitute metadata");

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("substituted metadata must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!entry.join(GIT_CACHE_METADATA).exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&substitute);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn git_cache_invalidation_does_not_follow_a_substituted_entry_symlink() {
        let cache = temp_root("git-invalidation-symlink");
        let target = cache.join("target");
        let entry = cache.join("git-substituted");
        std::fs::create_dir_all(&target).expect("create invalidation target");
        let target_metadata = target.join(GIT_CACHE_METADATA);
        std::fs::write(&target_metadata, b"must remain").expect("write target metadata");
        std::os::unix::fs::symlink(&target, &entry).expect("substitute Git cache entry");

        let error = invalidate_git_cache_entry_from_retained_parent(&entry)
            .expect_err("invalidation must reject a substituted entry symlink");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert_eq!(
            std::fs::read(&target_metadata).expect("read retained target metadata"),
            b"must remain"
        );
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_rejects_transport_profile_substitution() {
        let (repo, _) = create_git_source("git-transport-metadata-source");
        let cache = temp_root("git-transport-metadata-cache");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry = git_cache_entry_root(&cache, &request);
        std::fs::write(
            entry.join(GIT_CACHE_METADATA),
            git_cache_metadata(
                request.locator_identity(),
                request.requested_revision(),
                GitExecutionTransport::Https,
            ),
        )
        .expect("substitute transport profile metadata");

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("substituted transport profile must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!entry.join(GIT_CACHE_METADATA).exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_rejects_repository_config_substitution_without_asking_git() {
        let (repo, _) = create_git_source("git-origin-source");
        let cache = temp_root("git-origin-cache");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
        let config = repository.join("config");
        assert_eq!(std::fs::read(&config).unwrap(), GIT_CONFIG_SHA1);
        let mut substituted = GIT_CONFIG_SHA1.to_vec();
        substituted.extend_from_slice(b"[remote \"origin\"]\n\turl = /substitute\n");
        std::fs::write(&config, substituted).expect("substitute repository config");
        let entry = git_cache_entry_root(&cache, &request);

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("any noncanonical repository configuration must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!entry.join(GIT_CACHE_METADATA).exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn verified_git_repository_rejects_replaced_repository_path() {
        let (repo, _) = create_git_source("git-retained-repository-source");
        let cache = temp_root("git-retained-repository-cache");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let verified = open_verified_git_repository(&cache, &request);
        let repository = verified.path().to_path_buf();
        let displaced = repository.with_file_name("repository.displaced");
        std::fs::rename(&repository, &displaced).expect("displace retained repository");
        std::fs::create_dir_all(repository.join("objects")).expect("create replacement repository");

        let error = verified
            .verify_identity()
            .expect_err("repository replacement must reject");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn verified_git_repository_rejects_replaced_objects_path() {
        let (repo, _) = create_git_source("git-retained-objects-source");
        let cache = temp_root("git-retained-objects-cache");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let verified = open_verified_git_repository(&cache, &request);
        let objects = verified.path().join("objects");
        let displaced = verified.path().join("objects.displaced");
        std::fs::rename(&objects, &displaced).expect("displace retained object store");
        std::fs::create_dir(&objects).expect("create replacement object store");

        let error = verified
            .verify_identity()
            .expect_err("object-store replacement must reject");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_forbidden_record_probe_rejects_non_not_found_errors() {
        let (repo, _) = create_git_source("git-forbidden-probe-source");
        let cache = temp_root("git-forbidden-probe-cache");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry = git_cache_entry_root(&cache, &request);
        let info = entry.join(GIT_CACHE_REPOSITORY).join("objects/info");
        std::fs::remove_dir(&info).expect("remove empty Git info directory");
        std::fs::write(&info, b"not a directory").expect("replace info with a regular file");

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("NotADirectory must not prove a forbidden record absent");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn git_cache_rejects_symlinks_in_owned_repository_namespaces() {
        for relative in ["config", "FETCH_HEAD", "HEAD"] {
            let (repo, _) = create_git_source(&format!("git-symlink-{relative}-source"));
            let cache = temp_root(&format!("git-symlink-{relative}-cache"));
            let request = local_git_request(&repo, "HEAD");
            resolve_git_source(&request, &cache, LocalSourceLimits::default())
                .expect("prime cache");
            let entry = git_cache_entry_root(&cache, &request);
            let repository = entry.join(GIT_CACHE_REPOSITORY);
            let path = repository.join(relative);
            let displaced = repository.join(format!("{relative}.displaced"));
            std::fs::rename(&path, &displaced).expect("displace repository file");
            std::os::unix::fs::symlink(&displaced, &path).expect("install repository symlink");

            let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
                .expect_err("repository symlink must reject");
            assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

            let _ = std::fs::remove_dir_all(&repo);
            make_tree_owner_writable(&cache);
            let _ = std::fs::remove_dir_all(&cache);
        }

        let (repo, _) = create_git_source("git-symlink-object-source");
        let cache = temp_root("git-symlink-object-cache");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
        let object = first_regular_descendant(&repository.join("objects"));
        let displaced = object.with_extension("displaced");
        std::fs::rename(&object, &displaced).expect("displace object payload");
        std::os::unix::fs::symlink(&displaced, &object).expect("install object symlink");

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("object-store symlink must reject");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn git_cache_rejects_multiply_linked_regular_files() {
        let (repo, _) = create_git_source("git-hardlink-source");
        let cache = temp_root("git-hardlink-cache");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry = git_cache_entry_root(&cache, &request);
        let config = entry.join(GIT_CACHE_REPOSITORY).join("config");
        std::fs::hard_link(&config, cache.join("config-alias"))
            .expect("add external hard link to repository file");

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("multiply-linked repository file must reject");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn git_cache_rejects_group_or_other_writable_custody() {
        use std::os::unix::fs::PermissionsExt;

        let (repo, _) = create_git_source("git-custody-source");
        let cache = temp_root("git-custody-cache");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o777))
            .expect("make cache externally writable");

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("externally writable cache custody must reject");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));

        std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn cache_custody_rejects_replaceable_nonsticky_ancestry() {
        use std::os::unix::fs::PermissionsExt;

        let parent = temp_root("replaceable-cache-parent");
        let cache = parent.join("cache");
        std::fs::create_dir_all(&cache).expect("create nested cache");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777))
            .expect("make parent replaceable");
        std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700))
            .expect("keep cache itself private");

        assert!(matches!(
            verify_git_cache_root_custody(&cache),
            Err(SourceResolveError::GitCacheInvalid { .. })
        ));

        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn cache_custody_rejects_logical_resident_byte_overflow() {
        let cache = temp_root("cache-byte-ceiling");
        std::fs::create_dir_all(&cache).expect("create cache");
        std::fs::write(cache.join("oversized"), b"12345").expect("write cache payload");

        assert!(matches!(
            verify_cache_custody(&cache, CacheCustodyKind::Git, 4),
            Err(SourceResolveError::GitCacheInvalid { .. })
        ));

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn bounded_cache_record_read_rejects_content_above_its_exact_limit() {
        let cache = temp_root("bounded-cache-record");
        std::fs::create_dir_all(&cache).expect("create cache record root");
        std::fs::write(cache.join("record"), b"12345").expect("write oversized cache record");

        let error =
            read_bounded_cache_record(CacheCustodyKind::Git, &cache, Path::new("record"), 4)
                .expect_err("oversized cache record must reject before unbounded allocation");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn bounded_cache_record_read_does_not_follow_a_symlink_leaf() {
        let cache = temp_root("bounded-cache-record-symlink");
        std::fs::create_dir_all(&cache).expect("create cache record root");
        let target = cache.join("target");
        std::fs::write(&target, b"outside").expect("write cache record target");
        std::os::unix::fs::symlink(&target, cache.join("record"))
            .expect("create cache record symlink");

        let error = read_bounded_cache_record(
            CacheCustodyKind::LocalSnapshot,
            &cache,
            Path::new("record"),
            64,
        )
        .expect_err("cache record read must not follow a symlink leaf");

        assert!(matches!(
            error,
            SourceResolveError::LocalSnapshotInvalid { .. }
        ));
        assert_eq!(std::fs::read(&target).expect("read target"), b"outside");
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn cache_publication_renames_a_direct_child_through_its_parent_capability() {
        let cache = temp_root("capability-publication");
        std::fs::create_dir_all(&cache).expect("create publication parent");
        let canonical_cache = cache
            .canonicalize()
            .expect("canonicalize publication parent");
        let staged = canonical_cache.join("staged");
        let publication = canonical_cache.join("published");
        std::fs::create_dir_all(&staged).expect("create publication stage");
        std::fs::write(staged.join("payload"), b"retained").expect("write staged payload");

        publish_cache_directory(
            CacheCustodyKind::Git,
            &canonical_cache,
            &staged,
            &publication,
        )
        .expect("publish through retained cache parent");

        assert!(!staged.exists());
        assert_eq!(
            std::fs::read(publication.join("payload")).expect("read published payload"),
            b"retained"
        );
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn cache_publication_rejects_a_preexisting_destination() {
        let cache = temp_root("capability-publication-existing");
        std::fs::create_dir_all(&cache).expect("create publication parent");
        let canonical_cache = cache
            .canonicalize()
            .expect("canonicalize publication parent");
        let staged = canonical_cache.join("staged");
        let publication = canonical_cache.join("published");
        std::fs::create_dir_all(&staged).expect("create publication stage");
        std::fs::create_dir(&publication).expect("create existing publication");

        let error = publish_cache_directory(
            CacheCustodyKind::LocalSnapshot,
            &canonical_cache,
            &staged,
            &publication,
        )
        .expect_err("publication must not replace an existing cache child");

        assert!(matches!(
            error,
            SourceResolveError::LocalSnapshotInvalid { .. }
        ));
        assert!(staged.is_dir());
        assert!(publication.is_dir());
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn git_cache_stage_and_metadata_use_explicit_private_modes() {
        use std::os::unix::fs::PermissionsExt;

        let (repository, _) = create_git_source("git-private-cache-modes");
        let cache = temp_root("git-private-cache-modes-cache");
        let request = local_git_request(&repository, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("materialize private Git cache entry");
        let entry = git_cache_entry_root(&cache, &request);

        assert_eq!(
            std::fs::symlink_metadata(&entry)
                .expect("inspect published cache entry")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::symlink_metadata(entry.join(GIT_CACHE_METADATA))
                .expect("inspect resolver metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let _ = std::fs::remove_dir_all(&repository);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn pending_git_cache_cleanup_does_not_remove_a_replacement_stage_name() {
        let cache = temp_root("git-retained-stage-cleanup");
        let retained_stage = cache.join("retained-stage");
        std::fs::create_dir_all(&cache).expect("create Git cache parent");
        let cache = cache.canonicalize().expect("canonicalize Git cache parent");
        let parent = open_absolute_directory_nofollow(&cache).expect("retain Git cache parent");
        let pending = PendingCacheEntry::create(&cache, &parent, "cleanup")
            .expect("create retained Git cache stage");
        let stage = pending.root.clone();

        std::fs::rename(&stage, &retained_stage).expect("relocate retained Git stage");
        std::fs::create_dir(&stage).expect("create replacement Git stage");
        std::fs::write(stage.join("sentinel"), b"replacement").expect("write replacement sentinel");
        drop(pending);

        assert_eq!(
            std::fs::read(stage.join("sentinel")).expect("read replacement sentinel"),
            b"replacement"
        );
        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::remove_dir_all(&retained_stage);
    }

    #[test]
    fn pending_git_cache_publication_rejects_a_replaced_stage_name() {
        let cache = temp_root("git-retained-stage-publication");
        let retained_stage = cache.join("retained-stage");
        std::fs::create_dir_all(&cache).expect("create Git cache parent");
        let cache = cache.canonicalize().expect("canonicalize Git cache parent");
        let parent = open_absolute_directory_nofollow(&cache).expect("retain Git cache parent");
        let mut pending = PendingCacheEntry::create(&cache, &parent, "publication")
            .expect("create retained Git cache stage");
        let stage = pending.root.clone();
        let publication = cache.join("published");

        std::fs::rename(&stage, &retained_stage).expect("relocate retained Git stage");
        std::fs::create_dir(&stage).expect("create replacement Git stage");
        let error = pending
            .publish(&cache, &publication, OsStr::new("published"))
            .expect_err("publication must reject a replaced Git stage name");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!publication.exists());
        drop(pending);
        assert!(stage.is_dir(), "cleanup must not remove the replacement");
        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::remove_dir_all(&retained_stage);
    }

    #[cfg(unix)]
    #[test]
    fn retained_git_cache_parent_owns_staging_and_invalidation_after_path_replacement() {
        let cache = temp_root("git-retained-parent-namespace");
        let retained_cache = cache.with_extension("retained");
        std::fs::create_dir_all(cache.join("entry")).expect("create retained cache entry");
        std::fs::write(cache.join("entry").join(GIT_CACHE_METADATA), b"retained")
            .expect("write retained metadata");
        let cache = cache.canonicalize().expect("canonicalize Git cache parent");
        let parent = open_absolute_directory_nofollow(&cache).expect("retain Git cache parent");

        std::fs::rename(&cache, &retained_cache).expect("replace Git cache parent path");
        std::fs::create_dir_all(cache.join("entry")).expect("create replacement cache entry");
        std::fs::write(cache.join("entry").join(GIT_CACHE_METADATA), b"replacement")
            .expect("write replacement metadata");

        let pending = PendingCacheEntry::create(&cache, &parent, "parent")
            .expect("create stage beneath retained cache parent");
        let retained_stage_name = pending.stage_name.clone();
        assert!(retained_cache.join(&retained_stage_name).is_dir());
        assert!(!cache.join(&retained_stage_name).exists());
        drop(pending);

        invalidate_git_cache_entry_from_open_parent(
            &cache,
            &parent,
            OsStr::new("entry"),
            &cache.join("entry"),
        )
        .expect("invalidate through retained Git cache parent");
        assert!(
            !retained_cache
                .join("entry")
                .join(GIT_CACHE_METADATA)
                .exists()
        );
        assert_eq!(
            std::fs::read(cache.join("entry").join(GIT_CACHE_METADATA))
                .expect("read replacement metadata"),
            b"replacement"
        );

        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::remove_dir_all(&retained_cache);
    }

    #[test]
    fn materialized_snapshot_writes_and_cleanup_remain_bound_to_the_open_stage() {
        let root = temp_root("retained-materialized-stage");
        let snapshots = root.join("snapshots");
        let retained_parent = root.join("retained-snapshots");
        std::fs::create_dir_all(&snapshots).expect("create snapshot parent");
        let pending = PendingMaterializedSnapshot::create(
            CacheCustodyKind::LocalSnapshot,
            &snapshots,
            ".source-test.stage",
        )
        .expect("create retained materialization stage");
        let stage_name = pending.stage_name.clone();

        std::fs::rename(&snapshots, &retained_parent).expect("replace snapshot parent path");
        std::fs::create_dir(&snapshots).expect("create replacement snapshot parent");
        write_snapshot_file_from_open_root(
            CacheCustodyKind::LocalSnapshot,
            pending.directory().expect("retain stage directory"),
            Path::new("payload"),
            &pending.root,
            b"retained",
            false,
        )
        .expect("write through retained stage");

        assert_eq!(
            std::fs::read(retained_parent.join(&stage_name).join("payload"))
                .expect("read retained stage payload"),
            b"retained"
        );
        assert!(!snapshots.join(&stage_name).exists());
        drop(pending);
        assert!(!retained_parent.join(&stage_name).exists());
        assert!(snapshots.is_dir());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn materialized_snapshot_publication_rejects_a_replaced_stage_name() {
        let root = temp_root("replaced-materialized-stage");
        let snapshots = root.join("snapshots");
        std::fs::create_dir_all(&snapshots).expect("create snapshot parent");
        let mut pending = PendingMaterializedSnapshot::create(
            CacheCustodyKind::Git,
            &snapshots,
            ".tree-test.stage",
        )
        .expect("create retained materialization stage");
        let displaced = snapshots.join("displaced-stage");
        std::fs::rename(&pending.root, &displaced).expect("displace retained stage name");
        std::fs::create_dir(&pending.root).expect("create replacement stage directory");
        let replacement = pending.root.clone();
        let publication = snapshots.join("tree-test");

        let error = pending
            .publish(&snapshots, &publication)
            .expect_err("replacement stage name must not publish");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(pending.root.is_dir());
        assert!(!publication.exists());
        drop(pending);
        assert!(!displaced.exists());
        assert!(replacement.is_dir());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn materialized_snapshot_write_rejects_a_nested_directory_symlink_substitution() {
        let root = temp_root("materialized-stage-nested-symlink");
        let stage = root.join("stage");
        let target = root.join("target");
        std::fs::create_dir_all(stage.join("nested")).expect("create stage directory");
        std::fs::create_dir(&target).expect("create substitution target");
        let stage_directory = open_absolute_directory_nofollow(
            &stage.canonicalize().expect("canonicalize stage directory"),
        )
        .expect("open stage directory");
        std::fs::remove_dir(stage.join("nested")).expect("remove nested stage directory");
        std::os::unix::fs::symlink(&target, stage.join("nested"))
            .expect("substitute nested directory symlink");

        let error = write_snapshot_file_from_open_root(
            CacheCustodyKind::LocalSnapshot,
            &stage_directory,
            Path::new("nested/payload"),
            &stage,
            b"must not escape",
            false,
        )
        .expect_err("nested symlink substitution must reject");

        assert!(matches!(
            error,
            SourceResolveError::LocalSnapshotInvalid { .. }
        ));
        assert!(!target.join("payload").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn published_snapshot_mode_verification_remains_bound_to_its_open_root() {
        use std::os::unix::fs::PermissionsExt;

        let parent = temp_root("published-snapshot-open-root");
        let publication = parent.join("publication");
        let retained = parent.join("retained");
        std::fs::create_dir_all(publication.join("nested"))
            .expect("create published snapshot tree");
        std::fs::write(publication.join("nested/payload"), b"retained")
            .expect("write published snapshot payload");
        make_snapshot_read_only(&publication).expect("finalize published snapshot modes");
        let canonical_publication = publication
            .canonicalize()
            .expect("canonicalize published snapshot");
        let directory = open_absolute_directory_nofollow(&canonical_publication)
            .expect("open published snapshot root");

        std::fs::rename(&publication, &retained).expect("replace publication root path");
        std::fs::create_dir(&publication).expect("create replacement publication root");
        std::fs::set_permissions(&publication, std::fs::Permissions::from_mode(0o777))
            .expect("make replacement publication writable");

        verify_open_snapshot_tree_modes(
            CacheCustodyKind::LocalSnapshot,
            &directory,
            &canonical_publication,
        )
        .expect("verification must remain on the retained publication");
        assert_eq!(
            std::fs::read(retained.join("nested/payload")).expect("read retained payload"),
            b"retained"
        );
        std::fs::set_permissions(&publication, std::fs::Permissions::from_mode(0o700)).unwrap();
        make_tree_owner_writable(&retained);
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[cfg(unix)]
    #[test]
    fn open_cache_parent_publication_is_not_redirected_by_path_replacement() {
        let root = temp_root("capability-publication-parent-replacement");
        let cache = root.join("cache");
        let retained = root.join("retained");
        std::fs::create_dir_all(cache.join("staged")).expect("create publication stage");
        let canonical_cache = cache.canonicalize().expect("canonicalize cache parent");
        let directory =
            open_absolute_directory_nofollow(&canonical_cache).expect("open cache parent");

        std::fs::rename(&cache, &retained).expect("replace opened cache parent path");
        std::fs::create_dir(&cache).expect("create replacement cache parent");
        publish_cache_directory_from_open_parent(
            CacheCustodyKind::Git,
            &canonical_cache,
            &directory,
            OsStr::new("staged"),
            OsStr::new("published"),
            None,
        )
        .expect("publish through retained parent handle");

        assert!(retained.join("published").is_dir());
        assert!(!cache.join("published").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn git_cache_custody_does_not_follow_replaced_directory_leaf() {
        assert_cache_custody_does_not_follow_replaced_directory_leaf(CacheCustodyKind::Git);
    }

    #[cfg(unix)]
    #[test]
    fn local_cache_custody_does_not_follow_replaced_directory_leaf() {
        assert_cache_custody_does_not_follow_replaced_directory_leaf(
            CacheCustodyKind::LocalSnapshot,
        );
    }

    #[cfg(unix)]
    fn assert_cache_custody_does_not_follow_replaced_directory_leaf(kind: CacheCustodyKind) {
        let cache = temp_root("cache-nofollow-replaced-directory");
        std::fs::create_dir_all(cache.join("classified")).expect("create classified directory");
        std::fs::create_dir_all(cache.join("replacement")).expect("create replacement directory");
        let canonical_cache = cache.canonicalize().expect("canonicalize cache root");
        let directory =
            open_absolute_directory_nofollow(&canonical_cache).expect("open cache root capability");
        let classified = directory
            .symlink_metadata("classified")
            .expect("classify cache directory");
        assert!(classified.is_dir());

        std::fs::remove_dir(cache.join("classified")).expect("remove classified directory");
        std::os::unix::fs::symlink("replacement", cache.join("classified"))
            .expect("replace cache directory with symlink");
        let error = open_cache_custody_directory(
            &directory,
            Path::new("classified"),
            &canonical_cache.join("classified"),
            &classified,
            kind,
        )
        .expect_err("cache custody must not follow a replacement directory symlink");
        assert!(matches!(
            (kind, error),
            (
                CacheCustodyKind::Git,
                SourceResolveError::GitCacheInvalid { .. }
            ) | (
                CacheCustodyKind::LocalSnapshot,
                SourceResolveError::LocalSnapshotInvalid { .. }
            )
        ));

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn cache_custody_rejects_replaced_concrete_directory_identity() {
        let cache = temp_root("cache-replaced-concrete-directory");
        std::fs::create_dir_all(cache.join("classified")).expect("create classified directory");
        let canonical_cache = cache.canonicalize().expect("canonicalize cache root");
        let directory =
            open_absolute_directory_nofollow(&canonical_cache).expect("open cache root capability");
        let classified = directory
            .symlink_metadata("classified")
            .expect("classify cache directory");

        std::fs::rename(cache.join("classified"), cache.join("retained"))
            .expect("retain classified directory identity");
        std::fs::create_dir(cache.join("classified")).expect("replace with concrete directory");
        let error = open_cache_custody_directory(
            &directory,
            Path::new("classified"),
            &canonical_cache.join("classified"),
            &classified,
            CacheCustodyKind::Git,
        )
        .expect_err("cache custody must reject a different concrete directory identity");
        assert!(matches!(
            error,
            SourceResolveError::GitCacheInvalid { message, .. }
                if message.contains("changed between classification")
        ));

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn cache_custody_entry_capacity_accepts_the_exact_ceiling_only() {
        assert!(cache_custody_has_capacity(CACHE_CUSTODY_ENTRY_LIMIT - 1, 0));
        assert!(!cache_custody_has_capacity(CACHE_CUSTODY_ENTRY_LIMIT, 0));
        assert!(!cache_custody_has_capacity(usize::MAX, 1));
    }

    #[test]
    fn cache_custody_wide_tree_does_not_retain_one_handle_per_sibling() {
        let cache = temp_root("cache-wide-directory");
        std::fs::create_dir_all(&cache).expect("create cache root");
        for index in 0..1_024 {
            std::fs::create_dir(cache.join(format!("directory-{index:04}")))
                .expect("create sibling cache directory");
        }
        let cache = cache.canonicalize().expect("canonicalize cache root");

        verify_cache_custody(&cache, CacheCustodyKind::Git, 0)
            .expect("wide custody walk must retain paths rather than sibling handles");

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(unix)]
    #[test]
    fn cache_custody_walk_remains_bound_to_open_root_after_path_replacement() {
        let cache = temp_root("cache-open-root-replacement");
        let retained = cache.with_extension("retained");
        std::fs::create_dir_all(&cache).expect("create cache root");
        let canonical_cache = cache.canonicalize().expect("canonicalize cache root");
        let directory =
            open_absolute_directory_nofollow(&canonical_cache).expect("open cache root capability");

        std::fs::rename(&cache, &retained).expect("relocate opened cache root");
        std::fs::create_dir_all(&cache).expect("create replacement cache root");
        std::fs::write(
            cache.join("replacement"),
            b"payload exceeding retained ceiling",
        )
        .expect("write replacement payload");

        verify_cache_custody_from_open_root(&canonical_cache, directory, CacheCustodyKind::Git, 3)
            .expect("custody walk must remain bound to the opened cache root");

        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::remove_dir_all(&retained);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cache_custody_acl_observation_remains_bound_to_open_root() {
        let cache = temp_root("cache-open-root-acl-replacement");
        let retained = cache.with_extension("retained");
        std::fs::create_dir_all(&cache).expect("create cache root");
        let canonical_cache = cache.canonicalize().expect("canonicalize cache root");
        let directory =
            open_absolute_directory_nofollow(&canonical_cache).expect("open cache root capability");

        std::fs::rename(&cache, &retained).expect("relocate opened cache root");
        std::fs::create_dir_all(&cache).expect("create replacement cache root");
        change_macos_acl(&cache, &["+a", "everyone allow write"]);

        verify_cache_custody_from_open_root(&canonical_cache, directory, CacheCustodyKind::Git, 0)
            .expect("ACL observation must remain on the retained cache root");

        change_macos_acl(&cache, &["-N"]);
        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::remove_dir_all(&retained);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cache_ancestry_acl_open_rejects_classified_directory_replacement() {
        let cache = temp_root("cache-ancestry-acl-replacement");
        let retained = cache.with_extension("retained");
        std::fs::create_dir_all(&cache).expect("create classified cache directory");
        let cache = cache.canonicalize().expect("canonicalize cache directory");
        let classified =
            std::fs::symlink_metadata(&cache).expect("classify cache directory before replacement");

        std::fs::rename(&cache, &retained).expect("relocate classified cache directory");
        std::fs::create_dir(&cache).expect("create replacement cache directory");
        change_macos_acl(&cache, &["+a", "everyone allow write"]);

        let error = verify_macos_open_cache_directory_acl_custody(
            CacheCustodyKind::Git,
            &cache,
            &classified,
        )
        .expect_err("different directory identity must reject before its ACL can contribute");
        assert!(matches!(
            error,
            SourceResolveError::GitCacheInvalid { message, .. }
                if message.contains("changed between classification")
        ));

        change_macos_acl(&cache, &["-N"]);
        let _ = std::fs::remove_dir_all(&cache);
        let _ = std::fs::remove_dir_all(&retained);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cache_custody_rejects_extended_acl_allow_entries_on_root_and_nodes() {
        use std::os::unix::fs::PermissionsExt;

        let cache = temp_root("cache-acl-custody");
        std::fs::create_dir_all(&cache).expect("create cache");
        let cache = cache.canonicalize().expect("canonicalize cache");
        std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700))
            .expect("make cache private");
        let payload = cache.join("payload");
        std::fs::write(&payload, b"custody").expect("write cache payload");
        std::fs::set_permissions(&payload, std::fs::Permissions::from_mode(0o600))
            .expect("make cache payload private");

        change_macos_acl(&cache, &["+a", "everyone allow write"]);
        let root_error = verify_cache_custody(&cache, CacheCustodyKind::Git, 1024)
            .expect_err("extended ACL allow on cache root must reject");
        assert!(matches!(
            &root_error,
            SourceResolveError::GitCacheInvalid { path, message }
                if path == &cache && message.contains("extended ACL allow")
        ));
        change_macos_acl(&cache, &["-N"]);

        change_macos_acl(&payload, &["+a", "everyone allow write"]);
        let node_error = verify_cache_custody(&cache, CacheCustodyKind::Git, 1024)
            .expect_err("extended ACL allow on cache node must reject");
        assert!(
            matches!(
                &node_error,
                SourceResolveError::GitCacheInvalid { path, message }
                    if path == &payload && message.contains("extended ACL allow")
            ),
            "unexpected cache node ACL error: {node_error:?}"
        );
        change_macos_acl(&payload, &["-N"]);
        change_macos_acl(&payload, &["+a", "everyone deny write"]);
        verify_cache_custody(&cache, CacheCustodyKind::Git, 1024)
            .expect("deny-only ACL does not broaden cache custody");

        change_macos_acl(&payload, &["-N"]);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cache_locks_reject_extended_acl_allow_entries() {
        let root = temp_root("cache-lock-acl-custody");
        std::fs::create_dir_all(&root).expect("create cache lock root");

        for (name, kind) in [
            ("git.lock", CacheCustodyKind::Git),
            ("local.lock", CacheCustodyKind::LocalSnapshot),
        ] {
            let path = root.join(name);
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .expect("open cache lock");
            change_macos_acl(&path, &["+a", "everyone allow write"]);
            let error = verify_cache_lock_path_identity_for_test(kind, &path, &file)
                .expect_err("extended ACL allow on cache lock must reject");
            assert!(
                matches!(
                    (&kind, &error),
                    (CacheCustodyKind::Git, SourceResolveError::GitCacheInvalid { message, .. })
                        | (
                            CacheCustodyKind::LocalSnapshot,
                            SourceResolveError::LocalSnapshotInvalid { message, .. }
                        ) if message.contains("extended ACL allow")
                ),
                "unexpected cache lock ACL error: {error:?}"
            );
            change_macos_acl(&path, &["-N"]);
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn git_cache_reuse_rejects_extended_acl_allow_entry() {
        let (repository, _) = create_git_source("git-cache-acl-source");
        let cache = temp_root("git-cache-acl-cache");
        let request = local_git_request(&repository, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let cache = cache.canonicalize().expect("canonicalize Git cache");
        let entry = git_cache_entry_root(&cache, &request);
        change_macos_acl(&entry, &["+a", "everyone allow write"]);

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("extended ACL allow on Git cache must reject reuse");
        assert!(
            matches!(
                &error,
                SourceResolveError::GitCacheInvalid { path, message }
                    if path == &entry && message.contains("extended ACL allow")
            ),
            "unexpected Git cache ACL error: {error:?}"
        );

        let _ = std::fs::remove_dir_all(&repository);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn local_snapshot_reuse_rejects_extended_acl_allow_entry() {
        let source = temp_root("local-cache-acl-source");
        let cache = temp_root("local-cache-acl-cache");
        std::fs::create_dir_all(&source).expect("create source");
        std::fs::write(source.join("main.omg"), b"machine main() { }").expect("write source");
        let resolved = resolve_local_source_snapshot(&source, &cache, LocalSourceLimits::default())
            .expect("prime local snapshot cache");
        let payload = resolved.snapshot_root.join("main.omg");
        change_macos_acl(&payload, &["+a", "everyone allow write"]);

        let error = resolve_local_source_snapshot(&source, &cache, LocalSourceLimits::default())
            .expect_err("extended ACL allow on local snapshot must reject reuse");
        assert!(matches!(
            &error,
            SourceResolveError::LocalSnapshotInvalid { path, message }
                if path == &payload && message.contains("extended ACL allow")
        ));

        change_macos_acl(&payload, &["-N"]);
        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn cache_custody_byte_ceilings_are_source_scaled_and_absolutely_capped() {
        let small = LocalSourceLimits {
            max_bytes: 1024,
            ..LocalSourceLimits::default()
        };
        assert_eq!(
            git_cache_custody_byte_limit(small),
            CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE + 3 * 1024
        );
        assert_eq!(
            local_cache_custody_byte_limit(small),
            CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE + 1024
        );

        let unbounded_input = LocalSourceLimits {
            max_bytes: u64::MAX,
            ..LocalSourceLimits::default()
        };
        assert_eq!(
            git_cache_custody_byte_limit(unbounded_input),
            GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT
        );
        assert_eq!(
            local_cache_custody_byte_limit(unbounded_input),
            LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT
        );
    }

    #[cfg(unix)]
    #[test]
    fn local_snapshot_cache_rejects_group_or_other_writable_custody() {
        use std::os::unix::fs::PermissionsExt;

        let source = temp_root("local-custody-source");
        let cache = temp_root("local-custody-cache");
        std::fs::create_dir_all(&source).expect("create source");
        std::fs::write(source.join("main.omg"), b"machine main() { }").expect("write source");
        resolve_local_source_snapshot(&source, &cache, LocalSourceLimits::default())
            .expect("prime local snapshot cache");
        std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o777))
            .expect("make cache externally writable");

        let error = resolve_local_source_snapshot(&source, &cache, LocalSourceLimits::default())
            .expect_err("externally writable local cache custody must reject");
        assert!(matches!(
            error,
            SourceResolveError::LocalSnapshotInvalid { .. }
        ));

        std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_rejects_local_filter_configuration_without_running_it() {
        let (repo, _) = create_git_source("git-filter-source");
        std::fs::write(repo.join(".gitattributes"), "*.omg filter=omega-test\n")
            .expect("write attributes");
        run_test_git(&repo, ["add", ".gitattributes"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "declare filter"]);
        let cache = temp_root("git-filter-cache");
        let sentinel = cache.join("filter-ran");
        let request = local_git_request(&repo, "HEAD");
        resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
        let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
        run_test_git(
            &repository,
            [
                "config",
                "--local",
                "filter.omega-test.smudge",
                &format!("touch {}", sentinel.display()),
            ],
        );

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("local filter configuration must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!sentinel.exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_commands_seal_ambient_config_protocol_and_execution_injection() {
        let executor =
            GitExecutor::system(GitExecutionTransport::Https).expect("system Git executor");
        let helper_directory = executor
            .transport_executable
            .as_ref()
            .expect("HTTPS transport helper")
            .identity
            .invocation_path
            .parent()
            .expect("HTTPS helper parent")
            .to_path_buf();
        let working_directory = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary directory");
        let command =
            sealed_git_command(&executor, &working_directory).expect("sealed absolute Git command");
        let environment = command
            .get_envs()
            .map(|(key, value)| (key.to_owned(), value.map(OsStr::to_owned)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        let expected_environment = std::collections::BTreeMap::from([
            (
                OsString::from("GIT_ALLOW_PROTOCOL"),
                Some(OsString::from("https")),
            ),
            (
                OsString::from("GIT_ATTR_NOSYSTEM"),
                Some(OsString::from("1")),
            ),
            (
                OsString::from("GIT_CONFIG_GLOBAL"),
                Some(OsString::from(null_device())),
            ),
            (
                OsString::from("GIT_CONFIG_NOSYSTEM"),
                Some(OsString::from("1")),
            ),
            (
                OsString::from("GIT_EXEC_PATH"),
                Some(helper_directory.into_os_string()),
            ),
            (
                OsString::from("GIT_LFS_SKIP_SMUDGE"),
                Some(OsString::from("1")),
            ),
            (
                OsString::from("GIT_NO_LAZY_FETCH"),
                Some(OsString::from("1")),
            ),
            (
                OsString::from("GIT_PROTOCOL_FROM_USER"),
                Some(OsString::from("0")),
            ),
            (
                OsString::from("GIT_TERMINAL_PROMPT"),
                Some(OsString::from("0")),
            ),
            (OsString::from("LANG"), Some(OsString::from("C"))),
            (OsString::from("LC_ALL"), Some(OsString::from("C"))),
            (OsString::from("PATH"), Some(git_helper_path(&executor))),
        ]);
        assert_eq!(environment, expected_environment);
        assert_eq!(command.get_program(), executor.identity.path.as_os_str());
        assert_eq!(command.get_current_dir(), Some(working_directory.as_path()));
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "--no-replace-objects")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "protocol.allow=never")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "protocol.ext.allow=never")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "protocol.http.allow=never")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "protocol.git.allow=never")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "protocol.file.allow=never")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "protocol.https.allow=always")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "protocol.ssh.allow=never")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "http.followRedirects=false")
        );
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "fetch.recurseSubmodules=false")
        );
        assert!(arguments.iter().any(|argument| argument == "gc.auto=0"));
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "maintenance.auto=false")
        );
    }

    #[test]
    fn git_commands_admit_only_the_request_transport() {
        let working_directory = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary directory");
        for (transport, protocol) in [
            (GitExecutionTransport::Https, "https"),
            (GitExecutionTransport::Ssh, "ssh"),
            (GitExecutionTransport::File, "file"),
        ] {
            let executor = GitExecutor::system(transport).expect("system Git executor");
            let command = sealed_git_command(&executor, &working_directory)
                .expect("sealed absolute Git command");
            let environment = command
                .get_envs()
                .map(|(key, value)| (key.to_owned(), value.map(OsStr::to_owned)))
                .collect::<std::collections::BTreeMap<_, _>>();
            let arguments = command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>();

            assert_eq!(
                environment.get(OsStr::new("GIT_ALLOW_PROTOCOL")),
                Some(&Some(OsString::from(protocol)))
            );
            match transport {
                GitExecutionTransport::Https => {
                    let helper = executor
                        .transport_executable
                        .as_ref()
                        .expect("HTTPS transport executable identity");
                    assert!(helper.identity.invocation_path.is_absolute());
                    assert!(helper.identity.path.is_absolute());
                    assert_eq!(helper.identity.content_identity.len(), 64);
                    let helper_directory = helper.identity.invocation_path.parent().unwrap();
                    assert_eq!(
                        environment.get(OsStr::new("GIT_EXEC_PATH")),
                        Some(&Some(helper_directory.as_os_str().to_owned()))
                    );
                    assert_eq!(
                        environment.get(OsStr::new("PATH")),
                        Some(&Some(helper_directory.as_os_str().to_owned()))
                    );
                    assert!(!environment.contains_key(OsStr::new("GIT_SSH_COMMAND")));
                    assert!(!environment.contains_key(OsStr::new("GIT_SSH_VARIANT")));
                }
                GitExecutionTransport::Ssh => {
                    let transport_executable = executor
                        .transport_executable
                        .as_ref()
                        .expect("SSH transport executable identity");
                    assert!(transport_executable.identity.path.is_absolute());
                    assert_eq!(transport_executable.identity.content_identity.len(), 64);
                    assert_eq!(
                        environment.get(OsStr::new("GIT_SSH_COMMAND")),
                        Some(&Some(sealed_ssh_command(
                            &transport_executable.identity.path
                        )))
                    );
                    assert_eq!(
                        environment.get(OsStr::new("GIT_SSH_VARIANT")),
                        Some(&Some(OsString::from("ssh")))
                    );
                    assert!(!environment.contains_key(OsStr::new("GIT_EXEC_PATH")));
                }
                GitExecutionTransport::File => {
                    assert!(!environment.contains_key(OsStr::new("GIT_SSH_COMMAND")));
                    assert!(!environment.contains_key(OsStr::new("GIT_SSH_VARIANT")));
                    assert!(!environment.contains_key(OsStr::new("GIT_EXEC_PATH")));
                    assert!(executor.transport_executable.is_none());
                }
            }
            for (configured, candidate) in [
                ("file", GitExecutionTransport::File),
                ("https", GitExecutionTransport::Https),
                ("ssh", GitExecutionTransport::Ssh),
            ] {
                let expected = format!(
                    "protocol.{configured}.allow={}",
                    transport.permits(candidate)
                );
                assert!(
                    arguments.iter().any(|argument| argument == &expected),
                    "missing {expected:?} for {transport:?}"
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn git_executor_uses_committed_absolute_program_cleared_environment_and_explicit_cwd() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("sealed-git-executor");
        let working_directory = root.join("working");
        std::fs::create_dir_all(&working_directory).expect("create explicit working directory");
        let working_directory = working_directory
            .canonicalize()
            .expect("canonical explicit working directory");
        let fake_git = root.join("git");
        std::fs::write(
            &fake_git,
            b"#!/bin/sh\nprintf 'cwd='\npwd\nprintf 'home=%s\\n' \"${HOME-unset}\"\nprintf 'path=%s\\n' \"$PATH\"\n",
        )
        .expect("write fake Git executable");
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
            .expect("make fake Git executable");
        let executor = GitExecutor::open(&fake_git).expect("capture fake Git identity");

        let output = run_git_output(
            &executor,
            &working_directory,
            [OsStr::new("ignored-by-test-helper")],
        )
        .expect("run sealed fake Git");
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).expect("test helper emits UTF-8");
        assert!(
            stdout.contains(&format!("cwd={}\n", working_directory.display())),
            "sealed helper reported {stdout:?}"
        );
        assert!(stdout.contains("home=unset\n"));
        assert!(stdout.contains("path=/usr/bin:/bin\n"));

        let command = sealed_git_command(&executor, &working_directory)
            .expect("construct sealed fake Git command");
        assert_eq!(command.get_program(), fake_git.canonicalize().unwrap());
        assert_eq!(command.get_current_dir(), Some(working_directory.as_path()));

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn git_executor_rejects_relative_paths_and_executable_drift() {
        use std::os::unix::fs::PermissionsExt;

        assert!(matches!(
            GitExecutor::open(Path::new("git")),
            Err(SourceResolveError::GitExecutableInvalid { .. })
        ));

        let root = temp_root("git-executable-drift");
        std::fs::create_dir_all(&root).expect("create executable drift root");
        let fake_git = root.join("git");
        std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
            .expect("make fake Git executable");
        let executor = GitExecutor::open(&fake_git).expect("capture fake Git identity");
        let replacement = root.join("replacement");
        std::fs::write(&replacement, b"#!/bin/sh\nexit 1\n")
            .expect("write replacement Git executable");
        std::fs::rename(&replacement, &fake_git).expect("replace fake Git executable");

        assert!(matches!(
            executor.verify(),
            Err(SourceResolveError::GitExecutableChanged { .. })
        ));
        assert!(matches!(
            sealed_git_command(&executor, &root),
            Err(SourceResolveError::GitExecutableChanged { .. })
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn git_executor_rejects_unsafe_executable_modes_and_ancestry() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("git-executable-custody");
        std::fs::create_dir_all(&root).expect("create executable custody root");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("make executable custody root private");
        let fake_git = root.join("git");
        std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");

        for unsafe_mode in [0o720, 0o4700, 0o600] {
            std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(unsafe_mode))
                .expect("set unsafe Git executable mode");
            assert!(matches!(
                GitExecutor::open(&fake_git),
                Err(SourceResolveError::GitExecutableInvalid { .. })
            ));
        }

        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
            .expect("restore safe Git executable mode");
        let executor = GitExecutor::open(&fake_git).expect("capture safe Git executable");
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o720))
            .expect("make captured Git executable externally writable");
        assert!(matches!(
            executor.verify(),
            Err(SourceResolveError::GitExecutableChanged { .. })
        ));

        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
            .expect("restore Git executable before ancestry check");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o720))
            .expect("make Git executable ancestry externally writable");
        assert!(matches!(
            GitExecutor::open(&fake_git),
            Err(SourceResolveError::GitExecutableInvalid { .. })
        ));

        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("restore executable custody root");
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn git_executor_rejects_extended_acl_allow_entries_on_executable_and_ancestry() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("git-executable-acl-custody");
        std::fs::create_dir_all(&root).expect("create executable ACL custody root");
        let root = root
            .canonicalize()
            .expect("canonicalize executable ACL custody root");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("make executable ACL custody root private");
        let fake_git = root.join("git");
        std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
            .expect("make fake Git executable private");

        let executor = GitExecutor::open(&fake_git).expect("capture ACL-free Git executable");
        change_macos_acl(&fake_git, &["+a", "everyone allow write"]);
        let executable_acl_error = executor
            .verify()
            .expect_err("extended ACL allow on executable must reject");
        assert!(
            matches!(
                &executable_acl_error,
                SourceResolveError::GitExecutableInvalid { path, message }
                    if path == &fake_git && message.contains("extended ACL allow")
            ),
            "unexpected executable ACL error: {executable_acl_error:?}"
        );
        change_macos_acl(&fake_git, &["-N"]);
        executor
            .verify()
            .expect("removing executable ACL should restore custody");
        change_macos_acl(&fake_git, &["+a", "everyone deny write"]);
        executor
            .verify()
            .expect("deny-only executable ACL does not broaden custody");
        change_macos_acl(&fake_git, &["-N"]);

        change_macos_acl(&root, &["+a", "everyone allow write"]);
        let ancestry_acl_error = executor
            .verify()
            .expect_err("extended ACL allow on ancestry must reject");
        assert!(
            matches!(
                &ancestry_acl_error,
                SourceResolveError::GitExecutableInvalid { path, message }
                    if path == &root && message.contains("extended ACL allow")
            ),
            "unexpected ancestry ACL error: {ancestry_acl_error:?}"
        );
        change_macos_acl(&root, &["-N"]);
        executor
            .verify()
            .expect("removing ancestry ACL should restore custody");

        std::fs::remove_file(&fake_git).expect("remove fake Git executable");
        std::fs::remove_dir(&root).expect("remove executable ACL custody root");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn executable_acl_handle_open_rejects_classified_path_replacement() {
        use std::os::unix::fs::PermissionsExt;

        let temporary_root = temp_root("git-executable-acl-handle-replacement");
        std::fs::create_dir_all(&temporary_root).expect("create executable ACL test root");
        let root = temporary_root
            .canonicalize()
            .expect("canonicalize executable ACL test root");
        let executable = root.join("git");
        let retained = root.join("retained");
        std::fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("write classified executable");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .expect("make classified executable private");
        let classified =
            std::fs::symlink_metadata(&executable).expect("classify executable before replacement");

        std::fs::rename(&executable, &retained).expect("relocate classified executable");
        std::fs::write(&executable, b"#!/bin/sh\nexit 1\n").expect("write replacement executable");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .expect("make replacement executable private");
        change_macos_acl(&executable, &["+a", "everyone allow write"]);

        assert!(matches!(
            verify_macos_open_executable_acl_custody(&executable, &classified),
            Err(SourceResolveError::GitExecutableChanged { path }) if path == executable
        ));

        change_macos_acl(&executable, &["-N"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn system_git_executor_excludes_the_apple_dispatcher() {
        let executor =
            GitExecutor::system(GitExecutionTransport::Https).expect("concrete macOS Git executor");
        assert_ne!(executor.identity.path, Path::new("/usr/bin/git"));
        assert!(executor.identity.path.is_absolute());
    }

    #[cfg(unix)]
    #[test]
    fn git_executor_post_check_overrides_success_and_nonzero_exit_after_drift() {
        use std::os::unix::fs::PermissionsExt;

        for exit_status in [0, 7] {
            let root = temp_root(&format!("git-post-drift-{exit_status}"));
            std::fs::create_dir_all(&root).expect("create post-drift root");
            let fake_git = root.join("git");
            let replacement = root.join("git.replacement");
            std::fs::write(
                &fake_git,
                format!("#!/bin/sh\nmv \"$0.replacement\" \"$0\"\nexit {exit_status}\n"),
            )
            .expect("write self-replacing Git executable");
            std::fs::write(&replacement, b"#!/bin/sh\nexit 0\n")
                .expect("write replacement Git executable");
            std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
                .expect("make self-replacing Git executable");
            std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o700))
                .expect("make replacement Git executable");
            let executor = GitExecutor::open(&fake_git).expect("capture original Git identity");

            assert!(matches!(
                run_git_output(&executor, &root, [OsStr::new("ignored")]),
                Err(SourceResolveError::GitExecutableChanged { .. })
            ));

            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[cfg(unix)]
    #[test]
    fn git_executor_enforces_whole_resolution_launch_and_time_budgets() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("git-resolution-budget");
        std::fs::create_dir_all(&root).expect("create Git budget root");
        let fast_git = root.join("fast-git");
        std::fs::write(&fast_git, b"#!/bin/sh\nexit 0\n").expect("write fast fake Git");
        std::fs::set_permissions(&fast_git, std::fs::Permissions::from_mode(0o700))
            .expect("make fast fake Git executable");
        let launch_bounded = GitExecutor::open_with_budget(&fast_git, 1, Duration::from_secs(1))
            .expect("capture launch-bounded Git");
        run_git_output(&launch_bounded, &root, [OsStr::new("first")])
            .expect("first launch fits the budget");
        assert!(matches!(
            run_git_output(&launch_bounded, &root, [OsStr::new("second")]),
            Err(SourceResolveError::GitResolutionCommandLimit { limit: 1 })
        ));

        let slow_git = root.join("slow-git");
        std::fs::write(&slow_git, b"#!/bin/sh\nsleep 1\n").expect("write slow fake Git");
        std::fs::set_permissions(&slow_git, std::fs::Permissions::from_mode(0o700))
            .expect("make slow fake Git executable");
        let time_bounded = GitExecutor::open_with_budget(&slow_git, 1, Duration::from_millis(30))
            .expect("capture time-bounded Git");
        assert!(matches!(
            run_git_output(&time_bounded, &root, [OsStr::new("slow")]),
            Err(SourceResolveError::GitResolutionTimedOut { .. })
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn git_source_rejects_submodule_manifest() {
        let (repo, commit) = create_git_source("git-submodule");
        let cache = temp_root("git-submodule-cache");
        let request = local_git_request(&repo, "HEAD");
        let initial = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect("resolve initial source");
        let snapshot_source = initial.snapshot_root.join("main.omg");
        let initial_snapshot = std::fs::read(&snapshot_source).expect("read initial snapshot");

        std::fs::write(repo.join(".gitmodules"), "[submodule \"dep\"]\n")
            .expect("write gitmodules");
        std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n")
            .expect("change source");
        run_test_git(&repo, ["add", ".gitmodules"]);
        run_test_git(&repo, ["add", "main.omg"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "submodule manifest"]);

        let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
            .expect_err("submodule manifest should reject");

        assert!(matches!(
            error,
            SourceResolveError::GitSubmodulesUnsupported { .. }
        ));
        assert_eq!(
            std::fs::read(&snapshot_source).expect("read snapshot after rejection"),
            initial_snapshot,
            "the fetched submodule tree must be rejected before materialization"
        );
        assert!(!initial.snapshot_root.join("../source.identity").exists());
        assert!(!commit.is_empty());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn verified_published_capture_keeps_root_build_directory() {
        let root = temp_root("verified-exact-build-directory");
        std::fs::create_dir_all(root.join("build/nested")).expect("create exact source tree");
        std::fs::write(root.join("main.omg"), b"machine main() {}\n").expect("write source");
        std::fs::write(
            root.join("build/nested/generated.omg"),
            b"const VALUE: u8 = 1;\n",
        )
        .expect("write exact root build entry");
        let normalized = resolve_materialized_source(&root, LocalSourceLimits::default())
            .expect("derive exact materialized identity");
        let expected = SourceContentDigest::derive(normalized.content_identity.as_bytes());
        make_snapshot_read_only(&root).expect("make exact source tree read-only");

        let captured = capture_verified_package_source_snapshot(
            &root,
            &expected,
            LocalSourceLimits::default(),
        )
        .expect("capture exact published source");

        assert!(captured.iter().any(|entry| {
            entry.relative_path == b"build/nested/generated.omg"
                && matches!(entry.kind, VerifiedPackageSourceEntryKind::File { .. })
        }));
        make_tree_owner_writable(&root);
        let _ = std::fs::remove_dir_all(&root);
    }
}
