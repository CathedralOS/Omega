//! Join already-emitted comparisons to this target's selected provider evidence.
//!
//! Retained Terminal products, direct native compilation and inspection use this
//! same settlement. Checked trees and lowering occurrences establish the authored
//! application; backend records replay exact operation, plan and execution custody.
//! These sidecars neither replace canonical-artifact admission nor lower source into
//! native operations. Provider selection still arrives from the checked build route.

use boundary_applications::{
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
};
use checked_trees::{CheckedOperatorResolutionStatus, CheckedTrees};
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
    let expected_primitive = match occurrence.format {
        semantic_vocabulary::IeeeFloatFormat::Binary32 => checked_trees::types::PrimitiveType::F32,
        semantic_vocabulary::IeeeFloatFormat::Binary64 => checked_trees::types::PrimitiveType::F64,
    };
    if !occurrence.operator_use.is_valid()
        || operator_use.status != CheckedOperatorResolutionStatus::Resolved
        || operator_use.application_site() != occurrence.application_site
        || operator_use.selected_operator_symbol != occurrence.requirement_operator
        || operator_use.operands(&checked.typed).is_none()
        || checked
            .facts
            .operators
            .selected_float_comparison(&checked.typed, occurrence.operator_use)
            != Some((occurrence.comparison, expected_primitive))
    {
        return Err(fail(
            "authored operator occurrence, operands or selected provider drifted",
        ));
    }
    let application_count = checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .filter(|application| {
            application.site == occurrence.application_site
                && application.requirement_symbol == occurrence.requirement_operator
        })
        .count();
    if application_count != 1 {
        return Err(fail(
            "requires exactly one closed authored boundary application",
        ));
    }
    // This target's selected plan joins the occurrence by requirement
    // identity; the Terminal occurrence carries no selection.
    let Some((provider_plan_index, plan)) = provider_planning::selected_use_plan(
        checked,
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
    if retained.row_compiler_intrinsic_executions.as_slice() != [Some(execution)]
        || selected_dispatch::derive_selected_compiler_intrinsic_execution_identity(
            checked,
            plan,
            occurrence.requirement_operator,
        )? != Some(
            selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Closed(execution),
        )
    {
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

/// The checked primitive an emitted operand carrier denotes. The checked
/// classifier admits the address carrier to `selected_integer_comparison`;
/// it maps here so the use/occurrence join stays exact while the execution
/// identity still fails closed on it.
fn occurrence_primitive(
    integer_type: semantic_vocabulary::IntegerType,
) -> Option<checked_trees::types::PrimitiveType> {
    use checked_trees::types::PrimitiveType;
    use semantic_vocabulary::{IntegerCarrier, IntegerSign};
    if integer_type.carrier() == IntegerCarrier::Address {
        return (integer_type.bits() == 64).then_some(PrimitiveType::Addr);
    }
    Some(match (integer_type.sign(), integer_type.bits()) {
        (IntegerSign::Signed, 8) => PrimitiveType::I8,
        (IntegerSign::Signed, 16) => PrimitiveType::I16,
        (IntegerSign::Signed, 32) => PrimitiveType::I32,
        (IntegerSign::Signed, 64) => PrimitiveType::I64,
        (IntegerSign::Unsigned, 8) => PrimitiveType::U8,
        (IntegerSign::Unsigned, 16) => PrimitiveType::U16,
        (IntegerSign::Unsigned, 32) => PrimitiveType::U32,
        (IntegerSign::Unsigned, 64) => PrimitiveType::U64,
        _ => return None,
    })
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
    let Some(expected_primitive) = occurrence_primitive(occurrence.integer_type) else {
        return Err(fail("recorded operand carrier has no checked primitive"));
    };
    let selected_meaning = checked
        .facts
        .operators
        .selected_integer_comparison(&checked.typed, occurrence.operator_use);
    if !occurrence.operator_use.is_valid()
        || operator_use.status != CheckedOperatorResolutionStatus::Resolved
        || operator_use.application_site() != occurrence.application_site
        || operator_use.selected_operator_symbol != occurrence.requirement_operator
        || operator_use.operands(&checked.typed).is_none()
        || selected_meaning.and_then(|(operation, primitive)| {
            lowered_psi::LoweredSelectedIntegerComparisonOperation::admitted_emission(operation)
                .filter(|_| primitive == expected_primitive)
        }) != Some((
            occurrence.comparison,
            occurrence.operand_order,
            occurrence.negated,
        ))
    {
        return Err(fail(
            "authored operator occurrence, operands, emission or selected provider drifted",
        ));
    }
    let application_count = checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .filter(|application| {
            application.site == occurrence.application_site
                && application.requirement_symbol == occurrence.requirement_operator
        })
        .count();
    if application_count != 1 {
        return Err(fail(
            "requires exactly one closed authored boundary application",
        ));
    }
    // This target's selected plan joins the occurrence by requirement
    // identity; the Terminal occurrence carries no selection.
    let Some((provider_plan_index, plan)) = provider_planning::selected_use_plan(
        checked,
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
    if retained.row_compiler_intrinsic_executions.as_slice() != [Some(execution)]
        || selected_dispatch::derive_selected_compiler_intrinsic_execution_identity(
            checked,
            plan,
            occurrence.requirement_operator,
        )? != Some(
            selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Closed(execution),
        )
    {
        return Err(fail(
            "selected execution is not the exact authored integer comparison and operand type",
        ));
    }
    Ok(proposal)
}
