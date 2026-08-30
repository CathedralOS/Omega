//! High-level comparison, triage, and source-review operations.

use super::ReviewOnlyBaselineCapsule;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::manifest::PackageKey;
use crate::review::audit::{
    apply_root_role_change, assemble_update_source_review_records, triage_review_update_records,
};
use crate::review::compare::{
    compare_review_only_capability_records, compare_review_only_root_role_graphs,
};
use crate::review::{
    CompilerIssuedPackageReviewSet, CompilerReviewTriage, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, ReviewOnlyCapabilityConflictError,
    ReviewOnlyCapabilityConflictLimits, ReviewOnlyCapabilityConflictSet, ReviewOnlyRootRoleChange,
    ReviewOnlyRootRoleComparisonError,
};
use crate::resolution::source::PackageSourceCustody;
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

pub fn compare_review_only_root_role_from_baseline(
    baseline: &ReviewOnlyBaselineCapsule,
    candidate_sources: &ResolvedPackageSourceClosure,
) -> Result<Option<ReviewOnlyRootRoleChange>, ReviewOnlyRootRoleComparisonError> {
    compare_review_only_root_role_graphs(baseline.graph(), candidate_sources.graph())
}

pub fn triage_review_update_from_baseline(
    baseline: &ReviewOnlyBaselineCapsule,
    candidate: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ResolvedPackageSourceClosure,
    unavailable_baseline_sources: &BTreeSet<PackageKey>,
) -> CompilerReviewTriage {
    let mut triage =
        triage_review_update_records(baseline.packages(), candidate, unavailable_baseline_sources);
    if baseline.graph().root() == candidate_sources.graph().root() {
        if let Some(change) =
            compare_review_only_root_role_graphs(baseline.graph(), candidate_sources.graph())
                .expect("equal root identities are valid for role comparison")
        {
            apply_root_role_change(&mut triage, &change);
        }
    }
    triage
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
