//! Canonical byte production and bounded recovery for package-review evidence.
//!
//! Encoding owns stable framing only. It consumes inert evidence and does not
//! inspect compiler state or decide whether evidence should be admitted.

use crate::evidence::{CheckedPackageReviewProjection, PackageReviewCanonicalRow};

mod decode;
mod encode;
mod values;

pub use decode::{
    DecodedPackageReviewCanonicalRow, PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION,
    PackageReviewCanonicalRowRecoveryError, PackageReviewCanonicalRowRecoveryLimits,
    decode_package_review_canonical_row, decode_package_review_canonical_row_with_limits,
    encode_package_review_canonical_row, encode_package_review_canonical_row_with_limits,
};
pub(crate) use decode::{canonical_row_framing_for_ledger, canonical_row_subject_for_ledger};
pub use encode::{
    PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION,
    PackageReviewEncodingError,
};
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
