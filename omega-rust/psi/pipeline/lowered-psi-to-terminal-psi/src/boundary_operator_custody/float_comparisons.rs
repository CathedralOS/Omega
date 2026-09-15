use super::{CheckedBoundaryOperatorApplicationOccurrence, unsupported};
use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;

pub(super) fn replay(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
    occurrences: &mut Vec<CheckedBoundaryOperatorApplicationOccurrence>,
) -> Result<(), &'static str> {
    // An implicit Match comparison owns its arm application, not the enclosing
    // expression's result. Preserve that distinction when projecting source-free
    // demands; no comparison may disappear from the published occurrence roster.
    let terminal_comparison_count = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::IeeeFloatCompare { .. }
            )
        })
        .count();
    if terminal_comparison_count != lowered.selected_ieee_float_comparison_occurrences.len() {
        return unsupported("IEEE comparison occurrences do not cover the exact Terminal roster");
    }
    for comparison in &lowered.selected_ieee_float_comparison_occurrences {
        if !checked
            .facts
            .operators
            .uses
            .is_valid(comparison.operator_use)
        {
            return unsupported("IEEE comparison lost its checked operator occurrence");
        }
        let operator_use = checked.facts.operators.uses.get(comparison.operator_use);
        let selected_meaning = checked
            .facts
            .operators
            .selected_float_comparison(&checked.typed, comparison.operator_use);
        let expected_primitive = match comparison.format {
            semantic_vocabulary::IeeeFloatFormat::Binary32 => {
                checked_trees::types::PrimitiveType::F32
            }
            semantic_vocabulary::IeeeFloatFormat::Binary64 => {
                checked_trees::types::PrimitiveType::F64
            }
        };
        if operator_use.application_site() != comparison.application_site
            || operator_use.selected_operator_symbol != comparison.requirement_operator
            || operator_use.provider_plan_commitment != comparison.provider_plan_commitment
            || comparison.provider_plan_commitment.is_empty()
            || operator_use.operands(&checked.typed).is_none()
            || selected_meaning != Some((comparison.comparison, expected_primitive))
        {
            return unsupported("IEEE comparison changed its exact selected application");
        }
        let mut matching_operations = 0;
        for machine in &lowered.semantic_module.machines {
            if machine.id != comparison.terminal_machine {
                continue;
            }
            for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
                let terminal_psi::OperationKind::IeeeFloatCompare {
                    comparison: actual,
                    left,
                    right,
                } = operation.kind
                else {
                    continue;
                };
                if operation.id != comparison.terminal_operation || actual != comparison.comparison
                {
                    continue;
                }
                let values = machine
                    .parameters
                    .iter()
                    .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
                    .copied()
                    .chain(
                        machine
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .filter_map(|operation| operation.result.scalar()),
                    );
                let expected = semantic_vocabulary::ScalarType::IeeeFloat(comparison.format);
                if [left, right].into_iter().all(|operand| {
                    values
                        .clone()
                        .any(|value| value.id == operand && value.scalar_type == expected)
                }) {
                    matching_operations += 1;
                }
            }
        }
        if matching_operations != 1 {
            return unsupported("IEEE comparison does not name one exact Terminal operation");
        }
        let mut applications = checked
            .facts
            .operators
            .boundary_applications
            .iter()
            .enumerate()
            .filter(|(_, application)| {
                application.site == comparison.application_site
                    && application.requirement_symbol == comparison.requirement_operator
            });
        let application = applications.next();
        let (Some((application_index, _)), None) = (application, applications.next()) else {
            return unsupported("IEEE comparison does not rejoin one exact checked application");
        };
        occurrences.push(CheckedBoundaryOperatorApplicationOccurrence {
            application_index,
            terminal_operation: comparison.terminal_operation,
        });
    }
    Ok(())
}
