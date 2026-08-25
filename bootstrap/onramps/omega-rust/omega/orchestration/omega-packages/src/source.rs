use crate::identity::SourceContentDigest;
use command_group::{CommandGroup, GroupChild};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::BTreeSet;
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

const GIT_CACHE_POLICY: &[u8] = b"omega-git-cache-v5";
const GIT_CACHE_METADATA: &str = "source.identity";
const GIT_CACHE_REPOSITORY: &str = "repository";
const GIT_CACHE_SNAPSHOTS: &str = "snapshots";
const GIT_SNAPSHOT_METADATA: &str = "snapshot.identity";
const GIT_SNAPSHOT_SOURCE: &str = "source";
const GIT_SNAPSHOT_POLICY: &[u8] = b"omega-git-snapshot-v3";
const LOCAL_CACHE_SNAPSHOTS: &str = "local-snapshots";
const LOCAL_SNAPSHOT_METADATA: &str = "snapshot.identity";
const LOCAL_SNAPSHOT_SOURCE: &str = "source";
const LOCAL_SNAPSHOT_POLICY: &[u8] = b"omega-local-source-snapshot-v2";
const LOCAL_SNAPSHOT_CUSTODY_POLICY: &[u8] = b"omega-local-source-snapshot-custody-v1";
const DEFAULT_BUILD_OUTPUT_DIRECTORY: &str = "build";
const CANONICAL_DIRECTORY_MODE: u16 = 0o555;
const GIT_ORIGIN_FETCH: &str = "+refs/heads/*:refs/remotes/origin/*";
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const GIT_STDOUT_LIMIT: usize = 16 * 1024 * 1024;
const GIT_STDERR_LIMIT: usize = 1024 * 1024;
const GIT_EXECUTABLE_BYTE_LIMIT: u64 = 256 * 1024 * 1024;
const GIT_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const GIT_FIXED_COMMAND_ALLOWANCE: usize = 64;
const GIT_COMMAND_CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
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
pub struct GitSourceSpec {
    pub url: String,
    pub rev: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGitSource {
    pub url: String,
    pub requested_rev: String,
    pub commit: String,
    pub tree: String,
    pub snapshot_root: PathBuf,
    pub local: ResolvedLocalSource,
    /// Absolute helper identity observed before and after every Git launch.
    /// This is diagnostic custody, not certification of the executable.
    pub git_executable: GitExecutableIdentity,
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

#[derive(Debug)]
struct GitExecutor {
    identity: GitExecutableIdentity,
    metadata_identity: GitExecutableMetadataIdentity,
    started: Instant,
    timeout: Duration,
    launches: Cell<usize>,
    maximum_launches: usize,
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
                "Git executable `{}` is invalid: {message}",
                path.display()
            ),
            Self::GitExecutableChanged { path } => write!(
                output,
                "Git executable `{}` changed during source resolution",
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
    verify_local_snapshot_modes(root)?;
    let captured = capture_local_source(root, limits, SourceTreePolicy::ExactMaterialized)?;
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
    let requested_root = root.as_ref().to_path_buf();
    let captured = capture_local_source(&requested_root, limits, SourceTreePolicy::LocalPackage)?;
    publish_local_snapshot(requested_root, captured, cache_dir.as_ref(), limits)
}

pub fn resolve_git_source(
    spec: &GitSourceSpec,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let executor = GitExecutor::system()?;
    let result = (|| {
        let requested_rev = spec.rev.clone().unwrap_or_else(|| "HEAD".to_owned());
        let cache_dir = cache_dir.as_ref();
        std::fs::create_dir_all(cache_dir).map_err(|error| io_error(cache_dir, error))?;
        let cache_dir = cache_dir
            .canonicalize()
            .map_err(|error| io_error(cache_dir, error))?;
        let cache_identity = git_cache_identity(&spec.url, &requested_rev);
        let entry_root = cache_dir.join(format!("git-{cache_identity}"));
        let lock_path = cache_dir.join(format!("git-{cache_identity}.lock"));
        let _entry_lock = CacheEntryLock::acquire_with_git_budget(&lock_path, &executor)?;

        if entry_root.exists() {
            if let Err(error) =
                verify_git_cache_entry(&executor, &entry_root, &spec.url, &requested_rev)
            {
                invalidate_git_cache_entry(&entry_root);
                return Err(error);
            }
        } else {
            create_git_cache_entry(
                &executor,
                &cache_dir,
                &entry_root,
                &cache_identity,
                &spec.url,
                &requested_rev,
            )?;
        }

        let result = resolve_verified_git_cache_entry(
            &executor,
            &entry_root,
            &spec.url,
            &requested_rev,
            limits,
        );
        if result.is_err() {
            invalidate_git_cache_entry(&entry_root);
        }
        result
    })();
    let executable_result = executor.verify_content();
    reconcile_git_command_result(result, executable_result, Ok(()))
}

fn resolve_verified_git_cache_entry(
    executor: &GitExecutor,
    entry_root: &Path,
    url: &str,
    requested_rev: &str,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    verify_git_cache_entry(executor, entry_root, url, requested_rev)?;
    let repository = entry_root.join(GIT_CACHE_REPOSITORY);

    run_git(
        executor,
        &repository,
        [
            OsStr::new("fetch"),
            OsStr::new("--quiet"),
            OsStr::new("--depth=1"),
            OsStr::new("--no-tags"),
            OsStr::new("--no-recurse-submodules"),
            OsStr::new("--"),
            OsStr::new("origin"),
            OsStr::new(requested_rev),
        ],
    )?;
    verify_git_cache_entry(executor, entry_root, url, requested_rev)?;

    let commit = run_git_stdout(
        executor,
        &repository,
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new("FETCH_HEAD^{commit}"),
        ],
    )?;
    let commit = commit.trim().to_owned();
    let tree = run_git_stdout(
        executor,
        &repository,
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&format!("{commit}^{{tree}}")),
        ],
    )?;
    let tree = tree.trim().to_owned();
    verify_git_cache_entry(executor, entry_root, url, requested_rev)?;
    let entries = inspect_git_tree(executor, &repository, &tree, limits)?;
    let (snapshot_root, local) =
        resolve_git_snapshot(executor, entry_root, &tree, entries, limits)?;
    verify_git_cache_entry(executor, entry_root, url, requested_rev)?;
    executor.verify()?;
    Ok(ResolvedGitSource {
        url: url.to_owned(),
        requested_rev: requested_rev.to_owned(),
        commit,
        tree,
        snapshot_root,
        local,
        git_executable: executor.identity.clone(),
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

fn inspect_git_tree(
    executor: &GitExecutor,
    repository: &Path,
    tree: &str,
    limits: LocalSourceLimits,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    if !is_object_id(tree) {
        return Err(cache_invalid(
            repository,
            "Git returned an invalid tree object ID",
        ));
    }
    let listing = run_git_bytes_stdout(
        executor,
        repository,
        [
            OsStr::new("ls-tree"),
            OsStr::new("--full-tree"),
            OsStr::new("-r"),
            OsStr::new("-l"),
            OsStr::new("-z"),
            OsStr::new(tree),
        ],
    )?;
    let mut entries = parse_git_tree_entries(&listing, repository, limits)?;
    read_git_blobs_batch(executor, repository, &mut entries, limits)?;
    Ok(entries)
}

fn parse_git_tree_entries(
    listing: &[u8],
    repository: &Path,
    limits: LocalSourceLimits,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    let mut entries = Vec::new();
    let mut paths = BTreeSet::new();
    let mut directories = BTreeSet::new();
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
        if object_type != b"blob" {
            return Err(git_tree_invalid(
                path,
                "only validated blob objects may be materialized",
            ));
        }
        let size = std::str::from_utf8(fields[3])
            .ok()
            .and_then(|size| size.parse::<u64>().ok())
            .ok_or_else(|| git_tree_invalid(path, "blob size is missing or invalid"))?;
        let relative_path = validate_git_path(path, limits)?;
        if path
            .split(|byte| *byte == b'/')
            .any(|component| component.eq_ignore_ascii_case(b".gitmodules"))
        {
            return Err(SourceResolveError::GitSubmodulesUnsupported {
                path: relative_path,
            });
        }
        if !paths.insert(path.to_vec()) {
            return Err(git_tree_invalid(path, "duplicate path"));
        }
        insert_git_directory_paths(path, &mut directories);
        let identity_entry_count = entries
            .len()
            .checked_add(1)
            .and_then(|leaves| leaves.checked_add(directories.len()))
            .ok_or(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            })?;
        if identity_entry_count > limits.max_files {
            return Err(SourceResolveError::TooManyFiles {
                limit: limits.max_files,
            });
        }
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
        let kind = match mode {
            b"100644" => GitTreeEntryKind::File {
                executable: false,
                bytes: GitBlobBytes::empty(),
            },
            b"100755" => GitTreeEntryKind::File {
                executable: true,
                bytes: GitBlobBytes::empty(),
            },
            b"120000" => GitTreeEntryKind::Symlink {
                target_bytes: GitBlobBytes::empty(),
            },
            _ => return Err(git_tree_invalid(path, "unsupported Git entry mode")),
        };
        entries.push(GitTreeEntry {
            relative_bytes: path.to_vec(),
            relative_path,
            oid: oid.to_owned(),
            size,
            kind,
        });
    }

    entries.sort_by(|left, right| left.relative_bytes.cmp(&right.relative_bytes));
    for pair in entries.windows(2) {
        let mut prefix = pair[0].relative_bytes.clone();
        prefix.push(b'/');
        if pair[1].relative_bytes.starts_with(&prefix) {
            return Err(git_tree_invalid(
                &pair[1].relative_bytes,
                "a blob path cannot contain another blob path",
            ));
        }
    }
    Ok(entries)
}

