//! Selected comparison custody, separate from ordinary integer operations.
//!
//! Builtin comparisons and generated guards use the same Terminal operations
//! without selecting a provider. Operation kinds therefore cannot supply the
//! selected-occurrence census. Final admission checks completeness against the
//! artifact-bound checked boundary scope, then checks each row's
//! operation, provider and operands. The standalone row check establishes only
//! the validity of supplied rows, not completeness of source custody.

use super::TerminalIntegerComparisonOccurrenceProposal;

pub(super) fn validate_integer_comparison_coverage(
    module: &terminal_psi::TerminalModule,
    plans: &[effects::provider_plan::ProviderPlan],
    occurrences: &[TerminalIntegerComparisonOccurrenceProposal],
    realizations: &boundary_applications::TerminalBoundaryApplicationRealizations,
    checked_scope: &lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
) -> Result<(), &'static str> {
    TerminalIntegerComparisonOccurrenceProposal::validate_roster(module, plans, occurrences)?;
    // The caller has validated this sealed scope against the same canonical
    // artifact. Classify its exact operations, not the public companion's
    // claimed role: relabeling a selected comparison as a checked-body
    // realization must not erase the completeness obligation.
    let selected_count = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerEqual { .. }
                    | terminal_psi::OperationKind::IntegerLessThan { .. }
                    | terminal_psi::OperationKind::IntegerLessOrEqual { .. }
            ) && checked_scope
                .occurrences()
                .iter()
                .any(|occurrence| occurrence.terminal_operation() == operation.id)
        })
        .count();
    if selected_count != occurrences.len() {
        return Err(
            "Terminal proposal must retain every selected integer comparison occurrence exactly once",
        );
    }
    // Distinct occurrence coordinates plus this exact forward join and the
    // sealed-scope census establish both directions: no selected use may be
    // dropped, and no builtin comparison may acquire a selected provider row.
    for occurrence in occurrences {
        let matching = realizations
            .rows()
            .iter()
            .filter(|row| row.terminal_operation() == occurrence.terminal_operation)
            .collect::<Vec<_>>();
        let [realization] = matching.as_slice() else {
            return Err(
                "Terminal integer comparison requires one exact boundary realization companion",
            );
        };
        if realization.selected_plan_digest() != occurrence.provider_plan_commitment.as_bytes()
            || !matches!(realization.realization(),
                boundary_applications::BoundaryApplicationRealization::ExactCompilerIntrinsic { execution }
                if Some(*execution) == occurrence.execution_identity())
        {
            return Err("Terminal integer comparison changed its selected boundary realization");
        }
    }
    Ok(())
}
