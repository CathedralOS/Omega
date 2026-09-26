//! Rejoin portable IEEE comparison operations to their authored selected meanings.

use diagnostics::Diagnostic;
use typed_trees_to_checked_trees::checked_trees::{CheckedOperatorResolutionStatus, CheckedTrees};

pub(crate) fn associate(
    checked: &CheckedTrees,
    module: &terminal_psi::TerminalModule,
    selected: &abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts,
    provenance: &[crate::provider_planning::SelectedProviderReviewProvenance],
    occurrences: &[checked_trees_to_lowered_psi::lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence],
) -> Result<
    Vec<crate::compiler::report::TerminalIeeeFloatComparisonOccurrenceProposal>,
    Vec<Diagnostic>,
> {
    let proposals = occurrences
        .iter()
        .map(|occurrence| {
            associate_one(checked, selected, provenance, occurrence).map_err(|error| vec![error])
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::compiler::report::TerminalIeeeFloatComparisonOccurrenceProposal::validate_roster(
        module,
        selected.plans(),
        &proposals,
    )
    .map_err(|message| vec![Diagnostic::error(message)])?;
    Ok(proposals)
}

fn associate_one(
    checked: &CheckedTrees,
    selected: &abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts,
    provenance: &[crate::provider_planning::SelectedProviderReviewProvenance],
    occurrence: &checked_trees_to_lowered_psi::lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence,
) -> Result<crate::compiler::report::TerminalIeeeFloatComparisonOccurrenceProposal, Diagnostic> {
    let fail = |reason: &str| {
        Diagnostic::error(format!(
            "Terminal IEEE comparison {}: {reason}",
            occurrence.terminal_operation.get()
        ))
    };
    let operator_use = checked.facts.operators.uses.get(occurrence.operator_use);
    let expected_primitive = match occurrence.format {
        semantic_vocabulary::IeeeFloatFormat::Binary32 => {
            symbol_resolved_trees_to_typed_trees::typed_trees::types::PrimitiveType::F32
        }
        semantic_vocabulary::IeeeFloatFormat::Binary64 => {
            symbol_resolved_trees_to_typed_trees::typed_trees::types::PrimitiveType::F64
        }
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
    // This target's selected plan joins the occurrence by requirement
    // identity; the Terminal occurrence carries no selection.
    let Some((provider_plan_index, plan)) = crate::provider_planning::selected_use_plan(
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
    let proposal = crate::compiler::report::TerminalIeeeFloatComparisonOccurrenceProposal {
        terminal_machine: occurrence.terminal_machine,
        terminal_operation: occurrence.terminal_operation,
        provider_plan_index,
        provider_plan_commitment: plan.identity_digest(),
        comparison: occurrence.comparison,
        format: occurrence.format,
    };
    let execution = proposal.execution_identity();
    if retained.row_compiler_intrinsic_executions.as_slice() != [Some(execution)]
        || crate::selected_dispatch::derive_selected_compiler_intrinsic_execution_identity(
            checked,
            plan,
            occurrence.requirement_operator,
        )? != Some(
            crate::selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Closed(execution),
        )
    {
        return Err(fail(
            "selected execution is not the exact IEEE comparison and format",
        ));
    }
    Ok(proposal)
}
