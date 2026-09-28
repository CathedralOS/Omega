//! Exact retained-declaration to pass-manifest custody.

use crate::{OptimizationRun, PsiPassStep};

use crate::OptimizedAbstractProjectionError;
use crate::publication::error::{AppliedDecisionCustodyAxis, custody_error};

pub(super) fn validate(
    run: &OptimizationRun,
    steps: &[PsiPassStep],
) -> Result<(), OptimizedAbstractProjectionError> {
    validate_schedule(run, steps)?;
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
        } else if decision.consumed_facts() != declaration.consumed_facts().as_slice() {
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
    steps: &[PsiPassStep],
) -> Result<(), OptimizedAbstractProjectionError> {
    if run.pass_manifests().len() != steps.len() {
        return Err(custody_error(
            None,
            AppliedDecisionCustodyAxis::ManifestRoster,
        ));
    }
    for (manifest, step) in run.pass_manifests().iter().zip(steps) {
        let expected_rules = step.ordered_rules();
        let axis = if Some(manifest.pass()) != step.pass() {
            Some(AppliedDecisionCustodyAxis::ManifestPass)
        } else if manifest.ordered_rules() != expected_rules {
            Some(AppliedDecisionCustodyAxis::ManifestRuleOrder)
        } else if manifest.ordered_rule_set() != step.rule_set_identity() {
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
