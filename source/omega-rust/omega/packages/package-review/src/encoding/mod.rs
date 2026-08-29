use crate::evidence::{CheckedPackageReviewProjection, PackageReviewCanonicalRow};

mod canonical;
mod recovery;
mod values;

pub use canonical::{
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
        canonical::review::encode(self)
    }

    /// Independently framed rows for review-only conflict explanation.
    /// Package orchestration compares these bytes but never parses or
    /// reconstructs compiler semantic rows itself.
    pub fn canonical_rows(
        &self,
    ) -> Result<Vec<PackageReviewCanonicalRow>, PackageReviewEncodingError> {
        canonical::rows::encode_rows(self)
    }
}
