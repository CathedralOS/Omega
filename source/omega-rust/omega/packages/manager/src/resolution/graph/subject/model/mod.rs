//! Canonical source-closure subject model and construction.

mod construction;
mod error;
mod fingerprint;
mod limits;
mod request;
#[cfg(test)]
mod test_support;

pub use error::CanonicalSourceClosureSubjectError;
pub use fingerprint::CanonicalSourceClosureSubjectFingerprint;
pub use limits::CanonicalSourceClosureSubjectLimits;
pub use request::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection,
};

use super::super::ResolvedSourceIdentity;
use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::ProjectedDependencies;
use crate::resolution::source::PackageSourceNavigation;
use omega_target::TargetProfile;

#[cfg(test)]
mod tests;

pub(super) const SOURCE_CLOSURE_SUBJECT_MAGIC: &[u8] = b"OMEGA-SOURCE-CLOSURE-SUBJECT\0";
pub const SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION: u16 = 6;
pub(super) const SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN: &[u8] =
    b"OMEGA-SOURCE-CLOSURE-SUBJECT-FINGERPRINT\0";

/// Canonical, non-admitting subject for one exact resolved source closure.
///
/// The subject binds every package key and immutable resolution, the exact root
/// request, and every requester-local dependency request occurrence. Snapshot
/// roots, cache paths, transport execution observations, compiler evidence,
/// certificates, decisions, and artifacts are intentionally absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSourceClosureSubject {
    pub(super) target_profile: TargetProfile,
    pub(super) root: CanonicalRootSourceSelection,
    pub(super) packages: Vec<ResolvedSourceIdentity>,
    pub(super) package_navigations: Vec<PackageSourceNavigation>,
    pub(super) package_dependency_projections: Vec<ProjectedDependencies>,
    pub(super) dependency_requests: Vec<CanonicalDependencySourceSelection>,
    pub(super) canonical_bytes: Vec<u8>,
    pub(super) fingerprint: CanonicalSourceClosureSubjectFingerprint,
}

impl CanonicalSourceClosureSubject {
    pub const fn root(&self) -> &CanonicalRootSourceSelection {
        &self.root
    }

    pub const fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }

    pub const fn root_role(&self) -> BuildDeclarationKind {
        self.root.role()
    }

    pub fn packages(&self) -> &[ResolvedSourceIdentity] {
        &self.packages
    }

    pub fn package_navigation(&self, package: &PackageKey) -> Option<&PackageSourceNavigation> {
        self.packages
            .binary_search_by(|source| source.key().cmp(package))
            .ok()
            .map(|index| &self.package_navigations[index])
    }

    pub fn package_dependency_projection(
        &self,
        package: &PackageKey,
    ) -> Option<&ProjectedDependencies> {
        self.packages
            .binary_search_by(|source| source.key().cmp(package))
            .ok()
            .map(|index| &self.package_dependency_projections[index])
    }

    pub fn dependency_requests(&self) -> &[CanonicalDependencySourceSelection] {
        &self.dependency_requests
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn fingerprint(&self) -> &CanonicalSourceClosureSubjectFingerprint {
        &self.fingerprint
    }
}
