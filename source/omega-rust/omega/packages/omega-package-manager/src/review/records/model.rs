//! Review-only commitment and canonical-row value types.

use omega_package_compilation::PackageSourceConsumptionCommitment;
use omega_package_review::{
    DecodedPackageReviewCanonicalRow, PackageReviewCanonicalRow, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
};

/// Review-only identity of the exact package/toolchain source bytes consumed
/// by one compiler run. It is provenance, not admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlySourceConsumptionCommitment([u8; 32]);

impl ReviewOnlySourceConsumptionCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_recovered_digest(digest: [u8; 32]) -> Self {
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
/// Live rows are copied from an unforgeable compiler-issued review. Recovered
/// rows are constructed only by the compiler's strict recovery-frame decoder
/// and remain distinctly review-only; this type is never compiler evidence or
/// an admission artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyCanonicalRow {
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    key_bytes: Vec<u8>,
    canonical_bytes: Vec<u8>,
    source: PackageReviewCanonicalRowSource,
    recovery_bytes: Option<Vec<u8>>,
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
            recovery_bytes: None,
        }
    }

    pub(crate) fn from_recovered(
        row: &DecodedPackageReviewCanonicalRow,
        recovery_bytes: Vec<u8>,
    ) -> Self {
        Self {
            kind: row.kind(),
            risk: row.risk(),
            key_bytes: row.key_bytes().to_vec(),
            canonical_bytes: row.canonical_bytes().to_vec(),
            source: row.source().clone(),
            recovery_bytes: Some(recovery_bytes),
        }
    }

    pub(crate) fn recovery_bytes(&self) -> Option<&[u8]> {
        self.recovery_bytes.as_deref()
    }
}
