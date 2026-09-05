//! Closed failure vocabulary for local and Git source resolution.

use crate::identity::SourceContentDigest;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceResolveError {
    PrivateStorageUnavailable {
        message: String,
    },
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
    GitExecutionBoundaryInvalid {
        message: String,
    },
    GitResolutionCommandLimit {
        limit: usize,
    },
    GitResolutionTimedOut {
        timeout_millis: u64,
    },
    GitResolutionCapturedOutputLimit {
        ceiling: u64,
        attempted: u64,
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
    GitExactRevisionUnavailable {
        commit: String,
        tree: String,
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
            Self::PrivateStorageUnavailable { message } => {
                write!(output, "private resolver storage is unavailable: {message}")
            }
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
                "no launchable Git executable was found in the absolute entries of the snapshotted PATH",
            ),
            Self::GitExecutableInvalid { path, message } => write!(
                output,
                "Git resolver executable `{}` is invalid: {message}",
                path.display()
            ),
            Self::GitExecutionBoundaryInvalid { message } => {
                write!(output, "Git native execution boundary is invalid: {message}")
            }
            Self::GitResolutionCommandLimit { limit } => write!(
                output,
                "Git source resolution exceeded its {limit}-command launch ceiling"
            ),
            Self::GitResolutionTimedOut { timeout_millis } => write!(
                output,
                "Git source resolution exceeded its {timeout_millis}-millisecond whole-operation deadline"
            ),
            Self::GitResolutionCapturedOutputLimit { ceiling, attempted } => write!(
                output,
                "Git source resolution attempted to capture {attempted} bytes across all commands, exceeding its {ceiling}-byte cumulative output ceiling"
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
            Self::GitExactRevisionUnavailable { commit, tree } => write!(
                output,
                "recorded Git revision (commit `{commit}`, root tree `{tree}`) is unavailable in the retained cache"
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

pub(crate) fn cache_invalid(path: &Path, message: impl Into<String>) -> SourceResolveError {
    SourceResolveError::GitCacheInvalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

pub(crate) fn local_snapshot_invalid(
    path: &Path,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::LocalSnapshotInvalid {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

pub(crate) fn git_tree_invalid(
    path: impl AsRef<[u8]>,
    message: impl Into<String>,
) -> SourceResolveError {
    SourceResolveError::GitTreeInvalid {
        path: path.as_ref().to_vec(),
        message: message.into(),
    }
}
