use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

use crate::record::{
    PackageReviewCanonicalRow, PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewCanonicalRowSource,
};

/// Version of the package-review canonical-row recovery envelope.
pub const PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION: u16 = 19;

/// Resource ceilings applied while encoding or decoding one canonical-row
/// recovery envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageReviewCanonicalRowRecoveryLimits {
    pub(super) maximum_recovery_bytes: usize,
    pub(super) maximum_canonical_row_bytes: usize,
    pub(super) maximum_target_bytes: usize,
    pub(super) maximum_row_key_bytes: usize,
    pub(super) maximum_row_value_bytes: usize,
    pub(super) maximum_source_locations: usize,
    pub(super) maximum_source_path_bytes: usize,
    pub(super) maximum_total_source_path_bytes: usize,
    pub(super) maximum_compiler_derivations: usize,
}

impl PackageReviewCanonicalRowRecoveryLimits {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        maximum_recovery_bytes: usize,
        maximum_canonical_row_bytes: usize,
        maximum_target_bytes: usize,
        maximum_row_key_bytes: usize,
        maximum_row_value_bytes: usize,
        maximum_source_locations: usize,
        maximum_source_path_bytes: usize,
        maximum_total_source_path_bytes: usize,
        maximum_compiler_derivations: usize,
    ) -> Self {
        Self {
            maximum_recovery_bytes,
            maximum_canonical_row_bytes,
            maximum_target_bytes,
            maximum_row_key_bytes,
            maximum_row_value_bytes,
            maximum_source_locations,
            maximum_source_path_bytes,
            maximum_total_source_path_bytes,
            maximum_compiler_derivations,
        }
    }
}

impl Default for PackageReviewCanonicalRowRecoveryLimits {
    fn default() -> Self {
        Self::new(
            64 * 1024 * 1024,
            4 * 1024 * 1024,
            4 * 1024,
            1024 * 1024,
            4 * 1024 * 1024,
            262_144,
            1024 * 1024,
            16 * 1024 * 1024,
            4,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewCanonicalRowRecoveryError {
    message: &'static str,
}

impl PackageReviewCanonicalRowRecoveryError {
    pub(super) const fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for PackageReviewCanonicalRowRecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for PackageReviewCanonicalRowRecoveryError {}

/// A review-only row envelope decoded by the compiler together with the
/// package and target parsed from its canonical outer frame.
///
/// The row payload remains opaque: decoding establishes canonical framing and
/// source-sidecar shape, not semantic re-issuance. This is restart metadata for
/// review, not compiler-issued package admission evidence.
#[derive(Debug, Clone)]
pub struct DecodedPackageReviewCanonicalRow {
    pub(super) package: PackageKeyIdentity,
    pub(super) target: TargetProfile,
    pub(super) row: PackageReviewCanonicalRow,
}

impl DecodedPackageReviewCanonicalRow {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.row.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.row.risk
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.row.key_bytes
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.row.canonical_bytes
    }

    pub const fn source(&self) -> &PackageReviewCanonicalRowSource {
        &self.row.source
    }
}
