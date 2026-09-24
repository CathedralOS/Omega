//! Rejoin portable integer comparison operations to their authored selected
//! meanings.
//!
//! This is the integer counterpart of `float_comparisons`: every emitted
//! `IntegerEqual`/`IntegerLessThan`/`IntegerLessOrEqual` the checked program
//! selected through a boundary operator must rejoin its exact checked use,
//! its one boundary application, and its exact selected provider plan before
//! the occurrence may travel into native realization. The recorded emission
//! triple (emitted operation, operand order, negation) must be the one
//! admitted emission of the authored spelling, so a swapped or negated
//! comparison can never silently substitute a different selected semantic.

use checked_trees::{CheckedOperatorResolutionStatus, CheckedTrees};
use diagnostics::Diagnostic;

#[cfg(test)]
mod tests;

pub(crate) fn associate(
    checked: &CheckedTrees,
    module: &terminal_psi::TerminalModule,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[provider_planning::SelectedProviderReviewProvenance],
    occurrences: &[lowered_psi::LoweredSelectedIntegerComparisonOccurrence],
) -> Result<Vec<compilation_report::TerminalIntegerComparisonOccurrenceProposal>, Vec<Diagnostic>> {
    let proposals = occurrences
        .iter()
        .map(|occurrence| {
            associate_one(checked, selected, provenance, occurrence).map_err(|error| vec![error])
        })
        .collect::<Result<Vec<_>, _>>()?;
    compilation_report::TerminalIntegerComparisonOccurrenceProposal::validate_roster(
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
) -> Option<typed_trees::types::PrimitiveType> {
    use semantic_vocabulary::{IntegerCarrier, IntegerSign};
    use typed_trees::types::PrimitiveType;
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

fn associate_one(
    checked: &CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[provider_planning::SelectedProviderReviewProvenance],
    occurrence: &lowered_psi::LoweredSelectedIntegerComparisonOccurrence,
) -> Result<compilation_report::TerminalIntegerComparisonOccurrenceProposal, Diagnostic> {
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
        || operator_use.provider_plan_report_fingerprint
            != occurrence.provider_plan_report_fingerprint
        || operator_use.provider_plan_commitment != occurrence.provider_plan_commitment
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
    let applications = checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .filter(|application| {
            application.site == occurrence.application_site
                && application.requirement_symbol == occurrence.requirement_operator
        })
        .collect::<Vec<_>>();
    if applications.len() != 1 {
        return Err(fail(
            "requires exactly one closed authored boundary application",
        ));
    }
    let plans = selected
        .plans()
        .iter()
        .enumerate()
        .filter(|(_, plan)| {
            plan.report_fingerprint() == occurrence.provider_plan_report_fingerprint
                && plan.identity_digest().as_bytes()
                    == occurrence.provider_plan_commitment.as_bytes()
        })
        .collect::<Vec<_>>();
    let [(provider_plan_index, plan)] = plans.as_slice() else {
        return Err(fail("requires exactly one exact selected provider plan"));
    };
    let Some(retained) = provenance.get(*provider_plan_index) else {
        return Err(fail("selected provider provenance is missing"));
    };
    if retained.plan != **plan
        || plan.rows.len() != 1
        || retained.provider.row_requirements.as_slice() != [occurrence.requirement_operator]
    {
        return Err(fail(
            "selected provider provenance does not rejoin the exact requirement",
        ));
    }
    let proposal = compilation_report::TerminalIntegerComparisonOccurrenceProposal {
        terminal_machine: occurrence.terminal_machine,
        terminal_operation: occurrence.terminal_operation,
        provider_plan_index: *provider_plan_index,
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
