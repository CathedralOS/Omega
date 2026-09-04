//! Selected boundary-operator structural-scalar return planning.

use super::*;

pub(super) fn build_selected_operator_structural_scalar_return_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    structural_realizations: &[CheckedStructuralScalarReturnMachinePlan],
    applications: &[crate::SelectedOperatorApplication],
) -> Option<CheckedSelectedOperatorStructuralScalarReturnMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    let [StatementNode::Expression(expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        statement_index: 0,
        role: psi_checked_trees::CheckedValueStatementRole::Expression,
    };
    let matching = applications
        .iter()
        .filter(|application| application.expression == *expression && application.origin == origin)
        .collect::<Vec<_>>();
    let [application] = matching.as_slice() else {
        return None;
    };
    if machine.attached_data.is_some()
        || !program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
        || application.provider_plan_report_fingerprint == 0
        || application.provider_plan_commitment.is_empty()
    {
        return None;
    }
    let operator = program
        .operators()
        .iter()
        .find(|operator| operator.symbol == application.requirement_operator)?;
    if !operator.is_boundary
        || program.operator_parameters(operator).len() != application.operands.len()
    {
        return None;
    }
    let realization = structural_realizations.iter().find(|plan| {
        plan.machine == application.realization_machine
            && plan.state == application.realization_state
    })?;
    if !realization.scalar_parameters.is_empty()
        || realization.structural_parameters.len() != application.operands.len()
        || realization.result_type != program.primitive_type_reference(state.return_type)?
    {
        return None;
    }
    let source_flow = state_flow(facts, machine.symbol, state.symbol)?;
    let realization_flow = state_flow(
        facts,
        application.realization_machine,
        application.realization_state,
    )?;
    if !service_reach_is_empty(facts, source_flow.service_reach)
        || !service_reach_is_empty(facts, realization_flow.service_reach)
    {
        return None;
    }
    let binders = machine_binders(program, machine);
    let source_parameters = program.state_parameters(state);
    let mut structural_parameters = source_parameters
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            if parameter.is_self
                || parameter.is_const
                || parameter.is_mutable
                || is_reference(program, parameter.type_reference)
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            {
                return None;
            }
            let type_identity = shapes.add_type(parameter.type_reference, &binders, &[])?;
            let qualifications =
                parameter_qualifications(program, shapes, parameter.type_reference, &binders)?;
            if crate::checks::type_multiplicity(program, parameter.type_reference)
                != Multiplicity::Affine
                || !qualifications.is_empty()
            {
                return None;
            }
            Some(CheckedUnitStructuralParameterPlan {
                position: u32::try_from(position).ok()?,
                is_self: false,
                type_identity,
                multiplicity: Multiplicity::Affine,
                access: CheckedStructuralAccess::Owned,
                qualifications,
                fused_service_erasure: None,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if structural_parameters.is_empty() {
        return None;
    }
    let argument_source_positions = application
        .operands
        .iter()
        .map(|operand| {
            let ExpressionNode::Name(path) = program.expression_table.expression(*operand) else {
                return None;
            };
            if program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
            {
                return None;
            }
            source_parameters
                .iter()
                .position(|parameter| parameter.symbol == path.symbol)
                .and_then(|position| u32::try_from(position).ok())
        })
        .collect::<Option<Vec<_>>>()?;
    if argument_source_positions.len() != structural_parameters.len()
        || argument_source_positions
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != structural_parameters.len()
    {
        return None;
    }
    for (target_position, source_position) in argument_source_positions.iter().enumerate() {
        let source_index = structural_parameters
            .iter()
            .position(|parameter| parameter.position == *source_position)?;
        let target = realization.structural_parameters.get(target_position)?;
        let source_shape = &shapes
            .types
            .get(&structural_parameters[source_index].type_identity)?
            .shape;
        let target_shape = &shapes.types.get(&target.type_identity)?.shape;
        if source_shape != target_shape
            || structural_parameters[source_index].multiplicity != target.multiplicity
            || structural_parameters[source_index].access != target.access
            || structural_parameters[source_index].qualifications != target.qualifications
        {
            return None;
        }
        // A direct `Buffer<i32, 4>` source spelling and the specialized
        // realization's substituted generic spelling may carry distinct
        // private type-reference identities. Exact operator application and
        // specialization custody already joined them above; identical checked
        // structural shapes let the call use the realization's canonical
        // Terminal carrier without preserving either source spelling.
        structural_parameters[source_index].type_identity = target.type_identity.clone();
    }
    let contract = facts
        .contract_plans
        .for_machine(application.realization_machine)?;
    Some(CheckedSelectedOperatorStructuralScalarReturnMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        structural_parameters,
        result_type: realization.result_type,
        return_statement_ordinal: 0,
        requirement_operator: application.requirement_operator,
        provider_plan_report_fingerprint: application.provider_plan_report_fingerprint,
        provider_plan_commitment: application.provider_plan_commitment,
        realization_machine: application.realization_machine,
        realization_state: application.realization_state,
        realization_contract_report_fingerprint: contract.report_fingerprint,
        realization_contract_commitment: contract.commitment,
        service_reach: realization_flow.service_reach,
        argument_source_positions,
    })
}
