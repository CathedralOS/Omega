//! Join already-emitted comparisons to this target's selected provider evidence.
//!
//! Retained Terminal products and direct native compilation use this same
//! settlement. Stage 05 recorded each occurrence's authored comparison meaning
//! from the checker's classification, so settlement reads it rather than
//! classifying the typed trees again; it joins the occurrence to this target's
//! selected plan and provider provenance. These sidecars neither replace
//! canonical-artifact admission nor lower source into native operations.

use boundary_applications::{
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;

/// Rejoin emitted IEEE comparisons to exact authored uses, selected plans and executions.
/// Returns source-free backend records after replaying their Terminal roster.
/// This association grants no native execution or artifact-publication authority.
pub fn settle_ieee_float_comparison_occurrences(
    checked: &CheckedTrees,
    module: &terminal_psi::TerminalModule,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[provider_planning::SelectedProviderReviewProvenance],
    occurrences: &[lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence],
) -> Result<Vec<TerminalIeeeFloatComparisonOccurrenceProposal>, Vec<Diagnostic>> {
    let proposals = occurrences
        .iter()
        .map(|occurrence| {
            settle_ieee_float_occurrence(checked, selected, provenance, occurrence)
                .map_err(|error| vec![error])
        })
        .collect::<Result<Vec<_>, _>>()?;
    TerminalIeeeFloatComparisonOccurrenceProposal::validate_roster(
        module,
        selected.plans(),
        &proposals,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    Ok(proposals)
}

fn settle_ieee_float_occurrence(
    checked: &CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[provider_planning::SelectedProviderReviewProvenance],
    occurrence: &lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence,
) -> Result<TerminalIeeeFloatComparisonOccurrenceProposal, Diagnostic> {
    let fail = |reason: &str| {
        Diagnostic::error(format!(
            "Terminal IEEE comparison {}: {reason}",
            occurrence.terminal_operation.get()
        ))
    };
    let operator_use = checked.facts.operators.uses.get(occurrence.operator_use);
    // This target's selected plan joins the occurrence by requirement
    // identity; the Terminal occurrence carries no selection.
    let Some((provider_plan_index, plan)) = provider_planning::selected_use_plan(
        checked,
        source::DependencyScope::Product,
        selected.plans(),
        occurrence.requirement_operator,
        operator_use.origin,
    ) else {
        return Err(fail("requires exactly one exact selected provider plan"));
    };
    let Some(retained) = provenance.get(provider_plan_index) else {
        return Err(fail("selected provider provenance is missing"));
    };
    if retained.plan != *plan
        || plan.rows.len() != 1
        || retained.provider.row_requirements.as_slice() != [occurrence.requirement_operator]
    {
        return Err(fail(
            "selected provider provenance does not rejoin the exact requirement",
        ));
    }
    let proposal = TerminalIeeeFloatComparisonOccurrenceProposal {
        terminal_machine: occurrence.terminal_machine,
        terminal_operation: occurrence.terminal_operation,
        provider_plan_index,
        provider_plan_commitment: plan.identity_digest(),
        comparison: occurrence.comparison,
        format: occurrence.format,
    };
    let execution = proposal.execution_identity();
    if retained.row_compiler_intrinsic_executions.as_slice() != [Some(execution)] {
        return Err(fail(
            "selected execution is not the exact IEEE comparison and format",
        ));
    }
    Ok(proposal)
}

/// Rejoin emitted integer comparisons to exact authored meanings and selected executions.
/// The emission triple preserves operand order and negation; unrecognized meanings reject.
/// This association grants no native execution or artifact-publication authority.
pub fn settle_integer_comparison_occurrences(
    checked: &CheckedTrees,
    module: &terminal_psi::TerminalModule,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[provider_planning::SelectedProviderReviewProvenance],
    occurrences: &[lowered_psi::LoweredSelectedIntegerComparisonOccurrence],
) -> Result<Vec<TerminalIntegerComparisonOccurrenceProposal>, Vec<Diagnostic>> {
    let proposals = occurrences
        .iter()
        .map(|occurrence| {
            settle_integer_occurrence(checked, selected, provenance, occurrence)
                .map_err(|error| vec![error])
        })
        .collect::<Result<Vec<_>, _>>()?;
    TerminalIntegerComparisonOccurrenceProposal::validate_roster(
        module,
        selected.plans(),
        &proposals,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    Ok(proposals)
}

fn settle_integer_occurrence(
    checked: &CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[provider_planning::SelectedProviderReviewProvenance],
    occurrence: &lowered_psi::LoweredSelectedIntegerComparisonOccurrence,
) -> Result<TerminalIntegerComparisonOccurrenceProposal, Diagnostic> {
    let fail = |reason: &str| {
        Diagnostic::error(format!(
            "Terminal integer comparison {}: {reason}",
            occurrence.terminal_operation.get()
        ))
    };
    let operator_use = checked.facts.operators.uses.get(occurrence.operator_use);
    // This target's selected plan joins the occurrence by requirement
    // identity; the Terminal occurrence carries no selection.
    let Some((provider_plan_index, plan)) = provider_planning::selected_use_plan(
        checked,
        source::DependencyScope::Product,
        selected.plans(),
        occurrence.requirement_operator,
        operator_use.origin,
    ) else {
        return Err(fail("requires exactly one exact selected provider plan"));
    };
    let Some(retained) = provenance.get(provider_plan_index) else {
        return Err(fail("selected provider provenance is missing"));
    };
    if retained.plan != *plan
        || plan.rows.len() != 1
        || retained.provider.row_requirements.as_slice() != [occurrence.requirement_operator]
    {
        return Err(fail(
            "selected provider provenance does not rejoin the exact requirement",
        ));
    }
    let proposal = TerminalIntegerComparisonOccurrenceProposal {
        terminal_machine: occurrence.terminal_machine,
        terminal_operation: occurrence.terminal_operation,
        provider_plan_index,
        provider_plan_commitment: plan.identity_digest(),
        comparison: occurrence.comparison,
        operand_order: occurrence.operand_order,
        negated: occurrence.negated,
        integer_type: occurrence.integer_type,
    };
    let Some(execution) = proposal.execution_identity() else {
        return Err(fail(
            "recorded emission has no admitted authored comparison meaning",
        ));
    };
    if retained.row_compiler_intrinsic_executions.as_slice() != [Some(execution)] {
        return Err(fail(
            "selected execution is not the exact authored integer comparison and operand type",
        ));
    }
    Ok(proposal)
}
