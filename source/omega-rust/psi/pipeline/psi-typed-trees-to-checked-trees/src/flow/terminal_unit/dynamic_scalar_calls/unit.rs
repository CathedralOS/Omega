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
) -> Option<CheckedDynamicUnitCall> {
    let crate::CallSite::Statement(call) = call_site else {
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
        || call.receiver_symbol != flow_call.receiver_symbol
        || call.target_symbol != flow_call.target_symbol
        || call.static_requirement_dispatch.is_some()
        || !call.machine_arguments.is_empty()
        || !program
            .statement_table
            .expression_handles(call.arguments)
            .is_empty()
        || !call.evidence_arguments.is_empty()
        || call.discards_result
    {
        return None;
    }
    let [receiver_name] = program.statement_table.name_path_members(call.receiver) else {
        return None;
    };

    let statements = program.statement_table.statements(state.statement_nodes);
    if !matches!(statements.get(flow_call.statement_index), Some(StatementNode::Call(candidate)) if std::ptr::eq(candidate, call))
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
                && selection.binding == call.receiver_symbol
                && selection.binding_name == *receiver_name
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
    let selection = selection.clone();
    let selected_conformance = selection.conformance.filter(|symbol| symbol.is_valid())?;

    let (source_parameter_position, caller_parameter_access, source_access) =
        checked_source_argument(program, facts, state, statements, &selection)?;
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
    let checked_call_service_reach =
        checked_call_service_reach(facts, state.symbol, flow_call, coordinate)?;
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
        caller_machine: machine.symbol,
        caller_state: state.symbol,
        caller_attachment_type_identity,
        caller_multiplicity: attachment.properties.multiplicity,
        caller_parameter_access,
        caller_contract_report_fingerprint: caller_contract.report_fingerprint,
        caller_contract_commitment: caller_contract.commitment,
        caller_service_reach,
        coordinate,
        receiver_binding: selection.binding,
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
