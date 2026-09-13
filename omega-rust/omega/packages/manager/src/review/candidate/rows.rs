//! Review-only commitment and canonical-row value types.

use package_compilation::PackageSourceConsumptionCommitment;
use package_evidence::record::{
    PackageReviewCanonicalRow, PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewCanonicalRowSource,
};

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

/// Opaque canonical comparison row used by package review orchestration.
///
/// Rows are copied from a compiler-issued review for comparison. This type is
/// never compiler evidence or an admission artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyCanonicalRow {
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    key_bytes: Vec<u8>,
    canonical_bytes: Vec<u8>,
    source: PackageReviewCanonicalRowSource,
}

impl ReviewOnlyCanonicalRow {
    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.risk
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn source(&self) -> &PackageReviewCanonicalRowSource {
        &self.source
    }

    pub(crate) fn from_compiler_issued(row: &PackageReviewCanonicalRow) -> Self {
        Self {
            kind: row.kind(),
            risk: row.risk(),
            key_bytes: row.key_bytes().to_vec(),
            canonical_bytes: row.canonical_bytes().to_vec(),
            source: row.source().clone(),
        }
    }
}
