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

pub(super) fn selected_operator_structural_result_local<'applications>(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statements: &[StatementNode],
    applications: &'applications [crate::SelectedOperatorApplication],
) -> Option<(
    &'applications crate::SelectedOperatorApplication,
    CheckedUnitStructuralResultBindingPlan,
    SymbolHandle,
)> {
    let StatementNode::LocalData(local) = statements.first()? else {
        return None;
    };
    if local.is_mutable
        || !local.initial_value.is_valid()
        || program
            .primitive_type_reference(local.type_reference)
            .is_some()
        || is_reference(program, local.type_reference)
        || crate::checks::type_multiplicity(program, local.type_reference) != Multiplicity::Affine
        || type_graph_requires_nominal_drop(program, local.type_reference)
    {
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
    let binders = machine_binders(program, machine);
    if !parameter_qualifications(program, shapes, local.type_reference, &binders)?.is_empty() {
        return None;
    }
    Some((
        *application,
        CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_type(local.type_reference, &binders, &[])?,
            multiplicity: Multiplicity::Affine,
        },
        local.symbol,
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

pub(super) fn free_selected_operator_structural_signature(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    state: &psi_typed_trees::state::State,
    binders: &[(SymbolHandle, String)],
) -> Option<Vec<CheckedUnitStructuralParameterPlan>> {
    if !binders.is_empty() {
        return None;
    }
    let parameters = program
        .state_parameters(state)
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
            let type_identity = shapes.add_type(parameter.type_reference, binders, &[])?;
            let qualifications =
                parameter_qualifications(program, shapes, parameter.type_reference, binders)?;
            let multiplicity = crate::checks::type_multiplicity(program, parameter.type_reference);
            let access = structural_access_for_type_reference(program, parameter.type_reference)?;
            if multiplicity != Multiplicity::Affine
                || access != CheckedStructuralAccess::Owned
                || !qualifications.is_empty()
            {
                return None;
            }
            Some(CheckedUnitStructuralParameterPlan {
                position: u32::try_from(position).ok()?,
                is_self: false,
                type_identity,
                multiplicity,
                access,
                qualifications,
                fused_service_erasure: None,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    (!parameters.is_empty()).then_some(parameters)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_selected_operator_structural_scalar_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    source_machine: &psi_typed_trees::machine::Machine,
    source_state: &psi_typed_trees::state::State,
    structural_parameters: &mut [CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    application: &crate::SelectedOperatorApplication,
    result: CheckedUnitScalarResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    if source_machine.attached_data.is_some()
        || !program.machine_contracts(source_machine).is_empty()
        || !program.state_contracts(source_state).is_empty()
        || machine_has_content_evidence(facts, source_machine.symbol, source_state.symbol)
        || !entry_claims.is_empty()
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
    let realizations = facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .iter()
        .filter(|plan| {
            plan.machine == application.realization_machine
                && plan.state == application.realization_state
        })
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        return None;
    };
    if realization.scalar_parameters.len() + realization.structural_parameters.len()
        != application.operands.len()
        || realization.result_type != result.primitive_type
        || structural_parameters.len() != realization.structural_parameters.len()
    {
        return None;
    }
    let source_flow = state_flow(facts, source_machine.symbol, source_state.symbol)?;
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
    let source_parameters = program.state_parameters(source_state);
    let mut target_positions = realization
        .structural_parameters
        .iter()
        .map(|parameter| parameter.position)
        .chain(
            realization
                .scalar_parameters
                .iter()
                .map(|parameter| parameter.source_position),
        )
        .collect::<Vec<_>>();
    target_positions.sort_unstable();
    if target_positions
        != (0..application.operands.len())
            .map(|position| u32::try_from(position).ok())
            .collect::<Option<Vec<_>>>()?
    {
        return None;
    }
    let argument_source_positions = realization
        .structural_parameters
        .iter()
        .map(|target| {
            let operand = *application
                .operands
                .get(usize::try_from(target.position).ok()?)?;
            let ExpressionNode::Name(path) = program.expression_table.expression(operand) else {
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
    if argument_source_positions
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != structural_parameters.len()
    {
        return None;
    }

    for plan in &facts
        .flow
        .terminal_structural_scalar_returns
        .structural_types
    {
        if shapes
            .types
            .get(&plan.identity)
            .is_some_and(|existing| existing != plan)
        {
            return None;
        }
        shapes.types.insert(plan.identity.clone(), plan.clone());
    }

    let mut structural_arguments = Vec::with_capacity(realization.structural_parameters.len());
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
            || structural_parameters[source_index].multiplicity != Multiplicity::Affine
            || structural_parameters[source_index].access != CheckedStructuralAccess::Owned
            || !structural_parameters[source_index]
                .qualifications
                .is_empty()
            || target.is_self
            || target.multiplicity != Multiplicity::Affine
            || target.access != CheckedStructuralAccess::Owned
            || !target.qualifications.is_empty()
            || target.fused_service_erasure.is_some()
        {
            return None;
        }
        structural_parameters[source_index].type_identity = target.type_identity.clone();
        structural_arguments.push(CheckedUnitStructuralArgumentPlan {
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(source_index).ok()?,
            },
            path: Vec::new(),
            type_identity: target.type_identity.clone(),
            access: CheckedStructuralAccess::Owned,
        });
    }
    let scalar_arguments = realization
        .scalar_parameters
        .iter()
        .map(|target| {
            let operand = *application
                .operands
                .get(usize::try_from(target.source_position).ok()?)?;
            crate::values::lower_unit_scalar_argument(
                program,
                &facts.operators,
                source_state,
                usize::try_from(result.statement_index).ok()?,
                operand,
                target.primitive_type,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let contract = facts
        .contract_plans
        .for_machine(application.realization_machine)?;
    Some(
        CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
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
            service_reach: realization_flow.service_reach,
            scalar_arguments,
            structural_arguments,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_selected_operator_structural_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    source_machine: &psi_typed_trees::machine::Machine,
    source_state: &psi_typed_trees::state::State,
    structural_parameters: &mut [CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    application: &crate::SelectedOperatorApplication,
    mut result: CheckedUnitStructuralResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    if source_machine.attached_data.is_some()
        || !program.machine_contracts(source_machine).is_empty()
        || !program.state_contracts(source_state).is_empty()
        || machine_has_content_evidence(facts, source_machine.symbol, source_state.symbol)
        || !entry_claims.is_empty()
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
    let realization = facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .iter()
        .find(|plan| {
            plan.machine == application.realization_machine
                && plan.state == application.realization_state
        })?;
    if realization.scalar_parameters.len() + 1 != application.operands.len()
        || structural_parameters.len() != 1
        || result.multiplicity != Multiplicity::Affine
    {
        return None;
    }
    let source_flow = state_flow(facts, source_machine.symbol, source_state.symbol)?;
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
    for plan in &facts.flow.terminal_structural_returns.structural_types {
        if shapes
            .types
            .get(&plan.identity)
            .is_some_and(|existing| existing != plan)
        {
            return None;
        }
        shapes.types.insert(plan.identity.clone(), plan.clone());
    }
    let target = &realization.structural_parameter;
    let operand = *application
        .operands
        .get(usize::try_from(target.position).ok()?)?;
    let ExpressionNode::Name(path) = program.expression_table.expression(operand) else {
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
    let source_parameters = program.state_parameters(source_state);
    let source_position = source_parameters
        .iter()
        .position(|parameter| parameter.symbol == path.symbol)?;
    let source_index = structural_parameters
        .iter()
        .position(|parameter| parameter.position as usize == source_position)?;
    let source_shape = &shapes
        .types
        .get(&structural_parameters[source_index].type_identity)?
        .shape;
    let target_shape = &shapes.types.get(&target.type_identity)?.shape;
    let result_shape = &shapes.types.get(&realization.result.type_identity)?.shape;
    let local_result_shape = &shapes.types.get(&result.type_identity)?.shape;
    if source_shape != target_shape
        || result_shape != local_result_shape
        || structural_parameters[source_index].multiplicity != Multiplicity::Affine
        || structural_parameters[source_index].access != CheckedStructuralAccess::Owned
        || !structural_parameters[source_index]
            .qualifications
            .is_empty()
        || target.multiplicity != Multiplicity::Affine
        || target.access != CheckedStructuralAccess::Owned
        || !target.qualifications.is_empty()
        || realization.result.multiplicity != Multiplicity::Affine
        || !realization.result.qualifications.is_empty()
    {
        return None;
    }
    structural_parameters[source_index].type_identity = target.type_identity.clone();
    result.type_identity = realization.result.type_identity.clone();
    let structural_arguments = vec![CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
            parameter_index: u32::try_from(source_index).ok()?,
        },
        path: Vec::new(),
        type_identity: target.type_identity.clone(),
        access: CheckedStructuralAccess::Owned,
    }];
    let scalar_arguments = realization
        .scalar_parameters
        .iter()
        .map(|target| {
            let operand = *application
                .operands
                .get(usize::try_from(target.source_position).ok()?)?;
            crate::values::lower_unit_scalar_argument(
                program,
                &facts.operators,
                source_state,
                usize::try_from(result.statement_index).ok()?,
                operand,
                target.primitive_type,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let contract = facts
        .contract_plans
        .for_machine(application.realization_machine)?;
    Some(
        CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
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
            service_reach: realization_flow.service_reach,
            scalar_arguments,
            structural_arguments,
            discard_result_on_return: true,
        },
    )
}
