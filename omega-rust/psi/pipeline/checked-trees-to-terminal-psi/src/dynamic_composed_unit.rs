//! Source-free lowering for bounded local dynamic calls.
//!
//! A never-rebound value lowers to a direct call. A value rebound exactly once
//! retains two selections and an indirect descriptor call. A checked forwarded
//! call additionally preserves its dynamic parameter, caller argument, and
//! parameter dispatch instead of composing the helper away. Every lane retains
//! the exact conformance application, source field subloan, requirement row,
//! and realization callable in Terminal custody.

use checked_trees::{
    CheckedBooleanExpression, CheckedDynamicScalarCallPlan, CheckedDynamicSelectionPlan,
    CheckedReboundDynamicScalarCallPlan, CheckedScalarExpression, CheckedStructuralAccess,
    CheckedStructuralPredicatePathSegment, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape,
};
use language_semantics::{Multiplicity, ServiceReachSummary};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    Block, ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, MachineContract, Operation,
    OperationKind, OperationResult, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, TerminalDirectDynamicDispatch,
    TerminalDynamicConformanceSelection, TerminalDynamicDescriptorArgument,
    TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource,
    TerminalDynamicDispatchCatalog, TerminalDynamicRequirement, TerminalIndirectDynamicDispatch,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalParameterDynamicDispatch,
    TerminalReboundDynamicDescriptor, TerminalStoredDynamicDescriptor,
    TerminalStoredDynamicDispatch, Terminator, ValueDeclaration, VocabularyMarker,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};

use super::*;

mod continuation;
mod join;
mod unit;
mod unit_join;

pub(super) fn lower_joined_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedJoinedDynamicScalarCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    join::lower(checked, plan)
}

pub(super) fn lower_joined_dynamic_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedJoinedDynamicUnitCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    unit_join::lower(checked, plan)
}

pub(super) fn lower_direct_dynamic_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedDynamicUnitCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    unit::lower_direct_dynamic_unit_machine(checked, plan)
}

pub(super) fn lower_rebound_dynamic_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedReboundDynamicUnitCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    unit::lower_rebound_dynamic_unit_machine(checked, plan)
}

struct DynamicCallerShape {
    attachment_type_identity: String,
}

#[derive(Clone)]
struct LoweredDynamicRealization {
    source_machine: symbols::SymbolHandle,
    source_state: symbols::SymbolHandle,
    callable_identity: String,
    machine: semantic_vocabulary::MachineId,
    result: ClosedConformanceCallableResult,
}

#[derive(Clone, Copy)]
struct ForwardedHelperIds {
    machine: semantic_vocabulary::MachineId,
    block: semantic_vocabulary::BlockId,
    operation: semantic_vocabulary::OperationId,
    operation_value: semantic_vocabulary::ValueId,
    result_value: semantic_vocabulary::ValueId,
    edge: semantic_vocabulary::EdgeId,
}

#[derive(Clone, Copy)]
enum DynamicLoweringLane<'a> {
    Direct,
    Rebound(&'a CheckedDynamicSelectionPlan),
    Stored(&'a checked_trees::CheckedStoredDynamicScalarCallPlan),
}

pub(super) fn lower_direct_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    lower_dynamic_composed_unit_machine(checked, plan, DynamicLoweringLane::Direct)
}

pub(super) fn lower_rebound_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedReboundDynamicScalarCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    lower_dynamic_composed_unit_machine(
        checked,
        &plan.latest,
        DynamicLoweringLane::Rebound(&plan.initial),
    )
}

pub(super) fn lower_stored_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedStoredDynamicScalarCallPlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    lower_dynamic_composed_unit_machine(checked, &plan.call, DynamicLoweringLane::Stored(plan))
}

fn lower_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    lane: DynamicLoweringLane<'_>,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let caller = match lane {
        DynamicLoweringLane::Direct => validate_exact_direct_plan(checked, plan)?,
        DynamicLoweringLane::Rebound(initial) => {
            validate_exact_rebound_plan(checked, plan, initial)?
        }
        DynamicLoweringLane::Stored(stored) => validate_exact_stored_plan(checked, stored)?,
    };
    if let Some(unit_continuation) = &plan.unit_continuation {
        return continuation::lower(checked, plan, unit_continuation, caller, lane);
    }
    let (structural_types, type_ids) =
        lower_dynamic_structural_types(checked, plan, &caller.attachment_type_identity)?;
    let caller_attachment = lookup_type_id(&type_ids, &caller.attachment_type_identity)?;
    let caller_access = match plan.caller_parameter_access {
        CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        _ => return unsupported("direct dynamic caller requires a borrowed self parameter"),
    };
    let caller_self = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: true,
        structural_type: caller_attachment,
        multiplicity: terminal_structural_multiplicity(plan.caller_multiplicity),
        access: caller_access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let caller_parameters = vec![caller_self.clone()];
    let source = validate_and_lower_source(&caller_self, plan, &structural_types, &type_ids)?;

    let caller_machine = machine_id(1);
    let has_caller_store = plan.caller_structural_scalar_field_store.is_some();
    let has_descriptor_store = matches!(lane, DynamicLoweringLane::Stored(_));
    let call_operation = operation_id(if has_caller_store {
        3
    } else if has_descriptor_store {
        2
    } else {
        1
    });
    let call_result_value = value_id(if has_caller_store { 2 } else { 1 });
    let call_result_type = terminal_scalar_type(plan.result.primitive_type)?;
    let source_type = lookup_type_id(&type_ids, &plan.source_type_identity)?;
    let all_realizations = collect_dynamic_realizations(checked, plan)?;
    let lowered_realizations = retain_realizations_for_lane(&all_realizations, plan, lane)?;
    let selected_realizations = lowered_realizations
        .iter()
        .filter(|candidate| {
            candidate.source_machine == plan.realization_machine
                && candidate.source_state == plan.realization_state
        })
        .collect::<Vec<_>>();
    let [selected_realization] = selected_realizations.as_slice() else {
        return unsupported("direct dynamic selected realization is absent or ambiguous");
    };
    let realization_machine = selected_realization.machine;
    let callable_result = selected_realization.result;
    let callable_identity = selected_realization.callable_identity.clone();
    if callable_result != terminal_callable_result(plan.result.primitive_type)?
        || callable_identity != plan.realization_identity
    {
        return unsupported("direct dynamic selected realization callable drifted");
    }

    let (application, selected_row) =
        lower_exact_application(checked, plan, caller_machine, &lowered_realizations)?;
    let initial_application = match lane {
        DynamicLoweringLane::Rebound(initial)
            if initial.fact.conformance != plan.selection.conformance
                || initial.fact.rows != plan.selection.rows =>
        {
            Some(lower_initial_rebound_application(
                checked,
                plan.target_trait,
                initial,
                caller_machine,
            )?)
        }
        _ => None,
    };
    let mut next_block = 2_u64;
    let mut next_place = 2_u64;
    let mut next_operation = if has_caller_store {
        4
    } else if has_descriptor_store {
        3
    } else {
        2
    };
    let mut next_value = if has_caller_store { 3 } else { 2 };
    let mut next_edge = 2_u64;
    let forwarded_helpers = forwarded_helper_chain_ids(
        plan,
        &lowered_realizations,
        &mut next_block,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
    )?;
    let (mut dynamic_dispatch, call_kind) = lower_dynamic_call_custody(
        lane,
        &caller_self,
        plan,
        &structural_types,
        &type_ids,
        caller_machine,
        call_operation,
        source,
        initial_application.as_ref(),
        &application,
        &selected_row,
        callable_identity,
        realization_machine,
        forwarded_helpers.first().copied(),
    )?;
    if forwarded_helpers.len() > 1 {
        extend_parameter_forwarding_catalog(&mut dynamic_dispatch, &forwarded_helpers)?;
    }

    let caller_block = block_id(1);
    let caller_reach = lower_installation_machine_service_ceiling(
        checked,
        plan.caller_machine,
        checked
            .facts
            .service_reaches
            .plan_for_machine(plan.caller_machine)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic caller has no checked service contract",
            ))?,
        exact_machine_service_summary(checked, plan.caller_machine)?,
        &[],
    )?;
    let root_service_reach = lower_root_service_reach(checked, plan.caller_machine, &[])?;
    let mut caller_operations =
        lower_caller_store_operations(plan, &caller_self, &structural_types, &type_ids)?;
    if has_descriptor_store {
        caller_operations.push(Operation {
            id: operation_id(1),
            result: OperationResult::Unit,
            kind: OperationKind::StoreDynamicDescriptor {
                descriptor_ordinal: 0,
            },
        });
    }
    caller_operations.push(Operation {
        id: call_operation,
        result: OperationResult::Scalar(ValueDeclaration {
            id: call_result_value,
            scalar_type: call_result_type,
        }),
        kind: call_kind,
    });
    let realization_machines = materialize_dynamic_realizations(
        checked,
        plan,
        &lowered_realizations,
        source_type,
        &structural_types,
        &mut next_block,
        &mut next_place,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
    )?;
    let forwarded_helper_machines = materialize_forwarded_helper_chain(
        checked,
        plan,
        &application,
        &selected_row,
        &forwarded_helpers,
    )?;

    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: caller_machine,
            structural_types,
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach,
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: {
                let mut applications = vec![application];
                applications.extend(initial_application);
                applications.sort_by(|left, right| {
                    (
                        left.owner,
                        left.declaration_identity.as_str(),
                        left.report_fingerprint,
                    )
                        .cmp(&(
                            right.owner,
                            right.declaration_identity.as_str(),
                            right.report_fingerprint,
                        ))
                });
                applications
            },
            dynamic_dispatch,
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: {
                let mut machines = vec![TerminalMachine {
                    id: caller_machine,
                    attachment: Some(caller_attachment),
                    parameters: Vec::new(),
                    structural_parameters: caller_parameters.clone(),
                    ranked_scc: None,
                    result: TerminalMachineResult::Unit,
                    structural_places: caller_parameters
                        .iter()
                        .map(|parameter| StructuralPlaceDeclaration {
                            id: parameter.place,
                            kind: StructuralPlaceKind::Parameter {
                                position: parameter.position,
                                is_self: parameter.is_self,
                            },
                        })
                        .collect(),
                    entry_claims: Vec::new(),
                    published_service_ceiling: caller_reach,
                    content_entry_claims: Vec::new(),
                    content_identity_reshuffles: Vec::new(),
                    content_partition_compositions: Vec::new(),
                    entry: caller_block,
                    blocks: vec![Block {
                        id: caller_block,
                        parameters: Vec::new(),
                        operations: caller_operations,
                        terminator: Terminator::ReturnUnit {
                            edge: edge_id(1),
                            trivial_affine_discards: Vec::new(),
                        },
                    }],
                    contract: empty_terminal_contract(caller_machine.get()),
                }];
                machines.extend(realization_machines);
                machines.extend(forwarded_helper_machines);
                machines
            },
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences: dynamic_source_call_occurrences_for_chain(
            plan,
            call_operation,
            &forwarded_helpers,
        )?,
        selected_ieee_float_fma_occurrences: Vec::new(),
    })
}

