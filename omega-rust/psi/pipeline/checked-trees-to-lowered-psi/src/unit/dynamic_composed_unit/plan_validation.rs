//! Validation of the exact direct, rebound, stored and forwarded dynamic
//! plans before lowering.

use crate::proofs::evidence_lowering;
use crate::unit::dynamic_composed_unit::applications::{
    count_selected_family_rows, exact_machine_service_summary, validate_empty_contract,
    validate_empty_service_summary,
};
use crate::unit::dynamic_composed_unit::dynamic_lanes::DynamicCallerShape;
use crate::unit::{CheckedTrees, LoweringError, unsupported};
use checked_trees::TypeIdentityRequest;
use checked_trees::{
    CheckedDynamicScalarCallPlan, CheckedDynamicSelectionPlan, CheckedStructuralAccess,
    CheckedUnitStructuralPathSegment,
};
use language_semantics::{Multiplicity, ServiceReachSummary};

pub(crate) fn validate_exact_direct_plan(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
) -> Result<DynamicCallerShape, LoweringError> {
    let store = plan.caller_structural_scalar_field_store.as_ref();
    if store.is_some() && plan.unit_continuation.is_some() {
        return unsupported(
            "direct dynamic result control cannot also retain a caller field store",
        );
    }
    let selection_statement_index = usize::from(store.is_some());
    let call_statement_index = u32::from(store.is_some()) + 1;
    validate_exact_dynamic_plan(
        checked,
        plan,
        selection_statement_index,
        call_statement_index,
        None,
    )
}

pub(crate) fn validate_exact_rebound_plan(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    initial: &CheckedDynamicSelectionPlan,
) -> Result<DynamicCallerShape, LoweringError> {
    if plan.caller_structural_scalar_field_store.is_some()
        || initial.fact.statement_index.checked_add(1) != Some(plan.selection.statement_index)
        || plan.selection.statement_index.checked_add(1)
            != usize::try_from(plan.coordinate.statement_index).ok()
        || initial.fact.machine != plan.caller_machine
        || initial.fact.state != plan.caller_state
        || initial.fact.binding != plan.receiver_binding
        || initial.fact.target_trait != plan.target_trait
        || initial.fact.conformance.is_none()
        || initial.fact.source_symbol != initial.field
        || initial.fact.source_data != plan.selection.source_data
        || initial.type_identity != plan.source_type_identity
        || initial.path.len() != 1
        || checked
            .facts
            .dynamic_conformances
            .binding_facts()
            .selections
            .into_iter()
            .filter(|selection| selection == &initial.fact)
            .count()
            != 1
    {
        return unsupported("rebound dynamic selection versions drifted from checked custody");
    }
    validate_exact_dynamic_plan(
        checked,
        plan,
        plan.selection.statement_index,
        plan.coordinate.statement_index,
        None,
    )
}

