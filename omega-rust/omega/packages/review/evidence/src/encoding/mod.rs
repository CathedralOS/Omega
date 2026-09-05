//! Canonical byte production and bounded recovery for package-review evidence.
//!
//! Encoding owns stable framing only. It consumes inert evidence and does not
//! inspect compiler state or decide whether evidence should be admitted.

use crate::record::{
    CheckedPackageReviewProjection, NonExecutableQuotientPackageReview, PackageReviewCanonicalRow,
};

mod encode;
mod recovery;

pub use recovery::{PackagePolicyRecoveryError, PackagePolicyRecoveryLimits};

pub const PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION: u16 = 1;
pub(crate) const EXTERNAL_SUPPLY_POLICY_MAGIC: &[u8] = b"OMEGA-EXTERNAL-SUPPLY-POLICY\0";

pub const PACKAGE_CONFORMANCE_POLICY_VERSION: u16 = 1;
pub(crate) const CONFORMANCE_POLICY_MAGIC: &[u8] = b"OMEGA-CONFORMANCE-POLICY\0";

pub const PACKAGE_PHYSICAL_CALLING_POLICY_VERSION: u16 = 1;
pub(crate) const PHYSICAL_CALLING_POLICY_MAGIC: &[u8] = b"OMEGA-PHYSICAL-CALLING-POLICY\0";

pub const PACKAGE_CALLING_POLICY_VERSION: u16 = 1;
pub(crate) const CALLING_POLICY_MAGIC: &[u8] = b"OMEGA-CALLING-POLICY\0";

pub const PACKAGE_REPRESENTATION_POLICY_VERSION: u16 = 1;
pub(crate) const REPRESENTATION_POLICY_MAGIC: &[u8] = b"OMEGA-REPRESENTATION-POLICY\0";

pub const PACKAGE_SELECTED_PROVIDER_POLICY_VERSION: u16 = 1;
pub(crate) const SELECTED_PROVIDER_POLICY_MAGIC: &[u8] = b"OMEGA-SELECTED-PROVIDER-POLICY\0";

pub const PACKAGE_TERMINAL_PERMISSION_POLICY_VERSION: u16 = 1;
pub(crate) const TERMINAL_PERMISSION_POLICY_MAGIC: &[u8] = b"OMEGA-TERMINAL-PERMISSION-POLICY\0";

pub const PACKAGE_CALLABLE_POLICY_VERSION: u16 = 1;
pub(crate) const CALLABLE_POLICY_MAGIC: &[u8] = b"OMEGA-CALLABLE-POLICY\0";

pub use encode::{
    PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION,
    PackageReviewEncodingError,
};
pub use recovery::{
    DecodedPackageReviewCanonicalRow, PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION,
    PackageReviewCanonicalRowRecoveryError, PackageReviewCanonicalRowRecoveryLimits,
    decode_package_review_canonical_row, decode_package_review_canonical_row_with_limits,
    encode_package_review_canonical_row, encode_package_review_canonical_row_with_limits,
};
pub(crate) use recovery::{canonical_row_framing_for_ledger, canonical_row_subject_for_ledger};
impl CheckedPackageReviewProjection {
    /// Versioned, source-handle-free comparison bytes for this review-only
    /// projection. These bytes are not a package certificate and must not be
    /// persisted as accepted evidence without the source/toolchain/compiler
    /// binding and remaining required admission-projection joins. Terminal
    /// evidence is separately required only for final-realization claims.
    pub fn canonical_review_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        encode::review::encode(self)
    }

    /// Independently framed rows for review-only conflict explanation.
    /// Package orchestration compares these bytes but never parses or
    /// reconstructs compiler semantic rows itself.
    pub fn canonical_rows(
        &self,
    ) -> Result<Vec<PackageReviewCanonicalRow>, PackageReviewEncodingError> {
        encode::rows::encode_rows(self)
    }
}

impl NonExecutableQuotientPackageReview {
    /// Independently framed, source-handle-free proof-only quotient rows. The
    /// rows remain review evidence and grant no checked or executable operation.
    pub fn canonical_rows(
        &self,
    ) -> Result<Vec<PackageReviewCanonicalRow>, PackageReviewEncodingError> {
        encode::quotients::encode_rows(self)
    }
}