fn validate_exact_direct_plan(
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

fn validate_exact_rebound_plan(
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

fn validate_exact_stored_plan(
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
        .normalized_type_identity_with_binders(destination.type_reference, &binders)
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
    let selected_rows = plan
        .selection
        .rows
        .iter()
        .filter(|row| {
            row.declaring_trait == plan.declaring_trait
                && row.requirement == plan.requirement
                && row.realization_machine == plan.realization_machine
                && row.realization_state == plan.realization_state
                && row.requirement_identity == plan.requirement_identity
                && row.realization_identity == plan.realization_identity
        })
        .count();
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
        })
        .collect::<Vec<_>>();
    let [selected_callable] = selected_callables.as_slice() else {
        return unsupported("direct dynamic selected callable is absent or ambiguous");
    };
    if selected_callable.return_expression != plan.realization_return_expression
        || selected_callable.structural_scalar_field_stores
            != plan.realization_structural_scalar_field_stores
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
            || store.destination_parameter_position != plan.source_parameter_position
            || store.carrier_path != plan.source_path
            || !crate::structural_scalar_store::checked_store_literal_matches(
                &store.value,
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

fn validate_parameter_forwarding_call(
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
fn validate_forwarded_dynamic_call_coordinates(
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

fn validate_and_lower_source(
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDynamicScalarCallPlan,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    validate_and_lower_selection_source(
        caller_self,
        plan,
        &plan.source_path,
        &plan.source_type_identity,
        structural_types,
        type_ids,
    )
}

fn validate_and_lower_selection_source(
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDynamicScalarCallPlan,
    source_path: &[CheckedUnitStructuralPathSegment],
    source_type_identity: &str,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    validate_and_lower_dynamic_source(
        caller_self,
        plan.source_parameter_position,
        plan.caller_parameter_access,
        plan.caller_multiplicity,
        plan.source_access,
        &plan.caller_attachment_type_identity,
        source_path,
        source_type_identity,
        structural_types,
        type_ids,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_and_lower_dynamic_source(
    caller_self: &StructuralParameterDeclaration,
    source_parameter_position: u32,
    caller_parameter_access: CheckedStructuralAccess,
    caller_multiplicity: Multiplicity,
    source_access: CheckedStructuralAccess,
    caller_attachment_type_identity: &str,
    source_path: &[CheckedUnitStructuralPathSegment],
    source_type_identity: &str,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    if source_parameter_position != caller_self.position
        || caller_parameter_access
            != match caller_self.access {
                StructuralAccess::SharedBorrow => CheckedStructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow => CheckedStructuralAccess::MutableBorrow,
                _ => return unsupported("direct dynamic caller self access is unsupported"),
            }
        || !caller_self.is_self
        || caller_self.multiplicity != terminal_structural_multiplicity(caller_multiplicity)
        || !matches!(
            source_access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        )
        || (source_access == CheckedStructuralAccess::MutableBorrow
            && caller_self.access != StructuralAccess::MutableBorrow)
    {
        return unsupported("direct dynamic caller self does not license the field subloan");
    }
    let attachment_id = lookup_type_id(type_ids, caller_attachment_type_identity)?;
    let source_type = lookup_type_id(type_ids, source_type_identity)?;
    let attachment = structural_types
        .iter()
        .find(|declaration| declaration.id == attachment_id)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic caller attachment declaration is absent",
        ))?;
    let [CheckedUnitStructuralPathSegment::Field(field_identity)] = source_path else {
        unreachable!("direct source path was validated")
    };
    let terminal_psi::StructuralTypeShape::Record { fields } = &attachment.shape else {
        return unsupported("direct dynamic caller attachment must be a record");
    };
    let matching_fields = fields
        .iter()
        .filter(|field| {
            field.identity == *field_identity
                && field.field_type == terminal_psi::StructuralFieldType::Structural(source_type)
        })
        .count();
    if matching_fields != 1 {
        return unsupported("direct dynamic source field no longer matches its structural carrier");
    }
    Ok(StructuralArgument {
        place: caller_self.place,
        path: lower_structural_path(source_path),
        access: match source_access {
            CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
            CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
            _ => unreachable!("borrowed dynamic source access was validated"),
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_dynamic_call_custody(
    lane: DynamicLoweringLane<'_>,
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDynamicScalarCallPlan,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
    caller_machine: semantic_vocabulary::MachineId,
    call_operation: semantic_vocabulary::OperationId,
    latest_source: StructuralArgument,
    initial_application: Option<&ClosedConformanceApplication>,
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
    callable_identity: String,
    realization_machine: semantic_vocabulary::MachineId,
    forwarded_helper: Option<ForwardedHelperIds>,
) -> Result<(TerminalDynamicDispatchCatalog, OperationKind), LoweringError> {
    let latest_selection = TerminalDynamicConformanceSelection {
        owner: caller_machine,
        ordinal: u32::from(matches!(lane, DynamicLoweringLane::Rebound(_))),
        source: latest_source.clone(),
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
    };
    let row_dispatch = |descriptor_ordinal| TerminalIndirectDynamicDispatch {
        owner: caller_machine,
        operation: call_operation,
        descriptor_ordinal,
        declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
        public_requirement_identity: selected_row.public_requirement_identity.clone(),
        requirement_identity: selected_row.requirement_identity.clone(),
        realization_identity: selected_row.realization_identity.clone(),
        realization_callable_identity: callable_identity.clone(),
        realization: realization_machine,
    };
    let stored_row_dispatch = |descriptor_ordinal| TerminalStoredDynamicDispatch {
        owner: caller_machine,
        operation: call_operation,
        descriptor_ordinal,
        declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
        public_requirement_identity: selected_row.public_requirement_identity.clone(),
        requirement_identity: selected_row.requirement_identity.clone(),
        realization_identity: selected_row.realization_identity.clone(),
        realization_callable_identity: callable_identity.clone(),
        realization: realization_machine,
    };
    Ok(match lane {
        DynamicLoweringLane::Direct => {
            if initial_application.is_some() {
                return unsupported("direct dynamic dispatch retained a rebound application");
            }
            let mut catalog = TerminalDynamicDispatchCatalog {
                parameters: Vec::new(),
                arguments: Vec::new(),
                selections: vec![latest_selection],
                rebound_descriptors: Vec::new(),
                stored_descriptors: Vec::new(),
                direct_dispatches: Vec::new(),
                indirect_dispatches: Vec::new(),
                stored_dispatches: Vec::new(),
                parameter_dispatches: Vec::new(),
            };
            let call_kind = if let Some(helper) = forwarded_helper {
                let (requirements, requirement_slot) =
                    dynamic_parameter_interface(application, selected_row)?;
                catalog.parameters.push(TerminalDynamicDescriptorParameter {
                    owner: helper.machine,
                    ordinal: 0,
                    source_position: 0,
                    trait_identity: application.trait_identity.clone(),
                    access: latest_source.access,
                    requirements,
                });
                catalog.arguments.push(TerminalDynamicDescriptorArgument {
                    owner: caller_machine,
                    operation: call_operation,
                    parameter_ordinal: 0,
                    source: TerminalDynamicDescriptorSource::Selection { ordinal: 0 },
                });
                catalog
                    .parameter_dispatches
                    .push(TerminalParameterDynamicDispatch {
                        owner: helper.machine,
                        operation: helper.operation,
                        parameter_ordinal: 0,
                        requirement_slot,
                    });
                OperationKind::CallStructuralScalar {
                    callee: helper.machine,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            } else {
                catalog
                    .direct_dispatches
                    .push(TerminalDirectDynamicDispatch {
                        owner: caller_machine,
                        operation: call_operation,
                        selection_ordinal: 0,
                        declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
                        public_requirement_identity: selected_row
                            .public_requirement_identity
                            .clone(),
                        requirement_identity: selected_row.requirement_identity.clone(),
                        realization_identity: selected_row.realization_identity.clone(),
                        realization_callable_identity: callable_identity,
                        realization: realization_machine,
                    });
                OperationKind::CallStructuralScalar {
                    callee: realization_machine,
                    arguments: Vec::new(),
                    structural_arguments: vec![latest_source],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            };
            (catalog, call_kind)
        }
        DynamicLoweringLane::Rebound(initial) => {
            let initial_source = validate_and_lower_selection_source(
                caller_self,
                plan,
                &initial.path,
                &initial.type_identity,
                structural_types,
                type_ids,
            )?;
            let mut catalog = TerminalDynamicDispatchCatalog {
                parameters: Vec::new(),
                arguments: Vec::new(),
                selections: vec![
                    TerminalDynamicConformanceSelection {
                        owner: caller_machine,
                        ordinal: 0,
                        source: initial_source,
                        conformance_application_report_fingerprint: initial_application
                            .unwrap_or(application)
                            .report_fingerprint,
                        conformance_application_commitment: initial_application
                            .unwrap_or(application)
                            .commitment,
                    },
                    latest_selection,
                ],
                rebound_descriptors: vec![TerminalReboundDynamicDescriptor {
                    owner: caller_machine,
                    ordinal: 0,
                    initial_selection_ordinal: 0,
                    rebound_selection_ordinal: 1,
                }],
                stored_descriptors: Vec::new(),
                direct_dispatches: Vec::new(),
                indirect_dispatches: Vec::new(),
                stored_dispatches: Vec::new(),
                parameter_dispatches: Vec::new(),
            };
            let call_kind = if let Some(helper) = forwarded_helper {
                let (requirements, requirement_slot) =
                    dynamic_parameter_interface(application, selected_row)?;
                catalog.parameters.push(TerminalDynamicDescriptorParameter {
                    owner: helper.machine,
                    ordinal: 0,
                    source_position: 0,
                    trait_identity: application.trait_identity.clone(),
                    access: latest_source.access,
                    requirements,
                });
                catalog.arguments.push(TerminalDynamicDescriptorArgument {
                    owner: caller_machine,
                    operation: call_operation,
                    parameter_ordinal: 0,
                    source: TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal: 0 },
                });
                catalog
                    .parameter_dispatches
                    .push(TerminalParameterDynamicDispatch {
                        owner: helper.machine,
                        operation: helper.operation,
                        parameter_ordinal: 0,
                        requirement_slot,
                    });
                OperationKind::CallStructuralScalar {
                    callee: helper.machine,
                    arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            } else {
                catalog.indirect_dispatches.push(row_dispatch(0));
                OperationKind::CallDynamicScalar {
                    descriptor_ordinal: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            };
            (catalog, call_kind)
        }
        DynamicLoweringLane::Stored(stored) => {
            if initial_application.is_some() || forwarded_helper.is_some() {
                return unsupported(
                    "stored dynamic dispatch acquired unrelated descriptor custody",
                );
            }
            let catalog = TerminalDynamicDispatchCatalog {
                parameters: Vec::new(),
                arguments: Vec::new(),
                selections: vec![latest_selection],
                rebound_descriptors: Vec::new(),
                stored_descriptors: vec![TerminalStoredDynamicDescriptor {
                    owner: caller_machine,
                    ordinal: 0,
                    establishment_operation: operation_id(1),
                    selection_ordinal: 0,
                    aggregate_type_identity: stored.destination_type_identity.clone(),
                    field_identity: stored.destination_field_identity.clone(),
                }],
                direct_dispatches: Vec::new(),
                indirect_dispatches: Vec::new(),
                stored_dispatches: vec![stored_row_dispatch(0)],
                parameter_dispatches: Vec::new(),
            };
            (
                catalog,
                OperationKind::CallDynamicScalar {
                    descriptor_ordinal: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            )
        }
    })
}

fn dynamic_parameter_interface(
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
) -> Result<(Vec<TerminalDynamicRequirement>, u32), LoweringError> {
    let mut selected_slot = None;
    let requirements = application
        .rows
        .iter()
        .enumerate()
        .map(|(ordinal, row)| {
            let slot = u32::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("dynamic parameter requirement ordinal exceeds u32")
            })?;
            if row == selected_row && selected_slot.replace(slot).is_some() {
                return unsupported("dynamic parameter selected requirement is duplicated");
            }
            let callable_identity =
                row.realization_callable_identity
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "dynamic parameter application row has no realization callable",
                    ))?;
            let matching = application
                .realization_callables
                .iter()
                .filter(|callable| callable.source_callable_identity == *callable_identity)
                .collect::<Vec<_>>();
            let [callable] = matching.as_slice() else {
                return unsupported(
                    "dynamic parameter application row callable is absent or ambiguous",
                );
            };
            Ok(TerminalDynamicRequirement {
                slot,
                declaring_trait_identity: row.declaring_trait_identity.clone(),
                public_requirement_identity: row.public_requirement_identity.clone(),
                result: callable.result,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let selected_slot = selected_slot.ok_or(LoweringError::Unsupported(
        "dynamic parameter selected requirement is absent",
    ))?;
    Ok((requirements, selected_slot))
}

fn forwarded_helper_chain_ids(
    plan: &CheckedDynamicScalarCallPlan,
    realizations: &[LoweredDynamicRealization],
    next_block: &mut u64,
    next_operation: &mut u64,
    next_value: &mut u64,
    next_edge: &mut u64,
) -> Result<Vec<ForwardedHelperIds>, LoweringError> {
    if !matches!(
        plan.origin,
        checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { .. }
    ) {
        if !plan.forwarding_transfers.is_empty() {
            return unsupported("local dynamic call retained forwarding transfers");
        }
        return Ok(Vec::new());
    }
    let first_machine = realizations
        .iter()
        .map(|realization| realization.machine.get())
        .max()
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic dispatch has no realization machine",
        ))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic helper machine identity overflowed",
        ))?;
    (0..=plan.forwarding_transfers.len())
        .map(|ordinal| {
            let ordinal = u64::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("forwarded dynamic helper count exceeds u64")
            })?;
            Ok(ForwardedHelperIds {
                machine: machine_id(first_machine.checked_add(ordinal).ok_or(
                    LoweringError::Unsupported(
                        "forwarded dynamic helper machine identity overflowed",
                    ),
                )?),
                block: block_id(allocate_dense(next_block)?),
                operation: operation_id(allocate_dense(next_operation)?),
                operation_value: value_id(allocate_dense(next_value)?),
                result_value: value_id(allocate_dense(next_value)?),
                edge: edge_id(allocate_dense(next_edge)?),
            })
        })
        .collect()
}

fn extend_parameter_forwarding_catalog(
    catalog: &mut TerminalDynamicDispatchCatalog,
    helpers: &[ForwardedHelperIds],
) -> Result<(), LoweringError> {
    let [template] = catalog.parameters.as_slice() else {
        return unsupported("multi-hop dynamic forwarding lost its first parameter interface");
    };
    let template = template.clone();
    let [dispatch] = catalog.parameter_dispatches.as_mut_slice() else {
        return unsupported("multi-hop dynamic forwarding lost its final parameter dispatch");
    };
    for helper in &helpers[1..] {
        let mut parameter = template.clone();
        parameter.owner = helper.machine;
        catalog.parameters.push(parameter);
    }
    for pair in helpers.windows(2) {
        catalog.arguments.push(TerminalDynamicDescriptorArgument {
            owner: pair[0].machine,
            operation: pair[0].operation,
            parameter_ordinal: 0,
            source: TerminalDynamicDescriptorSource::Parameter { ordinal: 0 },
        });
    }
    let final_helper = helpers.last().ok_or(LoweringError::Unsupported(
        "multi-hop dynamic forwarding has no final helper",
    ))?;
    dispatch.owner = final_helper.machine;
    dispatch.operation = final_helper.operation;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn materialize_forwarded_helper_for_source(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
    ids: ForwardedHelperIds,
    source_machine: symbols::SymbolHandle,
    next_helper: Option<semantic_vocabulary::MachineId>,
) -> Result<TerminalMachine, LoweringError> {
    let checked_contract = checked
        .facts
        .contract_plans
        .for_machine(source_machine)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic helper has no checked contract",
        ))?;
    validate_empty_contract(
        checked,
        source_machine,
        checked_contract.report_fingerprint,
        checked_contract.commitment,
    )?;
    let service_summary = exact_machine_service_summary(checked, source_machine)?;
    validate_empty_service_summary(checked, service_summary)?;
    let service_contract = checked
        .facts
        .service_reaches
        .plan_for_machine(source_machine)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic helper has no checked service contract",
        ))?;
    let published_service_ceiling = lower_installation_machine_service_ceiling(
        checked,
        source_machine,
        service_contract,
        service_summary,
        &[],
    )?;
    let (_, requirement_slot) = dynamic_parameter_interface(application, selected_row)?;
    let scalar_type = terminal_scalar_type(plan.result.primitive_type)?;
    Ok(TerminalMachine {
        id: ids.machine,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: ids.result_value,
            scalar_type,
        }),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: ids.block,
        blocks: vec![Block {
            id: ids.block,
            parameters: Vec::new(),
            operations: vec![Operation {
                id: ids.operation,
                result: OperationResult::Scalar(ValueDeclaration {
                    id: ids.operation_value,
                    scalar_type,
                }),
                kind: match next_helper {
                    Some(callee) => OperationKind::CallStructuralScalar {
                        callee,
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    None => OperationKind::CallDynamicParameterScalar {
                        parameter_ordinal: 0,
                        requirement_slot,
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                },
            }],
            terminator: Terminator::Return {
                edge: ids.edge,
                value: ids.operation_value,
                cleanup_actions: Vec::new(),
            },
        }],
        contract: empty_terminal_contract(ids.machine.get()),
    })
}

fn materialize_forwarded_helper_chain(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
    helpers: &[ForwardedHelperIds],
) -> Result<Vec<TerminalMachine>, LoweringError> {
    if helpers.is_empty() {
        return Ok(Vec::new());
    }
    let checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
        machine: final_source_machine,
        ..
    } = plan.origin
    else {
        return unsupported("forwarded helper chain requires a forwarded checked origin");
    };
    if plan.forwarding_transfers.len() + 1 != helpers.len() {
        return unsupported("forwarded helper chain length drifted from checked custody");
    }
    helpers
        .iter()
        .enumerate()
        .map(|(index, ids)| {
            let source_machine = plan
                .forwarding_transfers
                .get(index)
                .map(|transfer| transfer.caller_machine)
                .unwrap_or(final_source_machine);
            let next_helper = helpers.get(index + 1).map(|next| next.machine);
            materialize_forwarded_helper_for_source(
                checked,
                plan,
                application,
                selected_row,
                *ids,
                source_machine,
                next_helper,
            )
        })
        .collect()
}

fn dynamic_source_call_occurrences_for_chain(
    plan: &CheckedDynamicScalarCallPlan,
    caller_operation: semantic_vocabulary::OperationId,
    helpers: &[ForwardedHelperIds],
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    if helpers.len() <= 1 {
        return dynamic_source_call_occurrences(plan, caller_operation, helpers.first().copied());
    }
    let checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
        state: final_state,
        coordinate: final_coordinate,
        ..
    } = plan.origin
    else {
        return unsupported("forwarded source-call chain requires a forwarded checked origin");
    };
    let first_state = plan
        .forwarding_transfers
        .first()
        .ok_or(LoweringError::Unsupported(
            "multi-hop source-call chain lost its first transfer",
        ))?
        .caller_state;
    let mut occurrences = vec![LoweredSourceCallOccurrence {
        source_site: None,
        source_state: plan.caller_state,
        statement_index: usize::try_from(plan.coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("direct dynamic statement coordinate exceeds usize")
        })?,
        call_ordinal: usize::try_from(plan.coordinate.call_ordinal)
            .map_err(|_| LoweringError::Unsupported("direct dynamic call ordinal exceeds usize"))?,
        terminal_operation: caller_operation,
        source_target: first_state,
        source_values_before_call: Vec::new(),
    }];
    for (transfer, helper) in plan.forwarding_transfers.iter().zip(helpers) {
        occurrences.push(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: transfer.caller_state,
            statement_index: usize::try_from(transfer.coordinate.statement_index).map_err(
                |_| {
                    LoweringError::Unsupported(
                        "parameter forwarding statement coordinate exceeds usize",
                    )
                },
            )?,
            call_ordinal: usize::try_from(transfer.coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("parameter forwarding call ordinal exceeds usize")
            })?,
            terminal_operation: helper.operation,
            source_target: transfer.target_state,
            source_values_before_call: Vec::new(),
        });
    }
    let final_helper = helpers.last().ok_or(LoweringError::Unsupported(
        "multi-hop source-call chain has no final helper",
    ))?;
    occurrences.push(LoweredSourceCallOccurrence {
        source_site: None,
        source_state: final_state,
        statement_index: usize::try_from(final_coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("forwarded dynamic statement coordinate exceeds usize")
        })?,
        call_ordinal: usize::try_from(final_coordinate.call_ordinal).map_err(|_| {
            LoweringError::Unsupported("forwarded dynamic call ordinal exceeds usize")
        })?,
        terminal_operation: final_helper.operation,
        source_target: plan.requirement,
        source_values_before_call: Vec::new(),
    });
    Ok(occurrences)
}

