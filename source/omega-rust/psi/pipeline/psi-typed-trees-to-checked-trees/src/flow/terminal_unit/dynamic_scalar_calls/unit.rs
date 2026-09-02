//! Checked custody for terminal Unit-returning calls through local descriptors.
//!
//! This is deliberately separate from the scalar-result lane. The first rung
//! admits one argument-free Unit requirement with an operation-free checked
//! realization at the end of an attached Unit state, with either one selection
//! or one exact same-conformance reassignment. It publishes no fabricated
//! result carrier.

use super::*;

pub(super) enum CheckedDynamicUnitCall {
    Direct(psi_checked_trees::CheckedDynamicUnitCallPlan),
    Rebound(psi_checked_trees::CheckedReboundDynamicUnitCallPlan),
}

pub(super) struct ForwardedDynamicUnitCall<'program, 'facts> {
    pub machine: &'program psi_typed_trees::machine::Machine,
    pub state: &'program psi_typed_trees::state::State,
    pub flow_call: &'facts psi_checked_trees::FlowCallFact,
    pub call_site: crate::CallSite<'program>,
    pub transfer: psi_checked_trees::CheckedDynamicDescriptorTransferPlan,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_checked_dynamic_unit_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    binding_facts: &psi_checked_trees::DynamicConformanceBindingFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    flow_call: &psi_checked_trees::FlowCallFact,
    call_site: crate::CallSite<'_>,
    shapes: &mut ShapeCollector<'_>,
    forwarded: Option<ForwardedDynamicUnitCall<'_, '_>>,
) -> Option<CheckedDynamicUnitCall> {
    let crate::CallSite::Statement(caller_call) = call_site else {
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
        forwarded_parameter_type,
        origin,
    ) = match forwarded {
        Some(forwarded) => {
            let crate::CallSite::Statement(call) = forwarded.call_site else {
                return None;
            };
            if forwarded.transfer.target_machine != forwarded.machine.symbol
                || forwarded.transfer.target_state != forwarded.state.symbol
                || forwarded.transfer.parameter != forwarded.flow_call.receiver_symbol
            {
                return None;
            }
            let [parameter] = program.state_parameters(forwarded.state) else {
                return None;
            };
            if parameter.is_self
                || parameter.is_const
                || parameter.symbol != forwarded.transfer.parameter
            {
                return None;
            }
            (
                forwarded.state,
                forwarded.flow_call,
                call,
                forwarded.transfer.source_binding,
                forwarded.transfer.selection.binding_name.clone(),
                Some(parameter.type_reference),
                psi_checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
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
            None,
            psi_checked_trees::CheckedDynamicUnitCallOrigin::Local,
        ),
    };
    if coordinate.call_ordinal != 0
        || dispatch_flow_call.call_ordinal != 0
        || !dispatch_flow_call.has_receiver
        || !dispatch_flow_call.receiver_symbol.is_valid()
        || !dispatch_flow_call.target_symbol.is_valid()
        || dispatch_call.receiver_symbol != dispatch_flow_call.receiver_symbol
        || dispatch_call.target_symbol != dispatch_flow_call.target_symbol
        || dispatch_call.static_requirement_dispatch.is_some()
        || !dispatch_call.machine_arguments.is_empty()
        || !program
            .statement_table
            .expression_handles(dispatch_call.arguments)
            .is_empty()
        || !dispatch_call.evidence_arguments.is_empty()
        || dispatch_call.discards_result
    {
        return None;
    }
    let [receiver_name] = program
        .statement_table
        .name_path_members(dispatch_call.receiver)
    else {
        return None;
    };
    let expected_selection_name = if selection_name.as_str().is_empty() {
        receiver_name
    } else {
        &selection_name
    };

    let statements = program.statement_table.statements(state.statement_nodes);
    if !matches!(statements.get(flow_call.statement_index), Some(StatementNode::Call(candidate)) if std::ptr::eq(candidate, caller_call))
        || statements.len() != flow_call.statement_index.checked_add(1)?
    {
        return None;
    }

    let mut selections = binding_facts
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
    selections.sort_by_key(|selection| selection.statement_index);
    if selections
        .windows(2)
        .any(|pair| pair[0].statement_index >= pair[1].statement_index)
    {
        return None;
    }
    let (rebound_from, selection) = match selections.as_slice() {
        [selection] => (None, *selection),
        [initial, rebound] => (Some(*initial), *rebound),
        _ => return None,
    };
    if let Some(forwarded_selection) = forwarded_selection.as_ref()
        && selection != forwarded_selection
    {
        return None;
    }
    let selection = selection.clone();
    let selected_conformance = selection.conformance.filter(|symbol| symbol.is_valid())?;

    let (source_parameter_position, caller_parameter_access, source_access) =
        checked_source_argument(program, facts, state, statements, &selection)?;
    if forwarded_parameter_type.is_some_and(|type_reference| {
        structural_access_for_type_reference(program, type_reference) != Some(source_access)
    }) {
        return None;
    }
    let attachment = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == machine.attached_data_symbol)?;
    let caller_attachment_type_identity =
        shapes.add_attached_data(attachment, &machine_binders(program, machine))?;
    let (source_field, source_path, source_type_identity) =
        checked_self_attachment_source(program, machine, &selection)?;
    let source_definition = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == selection.source_data)?;
    // Changed-conformance rebinding is first admitted by the scalar-result lane.
    // Keep Unit plans fail-closed until their distinct application custody is
    // represented and verified end to end.
    if let Some(initial) = rebound_from
        && (initial.conformance != selection.conformance || initial.rows != selection.rows)
    {
        return None;
    }
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

    let target_trait = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == selection.target_trait)?;
    let conformance = program
        .conformances()
        .iter()
        .find(|candidate| candidate.symbol == selected_conformance)?;
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

    let declaring_trait = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == row.declaring_trait)?;
    let requirement = program
        .trait_machine_signatures(declaring_trait)
        .iter()
        .find(|candidate| candidate.symbol == row.requirement)?;
    let [requirement_self] = program.state_signature_parameters(requirement) else {
        return None;
    };
    if program
        .normalized_trait_requirement_overload_identity(declaring_trait, requirement)
        .identity()
        != row.requirement_identity
        || !is_unit(program, requirement.return_type)
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

    let realization_machine = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == row.realization_machine)?;
    let realization_state = program
        .machine_states(realization_machine)
        .iter()
        .find(|candidate| candidate.symbol == row.realization_state)?;
    let [realization_self] = program.state_parameters(realization_state) else {
        return None;
    };
    if realization_machine.supply_mode != MachineSupplyMode::CheckedBody
        || realization_machine.attached_data_symbol != selection.source_data
        || program
            .normalized_machine_overload_identity(realization_machine)?
            .identity()
            != row.realization_identity
        || !is_unit(program, realization_state.return_type)
        || !realization_self.is_self
        || structural_access_for_type_reference(program, realization_self.type_reference)
            != Some(source_access)
        || !program
            .statement_table
            .statements(realization_state.statement_nodes)
            .is_empty()
        || !program.state_contracts(realization_state).is_empty()
    {
        return None;
    }

    let contract = facts.contract_plans.for_machine(row.realization_machine)?;
    let realization_callables = checked_dynamic_unit_realization_callables(
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
    let caller_reach_fact = facts.service_reaches.for_machine(machine.symbol)?;
    let caller_service_reach = ServiceReachSummary {
        direct: caller_reach_fact.inferred_direct,
        transitive: caller_reach_fact.inferred_transitive,
    };
    let reach_rows = &facts.service_reaches.rows;
    if contract.report_fingerprint == 0
        || contract.commitment.is_zero()
        || caller_contract.report_fingerprint == 0
        || caller_contract.commitment.is_zero()
        || !reach_rows
            .services(checked_call_service_reach.direct)
            .is_empty()
        || !reach_rows
            .services(checked_call_service_reach.transitive)
            .is_empty()
        || !reach_rows.services(caller_service_reach.direct).is_empty()
        || !reach_rows
            .services(caller_service_reach.transitive)
            .is_empty()
    {
        return None;
    }

    let plan = psi_checked_trees::CheckedDynamicUnitCallPlan {
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
        realization_callables,
        realization_contract_report_fingerprint: contract.report_fingerprint,
        realization_contract_commitment: contract.commitment,
        checked_call_service_reach,
    };
    Some(match rebound_from {
        Some(initial) => {
            CheckedDynamicUnitCall::Rebound(psi_checked_trees::CheckedReboundDynamicUnitCallPlan {
                initial,
                latest: plan,
            })
        }
        None => CheckedDynamicUnitCall::Direct(plan),
    })
}

