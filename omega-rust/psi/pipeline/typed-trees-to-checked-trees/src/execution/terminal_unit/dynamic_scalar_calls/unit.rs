//! Checked custody for terminal Unit-returning calls through local descriptors.
//!
//! This is deliberately separate from the scalar-result lane. The first rung
//! admits one argument-free Unit requirement with an operation-free checked
//! realization at the end of an attached Unit state, with either one selection
//! or one exact same-conformance reassignment. It publishes no fabricated
//! result carrier.
use super::realization_callables::{
    checked_dynamic_realization_callables, dynamic_family_realization, dynamic_family_tuple,
};
use super::{
    CheckFacts, CheckedUnitCallCoordinate, Identifier, MachineSupplyMode, ServiceReachSummary,
    StatementNode, TypedTrees,
};
use crate::execution::terminal_unit::dynamic_scalar_calls::forwarded_calls::{
    ForwardedDynamicCall, forwarded_transfer_path_is_exact,
};
use crate::execution::terminal_unit::dynamic_scalar_calls::realization_bodies::checked_call_service_reach;
use crate::execution::terminal_unit::dynamic_scalar_calls::scalar_call_plans::{
    checked_rebound_dynamic_selection, checked_self_attachment_source, checked_source_argument,
};
use crate::execution::terminal_unit::types::{
    ShapeCollector, is_unit, machine_binders, structural_access_for_type_reference,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn build_checked_dynamic_unit_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    binding_facts: &checked_trees::DynamicConformanceBindingFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    flow_call: &checked_trees::FlowCallFact,
    call_site: crate::semantic::calls::CallSite<'_>,
    shapes: &mut ShapeCollector<'_>,
    forwarded: Option<ForwardedDynamicCall<'_, '_>>,
) -> Option<checked_trees::CheckedDynamicBinding<checked_trees::CheckedDynamicUnitCallPlan>> {
    let crate::semantic::calls::CallSite::Statement(caller_call) = call_site else {
        return None;
    };
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: u32::try_from(flow_call.statement_index).ok()?,
        call_ordinal: u32::try_from(flow_call.call_ordinal).ok()?,
    };
    let forwarded_selection = forwarded
        .as_ref()
        .and_then(|forwarded| forwarded.transfer.sole_selection().cloned());
    let forwarding_transfers = forwarded
        .as_ref()
        .map(|forwarded| forwarded.prior_transfers.clone())
        .unwrap_or_default();
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
            let crate::semantic::calls::CallSite::Statement(call) = forwarded.call_site else {
                return None;
            };
            if !forwarded_transfer_path_is_exact(&forwarded) {
                return None;
            }
            let [parameter] = program.state_parameters(forwarded.state) else {
                return None;
            };
            let forwarded_parameter = forwarded
                .prior_transfers
                .last()
                .map(|transfer| transfer.parameter)
                .unwrap_or(forwarded.transfer.parameter);
            if parameter.is_self || parameter.is_const || parameter.symbol != forwarded_parameter {
                return None;
            }
            (
                forwarded.state,
                forwarded.flow_call,
                call,
                forwarded.transfer.source_binding,
                forwarded.transfer.sole_selection()?.binding_name.clone(),
                Some(parameter.type_reference),
                checked_trees::CheckedDynamicUnitCallOrigin::Forwarded {
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
            checked_trees::CheckedDynamicUnitCallOrigin::Local,
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
    let family_tuple =
        dynamic_family_tuple(program, requirement, &dispatch_call.machine_arguments)?;

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

    let row_realization_machine =
        crate::lookup::machine_by_symbol(program, row.realization_machine)?;
    let row_realization_state = program
        .machine_states(row_realization_machine)
        .iter()
        .find(|candidate| candidate.symbol == row.realization_state)?;
    if row_realization_machine.supply_mode != MachineSupplyMode::CheckedBody
        || row_realization_machine.attached_data_symbol != selection.source_data
        || program
            .normalized_machine_overload_identity(row_realization_machine)?
            .identity()
            != row.realization_identity
    {
        return None;
    }
    let (realization_machine, realization_state, realization_identity) =
        dynamic_family_realization(
            program,
            row_realization_machine,
            row_realization_state,
            row.realization_identity.clone(),
            &family_tuple,
        )?;
    let [realization_self] = program.state_parameters(realization_state) else {
        return None;
    };
    if realization_machine.supply_mode != MachineSupplyMode::CheckedBody
        || realization_machine.attached_data_symbol != selection.source_data
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

    let contract = facts
        .contract_plans
        .for_machine(realization_machine.symbol)?;
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

    let plan = checked_trees::CheckedDynamicUnitCallPlan {
        origin,
        forwarding_transfers,
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
        realization_machine: realization_machine.symbol,
        realization_state: realization_state.symbol,
        realization_identity,
        family_tuple,
        realization_callables,
        realization_contract_report_fingerprint: contract.report_fingerprint,
        realization_contract_commitment: contract.commitment,
        checked_call_service_reach,
    };
    Some(match rebound_from {
        Some(initial) => checked_trees::CheckedDynamicBinding::Rebound {
            initial,
            latest: plan,
        },
        None => checked_trees::CheckedDynamicBinding::Direct(plan),
    })
}
