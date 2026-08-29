//! Common package-review evidence interface and live compiler adaptation.

use crate::identity::PackageKey;
use crate::review::CompilerIssuedPackageReview;
use omega_package_source::ImmutableSourceResolution;

use super::rows::{ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment};
use super::{build_observation_commitment, whole_review_commitment};

/// The package-manager-facing evidence common to a live compiler review and a
/// restart-stable review-only baseline record.
///
/// This trait is deliberately private. Implementing it does not issue accepted
/// evidence or permit construction of a package instance.
pub(crate) trait PackageReviewEvidence {
    fn key(&self) -> &PackageKey;
    fn resolution(&self) -> &ImmutableSourceResolution;
    fn projection_identity_matches(&self) -> bool;
    fn target_name(&self) -> &str;
    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment;
    fn build_observation_commitment(&self) -> Option<[u8; 32]>;
    fn whole_review_commitment(&self) -> [u8; 32];
    fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow];
}

impl PackageReviewEvidence for CompilerIssuedPackageReview {
    fn key(&self) -> &PackageKey {
        CompilerIssuedPackageReview::key(self)
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        CompilerIssuedPackageReview::resolution(self)
    }

    fn projection_identity_matches(&self) -> bool {
        self.projection().package() == self.key().identity()
    }

    fn target_name(&self) -> &str {
        self.projection().target().target_name()
    }

    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        CompilerIssuedPackageReview::source_consumption_commitment(self).into()
    }

    fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation_summary()
            .map(build_observation_commitment)
    }

    fn whole_review_commitment(&self) -> [u8; 32] {
        whole_review_commitment(self.canonical_review_bytes())
    }

    fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        self.comparison_rows()
    }
}
