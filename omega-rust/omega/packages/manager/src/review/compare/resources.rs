//! Bounded accounting for candidate-controlled comparison inputs.

use super::model::{ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictLimits};
use crate::review::candidate::PackageReviewEvidence;
use omega_package_evidence::record::PackageReviewCanonicalRowSource;

#[derive(Default)]
pub(super) struct ComparisonInputBudget {
    packages: usize,
    rows: usize,
    row_key_bytes: usize,
    encoded_row_bytes: usize,
    source_locations: usize,
    source_location_path_bytes: usize,
}

pub(super) fn account_review_resources(
    reviews: &[impl PackageReviewEvidence],
    limits: ReviewOnlyCapabilityConflictLimits,
    budget: &mut ComparisonInputBudget,
) -> Result<(), ReviewOnlyCapabilityConflictError> {
    budget.packages = budget.packages.saturating_add(reviews.len());
    if budget.packages > limits.maximum_packages {
        return Err(ReviewOnlyCapabilityConflictError::TooManyPackages {
            maximum: limits.maximum_packages,
        });
    }
    for review in reviews {
        budget.rows = budget.rows.saturating_add(review.canonical_rows().len());
        budget.row_key_bytes = review
            .canonical_rows()
            .iter()
            .fold(budget.row_key_bytes, |bytes, row| {
                bytes.saturating_add(row.key_bytes().len())
            });
        budget.encoded_row_bytes = review
            .canonical_rows()
            .iter()
            .fold(budget.encoded_row_bytes, |bytes, row| {
                bytes.saturating_add(row.canonical_bytes().len())
            });
        for row in review.canonical_rows() {
            let (locations, path_bytes) = source_metrics(row.source());
            budget.source_locations = budget.source_locations.saturating_add(locations);
            budget.source_location_path_bytes =
                budget.source_location_path_bytes.saturating_add(path_bytes);
        }
        if budget.rows > limits.maximum_rows {
            return Err(ReviewOnlyCapabilityConflictError::TooManyRows {
                maximum: limits.maximum_rows,
            });
        }
        if budget.row_key_bytes > limits.maximum_row_key_bytes {
            return Err(ReviewOnlyCapabilityConflictError::RowKeyBytesExceeded {
                maximum_bytes: limits.maximum_row_key_bytes,
            });
        }
        if budget.encoded_row_bytes > limits.maximum_encoded_row_bytes {
            return Err(ReviewOnlyCapabilityConflictError::EncodedRowBytesExceeded {
                maximum_bytes: limits.maximum_encoded_row_bytes,
            });
        }
        if budget.source_locations > limits.maximum_source_locations {
            return Err(ReviewOnlyCapabilityConflictError::TooManySourceLocations {
                maximum: limits.maximum_source_locations,
            });
        }
        if budget.source_location_path_bytes > limits.maximum_source_location_path_bytes {
            return Err(
                ReviewOnlyCapabilityConflictError::SourceLocationPathBytesExceeded {
                    maximum_bytes: limits.maximum_source_location_path_bytes,
                },
            );
        }
    }
    Ok(())
}

fn source_metrics(source: &PackageReviewCanonicalRowSource) -> (usize, usize) {
    let locations = source.authored_locations().unwrap_or_default();
    (
        locations.len(),
        locations.iter().fold(0usize, |bytes, location| {
            bytes.saturating_add(location.relative_path().len())
        }),
    )
}
