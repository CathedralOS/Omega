//! Building one checked dynamic scalar call with its realization callables
//! and source arguments.

use super::realization_callables::{
    checked_dynamic_realization_callables, dynamic_family_realization, dynamic_family_tuple,
};
use crate::execution::terminal_unit::dynamic_scalar_calls::forwarded_calls::{
    ForwardedDynamicCall, forwarded_transfer_path_is_exact,
};
use crate::execution::terminal_unit::dynamic_scalar_calls::realization_bodies::{
    checked_call_service_reach, checked_realization_scalar_body,
};
use crate::execution::terminal_unit::dynamic_scalar_calls::receivers::{
    CheckedDynamicScalarCall, dynamic_receiver_place,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedStructuralAccess, CheckedUnitCallCoordinate, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralPathSegment, ExpressionNode, MachineSupplyMode, ServiceReachSummary,
    ShapeCollector, StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees, machine_binders,
    structural_access_for_type_reference, terminal_field_identity,
};
use typed_trees::name::Identifier;
use typed_trees::type_identity::TypeIdentityRequest;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_checked_dynamic_scalar_call(
    program: &TypedTrees,
    facts: &CheckFacts,
    binding_facts: &checked_trees::DynamicConformanceBindingFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    flow_call: &checked_trees::FlowCallFact,
    call_site: crate::semantic_calls::CallSite<'_>,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    forwarded: Option<ForwardedDynamicCall<'_, '_>>,
    stored: Option<&checked_trees::DynamicDescriptorStorageFact>,
) -> Option<CheckedDynamicScalarCall> {
    let crate::semantic_calls::CallSite::Expression {
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
        .and_then(|forwarded| forwarded.transfer.sole_selection().cloned());
    let forwarding_transfers = forwarded
        .as_ref()
        .map(|forwarded| forwarded.prior_transfers.clone())
        .unwrap_or_default();
    let forwarding_helpers = forwarded
        .as_ref()
        .map(|forwarded| forwarded.helpers.clone())
        .unwrap_or_default();
    let (
        dispatch_state,
        dispatch_flow_call,
        dispatch_call,
        selection_binding,
        selection_name,
        origin,
    ) = match forwarded {
        Some(forwarded) => {
            if stored.is_some() {
                return None;
            }
            let crate::semantic_calls::CallSite::Expression { call, .. } = forwarded.call_site
            else {
                return None;
            };
            if !forwarded_transfer_path_is_exact(&forwarded) {
                return None;
            }
            (
                forwarded.state,
                forwarded.flow_call,
                call,
                forwarded.transfer.source_binding,
                forwarded.transfer.sole_selection()?.binding_name.clone(),
                checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
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
        None => {
            let (selection_binding, selection_name) = stored
                .map(|storage| {
                    (
                        storage.selection.binding,
                        storage.selection.binding_name.clone(),
                    )
                })
                .unwrap_or((flow_call.receiver_symbol, Identifier::default()));
            (
                state,
                flow_call,
                caller_call,
                selection_binding,
                selection_name,
                checked_trees::CheckedDynamicScalarCallOrigin::Local,
            )
        }
    };
    if coordinate.call_ordinal != 0
        || !dispatch_flow_call.has_receiver
        || !dispatch_flow_call.receiver_symbol.is_valid()
        || !dispatch_flow_call.target_symbol.is_valid()
        || dispatch_call.static_requirement_dispatch.is_some()
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

    let receiver_place = dynamic_receiver_place(program, dispatch_call.receiver)?;
    let receiver_name = receiver_place.path.last()?;
    let expected_selection_name = if selection_name.as_str().is_empty() {
        receiver_name
    } else {
        &selection_name
    };
    match stored {
        Some(storage) => {
            if receiver_place.root != storage.destination_binding
                || (receiver_place.leaf.is_valid()
                    && receiver_place.leaf != storage.destination_field)
                || receiver_place.path != storage.destination_path
                || storage.destination_field != dispatch_flow_call.receiver_symbol
                || storage.selection.binding != selection_binding
            {
                return None;
            }
        }
        None => {
            if receiver_place.path.len() != 1
                || receiver_place.leaf != dispatch_flow_call.receiver_symbol
                || receiver_place.root != receiver_place.leaf
            {
                return None;
            }
        }
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
    if forwarding_helpers.iter().any(|helper| {
        helper.call_result.primitive_type != result_type
            || helper.scalar_control.primitive_type != result_type
    }) {
        return None;
    }
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

    let realization_machines = program
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == row.realization_machine)
        .collect::<Vec<_>>();
    let [row_realization_machine] = realization_machines.as_slice() else {
        return None;
    };
    if row_realization_machine.supply_mode != MachineSupplyMode::CheckedBody
        || row_realization_machine.attached_data_symbol != selection.source_data
        || program
            .normalized_machine_overload_identity(row_realization_machine)?
            .identity()
            != row.realization_identity
    {
        return None;
    }
    let realization_states = program
        .machine_states(row_realization_machine)
        .iter()
        .filter(|candidate| candidate.symbol == row.realization_state)
        .collect::<Vec<_>>();
    let [row_realization_state] = realization_states.as_slice() else {
        return None;
    };
    let (realization_machine, realization_state, realization_identity) =
        dynamic_family_realization(
            program,
            row_realization_machine,
            row_realization_state,
            row.realization_identity.clone(),
            &family_tuple,
        )?;
    if realization_machine.supply_mode != MachineSupplyMode::CheckedBody
        || realization_machine.attached_data_symbol != selection.source_data
    {
        return None;
    }
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

    let contract = facts
        .contract_plans
        .for_machine(realization_machine.symbol)?;
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

    let mut plan = checked_trees::CheckedDynamicScalarCallPlan {
        origin,
        forwarding_transfers,
        forwarding_helpers,
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
        realization_machine: realization_machine.symbol,
        realization_state: realization_state.symbol,
        realization_identity,
        family_tuple,
        realization_return_expression: realization_body.return_expression,
        realization_structural_scalar_field_stores: realization_body.structural_scalar_field_stores,
        realization_callables,
        realization_contract_report_fingerprint: contract.report_fingerprint,
        realization_contract_commitment: contract.commitment,
        checked_call_service_reach,
        caller_structural_scalar_field_store,
        unit_continuation: None,
    };
    plan.unit_continuation =
        crate::execution::terminal_unit::composed_control::build_direct_dynamic_unit_continuation(
            program, facts, shapes, boundaries, machine, state, &plan, stored,
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
    if let Some(storage) = stored {
        if rebound_from.is_some() || storage.selection != plan.selection {
            return None;
        }
        let StatementNode::LocalData(destination) = statements.get(storage.statement_index)? else {
            return None;
        };
        if destination.symbol != storage.destination_binding {
            return None;
        }
        let destination_type_identity = program
            .type_identity(TypeIdentityRequest {
                binders: &machine_binders(program, machine),
                ..TypeIdentityRequest::ordinary(destination.type_reference)
            })
            .into_string();
        let destination_field_identity =
            terminal_field_identity(program, storage.destination_field)?;
        return Some(CheckedDynamicScalarCall::Stored(
            checked_trees::CheckedStoredDynamicScalarCallPlan {
                storage: storage.clone(),
                destination_type_identity,
                destination_field_identity,
                call: plan,
            },
        ));
    }
    Some(match rebound_from {
        Some(initial) => {
            CheckedDynamicScalarCall::Rebound(checked_trees::CheckedReboundDynamicScalarCallPlan {
                initial,
                latest: plan,
            })
        }
        None => CheckedDynamicScalarCall::Direct(plan),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn checked_rebound_dynamic_selection(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    call_statement_index: usize,
    initial: &checked_trees::DynamicConformanceBindingFact,
    rebound: &checked_trees::DynamicConformanceBindingFact,
    source_parameter_position: u32,
    caller_parameter_access: CheckedStructuralAccess,
    source_access: CheckedStructuralAccess,
    source_type_identity: &str,
) -> Option<checked_trees::CheckedDynamicSelectionPlan> {
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
    Some(checked_trees::CheckedDynamicSelectionPlan {
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
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    call_coordinate: CheckedUnitCallCoordinate,
    result_binding: SymbolHandle,
    selection: &checked_trees::DynamicConformanceBindingFact,
    destination_parameter_position: u32,
    caller_parameter_access: CheckedStructuralAccess,
    selected_carrier_field: SymbolHandle,
    selected_carrier_path: &[CheckedUnitStructuralPathSegment],
    source_definition: &typed_trees::data::DataDefinition,
) -> Option<checked_trees::CheckedStructuralScalarFieldStorePlan> {
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
        || *access != language_semantics::ReferenceAccess::Mutable
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
        facts::PlaceSegment::Field {
            symbol: carrier_field,
        },
        facts::PlaceSegment::Field {
            symbol: primitive_field,
        },
    ] = destination.segments.as_slice()
    else {
        return None;
    };
    if destination.root != facts::PlaceRoot::Symbol(destination_parameter.symbol)
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
            let typed_trees::data::DataMember::Field(field) = member else {
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
                    checked_trees::CheckedBooleanExpression::Constant(_)
                )
        );
    if !direct_literal || crate::values::scalar_expression_type(value) != Some(primitive_type) {
        return None;
    }

    Some(checked_trees::CheckedStructuralScalarFieldStorePlan {
        statement_index: 0,
        destination: checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter {
            position: destination_parameter_position,
        },
        carrier_path: selected_carrier_path.to_vec(),
        field_identity: terminal_field_identity(program, direct_field.symbol)?,
        primitive_type,
        value: checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(value.clone()),
    })
}

pub(crate) fn checked_source_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    selection: &checked_trees::DynamicConformanceBindingFact,
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
        language_semantics::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        language_semantics::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_semantics::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
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
    if source_place.root != facts::PlaceRoot::Symbol(self_parameter.symbol)
        || source_place.segments
            != [facts::PlaceSegment::Field {
                symbol: selection.source_symbol,
            }]
    {
        return None;
    }

    Some((source_parameter_position, root_access, cast_access))
}

pub(crate) fn checked_self_attachment_source(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    selection: &checked_trees::DynamicConformanceBindingFact,
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
            let typed_trees::data::DataMember::Field(field) = member else {
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
