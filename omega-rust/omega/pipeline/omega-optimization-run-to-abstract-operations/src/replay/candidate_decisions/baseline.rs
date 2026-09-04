//! Baseline policy batches, costs, and verdict mapping for all candidates.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations_optimizer::OptimizationRun;
use omega_optimization_core::OptimizationCandidateVerdict;
use omega_optimization_policy::{BaselineDecisionLog, BaselineDecisionOutcome};

use crate::OptimizedAbstractProjectionError;
use crate::error::{AppliedDecisionCustodyAxis, custody_error};

pub(super) fn validate(run: &OptimizationRun) -> Result<(), OptimizedAbstractProjectionError> {
    if BaselineDecisionLog::decode(&run.decisions().encode()).ok() != Some(run.decisions().clone())
    {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::BaselineRoster,
        ));
    }
    let decisions = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.decisions())
        .map(|decision| (decision.candidate(), decision))
        .collect::<BTreeMap<_, _>>();
    if decisions.len() != run.validated_candidates().len() {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::ValidatedRoster,
        ));
    }
    let declarations = run
        .validated_candidates()
        .iter()
        .map(|retained| (retained.declaration().identity(), retained.declaration()))
        .collect::<BTreeMap<_, _>>();
    if declarations.len() != run.validated_candidates().len() {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::ValidatedRoster,
        ));
    }
    let mut seen = BTreeSet::new();
    for record in &run.decisions().records {
        let mut rule = None;
        for considered in &record.considered {
            if !seen.insert(considered.candidate) {
                return Err(custody_error(
                    Some(considered.candidate),
                    AppliedDecisionCustodyAxis::BaselineRoster,
                ));
            }
            let Some(declaration) = declarations.get(&considered.candidate) else {
                return Err(custody_error(
                    Some(considered.candidate),
                    AppliedDecisionCustodyAxis::BaselineRoster,
                ));
            };
            let Some(decision) = decisions.get(&considered.candidate) else {
                return Err(custody_error(
                    Some(considered.candidate),
                    AppliedDecisionCustodyAxis::BaselineRoster,
                ));
            };
            if record.input != declaration.input() {
                return Err(custody_error(
                    Some(considered.candidate),
                    AppliedDecisionCustodyAxis::BaselineInput,
                ));
            }
            if considered.predicted_cost_delta != declaration.predicted_cost_delta() {
                return Err(custody_error(
                    Some(considered.candidate),
                    AppliedDecisionCustodyAxis::PredictedCostDelta,
                ));
            }
            if rule.is_some_and(|expected| expected != declaration.rule()) {
                return Err(custody_error(
                    Some(considered.candidate),
                    AppliedDecisionCustodyAxis::BaselineRule,
                ));
            }
            rule = Some(declaration.rule());
            let expected_verdict = match record.outcome {
                BaselineDecisionOutcome::Choose(chosen) if chosen == considered.candidate => {
                    OptimizationCandidateVerdict::Applied
                }
                BaselineDecisionOutcome::Choose(_) => OptimizationCandidateVerdict::Skipped(
                    omega_optimization_core::OptimizationReasonCode::Superseded,
                ),
                BaselineDecisionOutcome::Skip(reason) => {
                    OptimizationCandidateVerdict::Skipped(reason)
                }
            };
            if decision.verdict() != expected_verdict {
                return Err(custody_error(
                    Some(considered.candidate),
                    AppliedDecisionCustodyAxis::Verdict,
                ));
            }
        }
    }
    if seen.len() != run.validated_candidates().len() {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::BaselineRoster,
        ));
    }
    Ok(())
}