fn dynamic_source_call_occurrences(
    plan: &CheckedDynamicScalarCallPlan,
    caller_operation: semantic_vocabulary::OperationId,
    forwarded_helper: Option<ForwardedHelperIds>,
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    let statement_index = usize::try_from(plan.coordinate.statement_index).map_err(|_| {
        LoweringError::Unsupported("direct dynamic statement coordinate exceeds usize")
    })?;
    let call_ordinal = usize::try_from(plan.coordinate.call_ordinal)
        .map_err(|_| LoweringError::Unsupported("direct dynamic call ordinal exceeds usize"))?;
    let mut occurrences = vec![LoweredSourceCallOccurrence {
        source_site: None,
        source_state: plan.caller_state,
        statement_index,
        call_ordinal,
        terminal_operation: caller_operation,
        source_target: match plan.origin {
            checked_trees::CheckedDynamicScalarCallOrigin::Local => plan.requirement,
            checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { state, .. } => state,
        },
        source_values_before_call: Vec::new(),
    }];
    if let (
        Some(helper),
        checked_trees::CheckedDynamicScalarCallOrigin::Forwarded {
            state, coordinate, ..
        },
    ) = (forwarded_helper, plan.origin)
    {
        occurrences.push(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: state,
            statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("forwarded dynamic statement coordinate exceeds usize")
            })?,
            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("forwarded dynamic call ordinal exceeds usize")
            })?,
            terminal_operation: helper.operation,
            source_target: plan.requirement,
            source_values_before_call: Vec::new(),
        });
    }
    Ok(occurrences)
}

