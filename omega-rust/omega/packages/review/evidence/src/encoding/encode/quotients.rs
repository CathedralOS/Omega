use super::rows::encode_subject_row;
use super::values::quotients::{
    encode_quotient_correspondence, encode_quotient_correspondence_key,
};
use super::{PackageReviewEncodingError, PackageReviewEncodingLimits};
use crate::record::{
    NonExecutableQuotientPackageReview, PackageReviewCanonicalRow, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk,
};

pub(crate) fn encode_rows(
    review: &NonExecutableQuotientPackageReview,
) -> Result<Vec<PackageReviewCanonicalRow>, PackageReviewEncodingError> {
    let limits = PackageReviewEncodingLimits::default();
    if review.correspondences.len() != review.row_sources.len() {
        return Err(PackageReviewEncodingError::new(
            "non-executable quotient package-review row lost its source disposition",
        ));
    }
    if review.correspondences.len() > limits.maximum_rows {
        return Err(PackageReviewEncodingError::new(
            "non-executable quotient package review exceeds the row-count ceiling",
        ));
    }
    let mut rows = Vec::with_capacity(review.correspondences.len());
    for (certificate, source) in review.correspondences.iter().zip(&review.row_sources) {
        rows.push(encode_subject_row(
            review.package,
            review.target,
            limits,
            PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence,
            PackageReviewCanonicalRowRisk::Blocking,
            source.clone(),
            |encoder| encode_quotient_correspondence_key(encoder, certificate),
            |encoder| encode_quotient_correspondence(encoder, certificate),
        )?);
    }
    rows.sort_by(|left, right| left.key_bytes.cmp(&right.key_bytes));
    if rows
        .windows(2)
        .any(|pair| pair[0].key_bytes == pair[1].key_bytes)
    {
        return Err(PackageReviewEncodingError::new(
            "non-executable quotient package review contains duplicate canonical row keys",
        ));
    }
    Ok(rows)
}