pub(crate) fn validate_exact_stored_plan(
    checked: &CheckedTrees,
    stored: &checked_trees::CheckedStoredDynamicScalarCallPlan,
) -> Result<DynamicCallerShape, LoweringError> {
    let plan = &stored.call;
    let machines = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| machine.symbol == plan.caller_machine)
        .collect::<Vec<_>>();
    let [machine] = machines.as_slice() else {
        return unsupported("stored dynamic descriptor drifted from checked aggregate custody");
    };
    let states = checked
        .typed
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == plan.caller_state)
        .collect::<Vec<_>>();
    let [state] = states.as_slice() else {
        return unsupported("stored dynamic descriptor drifted from checked aggregate custody");
    };
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    let Some(checked_trees::statement::StatementNode::LocalData(destination)) =
        statements.get(stored.storage.statement_index)
    else {
        return unsupported("stored dynamic descriptor drifted from checked aggregate custody");
    };
    let fields = checked
        .typed
        .data_definitions()
        .iter()
        .flat_map(|definition| checked.typed.data_members(definition))
        .filter_map(|member| {
            let checked_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == stored.storage.destination_field).then_some(field)
        })
        .collect::<Vec<_>>();
    let [field] = fields.as_slice() else {
        return unsupported("stored dynamic descriptor drifted from checked aggregate custody");
    };
    let binders = checked
        .typed
        .machine_type_parameters(machine)
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.symbol, format!("$T{index}")))
        .collect::<Vec<_>>();
    let destination_type_identity = checked
        .typed
        .type_identity(TypeIdentityRequest {
            binders: &binders,
            ..TypeIdentityRequest::ordinary(destination.type_reference)
        })
        .into_string();
    let destination_field_identity = field
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| field.name.as_str().to_owned());
    let exact_storages = checked
        .facts
        .dynamic_conformances
        .storages
        .iter()
        .filter(|candidate| *candidate == &stored.storage)
        .count();
    let exact_plans = checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .stored_scalar_calls
        .iter()
        .filter(|candidate| *candidate == stored)
        .count();
    if exact_storages != 1
        || exact_plans != 1
        || stored.storage.selection != plan.selection
        || stored.storage.machine != plan.caller_machine
        || stored.storage.state != plan.caller_state
        || stored.storage.statement_index.checked_add(1)
            != usize::try_from(plan.coordinate.statement_index).ok()
        || stored.storage.selection.statement_index.checked_add(1)
            != Some(stored.storage.statement_index)
        || stored.storage.destination_path.len() != 2
        || stored.storage.source_path.len() != 1
        || destination.symbol != stored.storage.destination_binding
        || destination.name != stored.storage.destination_name
        || stored.storage.destination_path[0] != destination.name
        || stored.storage.destination_path[1] != field.name
        || stored.storage.source_binding != plan.receiver_binding
        || stored.storage.source_name != plan.selection.binding_name
        || stored.storage.source_path[0] != stored.storage.source_name
        || destination_type_identity != stored.destination_type_identity
        || destination_field_identity != stored.destination_field_identity
        || plan.caller_structural_scalar_field_store.is_some()
        || !plan.forwarding_transfers.is_empty()
        || !plan.forwarding_helpers.is_empty()
        || !matches!(
            plan.origin,
            checked_trees::CheckedDynamicScalarCallOrigin::Local
        )
    {
        return unsupported("stored dynamic descriptor drifted from checked aggregate custody");
    }
    validate_exact_dynamic_plan(
        checked,
        plan,
        stored.storage.selection.statement_index,
        plan.coordinate.statement_index,
        Some(stored.storage.destination_field),
    )
}