pub(super) fn build_checked_forwarded_dynamic_unit_calls(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
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
                let coordinate = CheckedUnitCallCoordinate {
                    statement_index: u32::try_from(outer_call.statement_index).ok()?,
                    call_ordinal: u32::try_from(outer_call.call_ordinal).ok()?,
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
                let crate::CallSite::Statement(call) = &outer_site else {
                    continue;
                };
                if call.static_requirement_dispatch.is_some()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.discards_result
                    || program
                        .statement_table
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
                    || !is_unit(program, target_state.return_type)
                    || !program.state_contracts(target_state).is_empty()
                {
                    continue;
                }
                let [StatementNode::Call(helper_call)] = program
                    .statement_table
                    .statements(target_state.statement_nodes)
                else {
                    continue;
                };
                let Some(helper_flow) =
                    state_flow(facts, target_machine.symbol, target_state.symbol)
                else {
                    continue;
                };
                let [inner_call] = facts.flow.control.calls.span_or_empty(helper_flow.calls) else {
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
                let crate::CallSite::Statement(inner_statement_call) = &inner_site else {
                    continue;
                };
                if !std::ptr::eq(*inner_statement_call, helper_call) {
                    continue;
                }
                let forwarded = ForwardedDynamicUnitCall {
                    machine: target_machine,
                    state: target_state,
                    flow_call: inner_call,
                    call_site: inner_site,
                    transfer,
                };
                let Some(call) = build_checked_dynamic_unit_call(
                    program,
                    facts,
                    binding_facts,
                    machine,
                    state,
                    outer_call,
                    outer_site,
                    shapes,
                    Some(forwarded),
                ) else {
                    continue;
                };
                match call {
                    CheckedDynamicUnitCall::Direct(plan) => plans.direct_unit_calls.push(plan),
                    CheckedDynamicUnitCall::Rebound(plan) => plans.rebound_unit_calls.push(plan),
                }
            }
        }
    }
    Some(())
}

