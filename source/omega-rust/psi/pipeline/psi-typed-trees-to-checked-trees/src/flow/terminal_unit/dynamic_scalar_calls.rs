//! Checked custody for scalar calls through local named dynamic values.
//!
//! This module is intentionally independent from Terminal Psi. It consumes
//! typed coordinates once, joins them to checked conformance, contract, value,
//! and service-reach facts, and publishes an all-or-nothing source-handle-free
//! roster for later checked-to-Terminal composition.

use super::*;

pub(super) fn build_checked_dynamic_dispatch_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
) -> psi_checked_trees::CheckedDynamicDispatchPlans {
    build_checked_dynamic_scalar_call_transaction(program, facts, shapes, boundaries)
        .unwrap_or_default()
}

fn build_checked_dynamic_scalar_call_transaction(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
) -> Option<psi_checked_trees::CheckedDynamicDispatchPlans> {
    let binding_facts = facts.dynamic_conformances.binding_facts();
    let mut plans = psi_checked_trees::CheckedDynamicDispatchPlans::default();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let flow = state_flow(facts, machine.symbol, state.symbol)?;
            for flow_call in facts.flow.control.calls.span_or_empty(flow.calls) {
                let call_site = crate::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    flow_call.statement_index,
                    flow_call.call_ordinal,
                )?;
                let Some(receiver_symbol) = local_receiver_symbol(program, &call_site) else {
                    continue;
                };
                let is_dynamic_receiver = binding_facts.selections.iter().any(|selection| {
                    selection.machine == machine.symbol
                        && selection.state == state.symbol
                        && selection.statement_index < flow_call.statement_index
                        && selection.binding == receiver_symbol
                });
                if !is_dynamic_receiver {
                    continue;
                }

                match build_checked_dynamic_scalar_call(
                    program,
                    facts,
                    &binding_facts,
                    machine,
                    state,
                    flow_call,
                    call_site,
                    shapes,
                    boundaries,
                )? {
                    CheckedDynamicScalarCall::Direct(plan) => {
                        plans.direct_scalar_calls.push(plan);
                    }
                    CheckedDynamicScalarCall::Rebound(plan) => {
                        plans.rebound_scalar_calls.push(plan);
                    }
                }
            }
        }
    }

    Some(plans)
}

enum CheckedDynamicScalarCall {
    Direct(psi_checked_trees::CheckedDynamicScalarCallPlan),
    Rebound(psi_checked_trees::CheckedReboundDynamicScalarCallPlan),
}