fn validate_exact_dynamic_plan(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    selection_statement_index: usize,
    call_statement_index: u32,
    expected_flow_receiver: Option<symbols::SymbolHandle>,
) -> Result<DynamicCallerShape, LoweringError> {
    let forwarded = match plan.origin {
        checked_trees::CheckedDynamicScalarCallOrigin::Local => None,
        checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
            machine,
            state,
            coordinate,
            parameter,
        } => Some((machine, state, coordinate, parameter)),
    };
    let store = plan.caller_structural_scalar_field_store.as_ref();
    let exact_selections = checked
        .facts
        .dynamic_conformances
        .binding_facts()
        .selections
        .into_iter()
        .filter(|selection| selection == &plan.selection)
        .count();
    if exact_selections != 1
        || plan.selection.machine != plan.caller_machine
        || plan.selection.state != plan.caller_state
        || plan.selection.binding != plan.receiver_binding
        || plan.selection.target_trait != plan.target_trait
        || plan.selection.conformance != Some(plan.selected_conformance)
        || plan.selection.source_symbol != plan.source_field
        || plan.selection.statement_index != selection_statement_index
        || plan.coordinate.statement_index != call_statement_index
        || plan.coordinate.call_ordinal != 0
        || plan.result.statement_index != plan.coordinate.statement_index
        || plan.result.binding_ordinal != 0
        || plan.selection.statement_index
            >= usize::try_from(plan.coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("direct dynamic statement coordinate exceeds usize")
            })?
    {
        return unsupported("direct dynamic dispatch plan no longer matches its checked selection");
    }
    // The retained selection rows name the provider template; a finite-family
    // plan names the tuple's bare specialization instance instead. The join
    // therefore expands each retained row's family roster and requires the
    // plan's `(family_tuple, realization_machine, realization_state)` to land
    // on exactly one expanded row, while `realization_identity` must equal the
    // bare normalized identity of the instance the plan names.
    let realization_identity =
        evidence_lowering::checked_dynamic_machine_identity(checked, plan.realization_machine)?;
    if realization_identity != plan.realization_identity {
        return unsupported("direct dynamic realization identity drifted from checking");
    }
    let selected_rows = count_selected_family_rows(
        checked,
        &plan.selection.rows,
        plan.declaring_trait,
        plan.requirement,
        &plan.requirement_identity,
        &plan.family_tuple,
        plan.realization_machine,
        plan.realization_state,
    )?;
    if selected_rows != 1 {
        return unsupported("direct dynamic dispatch lost its exact selected conformance row");
    }
    let selected_callables = plan
        .realization_callables
        .iter()
        .filter(|callable| {
            callable.declaring_trait == plan.declaring_trait
                && callable.requirement == plan.requirement
                && callable.realization_machine == plan.realization_machine
                && callable.realization_state == plan.realization_state
                && callable.requirement_identity == plan.requirement_identity
                && callable.realization_identity == plan.realization_identity
                && callable.family_tuple.as_ref() == plan.family_tuple.as_ref()
        })
        .collect::<Vec<_>>();
    let [selected_callable] = selected_callables.as_slice() else {
        return unsupported("direct dynamic selected callable is absent or ambiguous");
    };
    let checked_trees::CheckedDynamicRealizationBodyPlan::Scalar {
        result_type,
        return_expression,
        structural_scalar_field_stores,
    } = &selected_callable.body
    else {
        return unsupported("direct dynamic scalar call selected a Unit body");
    };
    if *result_type != plan.result.primitive_type
        || *return_expression != plan.realization_return_expression
        || *structural_scalar_field_stores != plan.realization_structural_scalar_field_stores
    {
        return unsupported("direct dynamic selected body drifted from checked custody");
    }
    if forwarded.is_none()
        && (checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(plan.caller_machine)
            .is_some()
            || checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_for_machine(plan.caller_machine)
                .is_some())
    {
        return unsupported("direct dynamic caller overlaps another checked Unit route");
    }
    let state_facts = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| {
            (state.machine_symbol == plan.caller_machine && state.state_symbol == plan.caller_state)
                .then_some(state)
        })
        .collect::<Vec<_>>();
    let [state] = state_facts.as_slice() else {
        return unsupported("direct dynamic caller has no exact checked flow state");
    };
    let calls = checked.facts.flow.control.calls.span_or_empty(state.calls);
    let matching_calls = calls
        .iter()
        .filter(|call| {
            call.statement_index == plan.coordinate.statement_index as usize
                && call.call_ordinal == plan.coordinate.call_ordinal as usize
                && match forwarded {
                    Some((_, state, _, _)) => {
                        let first_state = plan
                            .forwarding_transfers
                            .first()
                            .map(|transfer| transfer.caller_state)
                            .unwrap_or(state);
                        !call.has_receiver && call.target_symbol == first_state
                    }
                    None => {
                        call.receiver_symbol
                            == expected_flow_receiver.unwrap_or(plan.receiver_binding)
                            && call.target_symbol == plan.requirement
                    }
                }
        })
        .collect::<Vec<_>>();
    let [call] = matching_calls.as_slice() else {
        return unsupported("direct dynamic caller must retain one exact checked dynamic call");
    };
    if let Some(continuation) = &plan.unit_continuation {
        let expected_control_calls = [
            (
                continuation.when_true.statement_ordinal as usize,
                continuation.when_true.target_state,
            ),
            (
                continuation.when_false.statement_ordinal as usize,
                continuation.when_false.target_state,
            ),
        ];
        if calls.len() != 3
            || expected_control_calls.iter().any(|(statement, target)| {
                calls
                    .iter()
                    .filter(|candidate| {
                        candidate.statement_index == *statement
                            && candidate.call_ordinal == 0
                            && candidate.target_symbol == *target
                    })
                    .count()
                    != 1
            })
        {
            return unsupported("direct dynamic continuation lost its checked control calls");
        }
    } else if calls.len() != 1 {
        return unsupported("direct dynamic caller must contain one checked call");
    }
    let expected_statement_count = usize::try_from(call_statement_index + 1)
        .expect("bounded statement count")
        + usize::from(plan.unit_continuation.is_some()) * 2;
    if call.statement_index != plan.coordinate.statement_index as usize
        || call.call_ordinal != plan.coordinate.call_ordinal as usize
        || match forwarded {
            Some((machine, state, coordinate, parameter)) => {
                call.has_receiver
                    || call.target_symbol
                        != plan
                            .forwarding_transfers
                            .first()
                            .map(|transfer| transfer.caller_state)
                            .unwrap_or(state)
                    || !validate_forwarding_transfer_path(
                        checked, plan, machine, state, coordinate, parameter,
                    )?
                    || !validate_forwarded_dynamic_call(
                        checked, plan, machine, state, coordinate, parameter,
                    )?
            }
            None => {
                call.receiver_symbol != expected_flow_receiver.unwrap_or(plan.receiver_binding)
                    || call.target_symbol != plan.requirement
                    || !call.has_receiver
                    || call.service_reach != plan.checked_call_service_reach
            }
        }
        || checked
            .facts
            .flow
            .control
            .statements
            .span_or_empty(state.statements)
            .len()
            != expected_statement_count
    {
        return unsupported("direct dynamic call drifted from checked flow custody");
    }
    validate_empty_contract(
        checked,
        plan.caller_machine,
        plan.caller_contract_report_fingerprint,
        plan.caller_contract_commitment,
    )?;
    validate_empty_contract(
        checked,
        plan.realization_machine,
        plan.realization_contract_report_fingerprint,
        plan.realization_contract_commitment,
    )?;
    if plan.source_parameter_position != 0
        || !matches!(
            plan.caller_multiplicity,
            Multiplicity::Unrestricted | Multiplicity::Affine
        )
        || !matches!(
            plan.source_multiplicity,
            Multiplicity::Unrestricted | Multiplicity::Affine
        )
        || !matches!(
            plan.caller_parameter_access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        )
        || (store.is_some()
            && plan.caller_parameter_access != CheckedStructuralAccess::MutableBorrow)
        || (store.is_some() && plan.caller_multiplicity != Multiplicity::Unrestricted)
        || !matches!(
            plan.source_access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        )
        || (plan.source_access == CheckedStructuralAccess::MutableBorrow
            && plan.caller_parameter_access != CheckedStructuralAccess::MutableBorrow)
    {
        return unsupported("direct dynamic source must be an exact borrowed field subloan");
    }
    let [CheckedUnitStructuralPathSegment::Field(_)] = plan.source_path.as_slice() else {
        return unsupported("direct dynamic source must be one exact attachment field");
    };
    if let Some(store) = store
        && (store.statement_index != 0
            || store.destination.parameter_position() != Some(plan.source_parameter_position)
            || store.carrier_path != plan.source_path
            || !crate::emission::structural_scalar_store::checked_store_literal_matches(
                store.value.as_pure().ok_or(LoweringError::Unsupported(
                    "direct dynamic store computation is unsupported",
                ))?,
                store.primitive_type,
            ))
    {
        return unsupported("direct dynamic caller store drifted from checked custody");
    }
    validate_empty_service_summary(checked, plan.checked_call_service_reach)?;
    let caller_service_reach = exact_machine_service_summary(checked, plan.caller_machine)?;
    if caller_service_reach != plan.caller_service_reach {
        return unsupported("direct dynamic caller service reach drifted from checking");
    }
    if plan.unit_continuation.is_none() {
        validate_empty_service_summary(checked, caller_service_reach)?;
    }
    Ok(DynamicCallerShape {
        attachment_type_identity: plan.caller_attachment_type_identity.clone(),
    })
}