fn checked_dynamic_unit_realization_callables(
    program: &TypedTrees,
    facts: &CheckFacts,
    conformance: &psi_typed_trees::trait_definition::Conformance,
    selection: &psi_checked_trees::DynamicConformanceBindingFact,
    source_access: CheckedStructuralAccess,
) -> Option<Vec<psi_checked_trees::CheckedDynamicUnitRealizationCallablePlan>> {
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
            if !is_unit(program, requirement.return_type)
                || !is_unit(program, realization_state.return_type)
                || !requirement_self.is_self
                || !realization_self.is_self
                || structural_access_for_type_reference(program, requirement_self.type_reference)
                    != Some(source_access)
                || structural_access_for_type_reference(program, realization_self.type_reference)
                    != Some(source_access)
                || realization_machine.supply_mode != MachineSupplyMode::CheckedBody
                || realization_machine.attached_data_symbol != selection.source_data
                || !program
                    .statement_table
                    .statements(realization_state.statement_nodes)
                    .is_empty()
                || !program.state_contracts(realization_state).is_empty()
            {
                return None;
            }
            let contract = facts
                .contract_plans
                .for_machine(closed.realization_machine)?;
            if contract.report_fingerprint == 0 || contract.commitment.is_zero() {
                return None;
            }
            Some(
                psi_checked_trees::CheckedDynamicUnitRealizationCallablePlan {
                    declaring_trait: closed.declaring_trait,
                    requirement: closed.requirement,
                    requirement_identity,
                    realization_machine: closed.realization_machine,
                    realization_state: closed.realization_state,
                    realization_identity,
                    contract_report_fingerprint: contract.report_fingerprint,
                    contract_commitment: contract.commitment,
                },
            )
        })
        .collect()
}
