//! Recheck exact selected boundary-operator custody before Unit lowering.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_selected_operator_scalar_call(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    coordinate: psi_checked_trees::CheckedUnitCallCoordinate,
    result: psi_checked_trees::CheckedUnitScalarResultBindingPlan,
    requirement_operator: psi_symbols::SymbolHandle,
    provider_plan_report_fingerprint: u64,
    provider_plan_commitment: psi_checked_trees::CheckedProviderPlanCommitment,
    realization_machine: psi_symbols::SymbolHandle,
    realization_state: psi_symbols::SymbolHandle,
    realization_contract_report_fingerprint: u64,
    service_reach: psi_language_semantics::ServiceReachSummary,
    scalar_argument_count: usize,
) -> Result<(), LoweringError> {
    let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: machine.machine,
        state_symbol: machine.state,
        statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("selected operator statement coordinate exceeds usize")
        })?,
        role: psi_checked_trees::CheckedValueStatementRole::LocalInitializer,
    };
    let exact_uses = checked
        .facts
        .operators
        .named_uses
        .iter()
        .filter(|(_, operator_use)| {
            operator_use.origin == origin
                && operator_use.selected_operator_symbol == requirement_operator
                && operator_use.provider_plan_report_fingerprint == provider_plan_report_fingerprint
                && operator_use.provider_plan_commitment == provider_plan_commitment
        })
        .count()
        + checked
            .facts
            .operators
            .uses
            .iter()
            .filter(|(_, operator_use)| {
                operator_use.origin == origin
                    && operator_use.selected_operator_symbol == requirement_operator
                    && operator_use.provider_plan_report_fingerprint
                        == provider_plan_report_fingerprint
                    && operator_use.provider_plan_commitment == provider_plan_commitment
            })
            .count();
    if coordinate.call_ordinal != 0
        || provider_plan_report_fingerprint == 0
        || provider_plan_commitment.is_empty()
        || exact_uses != 1
    {
        return unsupported(
            "selected Unit operator application does not rejoin one exact checked authored use",
        );
    }

    let realization = checked
        .typed
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == realization_machine)
        .and_then(|realization| {
            checked
                .typed
                .machine_states(realization)
                .iter()
                .find(|state| state.symbol == realization_state)
        })
        .ok_or(LoweringError::Unsupported(
            "selected Unit operator realization lost its exact checked entry",
        ))?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(realization_machine)
        .ok_or(LoweringError::Unsupported(
            "selected Unit operator realization has no checked contract",
        ))?;
    let realization_reaches = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| {
            state.machine_symbol == realization_machine && state.state_symbol == realization_state
        })
        .map(|(_, state)| state.service_reach)
        .collect::<Vec<_>>();
    if contract.report_fingerprint != realization_contract_report_fingerprint
        || checked
            .typed
            .primitive_type_reference(realization.return_type)
            != Some(result.primitive_type)
        || checked.typed.state_parameters(realization).len() != scalar_argument_count
        || realization_reaches.as_slice() != [service_reach]
    {
        return unsupported(
            "selected Unit operator realization drifted from its checked machine, state, contract, result, arguments, or reach",
        );
    }
    Ok(())
}
