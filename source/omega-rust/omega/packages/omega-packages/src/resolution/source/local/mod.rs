//! Local-source capture, immutable publication, and verification.

use super::*;

pub(super) mod capture;
pub(super) mod snapshot;

pub(super) use capture::*;
pub(super) use snapshot::*;

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
