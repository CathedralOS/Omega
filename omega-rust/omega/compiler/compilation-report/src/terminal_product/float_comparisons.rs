use super::TerminalIeeeFloatComparisonOccurrenceProposal;
use semantic_vocabulary::ScalarType;

pub(super) fn validate_float_comparison_occurrences(
    module: &terminal_psi::TerminalModule,
    plans: &[effects::provider_plan::ProviderPlan],
    occurrences: &[TerminalIeeeFloatComparisonOccurrenceProposal],
) -> Result<(), &'static str> {
    let comparisons = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine.blocks.iter().flat_map(move |block| {
                block.operations.iter().filter_map(move |operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::IeeeFloatCompare { .. }
                    )
                    .then_some((machine, operation))
                })
            })
        })
        .collect::<Vec<_>>();
    if comparisons.len() != occurrences.len() {
        return Err("Terminal proposal must retain every IEEE comparison occurrence exactly once");
    }
    let mut seen = std::collections::BTreeSet::new();
    for occurrence in occurrences {
        if !seen.insert((occurrence.terminal_machine, occurrence.terminal_operation)) {
            return Err("Terminal proposal repeats an IEEE comparison occurrence");
        }
        let plan = plans
            .get(occurrence.provider_plan_index)
            .ok_or("Terminal IEEE comparison names an absent selected plan")?;
        if plan.identity_digest() != occurrence.provider_plan_commitment {
            return Err("Terminal IEEE comparison changed its exact selected plan");
        }
        let matching = comparisons
            .iter()
            .filter(|(machine, operation)| {
                machine.id == occurrence.terminal_machine
                    && operation.id == occurrence.terminal_operation
            })
            .collect::<Vec<_>>();
        let [(machine, operation)] = matching.as_slice() else {
            return Err("Terminal IEEE comparison does not name one exact operation");
        };
        let terminal_psi::OperationKind::IeeeFloatCompare {
            comparison,
            left,
            right,
        } = operation.kind
        else {
            return Err("Terminal IEEE comparison changed operation kind");
        };
        if comparison != occurrence.comparison
            || operation
                .result
                .scalar()
                .is_none_or(|result| result.scalar_type != ScalarType::Boolean)
        {
            return Err("Terminal IEEE comparison changed operation or result type");
        }
        for operand in [left, right] {
            let mut declarations = machine
                .parameters
                .iter()
                .copied()
                .chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.parameters)
                        .copied(),
                )
                .chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter_map(|operation| operation.result.scalar()),
                )
                .filter(|declaration| declaration.id == operand);
            if declarations.next().is_none_or(|declaration| {
                declaration.scalar_type != ScalarType::IeeeFloat(occurrence.format)
            }) || declarations.next().is_some()
            {
                return Err("Terminal IEEE comparison changed operand format or identity");
            }
        }
    }
    Ok(())
}
