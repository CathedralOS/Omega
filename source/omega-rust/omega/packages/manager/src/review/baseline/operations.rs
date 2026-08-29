//! High-level comparison, triage, and source-review operations.

use super::ReviewOnlyBaselineCapsule;
use crate::resolution::{PackageKey, PackageSourceCustody, ResolvedPackageSourceClosure};
use crate::review::advisory::assemble_update_source_review_records;
use crate::review::comparison::compare_review_only_capability_records;
use crate::review::triage::triage_review_update_records;
use crate::review::{
    CompilerIssuedPackageReviewSet, CompilerReviewTriage, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyCapabilityConflictSet,
};
use std::collections::BTreeSet;

pub fn compare_review_only_capabilities_from_baseline(
    baseline: &ReviewOnlyBaselineCapsule,
    candidate: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: ReviewOnlyCapabilityConflictLimits,
) -> Result<ReviewOnlyCapabilityConflictSet, ReviewOnlyCapabilityConflictError> {
    compare_review_only_capability_records(
        baseline.packages(),
        candidate,
        candidate_sources,
        limits,
    )
}

pub fn triage_review_update_from_baseline(
    baseline: &ReviewOnlyBaselineCapsule,
    candidate: &CompilerIssuedPackageReviewSet,
    unavailable_baseline_sources: &BTreeSet<PackageKey>,
) -> CompilerReviewTriage {
    triage_review_update_records(baseline.packages(), candidate, unavailable_baseline_sources)
}

pub fn assemble_update_source_review_from_baseline(
    baseline: &ReviewOnlyBaselineCapsule,
    candidate: &CompilerIssuedPackageReviewSet,
    recovered_baseline_sources: &[PackageSourceCustody],
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: PackageSourceReviewLimits,
) -> Result<PackageSourceReviewInput, PackageSourceReviewError> {
    assemble_update_source_review_records(
        baseline.packages(),
        candidate,
        recovered_baseline_sources,
        candidate_sources,
        limits,
    )
}