fn terminal_structural_multiplicity(multiplicity: Multiplicity) -> StructuralMultiplicity {
    match multiplicity {
        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        Multiplicity::Affine => StructuralMultiplicity::Affine,
        Multiplicity::Linear => StructuralMultiplicity::Linear,
    }
}

/// A shared field projection retains its caller root's consumption bound even
/// when the projected field's own declared carrier is copyable.
fn terminal_projected_source_multiplicity(
    plan: &CheckedDynamicScalarCallPlan,
) -> StructuralMultiplicity {
    terminal_projected_source_multiplicity_for(plan.caller_multiplicity)
}

fn terminal_projected_source_multiplicity_for(
    caller_multiplicity: Multiplicity,
) -> StructuralMultiplicity {
    match caller_multiplicity {
        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
        Multiplicity::Affine => StructuralMultiplicity::Affine,
        Multiplicity::Linear => StructuralMultiplicity::Linear,
    }
}

fn lower_dynamic_structural_types(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    caller_attachment: &str,
) -> Result<
    (
        Vec<terminal_psi::StructuralTypeDeclaration>,
        Vec<(String, semantic_vocabulary::StructuralTypeId)>,
    ),
    LoweringError,
> {
    lower_dynamic_structural_types_for_source(
        checked,
        caller_attachment,
        &plan.caller_attachment_type_identity,
        &plan.source_path,
        &plan.source_type_identity,
    )
}

