mod patches;
mod validation;

use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::review::audit::triage_review_update_records;
use crate::review::candidate::PackageReviewEvidence;
use crate::review::candidate::validation::{
    validate_review_only_closure, validate_review_only_records,
};
use crate::review::{CompilerIssuedPackageReviewSet, triage_initial_install};
use crate::resolution::source::PackageSourceCustody;

use super::error::{PackageSourceReviewCustodyRole, PackageSourceReviewError};
use super::input::{PackageSourceReviewInput, PackageSourceReviewLimits};
use patches::assemble_source_patches;
use validation::{
    map_closure_validation_error, map_set_validation_error, validate_partial_custody,
};

/// Assemble initial-install review input. Pure candidates remain represented
/// in deterministic triage but receive source packets only when compiler facts
/// already recommend audit.
pub fn assemble_initial_source_review(
    candidate_reviews: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: PackageSourceReviewLimits,
) -> Result<PackageSourceReviewInput, PackageSourceReviewError> {
    validate_review_only_closure(candidate_sources, candidate_reviews).map_err(|error| {
        map_closure_validation_error(PackageSourceReviewCustodyRole::Candidate, error)
    })?;
    let triage = triage_initial_install(candidate_reviews);
    assemble_source_patches(triage, &Default::default(), candidate_sources, limits, true)
}

/// Assemble update review input from compiler-issued baseline/candidate rows,
/// every recovered old custody, and the complete candidate closure.
///
/// Missing old custody is derived here and cannot erase the accepted compiler
/// baseline. It selects standalone candidate review for that exact package.
pub fn assemble_update_source_review(
    baseline_reviews: &CompilerIssuedPackageReviewSet,
    candidate_reviews: &CompilerIssuedPackageReviewSet,
    recovered_baseline_sources: &[PackageSourceCustody],
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: PackageSourceReviewLimits,
) -> Result<PackageSourceReviewInput, PackageSourceReviewError> {
    assemble_update_source_review_records(
        baseline_reviews.reviews(),
        candidate_reviews,
        recovered_baseline_sources,
        candidate_sources,
        limits,
    )
}

pub(crate) fn assemble_update_source_review_records<B: PackageReviewEvidence>(
    baseline_reviews: &[B],
    candidate_reviews: &CompilerIssuedPackageReviewSet,
    recovered_baseline_sources: &[PackageSourceCustody],
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: PackageSourceReviewLimits,
) -> Result<PackageSourceReviewInput, PackageSourceReviewError> {
    validate_review_only_closure(candidate_sources, candidate_reviews).map_err(|error| {
        map_closure_validation_error(PackageSourceReviewCustodyRole::Candidate, error)
    })?;
    validate_review_only_records(baseline_reviews).map_err(|error| {
        map_set_validation_error(PackageSourceReviewCustodyRole::Baseline, error)
    })?;
    let baseline_sources = validate_partial_custody(
        baseline_reviews,
        recovered_baseline_sources,
        PackageSourceReviewCustodyRole::Baseline,
    )?;
    let unavailable = baseline_reviews
        .iter()
        .filter(|review| !baseline_sources.contains_key(review.key()))
        .map(|review| review.key().clone())
        .collect();
    let triage = triage_review_update_records(baseline_reviews, candidate_reviews, &unavailable);
    assemble_source_patches(triage, &baseline_sources, candidate_sources, limits, false)
}
