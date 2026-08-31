//! Canonical package-review framing and semantic row encoding.
//!
//! `review` and `rows` assemble the public persistence products. `values`
//! contains their subordinate evidence-family encoders; it is deliberately a
//! child of this owner rather than a peer persistence domain.

pub(super) mod declarations;
pub(super) mod encoder;
pub(super) mod review;
pub(super) mod rows;
mod values;

pub(crate) const MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW\0";
pub const PACKAGE_REVIEW_ENCODING_VERSION: u16 = 104;
pub(crate) const ROW_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW\0";
pub const PACKAGE_REVIEW_ROW_ENCODING_VERSION: u16 = 62;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageReviewEncodingLimits {
    maximum_review_bytes: usize,
    maximum_rows: usize,
    maximum_row_key_bytes: usize,
    maximum_row_bytes: usize,
    maximum_total_row_bytes: usize,
}

impl PackageReviewEncodingLimits {
    pub const fn new(
        maximum_review_bytes: usize,
        maximum_rows: usize,
        maximum_row_key_bytes: usize,
        maximum_row_bytes: usize,
        maximum_total_row_bytes: usize,
    ) -> Self {
        Self {
            maximum_review_bytes,
            maximum_rows,
            maximum_row_key_bytes,
            maximum_row_bytes,
            maximum_total_row_bytes,
        }
    }
}

impl Default for PackageReviewEncodingLimits {
    fn default() -> Self {
        Self::new(
            16 * 1024 * 1024,
            65_536,
            1024 * 1024,
            4 * 1024 * 1024,
            16 * 1024 * 1024,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewEncodingError {
    message: &'static str,
}

impl PackageReviewEncodingError {
    pub(crate) const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl std::fmt::Display for PackageReviewEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for PackageReviewEncodingError {}
