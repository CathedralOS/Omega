//! Exact retained-declaration to pass-manifest custody.

use crate::{OptimizationRun, OrderedRuleRegistry};

use crate::OptimizedAbstractProjectionError;
use crate::publication::error::{AppliedDecisionCustodyAxis, custody_error};

pub(super) fn validate(
    run: &OptimizationRun,
    registries: &[OrderedRuleRegistry],
) -> Result<(), OptimizedAbstractProjectionError> {
    validate_schedule(run, registries)?;
    let manifested = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| {
            manifest
                .decisions()
                .iter()
                .map(move |decision| (manifest.pass(), decision))
        })
        .collect::<Vec<_>>();
    if manifested.len() != run.validated_candidates().len() {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::ValidatedRoster,
        ));
    }
    for ((pass, decision), retained) in manifested.into_iter().zip(run.validated_candidates()) {
        let declaration = retained.declaration();
        let candidate = Some(declaration.identity());
        let axis = if pass != retained.pass() {
            Some(AppliedDecisionCustodyAxis::ValidatedPass)
        } else if decision.input() != declaration.input() {
            Some(AppliedDecisionCustodyAxis::Input)
        } else if decision.candidate() != declaration.identity() {
            Some(AppliedDecisionCustodyAxis::Candidate)
        } else if decision.rule() != declaration.rule() {
            Some(AppliedDecisionCustodyAxis::Rule)
        } else if decision.validator() != Some(retained.validator()) {
            Some(AppliedDecisionCustodyAxis::Validator)
        } else if decision.consumed_analyses() != declaration.required_analyses() {
            Some(AppliedDecisionCustodyAxis::ConsumedAnalyses)
        } else if decision.consumed_facts() != declaration.consumed_facts() {
            Some(AppliedDecisionCustodyAxis::ConsumedFacts)
        } else {
            None
        };
        if let Some(axis) = axis {
            return Err(custody_error(candidate, axis));
        }
    }
    Ok(())
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
