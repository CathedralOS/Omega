//! Checked custody for calls through local named dynamic values.
//!
//! Scalar-result calls remain in this owner. Unit-returning statement calls
//! use the focused `unit` child so they cannot acquire a fabricated result
//! carrier by sharing the scalar path.
//!
//! This module is intentionally independent from Terminal Psi. It consumes
//! typed coordinates once, joins them to checked conformance, contract, value,
//! and service-reach facts, and publishes an all-or-nothing source-handle-free
//! roster for later checked-to-Terminal composition.

use super::*;
use psi_typed_trees::name::Identifier;

mod unit;

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
    plans.transfers = build_checked_dynamic_descriptor_transfers(program, facts, &binding_facts);

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

                match &call_site {
                    crate::CallSite::Statement(_) => {
                        match unit::build_checked_dynamic_unit_call(
                            program,
                            facts,
                            &binding_facts,
                            machine,
                            state,
                            flow_call,
                            call_site,
                            shapes,
                            None,
                        )? {
                            unit::CheckedDynamicUnitCall::Direct(plan) => {
                                plans.direct_unit_calls.push(plan);
                            }
                            unit::CheckedDynamicUnitCall::Rebound(plan) => {
                                plans.rebound_unit_calls.push(plan);
                            }
                        }
                    }
                    _ => match build_checked_dynamic_scalar_call(
                        program,
                        facts,
                        &binding_facts,
                        machine,
                        state,
                        flow_call,
                        call_site,
                        shapes,
                        boundaries,
                        None,
                    )? {
                        CheckedDynamicScalarCall::Direct(plan) => {
                            plans.direct_scalar_calls.push(plan);
                        }
                        CheckedDynamicScalarCall::Rebound(plan) => {
                            plans.rebound_scalar_calls.push(plan);
                        }
                    },
                }
            }
        }
    }

    build_checked_forwarded_dynamic_scalar_calls(
        program,
        facts,
        shapes,
        boundaries,
        &binding_facts,
        &mut plans,
    )?;
    unit::build_checked_forwarded_dynamic_unit_calls(
        program,
        facts,
        shapes,
        &binding_facts,
        &mut plans,
    )?;

    Some(plans)
}

fn build_checked_dynamic_descriptor_transfers(
    program: &TypedTrees,
    facts: &CheckFacts,
    binding_facts: &psi_checked_trees::DynamicConformanceBindingFacts,
) -> Vec<psi_checked_trees::CheckedDynamicDescriptorTransferPlan> {
    let mut transfers = Vec::new();
    for caller in program.machines() {
        for caller_state in program.machine_states(caller) {
            let Some(flow) = state_flow(facts, caller.symbol, caller_state.symbol) else {
                continue;
            };
            for call in facts.flow.control.calls.span_or_empty(flow.calls) {
                let Some(call_site) = crate::find_call_site(
                    program,
                    caller.symbol,
                    caller_state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    continue;
                };
                let Some(target_state) = crate::find_state(program, call.target_symbol) else {
                    continue;
                };
                let Some(target_machine) = program.machines().iter().find(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .any(|state| state.symbol == target_state.symbol)
                }) else {
                    continue;
                };
                let arguments = crate::call_site_argument_expressions(program, &call_site);
                let parameters = program
                    .state_parameters(target_state)
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .collect::<Vec<_>>();
                if arguments.len() != parameters.len() {
                    continue;
                }
                for (parameter_position, (parameter, argument)) in
                    parameters.into_iter().zip(arguments).enumerate()
                {
                    let Some(target_trait) =
                        bare_dynamic_parameter_trait(program, parameter.type_reference)
                    else {
                        continue;
                    };
                    let ExpressionNode::Name(source_path) =
                        program.expression_table.expression(*argument)
                    else {
                        continue;
                    };
                    let [source_name] = program
                        .expression_table
                        .name_path_members(source_path.members)
                    else {
                        continue;
                    };
                    let mut selections = binding_facts
                        .selections
                        .iter()
                        .filter(|selection| {
                            selection.machine == caller.symbol
                                && selection.state == caller_state.symbol
                                && selection.binding == source_path.symbol
                                && selection.binding_name == *source_name
                                && selection.statement_index < call.statement_index
                                && selection.target_trait == target_trait
                        })
                        .collect::<Vec<_>>();
                    selections.sort_by_key(|selection| selection.statement_index);
                    let Some(selection) = selections.last() else {
                        continue;
                    };
                    transfers.push(psi_checked_trees::CheckedDynamicDescriptorTransferPlan {
                        caller_machine: caller.symbol,
                        caller_state: caller_state.symbol,
                        coordinate: CheckedUnitCallCoordinate {
                            statement_index: match u32::try_from(call.statement_index) {
                                Ok(index) => index,
                                Err(_) => continue,
                            },
                            call_ordinal: match u32::try_from(call.call_ordinal) {
                                Ok(ordinal) => ordinal,
                                Err(_) => continue,
                            },
                        },
                        target_machine: target_machine.symbol,
                        target_state: target_state.symbol,
                        parameter_position: match u32::try_from(parameter_position) {
                            Ok(position) => position,
                            Err(_) => continue,
                        },
                        parameter: parameter.symbol,
                        target_trait,
                        source_binding: source_path.symbol,
                        selection: (*selection).clone(),
                    });
                }
            }
        }
    }
    transfers.sort_by_key(|transfer| {
        (
            transfer.caller_machine.arena_index(),
            transfer.caller_machine.generation(),
            transfer.caller_state.arena_index(),
            transfer.caller_state.generation(),
            transfer.coordinate.statement_index,
            transfer.coordinate.call_ordinal,
            transfer.parameter_position,
        )
    });
    transfers
}

