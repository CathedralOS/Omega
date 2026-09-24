use super::{CheckedBoundaryOperatorApplicationOccurrence, unsupported};
use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;

use checked_trees::CheckedUnitEffectOperationPlan;

pub(super) fn replay(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
    occurrences: &mut Vec<CheckedBoundaryOperatorApplicationOccurrence>,
) -> Result<usize, &'static str> {
    let mut matched_ieee_float_fmas = 0_usize;
    for machine in &checked.facts.flow.terminal_unit_effects.machines {
        for operation in &machine.operations {
            let (statement_index, requirement, terminal_operation) = match operation {
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                    coordinate,
                    requirement_operator,
                    realization_machine,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                    coordinate,
                    requirement_operator,
                    realization_machine,
                    ..
                }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                    coordinate,
                    requirement_operator,
                    realization_machine,
                    ..
                } => {
                    let statement_index = usize::try_from(coordinate.statement_index)
                        .map_err(|_| "selected operator statement coordinate exceeds usize")?;
                    let call_ordinal = usize::try_from(coordinate.call_ordinal)
                        .map_err(|_| "selected operator call coordinate exceeds usize")?;
                    let matching = lowered
                        .source_call_occurrences
                        .iter()
                        .filter(|occurrence| {
                            occurrence.source_state == machine.state
                                && occurrence.statement_index == statement_index
                                && occurrence.call_ordinal == call_ordinal
                                && occurrence.source_target == *realization_machine
                        })
                        .collect::<Vec<_>>();
                    if matching.is_empty() {
                        continue;
                    }
                    let [matching] = matching.as_slice() else {
                        return unsupported(
                            "selected operator application maps to duplicate Terminal call occurrences",
                        );
                    };
                    (
                        statement_index,
                        *requirement_operator,
                        matching.terminal_operation,
                    )
                }
                CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                    coordinate,
                    requirement_operator,
                    ..
                } => {
                    let statement_index = usize::try_from(coordinate.statement_index)
                        .map_err(|_| "selected IEEE FMA statement coordinate exceeds usize")?;
                    let call_ordinal = usize::try_from(coordinate.call_ordinal)
                        .map_err(|_| "selected IEEE FMA call coordinate exceeds usize")?;
                    let matching = lowered
                        .selected_ieee_float_fma_occurrences
                        .iter()
                        .filter(|occurrence| {
                            occurrence.source_state == machine.state
                                && occurrence.statement_index == statement_index
                                && occurrence.call_ordinal == call_ordinal
                                && occurrence.requirement_operator == *requirement_operator
                        })
                        .collect::<Vec<_>>();
                    if matching.is_empty() {
                        continue;
                    }
                    let [matching] = matching.as_slice() else {
                        return unsupported(
                            "selected IEEE FMA application maps to duplicate Terminal occurrences",
                        );
                    };
                    matched_ieee_float_fmas += 1;
                    (
                        statement_index,
                        *requirement_operator,
                        matching.terminal_operation,
                    )
                }
                _ => continue,
            };
            let matching_applications = checked
                .facts
                .operators
                .boundary_applications
                .iter()
                .enumerate()
                .filter(|(_, application)| {
                    application.requirement_symbol == requirement
                        && matches!(
                            application.site,
                            checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                                origin: checked_trees::CheckedValueOrigin::StateStatement {
                                    machine_symbol,
                                    state_symbol,
                                    statement_index: application_statement,
                                    role: checked_trees::CheckedValueStatementRole::LocalInitializer,
                                },
                                ..
                            } if machine_symbol == machine.machine
                                && state_symbol == machine.state
                                && application_statement == statement_index
                        )
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [application_index] = matching_applications.as_slice() else {
                return unsupported(
                    "lowered boundary-operator occurrence does not rejoin one exact checked application",
                );
            };
            occurrences.push(CheckedBoundaryOperatorApplicationOccurrence {
                application_index: *application_index,
                terminal_operation,
            });
        }
    }
    Ok(matched_ieee_float_fmas)
}
