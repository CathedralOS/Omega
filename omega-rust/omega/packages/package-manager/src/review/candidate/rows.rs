//! Review-only commitment value types.

use package_compilation::PackageSourceConsumptionCommitment;
/// Review-only identity of the exact package/toolchain source bytes consumed
/// by one compiler run. It is provenance, not admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlySourceConsumptionCommitment([u8; 32]);

impl ReviewOnlySourceConsumptionCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

impl From<PackageSourceConsumptionCommitment> for ReviewOnlySourceConsumptionCommitment {
    fn from(commitment: PackageSourceConsumptionCommitment) -> Self {
        Self(commitment.digest())
    }
}