fn validate_forwarding_transfer_path(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    final_machine: symbols::SymbolHandle,
    final_state: symbols::SymbolHandle,
    _final_coordinate: checked_trees::CheckedUnitCallCoordinate,
    final_parameter: symbols::SymbolHandle,
) -> Result<bool, LoweringError> {
    let transfers = &checked
        .facts
        .flow
        .terminal_unit_effects
        .dynamic_dispatch
        .transfers;
    let first_machine = plan
        .forwarding_transfers
        .first()
        .map(|transfer| transfer.caller_machine)
        .unwrap_or(final_machine);
    let first_state = plan
        .forwarding_transfers
        .first()
        .map(|transfer| transfer.caller_state)
        .unwrap_or(final_state);
    let roots = transfers
        .iter()
        .filter(|transfer| {
            transfer.caller_machine == plan.caller_machine
                && transfer.caller_state == plan.caller_state
                && transfer.coordinate == plan.coordinate
                && transfer.target_machine == first_machine
                && transfer.target_state == first_state
                && transfer.parameter_position == 0
                && transfer.target_trait == plan.target_trait
                && transfer.source_binding == plan.receiver_binding
                && transfer.source
                    == checked_trees::CheckedDynamicDescriptorTransferSource::Selection
                && transfer.sole_selection() == Some(&plan.selection)
        })
        .collect::<Vec<_>>();
    let [root] = roots.as_slice() else {
        return Ok(false);
    };
    let [root_path] = root.source_paths.as_slice() else {
        return Ok(false);
    };
    let mut expected_path = root_path.clone();
    let mut machine = root.target_machine;
    let mut state = root.target_state;
    let mut source_parameter = root.parameter;
    for transfer in &plan.forwarding_transfers {
        if transfers
            .iter()
            .filter(|candidate| *candidate == transfer)
            .count()
            != 1
            || transfer.caller_machine != machine
            || transfer.caller_state != state
            || transfer.parameter_position != 0
            || transfer.target_trait != plan.target_trait
            || transfer.source_binding != source_parameter
            || transfer.source
                != (checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                    parameter_position: 0,
                })
            || !validate_parameter_forwarding_call(checked, transfer)?
        {
            return Ok(false);
        }
        expected_path.edges.push(transfer.edge());
        if !transfer.source_paths.contains(&expected_path) {
            return Ok(false);
        }
        machine = transfer.target_machine;
        state = transfer.target_state;
        source_parameter = transfer.parameter;
    }
    Ok(machine == final_machine && state == final_state && source_parameter == final_parameter)
}

