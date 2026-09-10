//! Rejoin portable IEEE comparison operations to their authored selected meanings.

use checked_trees::{CheckedOperatorResolutionStatus, CheckedTrees};
use diagnostics::Diagnostic;

#[cfg(test)]
mod tests;

pub(super) fn associate(
    checked: &CheckedTrees,
    module: &terminal_psi::TerminalModule,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[provider_planning::plans::SelectedProviderReviewProvenance],
    occurrences: &[lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence],
) -> Result<Vec<compilation_report::TerminalIeeeFloatComparisonOccurrenceProposal>, Vec<Diagnostic>>
{
    let proposals = occurrences
        .iter()
        .map(|occurrence| {
            associate_one(checked, selected, provenance, occurrence).map_err(|error| vec![error])
        })
        .collect::<Result<Vec<_>, _>>()?;
    compilation_report::TerminalIeeeFloatComparisonOccurrenceProposal::validate_roster(
        module,
        selected.plans(),
        &proposals,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    Ok(proposals)
}

fn associate_one(
    checked: &CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[provider_planning::plans::SelectedProviderReviewProvenance],
    occurrence: &lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence,
) -> Result<compilation_report::TerminalIeeeFloatComparisonOccurrenceProposal, Diagnostic> {
    let fail = |reason: &str| {
        Diagnostic::error(format!(
            "Terminal IEEE comparison {}: {reason}",
            occurrence.terminal_operation.get()
        ))
    };
    let operator_use = checked.facts.operators.uses.get(occurrence.operator_use);
    let expected_primitive = match occurrence.format {
        semantic_vocabulary::IeeeFloatFormat::Binary32 => typed_trees::types::PrimitiveType::F32,
        semantic_vocabulary::IeeeFloatFormat::Binary64 => typed_trees::types::PrimitiveType::F64,
    };
    if !occurrence.operator_use.is_valid()
        || operator_use.status != CheckedOperatorResolutionStatus::Resolved
        || operator_use.application_site() != occurrence.application_site
        || operator_use.selected_operator_symbol != occurrence.requirement_operator
        || operator_use.provider_plan_report_fingerprint
            != occurrence.provider_plan_report_fingerprint
        || operator_use.provider_plan_commitment != occurrence.provider_plan_commitment
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
    let proposal = compilation_report::TerminalIeeeFloatComparisonOccurrenceProposal {
        terminal_machine: occurrence.terminal_machine,
        terminal_operation: occurrence.terminal_operation,
        provider_plan_index: *provider_plan_index,
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
