use super::{CheckedBoundaryOperatorApplicationOccurrence, unsupported};
use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;

pub(super) fn replay(
    checked: &CheckedTrees,
    lowered: &LoweredPsi,
    occurrences: &mut Vec<CheckedBoundaryOperatorApplicationOccurrence>,
) -> Result<(), &'static str> {
    for plan in &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .selected_operator_machines
    {
        let statement_index = usize::try_from(plan.return_statement_ordinal)
            .map_err(|_| "selected structural operator statement coordinate exceeds usize")?;
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
                        checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                            origin: checked_trees::CheckedValueOrigin::StateStatement {
                                machine_symbol,
                                state_symbol,
                                statement_index: application_statement,
                                role: checked_trees::CheckedValueStatementRole::Expression,
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
    Ok(())
}
