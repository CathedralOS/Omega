//! Highest changed-row risk used by review triage.

use crate::review::candidate::{PackageReviewEvidence, ReviewOnlyCanonicalRow};
use omega_package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
};
use std::cmp::Ordering;

/// Highest policy risk among changed canonical rows. A whole-review change
/// not represented by rows fails closed as opaque blocking here; the explicit
/// comparator reports the stronger structural error.
pub(crate) fn changed_review_risk(
    baseline: &impl PackageReviewEvidence,
    candidate: &impl PackageReviewEvidence,
) -> Option<PackageReviewCanonicalRowRisk> {
    let baseline_rows = baseline.canonical_rows();
    let candidate_rows = candidate.canonical_rows();
    let mut baseline_index = 0usize;
    let mut candidate_index = 0usize;
    let mut changed = None;
    while baseline_index < baseline_rows.len() || candidate_index < candidate_rows.len() {
        let baseline_row = baseline_rows.get(baseline_index);
        let candidate_row = candidate_rows.get(candidate_index);
        let ordering = match (baseline_row, candidate_row) {
            (Some(left), Some(right)) => row_coordinate(left).cmp(&row_coordinate(right)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => break,
        };
        let (baseline_row, candidate_row) = match ordering {
            Ordering::Less => {
                baseline_index += 1;
                (baseline_row, None)
            }
            Ordering::Greater => {
                candidate_index += 1;
                (None, candidate_row)
            }
            Ordering::Equal => {
                baseline_index += 1;
                candidate_index += 1;
                (baseline_row, candidate_row)
            }
        };
        if matches!((baseline_row, candidate_row), (Some(left), Some(right)) if left.canonical_bytes() == right.canonical_bytes())
        {
            continue;
        }
        let row_risk = merge_risk(
            baseline_row.map(ReviewOnlyCanonicalRow::risk),
            candidate_row.map(ReviewOnlyCanonicalRow::risk),
        );
        changed = Some(merge_risk(changed, Some(row_risk)));
    }
    if changed.is_none()
        && baseline.whole_review_commitment() != candidate.whole_review_commitment()
    {
        Some(PackageReviewCanonicalRowRisk::OpaqueBlocking)
    } else {
        changed
    }
}

fn row_coordinate(row: &ReviewOnlyCanonicalRow) -> (PackageReviewCanonicalRowKind, &[u8]) {
    (row.kind(), row.key_bytes())
}

fn merge_risk(
    baseline: Option<PackageReviewCanonicalRowRisk>,
    candidate: Option<PackageReviewCanonicalRowRisk>,
) -> PackageReviewCanonicalRowRisk {
    if baseline == Some(PackageReviewCanonicalRowRisk::OpaqueBlocking)
        || candidate == Some(PackageReviewCanonicalRowRisk::OpaqueBlocking)
    {
        PackageReviewCanonicalRowRisk::OpaqueBlocking
    } else if baseline == Some(PackageReviewCanonicalRowRisk::Blocking)
        || candidate == Some(PackageReviewCanonicalRowRisk::Blocking)
    {
        PackageReviewCanonicalRowRisk::Blocking
    } else {
        PackageReviewCanonicalRowRisk::AuditRecommended
    }
}
