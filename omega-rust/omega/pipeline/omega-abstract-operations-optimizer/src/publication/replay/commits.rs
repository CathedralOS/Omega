//! Candidate-contract rebind and independent commit application.

use std::collections::BTreeMap;

use crate::{OptimizationRun, OrderedRuleRegistry};
use omega_optimization_core::{
    AnalysisSet, OptimizationCandidateIdentity, OptimizationFactReference,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity,
};

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
        BTreeMap<OptimizationUnitIdentity, omega_optimization_unit::PsiOptimizationUnit>,
}

pub(super) fn replay(
    run: &OptimizationRun,
    registries: &[OrderedRuleRegistry],
) -> Result<ReplayedCommits, OptimizedAbstractProjectionError> {
    let initial = omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
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
            || declaration.provenance() != commit.provenance
        {
            return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
        }
        let contract = contract_for(registries, commit.rule).ok_or_else(|| {
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
        let accepted =
            omega_optimization_validation::validate_psi_rewrite_candidate(&unit, declaration)
                .map_err(OptimizedAbstractProjectionError::CandidateReplay)?;
        if accepted.candidate() != commit.candidate
            || accepted.validator() != commit.validator
            || accepted.unit().identity != commit.output
            || commit.input != unit.identity
        {
            return Err(OptimizedAbstractProjectionError::CommitReplayMismatch);
        }
        applied.push(ReplayedAppliedDecision {
            input: commit.input,
            candidate: commit.candidate,
            rule: commit.rule,
            validator: commit.validator,
            required_analyses: contract.required_analyses(),
            consumed_facts: declaration.consumed_facts(),
            predicted_cost_delta: declaration.predicted_cost_delta(),
        });
        unit = accepted.into_unit();
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
    registries: &[OrderedRuleRegistry],
    rule: OptimizationRuleIdentity,
) -> Option<OptimizationRuleContract> {
    registries
        .iter()
        .flat_map(|registry| registry.contracts())
        .find(|contract| contract.identity() == rule)
}

pub(super) fn bind_contract(
    candidate: OptimizationCandidateIdentity,
    declaration: &omega_optimization_unit::PsiRewriteCandidate,
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
