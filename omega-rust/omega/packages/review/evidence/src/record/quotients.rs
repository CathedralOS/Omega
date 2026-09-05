use super::PackageReviewCanonicalRowSource;
use language_semantics::quotient_correspondence::CanonicalQuotientCorrespondence;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// Proof-only package-review projection of the bounded direct quotient
/// correspondence batch: total faithful `define` and position-preserving
/// transport-backed `lift`.
///
/// This record is deliberately separate from [`super::CheckedPackageReviewProjection`]:
/// ordinary checking still rejects every quotient operation request. The
/// compiler can issue this review record only through the transactional
/// source-validation entrance that owns the complete batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonExecutableQuotientPackageReview {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) correspondences: Vec<CanonicalQuotientCorrespondence>,
    pub(crate) row_sources: Vec<PackageReviewCanonicalRowSource>,
}

impl NonExecutableQuotientPackageReview {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub fn correspondences(&self) -> &[CanonicalQuotientCorrespondence] {
        &self.correspondences
    }
}