fn lower_dynamic_structural_types_for_source(
    checked: &CheckedTrees,
    caller_attachment: &str,
    checked_caller_attachment: &str,
    source_path: &[CheckedUnitStructuralPathSegment],
    source_type_identity: &str,
) -> Result<
    (
        Vec<terminal_psi::StructuralTypeDeclaration>,
        Vec<(String, semantic_vocabulary::StructuralTypeId)>,
    ),
    LoweringError,
> {
    if caller_attachment != checked_caller_attachment {
        return unsupported("direct dynamic caller attachment identity drifted");
    }
    let roots = &checked.facts.flow.terminal_unit_effects.structural_types;
    let caller_roots = roots
        .iter()
        .filter(|candidate| candidate.identity == caller_attachment)
        .collect::<Vec<_>>();
    let [caller] = caller_roots.as_slice() else {
        return unsupported("direct dynamic caller attachment shape is absent or ambiguous");
    };
    let [CheckedUnitStructuralPathSegment::Field(source_field)] = source_path else {
        return unsupported("direct dynamic source must be one exact attachment field");
    };
    let CheckedUnitStructuralTypeShape::Record { fields } = &caller.shape else {
        return unsupported("direct dynamic caller attachment must be a record");
    };
    let matching_fields = fields
        .iter()
        .filter(|field| {
            field.identity == *source_field
                && field.field_type
                    == CheckedUnitStructuralFieldType::Structural {
                        type_identity: source_type_identity.to_owned(),
                    }
        })
        .count();
    if matching_fields != 1 {
        return unsupported("direct dynamic checked source field no longer matches its carrier");
    }
    attached_unit::lower_unit_structural_type_roots(
        checked,
        &[
            caller_attachment.to_owned(),
            source_type_identity.to_owned(),
        ],
    )
}

fn collect_dynamic_realizations(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    if plan.realization_callables.is_empty() {
        return unsupported("dynamic conformance has no checked realization callables");
    }
    plan.realization_callables
        .iter()
        .enumerate()
        .map(|(ordinal, callable)| {
            let ordinal = u64::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("dynamic realization ordinal exceeds u64")
            })?;
            let identity = evidence_lowering::checked_evidence_machine_identity(
                checked,
                callable.realization_machine,
            )?;
            if identity != callable.realization_identity {
                return unsupported("dynamic realization callable identity drifted");
            }
            let result = terminal_callable_result(callable.result_type)?;
            let machine = machine_id(ordinal.checked_add(2).ok_or(LoweringError::Unsupported(
                "dynamic realization machine identity overflowed",
            ))?);
            Ok(LoweredDynamicRealization {
                source_machine: callable.realization_machine,
                source_state: callable.realization_state,
                callable_identity: identity,
                machine,
                result,
            })
        })
        .collect()
}

fn retain_realizations_for_lane(
    all: &[LoweredDynamicRealization],
    plan: &CheckedDynamicScalarCallPlan,
    lane: DynamicLoweringLane<'_>,
) -> Result<Vec<LoweredDynamicRealization>, LoweringError> {
    let retained = all
        .iter()
        .filter(|candidate| {
            matches!(lane, DynamicLoweringLane::Rebound(_))
                || (candidate.source_machine == plan.realization_machine
                    && candidate.source_state == plan.realization_state)
        })
        .cloned()
        .collect::<Vec<_>>();
    if retained.is_empty() {
        return unsupported("dynamic selected realization callable is absent");
    }
    Ok(retained)
}