fn bare_dynamic_parameter_trait(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            bare_dynamic_parameter_trait(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            bare_dynamic_parameter_trait(program, *base_type)
        }
        TypeReferenceNode::DynamicTrait {
            symbol,
            conformance: None,
            ..
        } if symbol.is_valid() => Some(*symbol),
        _ => None,
    }
}

fn build_checked_forwarded_dynamic_scalar_calls(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    binding_facts: &psi_checked_trees::DynamicConformanceBindingFacts,
    plans: &mut psi_checked_trees::CheckedDynamicDispatchPlans,
) -> Option<()> {
    for machine in program.machines() {
        if !machine.attached_data_symbol.is_valid() {
            continue;
        }
        for state in program.machine_states(machine) {
            let Some(flow) = state_flow(facts, machine.symbol, state.symbol) else {
                continue;
            };
            for outer_call in facts.flow.control.calls.span_or_empty(flow.calls) {
                if outer_call.has_receiver || outer_call.call_ordinal != 0 {
                    continue;
                }
                let Ok(statement_index) = u32::try_from(outer_call.statement_index) else {
                    continue;
                };
                let Ok(call_ordinal) = u32::try_from(outer_call.call_ordinal) else {
                    continue;
                };
                let coordinate = CheckedUnitCallCoordinate {
                    statement_index,
                    call_ordinal,
                };
                let matching_transfers = plans
                    .transfers
                    .iter()
                    .filter(|transfer| {
                        transfer.caller_machine == machine.symbol
                            && transfer.caller_state == state.symbol
                            && transfer.coordinate == coordinate
                    })
                    .collect::<Vec<_>>();
                let [transfer] = matching_transfers.as_slice() else {
                    continue;
                };
                let transfer = (*transfer).clone();
                let Some(outer_site) = crate::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    outer_call.statement_index,
                    outer_call.call_ordinal,
                ) else {
                    continue;
                };
                let crate::CallSite::Expression { call, .. } = &outer_site else {
                    continue;
                };
                if program
                    .expression_table
                    .expression_handles(call.arguments)
                    .len()
                    != 1
                {
                    continue;
                }

                let Some(target_state) = crate::find_state(program, transfer.target_state) else {
                    continue;
                };
                let Some(target_machine) = program.machines().iter().find(|candidate| {
                    candidate.symbol == transfer.target_machine
                        && program
                            .machine_states(candidate)
                            .iter()
                            .any(|candidate_state| candidate_state.symbol == target_state.symbol)
                }) else {
                    continue;
                };
                let [parameter] = program.state_parameters(target_state) else {
                    continue;
                };
                if parameter.is_self
                    || parameter.is_const
                    || !parameter.symbol.is_valid()
                    || parameter.symbol != transfer.parameter
                    || transfer.parameter_position != 0
                    || !program.state_contracts(target_state).is_empty()
                {
                    continue;
                }
                let helper_statements = program
                    .statement_table
                    .statements(target_state.statement_nodes);
                let [
                    StatementNode::LocalData(helper_result),
                    StatementNode::Transition(ret),
                ] = helper_statements
                else {
                    continue;
                };
                let TransitionTargetNode::Value(return_value) =
                    program.statement_table.transition_target(ret.target)
                else {
                    continue;
                };
                let ExpressionNode::Name(return_path) =
                    program.expression_table.expression(*return_value)
                else {
                    continue;
                };
                let [return_name] = program
                    .expression_table
                    .name_path_members(return_path.members)
                else {
                    continue;
                };
                if helper_result.is_mutable
                    || !helper_result.symbol.is_valid()
                    || ret.exit != TransitionExit::Ordinary
                    || ret.guard != TransitionGuardNode::Always
                    || ret.continuation.is_valid()
                    || return_path.symbol != helper_result.symbol
                    || return_name != &helper_result.name
                {
                    continue;
                }
                let Some(helper_flow) =
                    state_flow(facts, target_machine.symbol, target_state.symbol)
                else {
                    continue;
                };
                let helper_calls = facts.flow.control.calls.span_or_empty(helper_flow.calls);
                let [inner_call] = helper_calls else {
                    continue;
                };
                if inner_call.statement_index != 0
                    || inner_call.call_ordinal != 0
                    || inner_call.receiver_symbol != parameter.symbol
                    || !inner_call.has_receiver
                {
                    continue;
                }
                let Some(inner_site) = crate::find_call_site(
                    program,
                    target_machine.symbol,
                    target_state.symbol,
                    inner_call.statement_index,
                    inner_call.call_ordinal,
                ) else {
                    continue;
                };
                let forwarded = ForwardedDynamicCall {
                    machine: target_machine,
                    state: target_state,
                    flow_call: inner_call,
                    call_site: inner_site,
                    transfer,
                };
                let Some(plan) = build_checked_dynamic_scalar_call(
                    program,
                    facts,
                    binding_facts,
                    machine,
                    state,
                    outer_call,
                    outer_site,
                    shapes,
                    boundaries,
                    Some(forwarded),
                ) else {
                    continue;
                };
                match plan {
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
    Some(())
}

struct ForwardedDynamicCall<'program, 'facts> {
    machine: &'program psi_typed_trees::machine::Machine,
    state: &'program psi_typed_trees::state::State,
    flow_call: &'facts psi_checked_trees::FlowCallFact,
    call_site: crate::CallSite<'program>,
    transfer: psi_checked_trees::CheckedDynamicDescriptorTransferPlan,
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
    forwarded: Option<ForwardedDynamicCall<'_, '_>>,
) -> Option<CheckedDynamicScalarCall> {
    let crate::CallSite::Expression {
        expression: caller_expression,
        call: caller_call,
    } = call_site
    else {
        return None;
    };
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(flow_call.statement_index).ok()?,
        call_ordinal: u32::try_from(flow_call.call_ordinal).ok()?,
    };
    let forwarded_selection = forwarded
        .as_ref()
        .map(|forwarded| forwarded.transfer.selection.clone());
    let (
        dispatch_state,
        dispatch_flow_call,
        dispatch_call,
        selection_binding,
        selection_name,
        origin,
    ) = match forwarded {
        Some(forwarded) => {
            let crate::CallSite::Expression { call, .. } = forwarded.call_site else {
                return None;
            };
            if forwarded.transfer.target_machine != forwarded.machine.symbol
                || forwarded.transfer.target_state != forwarded.state.symbol
                || forwarded.transfer.parameter != forwarded.flow_call.receiver_symbol
            {
                return None;
            }
            (
                forwarded.state,
                forwarded.flow_call,
                call,
                forwarded.transfer.source_binding,
                forwarded.transfer.selection.binding_name.clone(),
                psi_checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
                    machine: forwarded.machine.symbol,
                    state: forwarded.state.symbol,
                    coordinate: CheckedUnitCallCoordinate {
                        statement_index: u32::try_from(forwarded.flow_call.statement_index).ok()?,
                        call_ordinal: u32::try_from(forwarded.flow_call.call_ordinal).ok()?,
                    },
                    parameter: forwarded.flow_call.receiver_symbol,
                },
            )
        }
        None => (
            state,
            flow_call,
            caller_call,
            flow_call.receiver_symbol,
            Identifier::default(),
            psi_checked_trees::CheckedDynamicScalarCallOrigin::Local,
        ),
    };
    if coordinate.call_ordinal != 0
        || !dispatch_flow_call.has_receiver
        || !dispatch_flow_call.receiver_symbol.is_valid()
        || !dispatch_flow_call.target_symbol.is_valid()
        || dispatch_call.static_requirement_dispatch.is_some()
        || !dispatch_call.machine_arguments.is_empty()
        || !program
            .expression_table
            .expression_handles(dispatch_call.arguments)
            .is_empty()
        || !dispatch_call.evidence_arguments.is_empty()
        || dispatch_call.quotient_operation.is_some()
        || dispatch_call.private_layout_operation.is_some()
    {
        return None;
    }

    let ExpressionNode::Name(receiver_path) =
        program.expression_table.expression(dispatch_call.receiver)
    else {
        return None;
    };
    let [receiver_name] = program
        .expression_table
        .name_path_members(receiver_path.members)
    else {
        return None;
    };
    let expected_selection_name = if selection_name.as_str().is_empty() {
        receiver_name
    } else {
        &selection_name
    };
    if !receiver_path.symbol.is_valid()
        || receiver_path.symbol != dispatch_flow_call.receiver_symbol
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
        || result_local.initial_value != caller_expression
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
                && selection.binding == selection_binding
                && selection.binding_name == *expected_selection_name
                && selection.statement_index < flow_call.statement_index
        })
        .collect::<Vec<_>>();
    binding_selections.sort_by_key(|selection| selection.statement_index);
    let (rebound_from, selection) = match binding_selections.as_slice() {
        [selection] => (None, *selection),
        [initial, rebound] => (Some(*initial), *rebound),
        _ => return None,
    };
    if let Some(forwarded_selection) = forwarded_selection.as_ref()
        && selection != forwarded_selection
    {
        return None;
    }
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
        .filter(|row| row.requirement == dispatch_flow_call.target_symbol)
        .collect::<Vec<_>>();
    let [row] = selected_rows.as_slice() else {
        return None;
    };
    let row = (*row).clone();
    if row.requirement_identity.is_empty()
        || row.realization_identity.is_empty()
        || program.symbols.name(row.requirement) != dispatch_call.target.as_str()
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
    let realization_body = checked_realization_scalar_body(
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
    let realization_callables = checked_dynamic_realization_callables(
        program,
        facts,
        conformance,
        &selection,
        source_access,
    )?;
    let dispatch_coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(dispatch_flow_call.statement_index).ok()?,
        call_ordinal: u32::try_from(dispatch_flow_call.call_ordinal).ok()?,
    };
    let checked_call_service_reach = checked_call_service_reach(
        facts,
        dispatch_state.symbol,
        dispatch_flow_call,
        dispatch_coordinate,
    )?;
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
        origin,
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
        receiver_binding: selection_binding,
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
        realization_return_expression: realization_body.return_expression,
        realization_structural_scalar_field_store: realization_body.structural_scalar_field_store,
        realization_callables,
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