fn insert_git_directory_paths(path: &[u8], directories: &mut BTreeSet<Vec<u8>>) {
    for separator in
        path.iter().enumerate().filter_map(
            |(index, byte)| {
                if *byte == b'/' { Some(index) } else { None }
            },
        )
    {
        directories.insert(path[..separator].to_vec());
    }
}

fn git_directory_paths(entries: &[GitTreeEntry]) -> BTreeSet<Vec<u8>> {
    let mut directories = BTreeSet::new();
    for entry in entries {
        insert_git_directory_paths(&entry.relative_bytes, &mut directories);
    }
    directories
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
            _ => depth += 1,
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
    let snapshots = entry_root.join(GIT_CACHE_SNAPSHOTS);
    std::fs::create_dir_all(&snapshots).map_err(|error| io_error(&snapshots, error))?;
    require_real_directory(&snapshots, "snapshot cache is not a real directory")?;
    let publication = snapshots.join(format!("tree-{tree}"));
    if publication.exists() {
        verify_snapshot_symlink_targets(&publication.join(GIT_SNAPSHOT_SOURCE), &entries)?;
        release_git_blob_payloads(&mut entries);
        return verify_git_snapshot(&publication, tree, &entries, limits);
    }

    let mut pending = PendingSnapshot::create(&snapshots, tree)?;
    let source = pending.root.join(GIT_SNAPSHOT_SOURCE);
    std::fs::create_dir(&source).map_err(|error| io_error(&source, error))?;
    let directory_paths = git_directory_paths(&entries);
    let identity_entry_count = entries.len().checked_add(directory_paths.len()).ok_or(
        SourceResolveError::TooManyFiles {
            limit: limits.max_files,
        },
    )?;
    let mut expected_identity = SourceIdentityHasher::new(identity_entry_count);
    let mut pending_directories = directory_paths.iter().peekable();
    for entry in &entries {
        executor.verify_budget()?;
        while pending_directories
            .peek()
            .is_some_and(|directory| directory.as_slice() < entry.relative_bytes.as_slice())
        {
            expected_identity.add_directory(
                pending_directories.next().expect("peeked directory"),
                CANONICAL_DIRECTORY_MODE,
            );
        }
        let destination = source.join(&entry.relative_path);
        let parent = destination
            .parent()
            .expect("validated Git paths always have a snapshot parent");
        std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        match &entry.kind {
            GitTreeEntryKind::File { executable, bytes } => {
                expected_identity.add_file(&entry.relative_bytes, *executable, bytes.as_slice())?;
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&destination)
                    .map_err(|error| io_error(&destination, error))?;
                file.write_all(bytes.as_slice())
                    .map_err(|error| io_error(&destination, error))?;
                file.sync_all()
                    .map_err(|error| io_error(&destination, error))?;
                set_snapshot_file_mode(&destination, *executable)?;
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                expected_identity.add_symlink(&entry.relative_bytes, target_bytes.as_slice());
                create_snapshot_symlink(target_bytes.as_slice(), &destination)?;
            }
        }
    }
    for directory in pending_directories {
        expected_identity.add_directory(directory, CANONICAL_DIRECTORY_MODE);
    }
    // Git trees contain only implicit directories and therefore carry no directory modes. Omega
    // canonicalizes every materialized non-root Git directory to 0555: readable/searchable and
    // consistent with the immutable published snapshot, but never writable.
    for directory in directory_paths.iter().rev() {
        let path = source.join(git_path_from_bytes(directory)?);
        set_snapshot_directory_read_only(&path)?;
    }

    // The staged source is re-read to bind publication identity. Release the
    // shared batch payload first so that this verification does not retain a
    // second package-sized in-memory copy.
    release_git_blob_payloads(&mut entries);
    let staged = resolve_materialized_source(&source, limits)?;
    let (expected_byte_count, expected_content_identity) = expected_identity.finish();
    if staged.file_count != entries.len()
        || staged.byte_count != expected_byte_count
        || staged.content_identity != expected_content_identity
    {
        return Err(cache_invalid(
            &source,
            "materialized snapshot did not preserve the validated Git tree exactly",
        ));
    }
    let metadata_path = pending.root.join(GIT_SNAPSHOT_METADATA);
    let mut metadata = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_path)
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .write_all(&git_snapshot_metadata(tree, &staged))
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .sync_all()
        .map_err(|error| io_error(&metadata_path, error))?;
    make_snapshot_read_only(&pending.root)?;
    std::fs::rename(&pending.root, &publication).map_err(|error| io_error(&publication, error))?;
    pending.published = true;

    // The returned identity is always calculated from the atomically published tree, never from
    // the staging directory or Git's mutable object-cache state.
    verify_git_snapshot(&publication, tree, &entries, limits)
}