pub(crate) fn validate_parameter_forwarding_call(
    checked: &CheckedTrees,
    transfer: &checked_trees::CheckedDynamicDescriptorTransferPlan,
) -> Result<bool, LoweringError> {
    let selections = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == transfer.caller_machine)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Ok(false);
    };
    if selection.signature != checked_trees::CheckedTerminalSignatureEligibility::Eligible {
        return Ok(false);
    }
    let states = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| {
            (state.machine_symbol == transfer.caller_machine
                && state.state_symbol == transfer.caller_state)
                .then_some(state)
        })
        .collect::<Vec<_>>();
    let [state] = states.as_slice() else {
        return Ok(false);
    };
    let calls = checked.facts.flow.control.calls.span_or_empty(state.calls);
    let [call] = calls else {
        return Ok(false);
    };
    let service_reach = exact_machine_service_summary(checked, transfer.caller_machine)?;
    validate_empty_service_summary(checked, service_reach)?;
    Ok(
        call.statement_index == transfer.coordinate.statement_index as usize
            && call.call_ordinal == transfer.coordinate.call_ordinal as usize
            && !call.has_receiver
            && call.target_symbol == transfer.target_state
            && call.service_reach == service_reach,
    )
}

fn validate_forwarded_dynamic_call(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    helper_machine: symbols::SymbolHandle,
    helper_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    parameter: symbols::SymbolHandle,
) -> Result<bool, LoweringError> {
    validate_forwarded_dynamic_call_coordinates(
        checked,
        plan.requirement,
        plan.checked_call_service_reach,
        helper_machine,
        helper_state,
        coordinate,
        parameter,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_forwarded_dynamic_call_coordinates(
    checked: &CheckedTrees,
    requirement: symbols::SymbolHandle,
    checked_call_service_reach: ServiceReachSummary,
    helper_machine: symbols::SymbolHandle,
    helper_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    parameter: symbols::SymbolHandle,
) -> Result<bool, LoweringError> {
    let selections = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .filter(|selection| selection.machine == helper_machine)
        .collect::<Vec<_>>();
    let [selection] = selections.as_slice() else {
        return Ok(false);
    };
    if selection.signature != checked_trees::CheckedTerminalSignatureEligibility::Eligible {
        return Ok(false);
    }
    let state_facts = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, state)| {
            (state.machine_symbol == helper_machine && state.state_symbol == helper_state)
                .then_some(state)
        })
        .collect::<Vec<_>>();
    let [state] = state_facts.as_slice() else {
        return Ok(false);
    };
    let calls = checked.facts.flow.control.calls.span_or_empty(state.calls);
    let [call] = calls else {
        return Ok(false);
    };
    Ok(call.statement_index == coordinate.statement_index as usize
        && call.call_ordinal == coordinate.call_ordinal as usize
        && call.receiver_symbol == parameter
        && call.target_symbol == requirement
        && call.has_receiver
        && call.service_reach == checked_call_service_reach)
}