fn checked_dynamic_realization_callables(
    program: &TypedTrees,
    facts: &CheckFacts,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    selection: &psi_checked_trees::DynamicConformanceBindingFact,
    source_access: psi_checked_trees::CheckedStructuralAccess,
) -> Option<Vec<psi_checked_trees::CheckedDynamicRealizationCallablePlan>> {
    let closed_rows = program.closed_conformance_rows(conformance)?;
    if closed_rows.len() != selection.rows.len() {
        return None;
    }
    closed_rows
        .iter()
        .zip(&selection.rows)
        .map(|(closed, retained)| {
            if closed.declaring_trait != retained.declaring_trait
                || closed.requirement != retained.requirement
                || closed.realization_machine != retained.realization_machine
                || closed.realization_state != retained.realization_state
            {
                return None;
            }
            let (requirement_identity, realization_identity) =
                crate::facts::normalized_dynamic_row_identities(program, closed).ok()?;
            if requirement_identity != retained.requirement_identity
                || realization_identity != retained.realization_identity
            {
                return None;
            }
            let declaring_trait = program
                .traits()
                .iter()
                .find(|definition| definition.symbol == closed.declaring_trait)?;
            let requirement = program
                .trait_machine_signatures(declaring_trait)
                .iter()
                .find(|candidate| candidate.symbol == closed.requirement)?;
            let realization_machine = program
                .machines()
                .iter()
                .find(|candidate| candidate.symbol == closed.realization_machine)?;
            let realization_state = program
                .machine_states(realization_machine)
                .iter()
                .find(|candidate| candidate.symbol == closed.realization_state)?;
            let [requirement_self] = program.state_signature_parameters(requirement) else {
                return None;
            };
            let [realization_self] = program.state_parameters(realization_state) else {
                return None;
            };
            let result_type = program.primitive_type_reference(requirement.return_type)?;
            if !matches!(result_type, PrimitiveType::Bool | PrimitiveType::I32)
                || program.primitive_type_reference(realization_state.return_type)
                    != Some(result_type)
                || !requirement_self.is_self
                || !realization_self.is_self
                || structural_access_for_type_reference(program, requirement_self.type_reference)
                    != Some(source_access)
                || structural_access_for_type_reference(program, realization_self.type_reference)
                    != Some(source_access)
                || realization_machine.supply_mode != MachineSupplyMode::CheckedBody
                || realization_machine.attached_data_symbol != selection.source_data
            {
                return None;
            }
            let body = checked_realization_scalar_body(
                program,
                facts,
                realization_machine,
                realization_state,
                result_type,
            )?;
            let contract = facts
                .contract_plans
                .for_machine(closed.realization_machine)?;
            if contract.report_fingerprint == 0 || contract.commitment.is_zero() {
                return None;
            }
            Some(psi_checked_trees::CheckedDynamicRealizationCallablePlan {
                declaring_trait: closed.declaring_trait,
                requirement: closed.requirement,
                requirement_identity,
                realization_machine: closed.realization_machine,
                realization_state: closed.realization_state,
                realization_identity,
                result_type,
                structural_scalar_field_store: body.structural_scalar_field_store,
                return_expression: body.return_expression,
                contract_report_fingerprint: contract.report_fingerprint,
                contract_commitment: contract.commitment,
            })
        })
        .collect()
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
        || initial.conformance.is_none()
        || rebound.conformance.is_none()
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

struct CheckedRealizationScalarBody {
    structural_scalar_field_store: Option<psi_checked_trees::CheckedStructuralScalarFieldStorePlan>,
    return_expression: CheckedScalarExpression,
}

fn checked_realization_scalar_body(
    program: &TypedTrees,
    facts: &CheckFacts,
    realization_machine: &psi_typed_trees::machine::Machine,
    realization_state: &psi_typed_trees::state::State,
    result_type: PrimitiveType,
) -> Option<CheckedRealizationScalarBody> {
    let statements = program
        .statement_table
        .statements(realization_state.statement_nodes);
    let (structural_scalar_field_store, return_statement_index, statement) = match statements {
        [statement] => (None, 0, statement),
        [StatementNode::Assignment(assignment), statement] => (
            Some(checked_realization_structural_scalar_field_store_plan(
                program,
                facts,
                realization_machine,
                realization_state,
                assignment,
            )?),
            1,
            statement,
        ),
        _ => return None,
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
                    statement_index: return_statement_index,
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
        u32::try_from(return_statement_index).ok()?,
        CheckedScalarExpressionRole::Return,
    ) {
        return Some(CheckedRealizationScalarBody {
            structural_scalar_field_store,
            return_expression: checked.clone(),
        });
    }

    let return_expression = checked_direct_self_field_return(
        program,
        realization_machine,
        realization_state,
        return_statement_index,
        expression,
        result_type,
    )?;
    Some(CheckedRealizationScalarBody {
        structural_scalar_field_store,
        return_expression,
    })
}

fn checked_realization_structural_scalar_field_store_plan(
    program: &TypedTrees,
    facts: &CheckFacts,
    realization_machine: &psi_typed_trees::machine::Machine,
    realization_state: &psi_typed_trees::state::State,
    assignment: &psi_typed_trees::statement::TableAssignment,
) -> Option<psi_checked_trees::CheckedStructuralScalarFieldStorePlan> {
    let self_parameters = program
        .state_parameters(realization_state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self)
        .collect::<Vec<_>>();
    let [(self_position, self_parameter)] = self_parameters.as_slice() else {
        return None;
    };
    let TypeReferenceNode::Reference { access, .. } = program
        .type_reference_table
        .type_reference(self_parameter.type_reference)
    else {
        return None;
    };
    if self_parameter.is_const
        || !self_parameter.is_mutable
        || *access != psi_language_semantics::ReferenceAccess::Mutable
        || !realization_machine.attached_data_symbol.is_valid()
    {
        return None;
    }

    let destination = crate::flow::canonical_place_from_expression_in_state(
        program,
        realization_state.symbol,
        0,
        assignment.target,
    )?;
    if destination.root != psi_facts::PlaceRoot::Symbol(self_parameter.symbol) {
        return None;
    }

    let attachments = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == realization_machine.attached_data_symbol)
        .collect::<Vec<_>>();
    let [attachment] = attachments.as_slice() else {
        return None;
    };
    let (carrier_path, field_symbol, field_owner) = match destination.segments.as_slice() {
        [psi_facts::PlaceSegment::Field { symbol }] if symbol.is_valid() => {
            (Vec::new(), *symbol, *attachment)
        }
        [
            psi_facts::PlaceSegment::Field {
                symbol: carrier_symbol,
            },
            psi_facts::PlaceSegment::Field {
                symbol: primitive_symbol,
            },
        ] if carrier_symbol.is_valid() && primitive_symbol.is_valid() => {
            let carrier_fields = program
                .data_members(attachment)
                .iter()
                .filter_map(|candidate| {
                    let psi_typed_trees::data::DataMember::Field(field) = candidate else {
                        return None;
                    };
                    (field.symbol == *carrier_symbol).then_some(field)
                })
                .collect::<Vec<_>>();
            let [carrier_field] = carrier_fields.as_slice() else {
                return None;
            };
            if carrier_field.relevance.is_erased() {
                return None;
            }
            let nested = crate::field_domain::data_definition_for_field_type(
                program,
                carrier_field.type_reference,
            )?;
            (
                vec![CheckedUnitStructuralPathSegment::Field(
                    terminal_field_identity(program, carrier_field.symbol)?,
                )],
                *primitive_symbol,
                nested,
            )
        }
        _ => return None,
    };
    let fields = program
        .data_members(field_owner)
        .iter()
        .filter_map(|candidate| {
            let psi_typed_trees::data::DataMember::Field(field) = candidate else {
                return None;
            };
            (field.symbol == field_symbol).then_some(field)
        })
        .collect::<Vec<_>>();
    let [field] = fields.as_slice() else {
        return None;
    };
    let primitive_type = program.primitive_type_reference(field.type_reference)?;
    if field.relevance.is_erased()
        || !matches!(primitive_type, PrimitiveType::Bool | PrimitiveType::I32)
    {
        return None;
    }

    let expected_mutation_path = crate::labels::canonical_place_label_from_parts(
        program,
        destination.root,
        &destination.segments,
    );
    let mutation_paths = facts
        .mutation
        .for_machine(realization_machine.symbol)?
        .state_write_frames
        .iter()
        .find(|frame| frame.state == realization_state.symbol)?
        .frame
        .complete_paths()?;
    if !matches!(mutation_paths, [path] if path == &expected_mutation_path) {
        return None;
    }

    let value = facts.values.scalar_expressions.expression_at(
        realization_state.symbol,
        0,
        CheckedScalarExpressionRole::AssignmentValue,
    )?;
    let direct_literal = matches!(value, CheckedScalarExpression::IntegerLiteral { .. })
        || matches!(
            value,
            CheckedScalarExpression::Boolean(expression)
                if matches!(expression.as_ref(), CheckedBooleanExpression::Constant(_))
        );
    if !direct_literal || crate::values::scalar_expression_type(value) != Some(primitive_type) {
        return None;
    }

    Some(psi_checked_trees::CheckedStructuralScalarFieldStorePlan {
        statement_index: 0,
        destination_parameter_position: u32::try_from(*self_position).ok()?,
        carrier_path,
        field_identity: terminal_field_identity(program, field.symbol)?,
        primitive_type,
        value: value.clone(),
    })
}

fn checked_direct_self_field_return(
    program: &TypedTrees,
    realization_machine: &psi_typed_trees::machine::Machine,
    realization_state: &psi_typed_trees::state::State,
    statement_index: usize,
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
        statement_index,
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