#[allow(clippy::too_many_arguments)]
fn materialize_dynamic_realizations(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    lowered: &[LoweredDynamicRealization],
    source_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_block: &mut u64,
    next_place: &mut u64,
    next_operation: &mut u64,
    next_value: &mut u64,
    next_edge: &mut u64,
) -> Result<Vec<TerminalMachine>, LoweringError> {
    lowered
        .iter()
        .map(|realization| {
            let matching = plan
                .realization_callables
                .iter()
                .filter(|candidate| {
                    candidate.realization_machine == realization.source_machine
                        && candidate.realization_state == realization.source_state
                        && candidate.realization_identity == realization.callable_identity
                })
                .collect::<Vec<_>>();
            let [callable] = matching.as_slice() else {
                return unsupported("dynamic realization checked body is absent or ambiguous");
            };
            validate_empty_contract(
                checked,
                callable.realization_machine,
                callable.contract_report_fingerprint,
                callable.contract_commitment,
            )?;
            let published_service_ceiling = if callable.realization_machine
                == plan.realization_machine
                && callable.realization_state == plan.realization_state
            {
                exact_empty_machine_service_ceiling(
                    checked,
                    callable.realization_machine,
                    plan.checked_call_service_reach,
                )?
            } else {
                let summary = exact_machine_service_summary(checked, callable.realization_machine)?;
                validate_empty_service_summary(checked, summary)?;
                let contract = checked
                    .facts
                    .service_reaches
                    .plan_for_machine(callable.realization_machine)
                    .ok_or(LoweringError::Unsupported(
                        "dynamic realization has no service contract",
                    ))?;
                lower_installation_machine_service_ceiling(
                    checked,
                    callable.realization_machine,
                    contract,
                    summary,
                    &[],
                )?
            };
            let scalar_type = terminal_scalar_type(callable.result_type)?;
            let block = block_id(allocate_dense(next_block)?);
            let place = place_id(allocate_dense(next_place)?);
            let edge = edge_id(allocate_dense(next_edge)?);
            let parameter = StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: true,
                structural_type: source_type,
                multiplicity: terminal_projected_source_multiplicity(plan),
                access: match plan.source_access {
                    CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
                    CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
                    _ => unreachable!("borrowed dynamic source access was validated"),
                },
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            };
            let operations = lower_realization_operations(
                &callable.structural_scalar_field_stores,
                &callable.return_expression,
                scalar_type,
                &parameter,
                structural_types,
                next_operation,
                next_value,
            )?;
            let returned = operations
                .last()
                .and_then(|operation| operation.result.scalar())
                .map(|value| value.id)
                .ok_or(LoweringError::Unsupported(
                    "dynamic realization did not emit one scalar result",
                ))?;
            let result_value = value_id(allocate_dense(next_value)?);
            Ok(TerminalMachine {
                id: realization.machine,
                attachment: Some(source_type),
                parameters: Vec::new(),
                structural_parameters: vec![parameter.clone()],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    id: result_value,
                    scalar_type,
                }),
                structural_places: vec![StructuralPlaceDeclaration {
                    id: parameter.place,
                    kind: StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling,
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    id: block,
                    parameters: Vec::new(),
                    operations,
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge,
                        value: returned,
                    },
                }],
                contract: empty_terminal_contract(realization.machine.get()),
            })
        })
        .collect()
}

fn lower_initial_rebound_application(
    checked: &CheckedTrees,
    target_trait_symbol: symbols::SymbolHandle,
    initial: &CheckedDynamicSelectionPlan,
    owner: semantic_vocabulary::MachineId,
) -> Result<ClosedConformanceApplication, LoweringError> {
    let conformance_symbol = initial.fact.conformance.ok_or(LoweringError::Unsupported(
        "initial dynamic selection has no named conformance",
    ))?;
    let conformances = checked
        .typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.symbol == conformance_symbol)
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return unsupported("initial dynamic selection lost its exact conformance declaration");
    };
    let traits = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == target_trait_symbol)
        .collect::<Vec<_>>();
    let [target_trait] = traits.as_slice() else {
        return unsupported("initial dynamic selection lost its exact target trait");
    };
    if initial.fact.target_trait != target_trait_symbol
        || conformance.carrier_symbol != initial.fact.source_data
        || conformance.trait_symbol != initial.fact.target_trait
        || !conformance.lifetime_parameters.is_empty()
        || !checked
            .typed
            .conformance_type_parameters(conformance)
            .is_empty()
        || !checked
            .typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .is_empty()
        || !conformance.trait_lifetime_arguments.is_empty()
        || !target_trait.lifetime_parameters.is_empty()
        || !checked.typed.trait_type_parameters(target_trait).is_empty()
    {
        return unsupported("generic initial dynamic conformance requires a later producer");
    }
    let closed_rows =
        checked
            .typed
            .closed_conformance_rows(conformance)
            .ok_or(LoweringError::Unsupported(
                "initial dynamic selection is not a closed conformance",
            ))?;
    if closed_rows.len() != initial.fact.rows.len() {
        return unsupported("initial dynamic selection row map is incomplete");
    }
    let rows = closed_rows
        .iter()
        .zip(&initial.fact.rows)
        .map(|(closed, retained)| {
            let requirement_identity = evidence_lowering::checked_evidence_requirement_identity(
                checked,
                closed.declaring_trait,
                closed.requirement,
            )?;
            let realization_identity = evidence_lowering::checked_evidence_machine_identity(
                checked,
                closed.realization_machine,
            )?;
            if closed.declaring_trait != retained.declaring_trait
                || closed.requirement != retained.requirement
                || closed.realization_machine != retained.realization_machine
                || closed.realization_state != retained.realization_state
                || requirement_identity != retained.requirement_identity
                || realization_identity != retained.realization_identity
            {
                return unsupported("initial dynamic selection row map drifted from checking");
            }
            Ok(ClosedConformanceRow {
                declaring_trait_identity: checked
                    .symbols
                    .display_path(closed.declaring_trait, "::"),
                public_requirement_identity: requirement_identity,
                requirement_identity: checked.symbols.display_path(closed.requirement, "::"),
                realization_identity: checked.symbols.display_path(closed.realization_state, "::"),
                realization_callable_identity: None,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut application = ClosedConformanceApplication {
        owner,
        declaration_identity: checked.symbols.display_path(conformance_symbol, "::"),
        telescope: Vec::new(),
        subject_identity: Some(initial.type_identity.clone()),
        trait_identity: checked.symbols.display_path(target_trait_symbol, "::"),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: Vec::new(),
        rows,
        report_fingerprint: 0,
        commitment: Default::default(),
    };
    application.report_fingerprint =
        closed_conformance_application_report_fingerprint(&application);
    application.commitment = closed_conformance_application_commitment(&application);
    Ok(application)
}

#[allow(clippy::too_many_arguments)]
fn lower_exact_application(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    owner: semantic_vocabulary::MachineId,
    lowered_realizations: &[LoweredDynamicRealization],
) -> Result<(ClosedConformanceApplication, ClosedConformanceRow), LoweringError> {
    let conformances = checked
        .typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.symbol == plan.selected_conformance)
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return unsupported("direct dynamic selection lost its exact conformance declaration");
    };
    let traits = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == plan.target_trait)
        .collect::<Vec<_>>();
    let [target_trait] = traits.as_slice() else {
        return unsupported("direct dynamic selection lost its exact target trait");
    };
    if conformance.carrier_symbol != plan.selection.source_data
        || conformance.trait_symbol != plan.target_trait
        || !conformance.lifetime_parameters.is_empty()
        || !checked
            .typed
            .conformance_type_parameters(conformance)
            .is_empty()
        || !checked
            .typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .is_empty()
        || !conformance.trait_lifetime_arguments.is_empty()
        || !target_trait.lifetime_parameters.is_empty()
        || !checked.typed.trait_type_parameters(target_trait).is_empty()
    {
        return unsupported("generic dynamic conformance applications require a later producer");
    }
    let closed_rows =
        checked
            .typed
            .closed_conformance_rows(conformance)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic selection is not a closed conformance",
            ))?;
    if closed_rows.len() != plan.selection.rows.len() {
        return unsupported("direct dynamic selection row map is incomplete");
    }

    let mut rows = Vec::with_capacity(closed_rows.len());
    let mut selected_row = None;
    for (closed, retained) in closed_rows.iter().zip(&plan.selection.rows) {
        let requirement_identity = evidence_lowering::checked_evidence_requirement_identity(
            checked,
            closed.declaring_trait,
            closed.requirement,
        )?;
        let realization_identity = evidence_lowering::checked_evidence_machine_identity(
            checked,
            closed.realization_machine,
        )?;
        if closed.declaring_trait != retained.declaring_trait
            || closed.requirement != retained.requirement
            || closed.realization_machine != retained.realization_machine
            || closed.realization_state != retained.realization_state
            || requirement_identity != retained.requirement_identity
            || realization_identity != retained.realization_identity
        {
            return unsupported("direct dynamic selection row map drifted from checking");
        }
        let selected = closed.declaring_trait == plan.declaring_trait
            && closed.requirement == plan.requirement
            && closed.realization_machine == plan.realization_machine
            && closed.realization_state == plan.realization_state;
        let matching_realizations = lowered_realizations
            .iter()
            .filter(|candidate| {
                candidate.source_machine == closed.realization_machine
                    && candidate.source_state == closed.realization_state
                    && candidate.callable_identity == realization_identity
            })
            .collect::<Vec<_>>();
        let matching_realization = match matching_realizations.as_slice() {
            [] if !selected => None,
            [matching] => Some(*matching),
            _ => return unsupported("dynamic conformance row callable is absent or ambiguous"),
        };
        let row = ClosedConformanceRow {
            declaring_trait_identity: checked.symbols.display_path(closed.declaring_trait, "::"),
            public_requirement_identity: requirement_identity,
            requirement_identity: checked.symbols.display_path(closed.requirement, "::"),
            realization_identity: checked.symbols.display_path(closed.realization_state, "::"),
            realization_callable_identity: matching_realization
                .map(|matching| matching.callable_identity.clone()),
        };
        if selected && selected_row.replace(row.clone()).is_some() {
            return unsupported("direct dynamic selected row is duplicated");
        }
        rows.push(row);
    }
    let selected_row = selected_row.ok_or(LoweringError::Unsupported(
        "direct dynamic selected row is absent",
    ))?;
    if selected_row.public_requirement_identity != plan.requirement_identity {
        return unsupported("direct dynamic public requirement identity drifted");
    }

    let mut realization_callables = lowered_realizations
        .iter()
        .map(|callable| ClosedConformanceRealizationCallable {
            source_callable_identity: callable.callable_identity.clone(),
            machine: callable.machine,
            result: callable.result,
        })
        .collect::<Vec<_>>();
    realization_callables.sort();
    realization_callables.dedup();
    if realization_callables.len() != lowered_realizations.len() {
        return unsupported("dynamic realization callable registry is not one-to-one");
    }
    let mut application = ClosedConformanceApplication {
        owner,
        declaration_identity: checked
            .symbols
            .display_path(plan.selected_conformance, "::"),
        telescope: Vec::new(),
        subject_identity: Some(plan.source_type_identity.clone()),
        trait_identity: checked.symbols.display_path(plan.target_trait, "::"),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables,
        rows,
        report_fingerprint: 0,
        commitment: Default::default(),
    };
    application.report_fingerprint =
        closed_conformance_application_report_fingerprint(&application);
    application.commitment = closed_conformance_application_commitment(&application);
    Ok((application, selected_row))
}

