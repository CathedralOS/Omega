//! Transport-erased custody for one declared immutable package source.
//!
//! Persistent sources retain a sealed physical root. An ordinary compilation
//! root retains its complete acquisition inventory instead: the root spelling
//! only routes consumers through that closed namespace. Keeping the capture
//! here prevents transport erasure from dropping the bytes and forcing loading
//! or Build to reopen a changed checkout.

use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::{
    DependencyProjections, DependencyPurpose, DependencySourceRequest,
};
use crate::resolution::source::{
    PackageSourceMaterialization, PackageSourceNavigation, PackageSourceSelectionEvidence,
};
use package_source::ImmutableSourceResolution;
use package_source::LocalSourceLimits;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Immutable source bound to its executed or accepted recorded package metadata.
/// Historical metadata never replaces resolver authentication of the bytes.
#[derive(Debug, Clone)]
pub(crate) struct PackageSourceCustody {
    key: PackageKey,
    role: BuildDeclarationKind,
    resolution: ImmutableSourceResolution,
    materialization: PackageSourceMaterialization,
    pub(crate) snapshot_root: PathBuf,
    captured_source: Option<Arc<source::CapturedSourceTree>>,
    navigation: PackageSourceNavigation,
    selection_evidence: PackageSourceSelectionEvidence,
    source_limits: LocalSourceLimits,
    dependency_projections: DependencyProjections,
    /// Retained resolver lane directory that may persist this custody's
    /// canonical source index. It is a storage location, not source
    /// identity, so equality and semantic equivalence ignore it.
    checked_source_cache_dir: Option<PathBuf>,
    /// Retained resolver lane directory that may persist this custody's
    /// compiled review records. A storage location, not source identity —
    /// equality and semantic equivalence ignore it.
    compiled_review_cache_dir: Option<PathBuf>,
}

/// Name of the lane child holding persistent checked-source indexes beneath
/// a retained resolver storage lane.
pub(crate) const CHECKED_SOURCE_CACHE_LANE: &str = "checked-source-cache";

/// Name of the lane child holding persistent compiled-review records beneath
/// a retained resolver storage lane.
pub(crate) const COMPILED_REVIEW_CACHE_LANE: &str = "compiled-review-cache";

/// Optional storage coordinates minted from the current resolver's retained
/// lanes. These select storage only, never source or compilation authority.
#[derive(Debug, Clone)]
pub(super) struct SourceCacheDirectories {
    checked_source: PathBuf,
    compiled_review: PathBuf,
}

impl SourceCacheDirectories {
    pub(super) fn retain(storage: &package_source::SourceResolver) -> Option<Self> {
        let retained = (|| {
            storage.verify_path_identity()?;
            let checked_source = storage
                .external_local_sources()
                .retain_child(CHECKED_SOURCE_CACHE_LANE)?;
            let compiled_review = retain_compiled_review_storage(storage)?;
            storage.verify_path_identity()?;
            Ok::<_, package_source::SourceResolveError>(Self {
                checked_source: checked_source.path().to_path_buf(),
                compiled_review: compiled_review.path().to_path_buf(),
            })
        })();
        retained.ok()
    }

    pub(super) fn apply_to(&self, custody: PackageSourceCustody) -> PackageSourceCustody {
        custody
            .with_checked_source_cache_dir(self.checked_source.clone())
            .with_compiled_review_cache_dir(self.compiled_review.clone())
    }
}

impl PartialEq for PackageSourceCustody {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.role == other.role
            && self.resolution == other.resolution
            && self.materialization == other.materialization
            && self.snapshot_root == other.snapshot_root
            && self.captured_source == other.captured_source
            && self.navigation == other.navigation
            && self.selection_evidence == other.selection_evidence
            && self.dependency_projections == other.dependency_projections
    }
}

impl Eq for PackageSourceCustody {}

impl PackageSourceCustody {
    pub(crate) fn from_resolved_parts<D>(
        key: PackageKey,
        role: BuildDeclarationKind,
        resolution: ImmutableSourceResolution,
        materialization: PackageSourceMaterialization,
        snapshot_root: PathBuf,
        navigation: PackageSourceNavigation,
        selection_evidence: PackageSourceSelectionEvidence,
        source_limits: LocalSourceLimits,
        projected_dependencies: D,
    ) -> Self
    where
        D: Into<DependencyProjections>,
    {
        debug_assert!(resolution.matches_lineage(key.source_lineage()));
        Self {
            key,
            role,
            resolution,
            materialization,
            snapshot_root,
            captured_source: None,
            navigation,
            selection_evidence,
            source_limits,
            dependency_projections: projected_dependencies.into(),
            checked_source_cache_dir: None,
            compiled_review_cache_dir: None,
        }
    }

    pub(crate) fn with_captured_source(mut self, tree: Arc<source::CapturedSourceTree>) -> Self {
        self.captured_source = Some(tree);
        self
    }

    pub fn captured_source(&self) -> Option<&Arc<source::CapturedSourceTree>> {
        self.captured_source.as_ref()
    }

    /// Attach the retained lane directory offered to the checked source
    /// cache during compiler-input preparation.
    pub(crate) fn with_checked_source_cache_dir(mut self, directory: PathBuf) -> Self {
        self.checked_source_cache_dir = Some(directory);
        self
    }

    /// Retained directory offered to the checked source cache, when
    /// resolution supplied one.
    pub(crate) fn checked_source_cache_dir(&self) -> Option<&Path> {
        self.checked_source_cache_dir.as_deref()
    }

    /// Attach the retained lane directory offered to the compiled review
    /// cache during dependency compilation.
    pub(crate) fn with_compiled_review_cache_dir(mut self, directory: PathBuf) -> Self {
        self.compiled_review_cache_dir = Some(directory);
        self
    }

    /// Retained directory offered to the compiled review cache, when
    /// resolution supplied one.
    pub(crate) fn compiled_review_cache_dir(&self) -> Option<&Path> {
        self.compiled_review_cache_dir.as_deref()
    }

    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub const fn role(&self) -> BuildDeclarationKind {
        self.role
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub const fn materialization(&self) -> &PackageSourceMaterialization {
        &self.materialization
    }

    pub fn snapshot_root(&self) -> &std::path::Path {
        &self.snapshot_root
    }

    pub const fn navigation(&self) -> &PackageSourceNavigation {
        &self.navigation
    }

    pub const fn selection_evidence(&self) -> &PackageSourceSelectionEvidence {
        &self.selection_evidence
    }

    pub fn source_limits(&self) -> LocalSourceLimits {
        self.source_limits
    }

    /// Authored requests for one purpose scope, in requester-local order.
    pub fn dependency_requests(&self, purpose: DependencyPurpose) -> &[DependencySourceRequest] {
        self.dependency_projections.requests(purpose)
    }

    pub const fn dependency_projections(&self) -> &DependencyProjections {
        &self.dependency_projections
    }

    pub(crate) fn semantically_equivalent(&self, other: &Self) -> bool {
        self.key == other.key
            && self.role == other.role
            && self.resolution == other.resolution
            && self.materialization == other.materialization
            && self.navigation == other.navigation
            && self.selection_evidence == other.selection_evidence
            && self.dependency_projections == other.dependency_projections
    }
}

pub(super) fn retain_compiled_review_storage(
    storage: &package_source::SourceResolver,
) -> Result<package_source::storage::RetainedStorageLane, package_source::SourceResolveError> {
    storage.verify_path_identity()?;
    let retained = storage
        .external_local_sources()
        .retain_child(COMPILED_REVIEW_CACHE_LANE)?;
    storage.verify_path_identity()?;
    Ok(retained)
}