fn local_receiver_symbol(
    program: &TypedTrees,
    call_site: &crate::CallSite<'_>,
) -> Option<SymbolHandle> {
    match call_site {
        crate::CallSite::Expression { call, .. } => {
            let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver)
            else {
                return None;
            };
            let [_name] = program.expression_table.name_path_members(path.members) else {
                return None;
            };
            Some(path.symbol)
        }
        crate::CallSite::Statement(call) => {
            let [_name] = program.statement_table.name_path_members(call.receiver) else {
                return None;
            };
            Some(call.receiver_symbol)
        }
        crate::CallSite::TransitionNamed { .. } => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_checked_dynamic_scalar_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    binding_facts: &psi_checked_trees::DynamicConformanceBindingFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    flow_call: &psi_checked_trees::FlowCallFact,
    call_site: crate::CallSite<'_>,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
) -> Option<CheckedDynamicScalarCall> {
    let crate::CallSite::Expression { expression, call } = call_site else {
        return None;
    };
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(flow_call.statement_index).ok()?,
        call_ordinal: u32::try_from(flow_call.call_ordinal).ok()?,
    };
    if coordinate.call_ordinal != 0
        || !flow_call.has_receiver
        || !flow_call.receiver_symbol.is_valid()
        || !flow_call.target_symbol.is_valid()
        || call.static_requirement_dispatch.is_some()
        || !call.machine_arguments.is_empty()
        || !program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
        || !call.evidence_arguments.is_empty()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }

    let ExpressionNode::Name(receiver_path) = program.expression_table.expression(call.receiver)
    else {
        return None;
    };
    let [receiver_name] = program
        .expression_table
        .name_path_members(receiver_path.members)
    else {
        return None;
    };
    if !receiver_path.symbol.is_valid()
        || receiver_path.symbol != flow_call.receiver_symbol
        || program
            .expression_table
            .name_path_member_symbols(receiver_path.member_symbols)
            .iter()
            .any(|symbol| *symbol != receiver_path.symbol)
    {
        return None;
    }

    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(result_local) = statements.get(flow_call.statement_index)? else {
        return None;
    };
    if result_local.is_mutable
        || !result_local.symbol.is_valid()
        || result_local.initial_value != expression
    {
        return None;
    }
    let result_type = program.primitive_type_reference(result_local.type_reference)?;
    let result_binding_ordinal = statements[..flow_call.statement_index]
        .iter()
        .filter(|statement| {
            matches!(
                statement,
                StatementNode::LocalData(local)
                    if !local.is_mutable
                        && local.initial_value.is_valid()
                        && program.primitive_type_reference(local.type_reference).is_some()
            )
        })
        .count();
    let result = CheckedUnitScalarResultBindingPlan {
        statement_index: coordinate.statement_index,
        binding_ordinal: u32::try_from(result_binding_ordinal).ok()?,
        primitive_type: result_type,
    };

    let mut binding_selections = binding_facts
        .selections
        .iter()
        .filter(|selection| {
            selection.machine == machine.symbol
                && selection.state == state.symbol
                && selection.binding == receiver_path.symbol
                && selection.binding_name == *receiver_name
                && selection.statement_index < flow_call.statement_index
        })
        .collect::<Vec<_>>();
    binding_selections.sort_by_key(|selection| selection.statement_index);
    let (rebound_from, selection) = match binding_selections.as_slice() {
        [selection] => (None, *selection),
        [initial, rebound] => (Some(*initial), *rebound),
        _ => return None,
    };
    if binding_selections
        .windows(2)
        .any(|pair| pair[0].statement_index >= pair[1].statement_index)
    {
        return None;
    }
    let selection = selection.clone();
    let selected_conformance = selection.conformance.filter(|symbol| symbol.is_valid())?;

    let (source_parameter_position, caller_parameter_access, source_access) =
        checked_source_argument(program, facts, state, statements, &selection)?;
    let attachments = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == machine.attached_data_symbol)
        .collect::<Vec<_>>();
    let [attachment] = attachments.as_slice() else {
        return None;
    };
    let caller_attachment_type_identity =
        shapes.add_attached_data(attachment, &machine_binders(program, machine))?;
    let (source_field, source_path, source_type_identity) =
        checked_self_attachment_source(program, machine, &selection)?;
    let source_definitions = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == selection.source_data)
        .collect::<Vec<_>>();
    let [source_definition] = source_definitions.as_slice() else {
        return None;
    };
    let rebound_from = match rebound_from {
        Some(initial) => Some(checked_rebound_dynamic_selection(
            program,
            facts,
            machine,
            state,
            statements,
            flow_call.statement_index,
            initial,
            &selection,
            source_parameter_position,
            caller_parameter_access,
            source_access,
            &source_type_identity,
        )?),
        None => None,
    };
    let caller_structural_scalar_field_store = checked_caller_structural_scalar_field_store_plan(
        program,
        facts,
        machine,
        state,
        statements,
        coordinate,
        result_local.symbol,
        &selection,
        source_parameter_position,
        caller_parameter_access,
        source_field,
        &source_path,
        source_definition,
    );

    let target_traits = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == selection.target_trait)
        .collect::<Vec<_>>();
    let [target_trait] = target_traits.as_slice() else {
        return None;
    };
    let conformances = program
        .conformances()
        .iter()
        .filter(|conformance| conformance.symbol == selected_conformance)
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return None;
    };
    if conformance.trait_name != target_trait.name {
        return None;
    }

    let selected_rows = selection
        .rows
        .iter()
        .filter(|row| row.requirement == flow_call.target_symbol)
        .collect::<Vec<_>>();
    let [row] = selected_rows.as_slice() else {
        return None;
    };
    let row = (*row).clone();
    if row.requirement_identity.is_empty()
        || row.realization_identity.is_empty()
        || program.symbols.name(row.requirement) != call.target.as_str()
    {
        return None;
    }

    let declaring_traits = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == row.declaring_trait)
        .collect::<Vec<_>>();
    let [declaring_trait] = declaring_traits.as_slice() else {
        return None;
    };
    let requirements = program
        .trait_machine_signatures(declaring_trait)
        .iter()
        .filter(|requirement| requirement.symbol == row.requirement)
        .collect::<Vec<_>>();
    let [requirement] = requirements.as_slice() else {
        return None;
    };
    let [requirement_self] = program.state_signature_parameters(requirement) else {
        return None;
    };
    if program
        .normalized_trait_requirement_overload_identity(declaring_trait, requirement)
        .identity()
        != row.requirement_identity
        || program.primitive_type_reference(requirement.return_type) != Some(result_type)
        || !requirement_self.is_self
        || structural_access_for_type_reference(program, requirement_self.type_reference)
            != Some(source_access)
    {
        return None;
    }

    let closed_rows = program
        .closed_conformance_rows(conformance)
        .unwrap_or_default()
        .iter()
        .filter(|candidate| {
            candidate.declaring_trait == row.declaring_trait
                && candidate.requirement == row.requirement
                && candidate.realization_machine == row.realization_machine
                && candidate.realization_state == row.realization_state
        })
        .collect::<Vec<_>>();
    let [closed_row] = closed_rows.as_slice() else {
        return None;
    };
    let normalized = crate::facts::normalized_dynamic_row_identities(program, closed_row).ok()?;
    if normalized.0 != row.requirement_identity || normalized.1 != row.realization_identity {
        return None;
    }

    let realization_machines = program
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == row.realization_machine)
        .collect::<Vec<_>>();
    let [realization_machine] = realization_machines.as_slice() else {
        return None;
    };
    if realization_machine.supply_mode != MachineSupplyMode::CheckedBody
        || realization_machine.attached_data_symbol != selection.source_data
        || program
            .normalized_machine_overload_identity(realization_machine)?
            .identity()
            != row.realization_identity
    {
        return None;
    }
    let realization_states = program
        .machine_states(realization_machine)
        .iter()
        .filter(|candidate| candidate.symbol == row.realization_state)
        .collect::<Vec<_>>();
    let [realization_state] = realization_states.as_slice() else {
        return None;
    };
    let [realization_self] = program.state_parameters(realization_state) else {
        return None;
    };
    if program.primitive_type_reference(realization_state.return_type) != Some(result_type)
        || !realization_self.is_self
        || structural_access_for_type_reference(program, realization_self.type_reference)
            != Some(source_access)
    {
        return None;
    }
    let realization_return_expression = checked_realization_scalar_return_expression(
        program,
        facts,
        realization_machine,
        realization_state,
        result_type,
    )?;

    let contract = facts.contract_plans.for_machine(row.realization_machine)?;
    if contract.report_fingerprint == 0 || contract.commitment.is_zero() {
        return None;
    }
    let checked_call_service_reach =
        checked_call_service_reach(facts, state.symbol, flow_call, coordinate)?;
    let caller_contract = facts.contract_plans.for_machine(machine.symbol)?;
    if caller_contract.report_fingerprint == 0 || caller_contract.commitment.is_zero() {
        return None;
    }
    let caller_reach_fact = facts.service_reaches.for_machine(machine.symbol)?;
    let caller_service_reach = ServiceReachSummary {
        direct: caller_reach_fact.inferred_direct,
        transitive: caller_reach_fact.inferred_transitive,
    };

    let mut plan = psi_checked_trees::CheckedDynamicScalarCallPlan {
        caller_machine: machine.symbol,
        caller_state: state.symbol,
        caller_attachment_type_identity,
        caller_multiplicity: attachment.properties.multiplicity,
        caller_parameter_access,
        caller_contract_report_fingerprint: caller_contract.report_fingerprint,
        caller_contract_commitment: caller_contract.commitment,
        caller_service_reach,
        coordinate,
        result_binding: result_local.symbol,
        result,
        receiver_binding: receiver_path.symbol,
        selection,
        source_parameter_position,
        source_access,
        source_field,
        source_path,
        source_type_identity,
        source_multiplicity: source_definition.properties.multiplicity,
        target_trait: target_trait.symbol,
        selected_conformance,
        declaring_trait: row.declaring_trait,
        requirement: row.requirement,
        requirement_identity: row.requirement_identity.clone(),
        realization_machine: row.realization_machine,
        realization_state: row.realization_state,
        realization_identity: row.realization_identity.clone(),
        realization_return_expression,
        realization_contract_report_fingerprint: contract.report_fingerprint,
        realization_contract_commitment: contract.commitment,
        checked_call_service_reach,
        caller_structural_scalar_field_store,
        unit_continuation: None,
    };
    plan.unit_continuation = super::composed_control::build_direct_dynamic_unit_continuation(
        program, facts, shapes, boundaries, machine, state, &plan,
    );
    let retained_statement_count = usize::try_from(plan.coordinate.statement_index)
        .ok()?
        .checked_add(1)?;
    if plan.unit_continuation.is_none()
        && program
            .statement_table
            .statements(state.statement_nodes)
            .len()
            != retained_statement_count
    {
        return None;
    }
    Some(match rebound_from {
        Some(initial) => CheckedDynamicScalarCall::Rebound(
            psi_checked_trees::CheckedReboundDynamicScalarCallPlan {
                initial,
                latest: plan,
            },
        ),
        None => CheckedDynamicScalarCall::Direct(plan),
    })
}