fn validate_empty_contract(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    report_fingerprint: u64,
    commitment: checked_trees::MachineContractCommitment,
) -> Result<(), LoweringError> {
    let contract =
        checked
            .facts
            .contract_plans
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic machine has no exact checked contract",
            ))?;
    if report_fingerprint == 0
        || commitment.is_zero()
        || contract.report_fingerprint != report_fingerprint
        || contract.commitment != commitment
        || !contract.closed_scalar_values.requires().is_empty()
        || !contract.closed_scalar_values.ensures().is_empty()
        || contract.closed_scalar_values.has_crash_clauses()
        || contract.closed_scalar_values.has_outcome_specific_clauses()
        || !contract.crash.published().is_empty()
    {
        return unsupported("direct dynamic machine requires an unsupported contract lane");
    }
    Ok(())
}

fn validate_empty_service_summary(
    checked: &CheckedTrees,
    summary: ServiceReachSummary,
) -> Result<(), LoweringError> {
    let mut services = Vec::new();
    collect_service_summary(&checked.facts.service_reaches.rows, summary, &mut services)?;
    if !services.is_empty() {
        return unsupported("direct dynamic scalar call with service reach is unsupported");
    }
    Ok(())
}

fn exact_machine_service_summary(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<ServiceReachSummary, LoweringError> {
    let fact =
        checked
            .facts
            .service_reaches
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic machine has no checked service reach",
            ))?;
    Ok(ServiceReachSummary {
        direct: fact.inferred_direct,
        transitive: fact.inferred_transitive,
    })
}

fn exact_empty_machine_service_ceiling(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    call: ServiceReachSummary,
) -> Result<Vec<semantic_vocabulary::ServiceId>, LoweringError> {
    let summary = exact_machine_service_summary(checked, machine)?;
    validate_empty_service_summary(checked, summary)?;
    let contract = checked
        .facts
        .service_reaches
        .plan_for_machine(machine)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic realization has no service contract",
        ))?;
    if !checked_unit_target_reach_matches(call, contract) {
        return unsupported("direct dynamic call reach drifted from its selected realization");
    }
    lower_installation_machine_service_ceiling(checked, machine, contract, summary, &[])
}

fn terminal_callable_result(
    primitive: PrimitiveType,
) -> Result<ClosedConformanceCallableResult, LoweringError> {
    match primitive {
        PrimitiveType::Bool => Ok(ClosedConformanceCallableResult::Bool),
        PrimitiveType::I32 => Ok(ClosedConformanceCallableResult::I32),
        _ => unsupported("direct dynamic Terminal custody currently admits only Bool and i32"),
    }
}

