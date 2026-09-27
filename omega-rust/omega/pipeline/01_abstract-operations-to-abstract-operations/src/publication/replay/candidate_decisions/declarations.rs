//! Independent reconstruction of every retained validated candidate.

use crate::{OptimizationRun, PsiPassStep, VerifiedPsiOptimizationSession};
use optimization_core::OptimizationCandidateVerdict;
use optimization_unit::PsiCandidateDeclaration;

use super::super::commits::{ReplayedCommits, bind_contract, contract_for};
use crate::OptimizedAbstractProjectionError;
use crate::publication::error::{AppliedDecisionCustodyAxis, custody_error};

pub(super) fn validate(
    run: &OptimizationRun,
    steps: &[PsiPassStep],
    replayed: &ReplayedCommits,
) -> Result<(), OptimizedAbstractProjectionError> {
    let decisions = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.decisions())
        .collect::<Vec<_>>();
    let applied_count = decisions
        .iter()
        .filter(|decision| decision.verdict() == OptimizationCandidateVerdict::Applied)
        .count();
    if applied_count != replayed.applied.len() || applied_count != run.commits().len() {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::AppliedRoster,
        ));
    }
    for (retained, decision) in run.validated_candidates().iter().zip(decisions) {
        let declaration = retained.declaration();
        let candidate = declaration.identity();
        let contract = contract_for(steps, declaration.rule()).ok_or_else(|| {
            custody_error(Some(candidate), AppliedDecisionCustodyAxis::RuleContract)
        })?;
        if contract.pass() != retained.pass() {
            return Err(custody_error(
                Some(candidate),
                AppliedDecisionCustodyAxis::ValidatedPass,
            ));
        }
        bind_contract(candidate, declaration, contract)?;
        let revision = replayed
            .revisions
            .get(&declaration.input())
            .ok_or_else(|| {
                custody_error(Some(candidate), AppliedDecisionCustodyAxis::InputRevision)
            })?;
        let validator = match declaration {
            PsiCandidateDeclaration::Rewrite(candidate) => {
                let accepted = optimization_unit_semantics::validate_psi_rewrite_candidate(
                    revision, candidate,
                )
                .map_err(OptimizedAbstractProjectionError::CandidateReplay)?;
                if accepted.candidate() != declaration.identity() {
                    return Err(custody_error(
                        Some(declaration.identity()),
                        AppliedDecisionCustodyAxis::Validator,
                    ));
                }
                accepted.validator()
            }
            PsiCandidateDeclaration::LoopInvariantScalarMotion(candidate) => {
                let session = VerifiedPsiOptimizationSession::from_transformed(
                    run.session().input().clone(),
                    revision.clone(),
                )
                .map_err(OptimizedAbstractProjectionError::CandidateReplay)?;
                let accepted =
                    crate::ranked_rewrites::loop_invariant_scalar_motion::validate_loop_invariant_scalar_motion(
                        &session, candidate,
                    )
                    .map_err(OptimizedAbstractProjectionError::LoopInvariantScalarMotionReplay)?;
                if accepted.candidate().identity() != declaration.identity() {
                    return Err(custody_error(
                        Some(declaration.identity()),
                        AppliedDecisionCustodyAxis::Validator,
                    ));
                }
                accepted.validator()
            }
        };
        if validator != retained.validator() {
            return Err(custody_error(
                Some(candidate),
                AppliedDecisionCustodyAxis::Validator,
            ));
        }
        let matching_commits = run
            .commits()
            .iter()
            .filter(|commit| commit.candidate == candidate)
            .collect::<Vec<_>>();
        let matching_replayed = replayed
            .applied
            .iter()
            .filter(|applied| applied.candidate == candidate)
            .collect::<Vec<_>>();
        match decision.verdict() {
            OptimizationCandidateVerdict::Applied => {
                let ([commit], [applied]) =
                    (matching_commits.as_slice(), matching_replayed.as_slice())
                else {
                    return Err(custody_error(
                        Some(candidate),
                        AppliedDecisionCustodyAxis::CommitRoster,
                    ));
                };
                if commit.declaration() != declaration
                    || commit.validator != retained.validator()
                    || applied.input != declaration.input()
                    || applied.rule != declaration.rule()
                    || applied.validator != retained.validator()
                    || applied.required_analyses != declaration.required_analyses()
                    || applied.consumed_facts != declaration.consumed_facts()
                    || applied.predicted_cost_delta != declaration.predicted_cost_delta()
                {
                    return Err(custody_error(
                        Some(candidate),
                        AppliedDecisionCustodyAxis::CommitDeclaration,
                    ));
                }
            }
            OptimizationCandidateVerdict::Skipped(_) => {
                if !matching_commits.is_empty() || !matching_replayed.is_empty() {
                    return Err(custody_error(
                        Some(candidate),
                        AppliedDecisionCustodyAxis::CommitRoster,
                    ));
                }
            }
            OptimizationCandidateVerdict::Rejected(_) => {
                return Err(custody_error(
                    Some(candidate),
                    AppliedDecisionCustodyAxis::Verdict,
                ));
            }
        }
    }
    Ok(())
}