#[allow(clippy::too_many_arguments)]
fn checked_rebound_dynamic_selection(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statements: &[StatementNode],
    call_statement_index: usize,
    initial: &psi_checked_trees::DynamicConformanceBindingFact,
    rebound: &psi_checked_trees::DynamicConformanceBindingFact,
    source_parameter_position: u32,
    caller_parameter_access: CheckedStructuralAccess,
    source_access: CheckedStructuralAccess,
    source_type_identity: &str,
) -> Option<psi_checked_trees::CheckedDynamicSelectionPlan> {
    if initial.statement_index.checked_add(1)? != rebound.statement_index
        || rebound.statement_index.checked_add(1)? != call_statement_index
        || initial.binding != rebound.binding
        || initial.binding_name != rebound.binding_name
        || initial.machine != rebound.machine
        || initial.state != rebound.state
        || initial.source_data != rebound.source_data
        || initial.target_trait != rebound.target_trait
        || initial.conformance != rebound.conformance
        || initial.rows != rebound.rows
    {
        return None;
    }
    let (initial_position, initial_caller_access, initial_source_access) =
        checked_source_argument(program, facts, state, statements, initial)?;
    if initial_position != source_parameter_position
        || initial_caller_access != caller_parameter_access
        || initial_source_access != source_access
    {
        return None;
    }
    let (source_field, source_path, initial_source_type_identity) =
        checked_self_attachment_source(program, machine, initial)?;
    if initial_source_type_identity != source_type_identity {
        return None;
    }
    Some(psi_checked_trees::CheckedDynamicSelectionPlan {
        fact: initial.clone(),
        field: source_field,
        path: source_path,
        type_identity: initial_source_type_identity,
    })
}