fn lower_caller_store_operations(
    plan: &CheckedDynamicScalarCallPlan,
    caller_self: &StructuralParameterDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<Vec<Operation>, LoweringError> {
    let Some(store) = &plan.caller_structural_scalar_field_store else {
        return Ok(Vec::new());
    };
    if caller_self.access != StructuralAccess::MutableBorrow
        || store.destination_parameter_position != caller_self.position
        || store.carrier_path != plan.source_path
    {
        return unsupported("direct dynamic caller store lost mutable carrier custody");
    }
    let source_type = lookup_type_id(type_ids, &plan.source_type_identity)?;
    let declaration = structural_types
        .iter()
        .find(|declaration| declaration.id == source_type)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic store carrier type is absent",
        ))?;
    let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return unsupported("direct dynamic store carrier must be a record");
    };
    let scalar_type = terminal_scalar_type(store.primitive_type)?;
    let matching = fields
        .iter()
        .filter(|field| {
            field.identity == store.field_identity
                && !field.relevance.is_erased()
                && field.field_type == terminal_psi::StructuralFieldType::Scalar(scalar_type)
        })
        .collect::<Vec<_>>();
    let [field] = matching.as_slice() else {
        return unsupported("direct dynamic store field is absent or ambiguous");
    };
    let constant = match &store.value {
        CheckedScalarExpression::IntegerLiteral { literal }
            if store.primitive_type.accepts_integer_literal()
                && store.primitive_type != PrimitiveType::Addr =>
        {
            if integer_landing_scalar_type(literal)? != scalar_type {
                return unsupported("direct dynamic store integer landing drifted");
            }
            OperationKind::IntegerConstant {
                value: integer_value(literal, scalar_type)?,
            }
        }
        CheckedScalarExpression::Boolean(boolean)
            if store.primitive_type == PrimitiveType::Bool =>
        {
            let CheckedBooleanExpression::Constant(value) = boolean.as_ref() else {
                return unsupported("direct dynamic store Boolean value is not constant");
            };
            OperationKind::BooleanConstant { value: *value }
        }
        _ => return unsupported("direct dynamic store value is unsupported"),
    };
    Ok(vec![
        Operation {
            id: operation_id(1),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(1),
                scalar_type,
            }),
            kind: constant,
        },
        Operation {
            id: operation_id(2),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: caller_self.place,
                path: lower_structural_path(&store.carrier_path),
                field: field.id,
                value: value_id(1),
            },
        },
    ])
}

fn lower_realization_operations(
    stores: &[checked_trees::CheckedStructuralScalarFieldStorePlan],
    expression: &CheckedScalarExpression,
    expected: semantic_vocabulary::ScalarType,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_operation: &mut u64,
    next_value: &mut u64,
) -> Result<Vec<Operation>, LoweringError> {
    let mut operations = lower_realization_store_operations(
        stores,
        parameter,
        structural_types,
        next_operation,
        next_value,
    )?;
    let operation = operation_id(allocate_dense(next_operation)?);
    let value = value_id(allocate_dense(next_value)?);
    if let CheckedScalarExpression::Boolean(boolean) = expression
        && let CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } = boolean.as_ref()
    {
        let [CheckedStructuralPredicatePathSegment::Field(field_identity)] = path.as_slice() else {
            return unsupported("direct dynamic realization field path is unsupported");
        };
        if *parameter_position != 0 || expected != semantic_vocabulary::ScalarType::Boolean {
            return unsupported("direct dynamic realization field result does not match self");
        }
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic realization self type is absent",
            ))?;
        let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
            return unsupported("direct dynamic realization self must be a record");
        };
        let matching = fields
            .iter()
            .filter(|field| {
                field.identity == *field_identity
                    && field.field_type
                        == terminal_psi::StructuralFieldType::Scalar(
                            semantic_vocabulary::ScalarType::Boolean,
                        )
            })
            .collect::<Vec<_>>();
        let [field] = matching.as_slice() else {
            return unsupported("direct dynamic realization Boolean field is absent or ambiguous");
        };
        operations.push(Operation {
            id: operation,
            result: OperationResult::Scalar(ValueDeclaration {
                id: value,
                scalar_type: semantic_vocabulary::ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                source: parameter.place,
                field: field.id,
            },
        });
        return Ok(operations);
    }

    if let CheckedScalarExpression::StructuralParameterField {
        parameter_position,
        path,
        primitive_type: PrimitiveType::I32,
    } = expression
    {
        let [CheckedStructuralPredicatePathSegment::Field(field_identity)] = path.as_slice() else {
            return unsupported("direct dynamic realization integer field path is unsupported");
        };
        if *parameter_position != 0 || expected != terminal_scalar_type(PrimitiveType::I32)? {
            return unsupported(
                "direct dynamic realization integer field result does not match self",
            );
        }
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic realization self type is absent",
            ))?;
        let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
            return unsupported("direct dynamic realization self must be a record");
        };
        let matching = fields
            .iter()
            .filter(|field| {
                field.identity == *field_identity
                    && field.field_type == terminal_psi::StructuralFieldType::Scalar(expected)
            })
            .collect::<Vec<_>>();
        let [field] = matching.as_slice() else {
            return unsupported("direct dynamic realization integer field is absent or ambiguous");
        };
        operations.push(Operation {
            id: operation,
            result: OperationResult::Scalar(ValueDeclaration {
                id: value,
                scalar_type: expected,
            }),
            kind: OperationKind::IntegerStructuralField {
                source: parameter.place,
                field: field.id,
            },
        });
        return Ok(operations);
    }

    unsupported("direct dynamic realization must return one exact Boolean or i32 self field")
}

fn lower_realization_store_operations(
    stores: &[checked_trees::CheckedStructuralScalarFieldStorePlan],
    parameter: &StructuralParameterDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_operation: &mut u64,
    next_value: &mut u64,
) -> Result<Vec<Operation>, LoweringError> {
    if stores.len() > 3 {
        return unsupported("dynamic realization has too many structural stores");
    }
    let mut operations = Vec::with_capacity(stores.len() * 2);
    for (statement_index, store) in stores.iter().enumerate() {
        if stores[..statement_index].iter().any(|earlier| {
            earlier.carrier_path == store.carrier_path
                && earlier.field_identity == store.field_identity
        }) {
            return unsupported("dynamic realization repeats a structural store destination");
        }
        operations.extend(lower_realization_store_operation(
            store,
            statement_index,
            parameter,
            structural_types,
            next_operation,
            next_value,
        )?);
    }
    Ok(operations)
}

fn lower_realization_store_operation(
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    statement_index: usize,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    next_operation: &mut u64,
    next_value: &mut u64,
) -> Result<Vec<Operation>, LoweringError> {
    let expected_statement_index = u32::try_from(statement_index)
        .map_err(|_| LoweringError::Unsupported("dynamic store index exceeds u32"))?;
    let lowered = crate::structural_scalar_store::lower_structural_scalar_store_destination(
        store,
        expected_statement_index,
        parameter,
        structural_types,
        &[],
        &[],
        crate::structural_scalar_store::StoreAccessPolicy::MutableOnly,
    )?;
    let scalar_type = lowered.scalar_type;
    let constant = match &store.value {
        CheckedScalarExpression::IntegerLiteral { literal }
            if store.primitive_type.accepts_integer_literal()
                && store.primitive_type != PrimitiveType::Addr =>
        {
            if integer_landing_scalar_type(literal)? != scalar_type {
                return unsupported("dynamic realization store integer landing drifted");
            }
            OperationKind::IntegerConstant {
                value: integer_value(literal, scalar_type)?,
            }
        }
        CheckedScalarExpression::Boolean(boolean)
            if store.primitive_type == PrimitiveType::Bool =>
        {
            let CheckedBooleanExpression::Constant(value) = boolean.as_ref() else {
                return unsupported("dynamic realization store Boolean value is not constant");
            };
            OperationKind::BooleanConstant { value: *value }
        }
        _ => return unsupported("dynamic realization store value is unsupported"),
    };
    let constant_operation = operation_id(allocate_dense(next_operation)?);
    let constant_value = value_id(allocate_dense(next_value)?);
    let store_operation = operation_id(allocate_dense(next_operation)?);
    Ok(vec![
        Operation {
            id: constant_operation,
            result: OperationResult::Scalar(ValueDeclaration {
                id: constant_value,
                scalar_type,
            }),
            kind: constant,
        },
        Operation {
            id: store_operation,
            result: OperationResult::Unit,
            kind: OperationKind::StructuralScalarFieldStore {
                destination: parameter.place,
                path: lowered.path,
                field: lowered.field,
                value: constant_value,
            },
        },
    ])
}

fn empty_terminal_contract(identity: u64) -> MachineContract {
    MachineContract {
        id: contract_id(identity),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}
