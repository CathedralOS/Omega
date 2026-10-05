//! Common package-review evidence interface and live compiler adaptation.

use crate::declarations::PackageKey;
use crate::review::CompilerIssuedPackageReview;
use package_evidence::record::PackagePolicyRow;
use package_source::ImmutableSourceResolution;

use super::build_observation_commitment;
use super::rows::ReviewOnlySourceConsumptionCommitment;

/// The package-manager-facing view of compiler review facts used by comparison
/// and source-triage operations.
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
    fn policy_commitment(&self) -> [u8; 32];
    fn policy_rows(&self) -> &[PackagePolicyRow];
}

impl PackageReviewEvidence for CompilerIssuedPackageReview {
    fn key(&self) -> &PackageKey {
        CompilerIssuedPackageReview::key(self)
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        CompilerIssuedPackageReview::resolution(self)
    }

    fn projection_identity_matches(&self) -> bool {
        self.policy().package() == self.key().identity()
    }

    fn target_name(&self) -> &str {
        self.policy().target().target_name()
    }

    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        CompilerIssuedPackageReview::source_consumption_commitment(self).into()
    }

    fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation_summary()
            .map(build_observation_commitment)
    }

    fn policy_commitment(&self) -> [u8; 32] {
        CompilerIssuedPackageReview::policy_commitment(self)
    }

    fn policy_rows(&self) -> &[PackagePolicyRow] {
        CompilerIssuedPackageReview::policy_rows(self)
    }
}
