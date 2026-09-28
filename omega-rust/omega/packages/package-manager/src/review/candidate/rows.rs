//! Review-only commitment and canonical-row value types.

use package_compilation::PackageSourceConsumptionCommitment;
use package_evidence::record::PackageReviewCanonicalRow;

/// One owned row collection; whole-review equality includes source associations
/// that the canonical row's semantic equality deliberately excludes.
#[derive(Debug, Clone, Eq)]
pub(super) struct RetainedReviewRows(pub(super) Vec<PackageReviewCanonicalRow>);

impl PartialEq for RetainedReviewRows {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
            && self
                .0
                .iter()
                .zip(&other.0)
                .all(|(left, right)| left.source() == right.source())
    }
}

/// Review-only identity of the exact package/toolchain source bytes consumed
/// by one compiler run. It is provenance, not admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlySourceConsumptionCommitment([u8; 32]);

impl ReviewOnlySourceConsumptionCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }

    #[cfg(test)]
    pub(crate) const fn for_test_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}

impl From<PackageSourceConsumptionCommitment> for ReviewOnlySourceConsumptionCommitment {
    fn from(commitment: PackageSourceConsumptionCommitment) -> Self {
        Self(commitment.digest())
    }
}
