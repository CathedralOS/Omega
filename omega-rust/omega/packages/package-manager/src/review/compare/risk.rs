//! Highest changed-row risk used by review triage.

use crate::review::candidate::PackageReviewEvidence;
use package_evidence::record::{PackagePolicyRow, PackagePolicyRowKind};
use std::cmp::Ordering;

/// Risk of one changed policy row, ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ChangedRowRisk {
    /// The change needs no decision but an audit is recommended.
    AuditRecommended,
    /// The change needs an explicit decision.
    Blocking,
}

/// Highest risk among the policy rows that differ between baseline and
/// candidate, or `None` when no row requiring a decision or recommending an
/// audit changed. A policy change that shows up in no row fails closed as
/// blocking.
pub(crate) fn changed_review_risk(
    baseline: &impl PackageReviewEvidence,
    candidate: &impl PackageReviewEvidence,
) -> Option<ChangedRowRisk> {
    let baseline_rows = baseline.policy_rows();
    let candidate_rows = candidate.policy_rows();
    let mut baseline_index = 0usize;
    let mut candidate_index = 0usize;
    let mut row_changed = false;
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
        row_changed = true;
        let row_risk = [baseline_row, candidate_row]
            .into_iter()
            .flatten()
            .filter_map(row_risk)
            .max();
        changed = changed.max(row_risk);
    }
    if !row_changed && baseline.policy_commitment() != candidate.policy_commitment() {
        Some(ChangedRowRisk::Blocking)
    } else {
        changed
    }
}

fn row_risk(row: &PackagePolicyRow) -> Option<ChangedRowRisk> {
    if row.update_requires_decision() {
        Some(ChangedRowRisk::Blocking)
    } else if row.audit_recommended_on_change() {
        Some(ChangedRowRisk::AuditRecommended)
    } else {
        None
    }
}

fn row_coordinate(row: &PackagePolicyRow) -> (PackagePolicyRowKind, &[u8]) {
    (row.kind(), row.key_bytes())
}