#[allow(clippy::too_many_arguments)]
fn checked_caller_structural_scalar_field_store_plan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    statements: &[StatementNode],
    call_coordinate: CheckedUnitCallCoordinate,
    result_binding: SymbolHandle,
    selection: &psi_checked_trees::DynamicConformanceBindingFact,
    destination_parameter_position: u32,
    caller_parameter_access: CheckedStructuralAccess,
    selected_carrier_field: SymbolHandle,
    selected_carrier_path: &[CheckedUnitStructuralPathSegment],
    source_definition: &psi_typed_trees::data::DataDefinition,
) -> Option<psi_checked_trees::CheckedStructuralScalarFieldStorePlan> {
    let [
        StatementNode::Assignment(assignment),
        StatementNode::LocalData(selection_local),
        StatementNode::LocalData(result_local),
    ] = statements.get(..3)?
    else {
        return None;
    };
    if selection.statement_index != 1
        || call_coordinate.statement_index != 2
        || call_coordinate.call_ordinal != 0
        || selection_local.symbol != selection.binding
        || result_local.symbol != result_binding
        || caller_parameter_access != CheckedStructuralAccess::MutableBorrow
    {
        return None;
    }

    let destination_parameter = program
        .state_parameters(state)
        .get(usize::try_from(destination_parameter_position).ok()?)?;
    let TypeReferenceNode::Reference { access, .. } = program
        .type_reference_table
        .type_reference(destination_parameter.type_reference)
    else {
        return None;
    };
    if !destination_parameter.is_self
        || destination_parameter.is_const
        || !destination_parameter.is_mutable
        || *access != psi_language_semantics::ReferenceAccess::Mutable
    {
        return None;
    }

    let destination = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        0,
        assignment.target,
    )?;
    let [
        psi_facts::PlaceSegment::Field {
            symbol: carrier_field,
        },
        psi_facts::PlaceSegment::Field {
            symbol: primitive_field,
        },
    ] = destination.segments.as_slice()
    else {
        return None;
    };
    if destination.root != psi_facts::PlaceRoot::Symbol(destination_parameter.symbol)
        || *carrier_field != selected_carrier_field
        || *carrier_field != selection.source_symbol
        || !primitive_field.is_valid()
    {
        return None;
    }

    let direct_fields = program
        .data_members(source_definition)
        .iter()
        .filter_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == *primitive_field).then_some(field)
        })
        .collect::<Vec<_>>();
    let [direct_field] = direct_fields.as_slice() else {
        return None;
    };
    let primitive_type = program.primitive_type_reference(direct_field.type_reference)?;
    if direct_field.relevance.is_erased() {
        return None;
    }

    let expected_mutation_path = crate::labels::canonical_place_label_from_parts(
        program,
        destination.root,
        &destination.segments,
    );
    let mutation_paths = facts
        .mutation
        .for_machine(machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == state.symbol)?
        .frame
        .complete_paths()?;
    if !matches!(mutation_paths, [path] if path == &expected_mutation_path) {
        return None;
    }

    let value = facts.values.scalar_expressions.expression_at(
        state.symbol,
        0,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    let direct_literal = matches!(value, CheckedScalarExpression::IntegerLiteral { .. })
        || matches!(
            value,
            CheckedScalarExpression::Boolean(expression)
                if matches!(
                    expression.as_ref(),
                    psi_checked_trees::CheckedBooleanExpression::Constant(_)
                )
        );
    if !direct_literal || crate::values::scalar_expression_type(value) != Some(primitive_type) {
        return None;
    }

    Some(psi_checked_trees::CheckedStructuralScalarFieldStorePlan {
        statement_index: 0,
        destination_parameter_position,
        carrier_path: selected_carrier_path.to_vec(),
        field_identity: terminal_field_identity(program, direct_field.symbol)?,
        primitive_type,
        value: value.clone(),
    })
}

