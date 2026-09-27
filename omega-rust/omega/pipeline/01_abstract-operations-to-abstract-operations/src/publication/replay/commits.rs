//! Candidate-contract rebind and independent commit application.

use std::collections::BTreeMap;

use crate::{OptimizationRun, PsiPassStep, VerifiedPsiOptimizationSession};
use optimization_core::{
    AnalysisSet, OptimizationCandidateIdentity, OptimizationFactReference,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};
use optimization_unit::PsiCandidateDeclaration;

use crate::OptimizedAbstractProjectionError;
use crate::publication::error::{AppliedDecisionCustodyAxis, custody_error};

pub(super) struct ReplayedAppliedDecision {
    pub(super) input: OptimizationUnitIdentity,
    pub(super) candidate: OptimizationCandidateIdentity,
    pub(super) rule: OptimizationRuleIdentity,
    pub(super) validator: OptimizationValidatorIdentity,
    pub(super) required_analyses: AnalysisSet,
    pub(super) consumed_facts: Vec<OptimizationFactReference>,
    pub(super) predicted_cost_delta: i64,
}

pub(super) struct ReplayedCommits {
    pub(super) applied: Vec<ReplayedAppliedDecision>,
    pub(super) revisions:
        BTreeMap<OptimizationUnitIdentity, optimization_unit::PsiOptimizationUnit>,
}

pub(super) fn replay(
    run: &OptimizationRun,
    steps: &[PsiPassStep],
) -> Result<ReplayedCommits, OptimizedAbstractProjectionError> {
    let initial = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        run.session().input().clone(),
        run.session().unit().fuel_schedule,
    )
    .map_err(|_| OptimizedAbstractProjectionError::InitialUnitProjection)?;
    let mut unit = initial.unit().clone();
    let mut revisions = BTreeMap::from([(unit.identity, unit.clone())]);
    let mut applied = Vec::with_capacity(run.commits().len());
    for commit in run.commits() {
        let declaration = commit.declaration();
        if declaration.input() != unit.identity
            || declaration.identity() != commit.candidate
            || declaration.rule() != commit.rule
        {
            return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
        }
        let contract = contract_for(steps, commit.rule).ok_or_else(|| {
            custody_error(
                Some(commit.candidate),
                AppliedDecisionCustodyAxis::RuleContract,
            )
        })?;
        bind_contract(commit.candidate, declaration, contract)?;
        if commit.predicted_cost_delta != declaration.predicted_cost_delta() {
            return Err(custody_error(
                Some(commit.candidate),
                AppliedDecisionCustodyAxis::CommitPredictedCostDelta,
            ));
        }
        let next = match declaration {
            PsiCandidateDeclaration::Rewrite(candidate) => {
                if declaration.provenance() != commit.provenance {
                    return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
                }
                let accepted =
                    optimization_unit_semantics::validate_psi_rewrite_candidate(&unit, candidate)
                        .map_err(OptimizedAbstractProjectionError::CandidateReplay)?;
                if accepted.candidate() != commit.candidate
                    || accepted.validator() != commit.validator
                    || accepted.unit().identity != commit.output
                    || commit.input != unit.identity
                {
                    return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
                }
                accepted.into_unit()
            }
            PsiCandidateDeclaration::LoopInvariantScalarMotion(candidate) => {
                // The relocation boundary is session-level: replay rebuilds
                // the verified session over the current revision, validates
                // the declared roster independently, and applies atomically.
                let session = VerifiedPsiOptimizationSession::from_transformed(
                    run.session().input().clone(),
                    unit.clone(),
                )
                .map_err(OptimizedAbstractProjectionError::CandidateReplay)?;
                let accepted =
                    crate::ranked_rewrites::loop_invariant_scalar_motion::validate_loop_invariant_scalar_motion(
                        &session, candidate,
                    )
                    .map_err(OptimizedAbstractProjectionError::LoopInvariantScalarMotionReplay)?;
                if accepted.candidate().identity() != commit.candidate
                    || accepted.validator() != commit.validator
                    || accepted.provenance() != commit.provenance
                    || commit.input != unit.identity
                {
                    return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
                }
                let applied_relocation =
                    crate::ranked_rewrites::loop_invariant_scalar_motion::apply_loop_invariant_scalar_motion(
                        session, accepted,
                    )
                    .map_err(OptimizedAbstractProjectionError::LoopInvariantScalarMotionReplay)?;
                let (applied_session, _, _) = applied_relocation.into_parts();
                let (_, transformed) = applied_session.into_parts();
                if transformed.identity != commit.output {
                    return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
                }
                transformed
            }
        };
        applied.push(ReplayedAppliedDecision {
            input: commit.input,
            candidate: commit.candidate,
            rule: commit.rule,
            validator: commit.validator,
            required_analyses: contract.required_analyses(),
            consumed_facts: declaration.consumed_facts(),
            predicted_cost_delta: declaration.predicted_cost_delta(),
        });
        unit = next;
        if revisions.insert(unit.identity, unit.clone()).is_some() {
            return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
        }
    }
    if unit != *run.session().unit() {
        return Err(OptimizedAbstractProjectionError::FinalUnitReplayMismatch);
    }
    Ok(ReplayedCommits { applied, revisions })
}

pub(super) fn contract_for(
    steps: &[PsiPassStep],
    rule: OptimizationRuleIdentity,
) -> Option<OptimizationRuleContract> {
    steps
        .iter()
        .flat_map(PsiPassStep::contracts)
        .find(|contract| contract.identity() == rule)
}

pub(super) fn bind_contract(
    candidate: OptimizationCandidateIdentity,
    declaration: &PsiCandidateDeclaration,
    contract: OptimizationRuleContract,
) -> Result<(), OptimizedAbstractProjectionError> {
    let axis = if declaration.required_analyses() != contract.required_analyses() {
        Some(AppliedDecisionCustodyAxis::RequiredAnalyses)
    } else if declaration.invalidated_analyses() != contract.invalidated_analyses() {
        Some(AppliedDecisionCustodyAxis::InvalidatedAnalyses)
    } else if declaration.safety_class() != contract.safety_class() {
        Some(AppliedDecisionCustodyAxis::SafetyClass)
    } else {
        None
    };
    match axis {
        Some(axis) => Err(custody_error(Some(candidate), axis)),
        None => Ok(()),
    }
}
