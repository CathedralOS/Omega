//! Exact checked D29 demand joins to emitted Terminal operations.

use super::*;

pub(super) fn checked_boundary_operator_occurrences(
    checked: &CheckedTrees,
    lowered: &LoweredTerminalPsi,
) -> Result<Vec<CheckedBoundaryOperatorApplicationOccurrence>, LoweringError> {
    let mut occurrences = Vec::new();
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
                } => {
                    let statement_index =
                        usize::try_from(coordinate.statement_index).map_err(|_| {
                            LoweringError::Unsupported(
                                "selected operator statement coordinate exceeds usize",
                            )
                        })?;
                    let call_ordinal = usize::try_from(coordinate.call_ordinal).map_err(|_| {
                        LoweringError::Unsupported(
                            "selected operator call coordinate exceeds usize",
                        )
                    })?;
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
                    let statement_index =
                        usize::try_from(coordinate.statement_index).map_err(|_| {
                            LoweringError::Unsupported(
                                "selected IEEE FMA statement coordinate exceeds usize",
                            )
                        })?;
                    let call_ordinal = usize::try_from(coordinate.call_ordinal).map_err(|_| {
                        LoweringError::Unsupported(
                            "selected IEEE FMA call coordinate exceeds usize",
                        )
                    })?;
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
                            psi_checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                                origin: psi_checked_trees::CheckedValueOrigin::StateStatement {
                                    machine_symbol,
                                    state_symbol,
                                    statement_index: application_statement,
                                    role: psi_checked_trees::CheckedValueStatementRole::LocalInitializer,
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
    for plan in &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .selected_operator_machines
    {
        let statement_index = usize::try_from(plan.return_statement_ordinal).map_err(|_| {
            LoweringError::Unsupported(
                "selected structural operator statement coordinate exceeds usize",
            )
        })?;
        let matching_occurrences = lowered
            .source_call_occurrences
            .iter()
            .filter(|occurrence| {
                occurrence.source_state == plan.state
                    && occurrence.statement_index == statement_index
                    && occurrence.call_ordinal == 0
                    && occurrence.source_target == plan.realization_machine
            })
            .collect::<Vec<_>>();
        if matching_occurrences.is_empty() {
            continue;
        }
        let [terminal_occurrence] = matching_occurrences.as_slice() else {
            return unsupported(
                "selected structural operator maps to duplicate Terminal call occurrences",
            );
        };
        let matching_applications = checked
            .facts
            .operators
            .boundary_applications
            .iter()
            .enumerate()
            .filter(|(_, application)| {
                application.requirement_symbol == plan.requirement_operator
                    && matches!(
                        application.site,
                        psi_checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                            origin: psi_checked_trees::CheckedValueOrigin::StateStatement {
                                machine_symbol,
                                state_symbol,
                                statement_index: application_statement,
                                role: psi_checked_trees::CheckedValueStatementRole::Expression,
                            },
                            ..
                        } if machine_symbol == plan.machine
                            && state_symbol == plan.state
                            && application_statement == statement_index
                    )
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [application_index] = matching_applications.as_slice() else {
            return unsupported(
                "selected structural operator does not rejoin one exact checked D29 application",
            );
        };
        occurrences.push(CheckedBoundaryOperatorApplicationOccurrence {
            application_index: *application_index,
            terminal_operation: terminal_occurrence.terminal_operation,
        });
    }
    occurrences.sort_by_key(|occurrence| occurrence.terminal_operation.get());
    let application_indices = occurrences
        .iter()
        .map(|occurrence| occurrence.application_index)
        .collect::<BTreeSet<_>>();
    let terminal_operations = occurrences
        .iter()
        .map(|occurrence| occurrence.terminal_operation)
        .collect::<BTreeSet<_>>();
    if application_indices.len() != occurrences.len()
        || terminal_operations.len() != occurrences.len()
    {
        return unsupported(
            "checked boundary-operator applications do not map one-to-one onto Terminal operations",
        );
    }
    if matched_ieee_float_fmas != lowered.selected_ieee_float_fma_occurrences.len() {
        return unsupported(
            "selected IEEE FMA Terminal occurrences do not all rejoin checked boundary applications",
        );
    }
    Ok(occurrences)
}