fn checked_source_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &psi_typed_trees::state::State,
    statements: &[StatementNode],
    selection: &psi_checked_trees::DynamicConformanceBindingFact,
) -> Option<(u32, CheckedStructuralAccess, CheckedStructuralAccess)> {
    let self_parameters = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self)
        .collect::<Vec<_>>();
    let [(source_parameter_position, self_parameter)] = self_parameters.as_slice() else {
        return None;
    };
    let source_parameter_position = u32::try_from(*source_parameter_position).ok()?;
    let root_access = structural_access_for_type_reference(program, self_parameter.type_reference)?;
    if !matches!(
        root_access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) {
        return None;
    }

    let local_declarations = statements
        .iter()
        .take(selection.statement_index.saturating_add(1))
        .filter_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == selection.binding).then_some(local)
        })
        .collect::<Vec<_>>();
    let [local] = local_declarations.as_slice() else {
        return None;
    };
    let local_access = structural_access_for_type_reference(program, local.type_reference)?;
    if !matches!(
        local_access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) || (local_access == CheckedStructuralAccess::MutableBorrow
        && root_access != CheckedStructuralAccess::MutableBorrow)
    {
        return None;
    }

    let occurrence_facts = facts
        .dynamic_conformances
        .selections
        .iter()
        .filter(|candidate| {
            candidate.machine == selection.machine
                && candidate.state == selection.state
                && candidate.binding == selection.binding
                && candidate.statement_index == selection.statement_index
                && candidate.source_symbol == selection.source_symbol
                && candidate.source_data == selection.source_data
                && candidate.target_trait == selection.target_trait
                && candidate.conformance == selection.conformance
                && candidate.rows == selection.rows
        })
        .collect::<Vec<_>>();
    let [occurrence_fact] = occurrence_facts.as_slice() else {
        return None;
    };
    let ExpressionNode::Cast(cast) = program
        .expression_table
        .expression(occurrence_fact.occurrence)
    else {
        return None;
    };
    let TypeReferenceNode::DynamicTrait {
        symbol,
        conformance,
        ..
    } = program
        .type_reference_table
        .type_reference(cast.target_type)
    else {
        return None;
    };
    if *symbol != selection.target_trait || *conformance != selection.conformance {
        return None;
    }
    let selection_value = match statements.get(selection.statement_index)? {
        StatementNode::LocalData(local) if local.symbol == selection.binding => local.initial_value,
        StatementNode::Assignment(assignment) => assignment.value,
        _ => return None,
    };
    let ExpressionNode::Borrow(selection_borrow) =
        program.expression_table.expression(selection_value)
    else {
        return None;
    };
    let cast_access = match selection_borrow.access {
        psi_language_semantics::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        psi_language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        psi_language_semantics::ReferenceAccess::WriteOnly => {
            CheckedStructuralAccess::WriteOnlyBorrow
        }
    };
    if selection_borrow.target != occurrence_fact.occurrence || cast_access != local_access {
        return None;
    }
    let source_place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        selection.statement_index,
        cast.value,
    )?;
    if source_place.root != psi_facts::PlaceRoot::Symbol(self_parameter.symbol)
        || source_place.segments
            != [psi_facts::PlaceSegment::Field {
                symbol: selection.source_symbol,
            }]
    {
        return None;
    }

    Some((source_parameter_position, root_access, cast_access))
}