fn release_git_blob_payloads(entries: &mut [GitTreeEntry]) {
    for entry in entries {
        match &mut entry.kind {
            GitTreeEntryKind::File { bytes, .. } => *bytes = GitBlobBytes::empty(),
            GitTreeEntryKind::Symlink { target_bytes } => {
                *target_bytes = GitBlobBytes::empty();
            }
        }
    }
}

fn verify_snapshot_symlink_targets(
    source: &Path,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    for entry in entries {
        let GitTreeEntryKind::Symlink { target_bytes } = &entry.kind else {
            continue;
        };
        let path = source.join(&entry.relative_path);
        let target = std::fs::read_link(&path).map_err(|error| io_error(&path, error))?;
        if raw_os_bytes(target.as_os_str()) != target_bytes.as_slice() {
            return Err(cache_invalid(
                &path,
                "snapshot symlink target does not match Git",
            ));
        }
    }
    Ok(())
}

fn verify_git_snapshot(
    publication: &Path,
    tree: &str,
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(PathBuf, ResolvedLocalSource), SourceResolveError> {
    require_real_directory(publication, "snapshot publication is not a real directory")?;
    let source = publication.join(GIT_SNAPSHOT_SOURCE);
    require_real_directory(&source, "snapshot source is not a real directory")?;
    let metadata_path = publication.join(GIT_SNAPSHOT_METADATA);
    require_regular_file(&metadata_path, "snapshot metadata is not a regular file")?;
    let metadata_length = std::fs::symlink_metadata(&metadata_path)
        .map_err(|error| io_error(&metadata_path, error))?
        .len();
    if metadata_length > 1024 {
        return Err(cache_invalid(
            &metadata_path,
            "snapshot metadata exceeds its limit",
        ));
    }
    let metadata =
        std::fs::read(&metadata_path).map_err(|error| io_error(&metadata_path, error))?;
    let expected = parse_git_snapshot_metadata(&metadata, &metadata_path)?;
    if expected.tree != tree {
        return Err(cache_invalid(
            &metadata_path,
            "snapshot tree identity does not match",
        ));
    }
    verify_snapshot_entry_kinds_and_modes(&source, entries)?;
    verify_snapshot_read_only(publication)?;
    let local = resolve_materialized_source(&source, limits)?;
    if local.file_count != expected.file_count
        || local.byte_count != expected.byte_count
        || local.content_identity != expected.content_identity
    {
        return Err(cache_invalid(
            publication,
            "published snapshot does not match resolver metadata",
        ));
    }
    Ok((source, local))
}

fn verify_snapshot_entry_kinds_and_modes(
    source: &Path,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    let expected_leaves = entries
        .iter()
        .map(|entry| entry.relative_bytes.clone())
        .collect::<BTreeSet<_>>();
    let mut expected_directories = BTreeSet::new();
    for entry in entries {
        let mut prefix = Vec::new();
        let components = entry
            .relative_bytes
            .split(|byte| *byte == b'/')
            .collect::<Vec<_>>();
        for component in components.iter().take(components.len().saturating_sub(1)) {
            if !prefix.is_empty() {
                prefix.push(b'/');
            }
            prefix.extend_from_slice(component);
            expected_directories.insert(prefix.clone());
        }

        let path = source.join(&entry.relative_path);
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
        match &entry.kind {
            GitTreeEntryKind::File { executable, .. } => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(cache_invalid(
                        &path,
                        "snapshot file kind does not match Git",
                    ));
                }
                verify_snapshot_file_mode(&path, &metadata, *executable)?;
            }
            GitTreeEntryKind::Symlink { .. } => {
                if !metadata.file_type().is_symlink() {
                    return Err(cache_invalid(
                        &path,
                        "snapshot symlink kind does not match Git",
                    ));
                }
            }
        }
    }

    let mut actual_leaves = BTreeSet::new();
    let mut directories = vec![source.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory).map_err(|error| io_error(&directory, error))? {
            let entry = entry.map_err(|error| io_error(&directory, error))?;
            let path = entry.path();
            let relative = path
                .strip_prefix(source)
                .expect("snapshot traversal starts at the source root");
            let relative_bytes = raw_os_bytes(relative.as_os_str());
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
            if metadata.is_dir() {
                if !expected_directories.contains(&relative_bytes) {
                    return Err(cache_invalid(
                        &path,
                        "snapshot contains an undeclared directory",
                    ));
                }
                verify_snapshot_directory_mode(&path, &metadata)?;
                directories.push(path);
            } else {
                actual_leaves.insert(relative_bytes);
            }
        }
    }
    if actual_leaves != expected_leaves {
        return Err(cache_invalid(
            source,
            "snapshot paths do not exactly match the validated Git tree",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn verify_snapshot_directory_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o7777 != u32::from(CANONICAL_DIRECTORY_MODE) {
        return Err(cache_invalid(
            path,
            "snapshot directory mode does not match canonical Git mode",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_snapshot_directory_mode(
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
fn verify_snapshot_file_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
    executable: bool,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    let expected = if executable { 0o555 } else { 0o444 };
    if metadata.permissions().mode() & 0o7777 != expected {
        return Err(cache_invalid(path, "snapshot file mode does not match Git"));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_snapshot_file_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
    executable: bool,
) -> Result<(), SourceResolveError> {
    if executable != is_executable(metadata) || !metadata.permissions().readonly() {
        return Err(cache_invalid(path, "snapshot file mode does not match Git"));
    }
    Ok(())
}

fn verify_snapshot_read_only(root: &Path) -> Result<(), SourceResolveError> {
    let mut paths = vec![root.to_path_buf()];
    while let Some(path) = paths.pop() {
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
        if !metadata.file_type().is_symlink() {
            verify_path_read_only(&path, &metadata)?;
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path).map_err(|error| io_error(&path, error))? {
                paths.push(entry.map_err(|error| io_error(&path, error))?.path());
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn verify_path_read_only(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o222 != 0 {
        return Err(cache_invalid(path, "published snapshot is writable"));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_path_read_only(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    if metadata.is_file() && !metadata.permissions().readonly() {
        return Err(cache_invalid(path, "published snapshot is writable"));
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
    require_local_snapshot_directory(publication, "snapshot publication is not a real directory")?;
    let source = publication.join(LOCAL_SNAPSHOT_SOURCE);
    require_local_snapshot_directory(&source, "snapshot source is not a real directory")?;
    let metadata_path = publication.join(LOCAL_SNAPSHOT_METADATA);
    require_local_snapshot_file(&metadata_path, "snapshot metadata is not a regular file")?;
    let metadata_length = std::fs::symlink_metadata(&metadata_path)
        .map_err(|error| io_error(&metadata_path, error))?
        .len();
    if metadata_length > 512 {
        return Err(local_snapshot_invalid(
            &metadata_path,
            "snapshot metadata exceeds its limit",
        ));
    }
    let metadata =
        std::fs::read(&metadata_path).map_err(|error| io_error(&metadata_path, error))?;
    let expected = parse_local_snapshot_metadata(&metadata, &metadata_path)?;
    if expected.content_identity != content_identity {
        return Err(local_snapshot_invalid(
            &metadata_path,
            "snapshot content identity does not match its cache key",
        ));
    }
    verify_local_snapshot_modes(publication)?;
    let normalized = resolve_materialized_source(&source, limits)?;
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

fn require_local_snapshot_directory(path: &Path, message: &str) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(local_snapshot_invalid(path, message));
    }
    Ok(())
}

fn require_local_snapshot_file(path: &Path, message: &str) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(local_snapshot_invalid(path, message));
    }
    Ok(())
}

fn verify_local_snapshot_modes(root: &Path) -> Result<(), SourceResolveError> {
    let mut directories = vec![root.to_path_buf()];
    let mut cursor = 0;
    while cursor < directories.len() {
        let directory = directories[cursor].clone();
        cursor += 1;
        verify_local_snapshot_directory_mode(&directory)?;
        for entry in std::fs::read_dir(&directory).map_err(|error| io_error(&directory, error))? {
            let entry = entry.map_err(|error| io_error(&directory, error))?;
            let path = entry.path();
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
            if metadata.is_dir() {
                directories.push(path);
            } else if metadata.is_file() {
                verify_local_snapshot_file_mode(&path, &metadata)?;
            } else if !metadata.file_type().is_symlink() {
                return Err(local_snapshot_invalid(
                    &path,
                    "snapshot contains an unsupported filesystem entry type",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn verify_local_snapshot_directory_mode(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = std::fs::symlink_metadata(path)
        .map_err(|error| io_error(path, error))?
        .permissions()
        .mode()
        & 0o777;
    if mode != 0o555 {
        return Err(local_snapshot_invalid(
            path,
            "snapshot directory mode is not canonical 0555",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_local_snapshot_directory_mode(_path: &Path) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
fn verify_local_snapshot_file_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = metadata.permissions().mode() & 0o777;
    if !matches!(mode, 0o444 | 0o555) {
        return Err(local_snapshot_invalid(
            path,
            "snapshot file mode is not canonical 0444 or 0555",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_local_snapshot_file_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    if !metadata.permissions().readonly() {
        return Err(local_snapshot_invalid(path, "snapshot file is writable"));
    }
    Ok(())
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
    repository: &Path,
    entries: &mut [GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    executor.verify_budget()?;
    if entries.is_empty() {
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
    for entry in entries.iter() {
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
    for entry in entries {
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
    for entry in entries.iter() {
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
    for (entry, range) in entries.iter_mut().zip(ranges) {
        match &mut entry.kind {
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

#[cfg(unix)]
fn create_snapshot_symlink(target: &[u8], destination: &Path) -> Result<(), SourceResolveError> {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    std::os::unix::fs::symlink(OsString::from_vec(target.to_vec()), destination)
        .map_err(|error| io_error(destination, error))
}

#[cfg(not(unix))]
fn create_snapshot_symlink(target: &[u8], destination: &Path) -> Result<(), SourceResolveError> {
    let target = std::str::from_utf8(target).map_err(|_| {
        git_tree_invalid(target, "symlink target cannot be represented on this host")
    })?;
    std::os::windows::fs::symlink_file(target, destination)
        .map_err(|error| io_error(destination, error))
}

#[cfg(unix)]
fn set_snapshot_file_mode(path: &Path, executable: bool) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if executable { 0o555 } else { 0o444 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn set_snapshot_file_mode(path: &Path, _executable: bool) -> Result<(), SourceResolveError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|error| io_error(path, error))?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).map_err(|error| io_error(path, error))
}

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

#[cfg(unix)]
fn set_snapshot_directory_read_only(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn set_snapshot_directory_read_only(_path: &Path) -> Result<(), SourceResolveError> {
    Ok(())
}

struct PendingSnapshot {
    root: PathBuf,
    published: bool,
}

impl PendingSnapshot {
    fn create(snapshots: &Path, tree: &str) -> Result<Self, SourceResolveError> {
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = snapshots.join(format!(
                ".tree-{tree}.stage-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    return Ok(Self {
                        root,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&root, error)),
            }
        }
        Err(cache_invalid(
            snapshots,
            "could not allocate a unique snapshot staging directory",
        ))
    }
}

impl Drop for PendingSnapshot {
    fn drop(&mut self) {
        if !self.published {
            make_tree_owner_writable(&self.root);
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

struct PendingLocalSnapshot {
    root: PathBuf,
    published: bool,
}

impl PendingLocalSnapshot {
    fn create(snapshots: &Path, identity: &str) -> Result<Self, SourceResolveError> {
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = snapshots.join(format!(
                ".source-{identity}.stage-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    return Ok(Self {
                        root,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&root, error)),
            }
        }
        Err(local_snapshot_invalid(
            snapshots,
            "could not allocate a unique snapshot staging directory",
        ))
    }
}

impl Drop for PendingLocalSnapshot {
    fn drop(&mut self) {
        if !self.published {
            make_tree_owner_writable(&self.root);
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

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
    entry_root: &Path,
    cache_identity: &str,
    url: &str,
    requested_rev: &str,
) -> Result<(), SourceResolveError> {
    let mut pending = PendingCacheEntry::create(cache_dir, cache_identity)?;
    let repository = pending.root.join(GIT_CACHE_REPOSITORY);
    let empty_template = pending.root.join("empty-template");
    std::fs::create_dir(&empty_template).map_err(|error| io_error(&empty_template, error))?;
    run_git(
        executor,
        &pending.root,
        [
            OsStr::new("init"),
            OsStr::new("--quiet"),
            OsStr::new("--bare"),
            OsStr::new("--template"),
            empty_template.as_os_str(),
            repository.as_os_str(),
        ],
    )?;
    run_git(
        executor,
        &repository,
        [
            OsStr::new("config"),
            OsStr::new("--local"),
            OsStr::new("remote.origin.url"),
            OsStr::new(url),
        ],
    )?;
    run_git(
        executor,
        &repository,
        [
            OsStr::new("config"),
            OsStr::new("--local"),
            OsStr::new("remote.origin.fetch"),
            OsStr::new(GIT_ORIGIN_FETCH),
        ],
    )?;
    std::fs::remove_dir(&empty_template).map_err(|error| io_error(&empty_template, error))?;

    let metadata_path = pending.root.join(GIT_CACHE_METADATA);
    let mut metadata = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_path)
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .write_all(&git_cache_metadata(url, requested_rev))
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .sync_all()
        .map_err(|error| io_error(&metadata_path, error))?;

    verify_git_cache_entry(executor, &pending.root, url, requested_rev)?;
    std::fs::rename(&pending.root, entry_root).map_err(|error| io_error(entry_root, error))?;
    pending.published = true;
    Ok(())
}

fn verify_git_cache_entry(
    executor: &GitExecutor,
    entry_root: &Path,
    url: &str,
    requested_rev: &str,
) -> Result<(), SourceResolveError> {
    require_real_directory(entry_root, "cache entry root is not a real directory")?;
    let metadata_path = entry_root.join(GIT_CACHE_METADATA);
    require_regular_file(&metadata_path, "resolver metadata is not a regular file")?;
    let expected_metadata = git_cache_metadata(url, requested_rev);
    let metadata_size = std::fs::symlink_metadata(&metadata_path)
        .map_err(|error| io_error(&metadata_path, error))?
        .len();
    if metadata_size != expected_metadata.len() as u64 {
        return Err(cache_invalid(
            &metadata_path,
            "resolver metadata has an unexpected length",
        ));
    }
    let actual_metadata =
        std::fs::read(&metadata_path).map_err(|error| io_error(&metadata_path, error))?;
    if actual_metadata != expected_metadata {
        return Err(cache_invalid(
            entry_root,
            "resolver metadata does not match the exact source locator and revision",
        ));
    }

    let repository = entry_root.join(GIT_CACHE_REPOSITORY);
    require_real_directory(&repository, "repository is not a real directory")?;
    require_real_directory(
        &repository.join("objects"),
        "Git object directory is not a real directory",
    )?;
    for forbidden in [
        repository.join("objects/info/alternates"),
        repository.join("objects/info/http-alternates"),
        repository.join("commondir"),
    ] {
        if std::fs::symlink_metadata(&forbidden).is_ok() {
            return Err(cache_invalid(
                &forbidden,
                "external Git object or directory indirection is forbidden",
            ));
        }
    }

    let config_path = repository.join("config");
    require_regular_file(
        &config_path,
        "local Git configuration is not a regular file",
    )?;
    if std::fs::symlink_metadata(&config_path)
        .map_err(|error| io_error(&config_path, error))?
        .len()
        > 64 * 1024
    {
        return Err(cache_invalid(
            &config_path,
            "local Git configuration exceeds the resolver limit",
        ));
    }
    let config = run_git_bytes_stdout(
        executor,
        &repository,
        [
            OsStr::new("config"),
            OsStr::new("--local"),
            OsStr::new("--no-includes"),
            OsStr::new("--null"),
            OsStr::new("--list"),
        ],
    )?;
    verify_local_git_config(entry_root, url, &config)?;

    let origin = run_git_bytes_stdout(
        executor,
        &repository,
        [
            OsStr::new("config"),
            OsStr::new("--local"),
            OsStr::new("--no-includes"),
            OsStr::new("--null"),
            OsStr::new("--get"),
            OsStr::new("remote.origin.url"),
        ],
    )?;
    let mut expected_origin = url.as_bytes().to_vec();
    expected_origin.push(0);
    if origin != expected_origin {
        return Err(cache_invalid(
            entry_root,
            "Git origin does not match the exact source locator",
        ));
    }
    Ok(())
}

fn verify_local_git_config(
    entry_root: &Path,
    url: &str,
    bytes: &[u8],
) -> Result<(), SourceResolveError> {
    let mut seen = BTreeSet::new();
    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let Some(separator) = record.iter().position(|byte| *byte == b'\n') else {
            return Err(cache_invalid(
                entry_root,
                "malformed local Git configuration",
            ));
        };
        let key = &record[..separator];
        let value = &record[separator + 1..];
        if !seen.insert(key.to_vec()) {
            return Err(cache_invalid(
                entry_root,
                "duplicate local Git configuration",
            ));
        }
        let allowed = match key {
            b"core.repositoryformatversion" => value == b"0",
            b"core.filemode" | b"core.ignorecase" | b"core.precomposeunicode" => {
                value == b"true" || value == b"false"
            }
            b"core.bare" => value == b"true",
            b"remote.origin.url" => value == url.as_bytes(),
            b"remote.origin.fetch" => value == GIT_ORIGIN_FETCH.as_bytes(),
            _ => false,
        };
        if !allowed {
            return Err(cache_invalid(
                entry_root,
                "local Git configuration contains a non-resolver setting",
            ));
        }
    }
    for required in [
        b"core.repositoryformatversion".as_slice(),
        b"core.filemode".as_slice(),
        b"core.bare".as_slice(),
        b"remote.origin.url".as_slice(),
        b"remote.origin.fetch".as_slice(),
    ] {
        if !seen.contains(required) {
            return Err(cache_invalid(
                entry_root,
                "local Git configuration is missing a resolver-owned setting",
            ));
        }
    }
    Ok(())
}

fn invalidate_git_cache_entry(entry_root: &Path) {
    let metadata_path = entry_root.join(GIT_CACHE_METADATA);
    let _ = std::fs::remove_file(metadata_path);
}

fn git_cache_identity(url: &str, requested_rev: &str) -> String {
    let mut hasher = Sha256::new();
    hash_bytes(&mut hasher, GIT_CACHE_POLICY);
    hash_bytes(&mut hasher, url.as_bytes());
    hash_bytes(&mut hasher, requested_rev.as_bytes());
    format_sha256(&hasher.finalize())
}

fn git_cache_metadata(url: &str, requested_rev: &str) -> Vec<u8> {
    let mut metadata = Vec::new();
    metadata.extend_from_slice(GIT_CACHE_POLICY);
    append_framed_bytes(&mut metadata, url.as_bytes());
    append_framed_bytes(&mut metadata, requested_rev.as_bytes());
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

fn require_regular_file(path: &Path, message: &str) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(cache_invalid(path, message));
    }
    Ok(())
}

struct CacheEntryLock {
    file: File,
}

impl CacheEntryLock {
    fn open_git(path: &Path) -> Result<File, SourceResolveError> {
        if let Ok(metadata) = std::fs::symlink_metadata(path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err(cache_invalid(path, "cache lock is not a regular file"));
        }
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| io_error(path, error))
    }

    fn acquire_with_git_budget(
        path: &Path,
        executor: &GitExecutor,
    ) -> Result<Self, SourceResolveError> {
        let file = Self::open_git(path)?;
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
        require_regular_file(path, "cache lock was replaced while being acquired")?;
        Ok(Self { file })
    }

    #[cfg(test)]
    fn acquire(path: &Path) -> Result<Self, SourceResolveError> {
        let file = Self::open_git(path)?;
        file.lock().map_err(|error| io_error(path, error))?;
        require_regular_file(path, "cache lock was replaced while being acquired")?;
        Ok(Self { file })
    }

    fn acquire_local(path: &Path) -> Result<Self, SourceResolveError> {
        if let Ok(metadata) = std::fs::symlink_metadata(path)
            && (metadata.file_type().is_symlink() || !metadata.is_file())
        {
            return Err(local_snapshot_invalid(
                path,
                "cache lock is not a regular file",
            ));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|error| io_error(path, error))?;
        file.lock().map_err(|error| io_error(path, error))?;
        let metadata = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(local_snapshot_invalid(
                path,
                "cache lock was replaced while being acquired",
            ));
        }
        Ok(Self { file })
    }
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
    published: bool,
}

impl PendingCacheEntry {
    fn create(cache_dir: &Path, cache_identity: &str) -> Result<Self, SourceResolveError> {
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = cache_dir.join(format!(
                ".git-{cache_identity}.stage-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    return Ok(Self {
                        root,
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
}

impl Drop for PendingCacheEntry {
    fn drop(&mut self) {
        if !self.published {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[derive(Debug)]
struct SourceEntry {
    relative_bytes: Vec<u8>,
    relative_path: PathBuf,
    kind: SourceEntryKind,
}

#[derive(Debug)]
enum SourceEntryKind {
    Directory { path: PathBuf },
    File { path: PathBuf },
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

    let mut source_entries = Vec::new();
    let mut visited_dirs = BTreeSet::new();
    visit_directory(
        &root,
        PathBuf::new(),
        0,
        &root,
        limits,
        policy,
        &mut visited_dirs,
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
            SourceEntryKind::Directory { path } => {
                let metadata =
                    std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(SourceResolveError::UnsupportedFileType { path });
                }
                identity.add_directory(&entry.relative_bytes, CANONICAL_DIRECTORY_MODE);
                CapturedLocalEntryKind::Directory
            }
            SourceEntryKind::File { path } => {
                let remaining = limits.max_bytes.checked_sub(identity.byte_count).ok_or(
                    SourceResolveError::TooManyBytes {
                        limit: limits.max_bytes,
                    },
                )?;
                let (bytes, executable) = read_file_bounded(&path, remaining, limits.max_bytes)?;
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
    require_local_snapshot_directory(
        &canonical_cache_dir,
        "local snapshot cache is not a real directory",
    )?;
    let snapshots = canonical_cache_dir.join(LOCAL_CACHE_SNAPSHOTS);
    std::fs::create_dir_all(&snapshots).map_err(|error| io_error(&snapshots, error))?;
    require_local_snapshot_directory(
        &snapshots,
        "local snapshot collection is not a real directory",
    )?;

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
    let mut pending = PendingLocalSnapshot::create(snapshots, identity)?;
    let source = pending.root.join(LOCAL_SNAPSHOT_SOURCE);
    std::fs::create_dir(&source).map_err(|error| io_error(&source, error))?;

    for entry in &captured.entries {
        let destination = source.join(&entry.relative_path);
        match &entry.kind {
            CapturedLocalEntryKind::Directory => {
                std::fs::create_dir_all(&destination)
                    .map_err(|error| io_error(&destination, error))?;
            }
            CapturedLocalEntryKind::File { bytes, executable } => {
                let parent = destination
                    .parent()
                    .expect("captured local paths always have a snapshot parent");
                std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&destination)
                    .map_err(|error| io_error(&destination, error))?;
                file.write_all(bytes)
                    .map_err(|error| io_error(&destination, error))?;
                file.sync_all()
                    .map_err(|error| io_error(&destination, error))?;
                set_snapshot_file_mode(&destination, *executable)?;
            }
            CapturedLocalEntryKind::Symlink { target_bytes } => {
                let parent = destination
                    .parent()
                    .expect("captured local paths always have a snapshot parent");
                std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
                create_snapshot_symlink(target_bytes, &destination)?;
            }
        }
    }
    for entry in captured.entries.iter().rev() {
        if matches!(entry.kind, CapturedLocalEntryKind::Directory) {
            set_snapshot_directory_read_only(&source.join(&entry.relative_path))?;
        }
    }

    let staged = resolve_materialized_source(&source, limits)?;
    if !same_source_identity(&staged, &captured.normalized) {
        return Err(local_snapshot_invalid(
            &source,
            "staged source does not match the captured local tree",
        ));
    }
    verify_live_source_unchanged(&captured.normalized, limits)?;

    let metadata_path = pending.root.join(LOCAL_SNAPSHOT_METADATA);
    let mut metadata = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_path)
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .write_all(&local_snapshot_metadata(&staged))
        .map_err(|error| io_error(&metadata_path, error))?;
    metadata
        .sync_all()
        .map_err(|error| io_error(&metadata_path, error))?;
    make_snapshot_read_only(&pending.root)?;
    std::fs::rename(&pending.root, publication).map_err(|error| io_error(publication, error))?;
    pending.published = true;
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
    real_dir: &Path,
    logical_dir: PathBuf,
    depth: usize,
    root: &Path,
    limits: LocalSourceLimits,
    policy: SourceTreePolicy,
    visited_dirs: &mut BTreeSet<PathBuf>,
    entries: &mut Vec<SourceEntry>,
) -> Result<(), SourceResolveError> {
    if depth > limits.max_depth {
        return Err(SourceResolveError::TooDeep {
            path: real_dir.to_path_buf(),
            limit: limits.max_depth,
        });
    }
    let canonical_dir = real_dir
        .canonicalize()
        .map_err(|error| io_error(real_dir, error))?;
    if !canonical_dir.starts_with(root) {
        return Err(SourceResolveError::SymlinkEscapesRoot {
            link: real_dir.to_path_buf(),
            target: canonical_dir,
        });
    }
    if !visited_dirs.insert(canonical_dir) {
        return Ok(());
    }

    let mut directory_entries = std::fs::read_dir(real_dir)
        .map_err(|error| io_error(real_dir, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(real_dir, error))?;
    directory_entries.sort_by_key(|entry| entry.file_name());

    for entry in directory_entries {
        let name = entry.file_name();
        if policy == SourceTreePolicy::LocalPackage
            && (name == ".git"
                || (logical_dir.as_os_str().is_empty() && name == DEFAULT_BUILD_OUTPUT_DIRECTORY))
        {
            continue;
        }
        let real_path = entry.path();
        let logical_path = logical_dir.join(&name);
        let metadata =
            std::fs::symlink_metadata(&real_path).map_err(|error| io_error(&real_path, error))?;
        if metadata.file_type().is_symlink() {
            let raw_target = read_and_validate_symlink_target(root, &real_path, policy)?;
            push_entry(
                entries,
                logical_path,
                SourceEntryKind::Symlink {
                    target_bytes: raw_os_bytes(raw_target.as_os_str()),
                },
                limits,
            )?;
        } else if metadata.is_dir() {
            push_entry(
                entries,
                logical_path.clone(),
                SourceEntryKind::Directory {
                    path: real_path.clone(),
                },
                limits,
            )?;
            visit_directory(
                &real_path,
                logical_path,
                depth + 1,
                root,
                limits,
                policy,
                visited_dirs,
                entries,
            )?;
        } else if metadata.is_file() {
            push_entry(
                entries,
                logical_path,
                SourceEntryKind::File { path: real_path },
                limits,
            )?;
        } else {
            return Err(SourceResolveError::UnsupportedFileType { path: real_path });
        }
    }
    Ok(())
}

fn read_and_validate_symlink_target(
    root: &Path,
    link: &Path,
    policy: SourceTreePolicy,
) -> Result<PathBuf, SourceResolveError> {
    // Package-local policy hashes link spelling, requires an existing canonical target inside this
    // root, and rejects targets under paths excluded from that package view. Exact resolver-owned
    // materializations have no exclusions. Target contents are visited independently through the
    // ordinary tree walk rather than dereferenced through the link.
    let raw_target = std::fs::read_link(link).map_err(|error| io_error(link, error))?;
    let absolute_target = if raw_target.is_absolute() {
        raw_target.clone()
    } else {
        link.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&raw_target)
    };
    let target = absolute_target
        .canonicalize()
        .map_err(|error| io_error(&absolute_target, error))?;
    if !target.starts_with(root) {
        return Err(SourceResolveError::SymlinkEscapesRoot {
            link: link.to_path_buf(),
            target,
        });
    }
    let relative_target = target
        .strip_prefix(root)
        .expect("root containment was checked above");
    if policy == SourceTreePolicy::LocalPackage
        && relative_target
            .components()
            .any(|component| component.as_os_str() == ".git")
    {
        return Err(SourceResolveError::SymlinkTargetsExcludedMetadata {
            link: link.to_path_buf(),
            target,
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
            target,
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

fn read_file_bounded(
    path: &Path,
    remaining: u64,
    limit: u64,
) -> Result<(Vec<u8>, bool), SourceResolveError> {
    let mut file = std::fs::File::open(path).map_err(|error| io_error(path, error))?;
    let metadata = file.metadata().map_err(|error| io_error(path, error))?;
    if !metadata.is_file() {
        return Err(SourceResolveError::UnsupportedFileType {
            path: path.to_path_buf(),
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
            .map_err(|error| io_error(path, error))?;
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

    Ok((bytes, is_executable(&metadata)))
}

#[cfg(unix)]
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
    fn system() -> Result<Self, SourceResolveError> {
        for candidate in system_git_candidates() {
            let path = Path::new(candidate);
            if path.is_file() {
                return Self::open_with_budget(
                    path,
                    GIT_FIXED_COMMAND_ALLOWANCE,
                    GIT_RESOLUTION_TIMEOUT,
                );
            }
        }
        Err(SourceResolveError::GitExecutableUnavailable)
    }

    #[cfg(test)]
    fn open(path: &Path) -> Result<Self, SourceResolveError> {
        Self::open_with_budget(path, GIT_FIXED_COMMAND_ALLOWANCE, GIT_RESOLUTION_TIMEOUT)
    }

    fn open_with_budget(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
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
        let metadata_identity = observe_git_executable_metadata(&canonical)?;
        let content_identity = hash_git_executable(&canonical)?;
        if observe_git_executable_metadata(&canonical)? != metadata_identity {
            return Err(SourceResolveError::GitExecutableChanged { path: canonical });
        }
        Ok(Self {
            identity: GitExecutableIdentity {
                path: canonical,
                content_identity,
            },
            metadata_identity,
            started,
            timeout,
            launches: Cell::new(0),
            maximum_launches,
        })
    }

    fn verify(&self) -> Result<(), SourceResolveError> {
        if observe_git_executable_metadata(&self.identity.path)? == self.metadata_identity {
            Ok(())
        } else {
            Err(SourceResolveError::GitExecutableChanged {
                path: self.identity.path.clone(),
            })
        }
    }

    fn verify_content(&self) -> Result<(), SourceResolveError> {
        self.verify()?;
        if hash_git_executable(&self.identity.path)? != self.identity.content_identity {
            return Err(SourceResolveError::GitExecutableChanged {
                path: self.identity.path.clone(),
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
        .env("PATH", git_helper_path(&executor.identity.path))
        .env("GIT_ALLOW_PROTOCOL", "file:https:http:ssh:git")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .env(
            "GIT_SSH_COMMAND",
            sealed_ssh_command(&executor.identity.path),
        )
        .env("GIT_SSH_VARIANT", "ssh")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("--no-replace-objects")
        .arg("-c")
        .arg(format!("core.hooksPath={}", null_device()))
        .args([
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.file.allow=always",
            "-c",
            "protocol.http.allow=always",
            "-c",
            "protocol.https.allow=always",
            "-c",
            "protocol.ssh.allow=always",
            "-c",
            "protocol.git.allow=always",
            "-c",
            "protocol.ext.allow=never",
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
    Ok(command)
}

#[cfg(unix)]
fn git_helper_path(_git_executable: &Path) -> OsString {
    OsString::from("/usr/bin:/bin")
}

#[cfg(unix)]
fn sealed_ssh_command(_git_executable: &Path) -> OsString {
    OsString::from(
        "/usr/bin/ssh -F /dev/null -oBatchMode=yes -oPasswordAuthentication=no -oKbdInteractiveAuthentication=no -oNumberOfPasswordPrompts=0 -oStrictHostKeyChecking=yes",
    )
}

#[cfg(windows)]
fn git_helper_path(git_executable: &Path) -> OsString {
    let mut directories = Vec::new();
    if let Some(parent) = git_executable.parent() {
        directories.push(parent.to_path_buf());
        if let Some(root) = parent.parent() {
            directories.push(root.join("bin"));
            directories.push(root.join("usr/bin"));
        }
    }
    std::env::join_paths(directories).unwrap_or_default()
}

#[cfg(windows)]
fn sealed_ssh_command(git_executable: &Path) -> OsString {
    let ssh = git_executable
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("usr/bin/ssh.exe"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Git\usr\bin\ssh.exe"));
    OsString::from(format!(
        "\"{}\" -F NUL -oBatchMode=yes -oPasswordAuthentication=no -oKbdInteractiveAuthentication=no -oNumberOfPasswordPrompts=0 -oStrictHostKeyChecking=yes",
        ssh.display()
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
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PackageName;
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
        std::env::temp_dir().join(format!(
            "omega-packages-{name}-{}-{stamp}",
            std::process::id()
        ))
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

    fn create_git_source(name: &str) -> (PathBuf, String) {
        let root = temp_root(name);
        std::fs::create_dir_all(&root).expect("create git source");
        run_test_git(&root, ["init", "--quiet"]);
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

    fn package_fixtures_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../../fixtures/packages")
    }

    fn git_cache_entry_root(cache: &Path, url: &str, requested_rev: &str) -> PathBuf {
        cache.join(format!("git-{}", git_cache_identity(url, requested_rev)))
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
        let executor = GitExecutor::system().expect("system Git executor");
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
        read_git_blobs_batch(&executor, &repo, &mut entries, LocalSourceLimits::default())
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
            &GitSourceSpec {
                url: repo.display().to_string(),
                rev: Some(commit.clone()),
            },
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
    fn git_source_fetches_only_the_selected_revision_depth() {
        let (repo, _) = create_git_source("git-shallow");
        std::fs::write(repo.join("main.omg"), "machine Main::changed() {}\n")
            .expect("change source");
        run_test_git(&repo, ["add", "main.omg"]);
        run_test_git(&repo, ["commit", "--quiet", "-m", "second"]);
        let cache = temp_root("git-shallow-cache");
        let url = format!("file://{}", repo.display());

        resolve_git_source(
            &GitSourceSpec {
                url: url.clone(),
                rev: Some("HEAD".to_owned()),
            },
            &cache,
            LocalSourceLimits::default(),
        )
        .expect("resolve a shallow exact revision");

        let repository = git_cache_entry_root(&cache, &url, "HEAD").join(GIT_CACHE_REPOSITORY);
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
    fn git_tree_rejects_traversal_metadata_and_nonportable_paths_before_materialization() {
        let repository = temp_root("git-tree-path-validation");
        let oid = "0123456789012345678901234567890123456789";
        for path in [
            b"../escape.omg".as_slice(),
            b"nested/../../escape.omg".as_slice(),
            b"/absolute.omg".as_slice(),
            b"nested\\ambiguous.omg".as_slice(),
            b"nested/.git/config".as_slice(),
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
    fn git_tree_entry_limit_counts_implicit_directories() {
        let repository = temp_root("git-tree-directory-limit");
        let oid = "0123456789012345678901234567890123456789";
        let listing = format!("100644 blob {oid} 0\tnested/main.omg\0");

        let error = parse_git_tree_entries(
            listing.as_bytes(),
            &repository,
            LocalSourceLimits {
                max_files: 1,
                ..LocalSourceLimits::default()
            },
        )
        .expect_err("implicit directory and blob must consume separate identity entries");

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
        let spec = GitSourceSpec {
            url: repo.display().to_string(),
            rev: Some("HEAD".to_owned()),
        };

        let resolved =
            resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("resolve kinds");
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
        let verified = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
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
        let spec = GitSourceSpec {
            url: repo.display().to_string(),
            rev: Some("HEAD".to_owned()),
        };

        let resolved = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
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
    fn git_snapshot_reuse_rehashes_and_rejects_published_tampering() {
        use std::os::unix::fs::PermissionsExt;

        let (repo, _) = create_git_source("git-snapshot-reuse");
        let cache = temp_root("git-snapshot-reuse-cache");
        let url = repo.display().to_string();
        let spec = GitSourceSpec {
            url: url.clone(),
            rev: Some("HEAD".to_owned()),
        };
        let first =
            resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("first resolve");
        let second = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect("reuse snapshot");
        assert_eq!(first.snapshot_root, second.snapshot_root);
        assert_eq!(first.local, second.local);

        std::fs::set_permissions(&first.snapshot_root, std::fs::Permissions::from_mode(0o755))
            .expect("make source root writable for tamper simulation");
        let source = first.snapshot_root.join("main.omg");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
            .expect("make source writable for tamper simulation");
        std::fs::write(&source, "machine Tampered::main() {}\n").expect("tamper snapshot");

        let error = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect_err("tampered published snapshot must reject");
        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        let entry = git_cache_entry_root(&cache, &url, "HEAD");
        assert!(!entry.join(GIT_CACHE_METADATA).exists());

        let _ = std::fs::remove_dir_all(&repo);
        make_tree_owner_writable(&cache);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_batch_failure_precedes_snapshot_staging() {
        let (repo, _) = create_git_source("git-snapshot-cleanup");
        let cache = temp_root("git-snapshot-cleanup-cache");
        let url = repo.display().to_string();
        let spec = GitSourceSpec {
            url: url.clone(),
            rev: Some("HEAD".to_owned()),
        };
        resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry_root = git_cache_entry_root(&cache, &url, "HEAD");
        let repository = entry_root.join(GIT_CACHE_REPOSITORY);
        let missing_oid = "0000000000000000000000000000000000000000";
        let executor = GitExecutor::system().expect("system Git executor");
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
        let error = read_git_blobs_batch(
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
        let spec = GitSourceSpec {
            url: repo.display().to_string(),
            rev: Some("HEAD".to_owned()),
        };
        let resolved =
            resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("prime cache");

        assert!(!resolved.snapshot_root.join("injected.omg").exists());
        assert_eq!(resolved.local.file_count, 1);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_identity_is_full_policy_versioned_and_injectively_framed() {
        let first = git_cache_identity("a\0b", "c");
        let second = git_cache_identity("a", "b\0c");

        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|character| character.is_ascii_hexdigit()));
        assert_ne!(first, second);
        assert_ne!(first, git_cache_identity("a\0b", "C"));
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
    fn git_cache_rejects_resolver_metadata_substitution() {
        let (repo, _) = create_git_source("git-metadata-source");
        let (substitute, _) = create_git_source("git-metadata-substitute");
        let cache = temp_root("git-metadata-cache");
        let url = repo.display().to_string();
        let substitute_url = substitute.display().to_string();
        let spec = GitSourceSpec {
            url: url.clone(),
            rev: Some("HEAD".to_owned()),
        };
        resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("prime cache");
        let entry = git_cache_entry_root(&cache, &url, "HEAD");
        std::fs::write(
            entry.join(GIT_CACHE_METADATA),
            git_cache_metadata(&substitute_url, "HEAD"),
        )
        .expect("substitute metadata");

        let error = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect_err("substituted metadata must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!entry.join(GIT_CACHE_METADATA).exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&substitute);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_cache_rejects_origin_substitution() {
        let (repo, _) = create_git_source("git-origin-source");
        let (substitute, _) = create_git_source("git-origin-substitute");
        let cache = temp_root("git-origin-cache");
        let url = repo.display().to_string();
        let substitute_url = substitute.display().to_string();
        let spec = GitSourceSpec {
            url: url.clone(),
            rev: Some("HEAD".to_owned()),
        };
        resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("prime cache");
        let repository = git_cache_entry_root(&cache, &url, "HEAD").join(GIT_CACHE_REPOSITORY);
        run_test_git(
            &repository,
            ["remote", "set-url", "origin", substitute_url.as_str()],
        );
        let entry = git_cache_entry_root(&cache, &url, "HEAD");

        let error = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect_err("substituted origin must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!entry.join(GIT_CACHE_METADATA).exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&substitute);
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
        let url = repo.display().to_string();
        let spec = GitSourceSpec {
            url,
            rev: Some("HEAD".to_owned()),
        };
        resolve_git_source(&spec, &cache, LocalSourceLimits::default()).expect("prime cache");
        let repository = git_cache_entry_root(&cache, &spec.url, "HEAD").join(GIT_CACHE_REPOSITORY);
        run_test_git(
            &repository,
            [
                "config",
                "--local",
                "filter.omega-test.smudge",
                &format!("touch {}", sentinel.display()),
            ],
        );

        let error = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
            .expect_err("local filter configuration must reject");

        assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
        assert!(!sentinel.exists());
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn git_commands_seal_ambient_config_protocol_and_execution_injection() {
        let executor = GitExecutor::system().expect("system Git executor");
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
                Some(OsString::from("file:https:http:ssh:git")),
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
                OsString::from("GIT_LFS_SKIP_SMUDGE"),
                Some(OsString::from("1")),
            ),
            (
                OsString::from("GIT_PROTOCOL_FROM_USER"),
                Some(OsString::from("0")),
            ),
            (
                OsString::from("GIT_SSH_COMMAND"),
                Some(sealed_ssh_command(&executor.identity.path)),
            ),
            (
                OsString::from("GIT_SSH_VARIANT"),
                Some(OsString::from("ssh")),
            ),
            (
                OsString::from("GIT_TERMINAL_PROMPT"),
                Some(OsString::from("0")),
            ),
            (OsString::from("LANG"), Some(OsString::from("C"))),
            (OsString::from("LC_ALL"), Some(OsString::from("C"))),
            (
                OsString::from("PATH"),
                Some(git_helper_path(&executor.identity.path)),
            ),
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
                .any(|argument| argument == "fetch.recurseSubmodules=false")
        );
        assert!(arguments.iter().any(|argument| argument == "gc.auto=0"));
        assert!(
            arguments
                .iter()
                .any(|argument| argument == "maintenance.auto=false")
        );
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

    #[cfg(target_os = "macos")]
    #[test]
    fn system_git_executor_excludes_the_apple_dispatcher() {
        let executor = GitExecutor::system().expect("concrete macOS Git executor");
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
        let url = repo.display().to_string();
        let spec = GitSourceSpec {
            url,
            rev: Some("HEAD".to_owned()),
        };
        let initial = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
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

        let error = resolve_git_source(&spec, &cache, LocalSourceLimits::default())
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
