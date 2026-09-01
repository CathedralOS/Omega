//! Exact selected boundary-operator applications inside bounded Unit plans.

use super::*;

pub(super) fn selected_operator_scalar_result_local<'applications>(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statements: &[StatementNode],
    applications: &'applications [crate::SelectedOperatorApplication],
) -> Option<(
    &'applications crate::SelectedOperatorApplication,
    CheckedUnitScalarResultBindingPlan,
)> {
    let StatementNode::LocalData(local) = statements.first()? else {
        return None;
    };
    if local.is_mutable || !local.initial_value.is_valid() {
        return None;
    }
    let matches = applications
        .iter()
        .filter(|application| {
            application.expression == local.initial_value
                && application.origin
                    == psi_checked_trees::CheckedValueOrigin::StateStatement {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index: 0,
                        role: psi_checked_trees::CheckedValueStatementRole::LocalInitializer,
                    }
        })
        .collect::<Vec<_>>();
    let [application] = matches.as_slice() else {
        return None;
    };
    Some((
        *application,
        CheckedUnitScalarResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            primitive_type: program.primitive_type_reference(local.type_reference)?,
        },
    ))
}

pub(super) fn build_selected_operator_scalar_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    source_state: &psi_typed_trees::state::State,
    application: &crate::SelectedOperatorApplication,
    result: CheckedUnitScalarResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    let realization_machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == application.realization_machine)?;
    if realization_machine.supply_mode != MachineSupplyMode::CheckedBody {
        return None;
    }
    let realization_states = program
        .machine_states(realization_machine)
        .iter()
        .filter(|state| state.symbol == application.realization_state)
        .collect::<Vec<_>>();
    let [realization_state] = realization_states.as_slice() else {
        return None;
    };
    if program.primitive_type_reference(realization_state.return_type)
        != Some(result.primitive_type)
    {
        return None;
    }
    let parameters = program.state_parameters(realization_state);
    if parameters.len() != application.operands.len()
        || parameters.iter().any(|parameter| {
            parameter.is_self
                || parameter.is_const
                || parameter.is_mutable
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
    {
        return None;
    }
    let scalar_arguments = application
        .operands
        .iter()
        .zip(parameters)
        .map(|(operand, parameter)| {
            crate::values::lower_unit_scalar_argument(
                program,
                &facts.operators,
                source_state,
                0,
                *operand,
                program.primitive_type_reference(parameter.type_reference)?,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let realization_graph = facts
        .flow
        .terminal_scalar_graphs
        .for_machine(application.realization_machine)?;
    let realization_entry = realization_graph.states.first()?;
    if realization_entry.state != application.realization_state
        || realization_entry.result_type != result.primitive_type
        || realization_entry.parameter_types.len() != parameters.len()
    {
        return None;
    }
    let contract = facts
        .contract_plans
        .for_machine(application.realization_machine)?;
    Some(CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
        coordinate: CheckedUnitCallCoordinate {
            statement_index: result.statement_index,
            call_ordinal: 0,
        },
        result,
        requirement_operator: application.requirement_operator,
        provider_plan_report_fingerprint: application.provider_plan_report_fingerprint,
        provider_plan_commitment: application.provider_plan_commitment,
        realization_machine: application.realization_machine,
        realization_state: application.realization_state,
        realization_contract_report_fingerprint: contract.report_fingerprint,
        realization_contract_commitment: contract.commitment,
        service_reach: state_flow(
            facts,
            application.realization_machine,
            application.realization_state,
        )?
        .service_reach,
        scalar_arguments,
    })
}