fn checked_self_attachment_source(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    selection: &psi_checked_trees::DynamicConformanceBindingFact,
) -> Option<(SymbolHandle, Vec<CheckedUnitStructuralPathSegment>, String)> {
    let [self_name, field_name] = selection.source_path.as_slice() else {
        return None;
    };
    if self_name.as_str() != "self"
        || field_name != &selection.source_name
        || !machine.attached_data_symbol.is_valid()
        || !selection.source_symbol.is_valid()
        || !selection.source_data.is_valid()
    {
        return None;
    }
    let attachments = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == machine.attached_data_symbol)
        .collect::<Vec<_>>();
    let [attachment] = attachments.as_slice() else {
        return None;
    };
    let fields = program
        .data_members(attachment)
        .iter()
        .filter_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == selection.source_symbol).then_some(field)
        })
        .collect::<Vec<_>>();
    let [field] = fields.as_slice() else {
        return None;
    };
    if field.name != *field_name || field.relevance.is_erased() {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        return None;
    };
    if *symbol != selection.source_data {
        return None;
    }
    let field_identity = terminal_field_identity(program, field.symbol)?;
    let source_type_identity = program
        .normalized_type_identity(field.type_reference)
        .into_string();
    (!source_type_identity.is_empty()).then_some((
        field.symbol,
        vec![CheckedUnitStructuralPathSegment::Field(field_identity)],
        source_type_identity,
    ))
}

