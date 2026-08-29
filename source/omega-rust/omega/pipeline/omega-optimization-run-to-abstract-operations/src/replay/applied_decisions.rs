//! Exact Applied-manifest and baseline-policy custody.

use omega_optimization_core::OptimizationCandidateVerdict;
use omega_optimization_policy::BaselineDecisionOutcome;
use omega_psi_optimizer::{OptimizationRun, OrderedRuleRegistry};

use super::commits::ReplayedAppliedDecision;
use crate::OptimizedAbstractProjectionError;
use crate::error::{AppliedDecisionCustodyAxis, custody_error};

pub(super) fn validate(
    run: &OptimizationRun,
    registries: &[OrderedRuleRegistry],
    replayed: &[ReplayedAppliedDecision],
) -> Result<(), OptimizedAbstractProjectionError> {
    validate_schedule(run, registries)?;
    let manifested = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.decisions())
        .filter(|decision| decision.verdict() == OptimizationCandidateVerdict::Applied)
        .collect::<Vec<_>>();
    if manifested.len() != replayed.len() {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::AppliedRoster,
        ));
    }
    for (decision, evidence) in manifested.into_iter().zip(replayed) {
        let candidate = Some(evidence.candidate);
        let axis = if decision.input() != evidence.input {
            Some(AppliedDecisionCustodyAxis::Input)
        } else if decision.candidate() != evidence.candidate {
            Some(AppliedDecisionCustodyAxis::Candidate)
        } else if decision.rule() != evidence.rule {
            Some(AppliedDecisionCustodyAxis::Rule)
        } else if decision.validator() != Some(evidence.validator) {
            Some(AppliedDecisionCustodyAxis::Validator)
        } else if decision.consumed_analyses() != evidence.required_analyses {
            Some(AppliedDecisionCustodyAxis::ConsumedAnalyses)
        } else if decision.consumed_facts() != evidence.consumed_facts {
            Some(AppliedDecisionCustodyAxis::ConsumedFacts)
        } else {
            None
        };
        if let Some(axis) = axis {
            return Err(custody_error(candidate, axis));
        }
    }
    validate_baseline(run, replayed)
}

fn validate_schedule(
    run: &OptimizationRun,
    registries: &[OrderedRuleRegistry],
) -> Result<(), OptimizedAbstractProjectionError> {
    if run.pass_manifests().len() != registries.len() {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::ManifestRoster,
        ));
    }
    for (manifest, registry) in run.pass_manifests().iter().zip(registries) {
        let expected_rules = registry
            .contracts()
            .map(|contract| contract.identity())
            .collect::<Vec<_>>();
        let axis = if Some(manifest.pass()) != registry.pass() {
            Some(AppliedDecisionCustodyAxis::ManifestPass)
        } else if manifest.ordered_rules() != expected_rules {
            Some(AppliedDecisionCustodyAxis::ManifestRuleOrder)
        } else if manifest.ordered_rule_set() != registry.identity() {
            Some(AppliedDecisionCustodyAxis::ManifestRuleSet)
        } else {
            None
        };
        if let Some(axis) = axis {
            return Err(custody_error(None, axis));
        }
    }
    Ok(())
}

fn validate_baseline(
    run: &OptimizationRun,
    replayed: &[ReplayedAppliedDecision],
) -> Result<(), OptimizedAbstractProjectionError> {
    let chosen = run
        .decisions()
        .records
        .iter()
        .filter_map(|record| match record.outcome {
            BaselineDecisionOutcome::Choose(candidate) => Some((record, candidate)),
            BaselineDecisionOutcome::Skip(_) => None,
        })
        .collect::<Vec<_>>();
    if chosen.len() != replayed.len() {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::BaselineRoster,
        ));
    }
    for ((record, chosen_candidate), evidence) in chosen.into_iter().zip(replayed) {
        let candidate = Some(evidence.candidate);
        if record.input != evidence.input {
            return Err(custody_error(
                candidate,
                AppliedDecisionCustodyAxis::BaselineInput,
            ));
        }
        if chosen_candidate != evidence.candidate {
            return Err(custody_error(
                candidate,
                AppliedDecisionCustodyAxis::BaselineOutcome,
            ));
        }
        let matching = record
            .considered
            .iter()
            .filter(|summary| summary.candidate == evidence.candidate)
            .collect::<Vec<_>>();
        if matching.len() != 1 || matching[0].predicted_cost_delta != evidence.predicted_cost_delta
        {
            return Err(custody_error(
                candidate,
                AppliedDecisionCustodyAxis::PredictedCostDelta,
            ));
        }
    }
    Ok(())
}