fn checked_realization_scalar_return_expression(
    program: &TypedTrees,
    facts: &CheckFacts,
    realization_machine: &psi_typed_trees::machine::Machine,
    realization_state: &psi_typed_trees::state::State,
    result_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let [statement] = program
        .statement_table
        .statements(realization_state.statement_nodes)
    else {
        return None;
    };
    let (expression, value_role) = match statement {
        StatementNode::Expression(expression) => (
            *expression,
            psi_checked_trees::CheckedValueStatementRole::Expression,
        ),
        StatementNode::Transition(transition)
            if transition.exit == TransitionExit::Ordinary
                && transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid() =>
        {
            let TransitionTargetNode::Value(expression) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            (
                *expression,
                psi_checked_trees::CheckedValueStatementRole::TransitionTargetValue,
            )
        }
        _ => return None,
    };
    let checked_values = facts
        .values
        .expression_values(expression)
        .filter(|(_, value)| {
            value.origin
                == psi_checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: realization_machine.symbol,
                    state_symbol: realization_state.symbol,
                    statement_index: 0,
                    role: value_role,
                }
                && value.primitive_type == Some(result_type)
        })
        .collect::<Vec<_>>();
    let [_checked_value] = checked_values.as_slice() else {
        return None;
    };

    if let Some(checked) = facts.values.scalar_expressions.expression_at(
        realization_state.symbol,
        0,
        CheckedScalarExpressionRole::Return,
    ) {
        return Some(checked.clone());
    }

    checked_direct_self_field_return(
        program,
        realization_machine,
        realization_state,
        expression,
        result_type,
    )
}

fn checked_direct_self_field_return(
    program: &TypedTrees,
    realization_machine: &psi_typed_trees::machine::Machine,
    realization_state: &psi_typed_trees::state::State,
    expression: psi_typed_trees::expression::ExpressionHandle,
    result_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let self_parameters = program
        .state_parameters(realization_state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self)
        .collect::<Vec<_>>();
    let [(self_position, self_parameter)] = self_parameters.as_slice() else {
        return None;
    };
    if !realization_machine.attached_data_symbol.is_valid() {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        realization_state.symbol,
        0,
        expression,
    )?;
    let [
        psi_facts::PlaceSegment::Field {
            symbol: field_symbol,
        },
    ] = place.segments.as_slice()
    else {
        return None;
    };
    if place.root != psi_facts::PlaceRoot::Symbol(self_parameter.symbol) || !field_symbol.is_valid()
    {
        return None;
    }
    let attachment = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == realization_machine.attached_data_symbol)
        .collect::<Vec<_>>();
    let [attachment] = attachment.as_slice() else {
        return None;
    };
    let fields = program
        .data_members(attachment)
        .iter()
        .filter_map(|candidate| {
            let psi_typed_trees::data::DataMember::Field(field) = candidate else {
                return None;
            };
            (field.symbol == *field_symbol).then_some(field)
        })
        .collect::<Vec<_>>();
    let [field] = fields.as_slice() else {
        return None;
    };
    if field.relevance.is_erased()
        || program.primitive_type_reference(field.type_reference) != Some(result_type)
    {
        return None;
    }
    let parameter_position = u32::try_from(*self_position).ok()?;
    let path = vec![
        psi_checked_trees::CheckedStructuralPredicatePathSegment::Field(terminal_field_identity(
            program,
            field.symbol,
        )?),
    ];
    Some(if result_type == PrimitiveType::Bool {
        CheckedScalarExpression::Boolean(Box::new(
            psi_checked_trees::CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            },
        ))
    } else {
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type: result_type,
        }
    })
}

fn checked_call_service_reach(
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    flow_call: &psi_checked_trees::FlowCallFact,
    coordinate: CheckedUnitCallCoordinate,
) -> Option<psi_language_semantics::ServiceReachSummary> {
    let state = facts.service_reaches.for_state(caller_state)?;
    let calls = facts
        .service_reaches
        .calls_for(state)
        .iter()
        .filter(|call| {
            u32::try_from(call.statement_index).ok() == Some(coordinate.statement_index)
                && u32::try_from(call.call_ordinal).ok() == Some(coordinate.call_ordinal)
                && call.target_state == flow_call.target_symbol
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        return None;
    };
    let summary = psi_language_semantics::ServiceReachSummary {
        direct: call.inferred_direct,
        transitive: call.inferred_transitive,
    };
    (summary == flow_call.service_reach).then_some(summary)
}
