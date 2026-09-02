//! Source-free lowering for bounded local dynamic scalar calls.
//!
//! A never-rebound value lowers to a direct call. A value rebound exactly once
//! retains two selections and an indirect descriptor call. Both lanes preserve
//! the exact conformance application, source field subloan, requirement row,
//! and realization callable in Terminal custody.

use psi_checked_trees::{
    CheckedBooleanExpression, CheckedDynamicScalarCallPlan, CheckedDynamicSelectionPlan,
    CheckedReboundDynamicScalarCallPlan, CheckedScalarExpression, CheckedStructuralAccess,
    CheckedStructuralPredicatePathSegment, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape,
};
use psi_core::StructuralPlaceKind;
use psi_language_semantics::{Multiplicity, ServiceReachSummary};
use psi_terminal::{
    Block, ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, MachineContract, Operation,
    OperationKind, OperationResult, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, TerminalDirectDynamicDispatch,
    TerminalDynamicConformanceSelection, TerminalDynamicDispatchCatalog,
    TerminalIndirectDynamicDispatch, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalReboundDynamicDescriptor, Terminator, ValueDeclaration, VocabularyMarker,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};

use super::*;

mod continuation;

struct DynamicCallerShape {
    attachment_type_identity: String,
}

#[derive(Clone)]
struct LoweredDynamicRealization {
    source_machine: psi_symbols::SymbolHandle,
    source_state: psi_symbols::SymbolHandle,
    callable_identity: String,
    machine: psi_core::MachineId,
    result: ClosedConformanceCallableResult,
}

#[derive(Clone, Copy)]
enum DynamicLoweringLane<'a> {
    Direct,
    Rebound(&'a CheckedDynamicSelectionPlan),
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
    let call_operation = operation_id(if has_caller_store { 3 } else { 1 });
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
    let (dynamic_dispatch, call_kind) = lower_dynamic_call_custody(
        lane,
        &caller_self,
        plan,
        &structural_types,
        &type_ids,
        caller_machine,
        call_operation,
        source,
        &application,
        &selected_row,
        callable_identity,
        realization_machine,
    )?;

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
    caller_operations.push(Operation {
        id: call_operation,
        result: OperationResult::Scalar(ValueDeclaration {
            id: call_result_value,
            scalar_type: call_result_type,
        }),
        kind: call_kind,
    });
    let mut next_block = 2_u64;
    let mut next_place = 2_u64;
    let mut next_operation = if has_caller_store { 4 } else { 2 };
    let mut next_value = if has_caller_store { 3 } else { 2 };
    let mut next_edge = 2_u64;
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
            closed_conformance_applications: vec![application],
            dynamic_dispatch,
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
                machines
            },
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences: vec![LoweredSourceCallOccurrence {
            source_site: None,
            source_state: plan.caller_state,
            statement_index: usize::try_from(plan.coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("direct dynamic statement coordinate exceeds usize")
            })?,
            call_ordinal: usize::try_from(plan.coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("direct dynamic call ordinal exceeds usize")
            })?,
            terminal_operation: call_operation,
            source_target: plan.requirement,
        }],
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
        || initial.fact.conformance != Some(plan.selected_conformance)
        || initial.fact.source_symbol != initial.field
        || initial.fact.source_data != plan.selection.source_data
        || initial.fact.rows != plan.selection.rows
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
    )
}

fn validate_exact_dynamic_plan(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    selection_statement_index: usize,
    call_statement_index: u32,
) -> Result<DynamicCallerShape, LoweringError> {
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
    if checked
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
            .is_some()
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
                && call.receiver_symbol == plan.receiver_binding
                && call.target_symbol == plan.requirement
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
        || call.receiver_symbol != plan.receiver_binding
        || call.target_symbol != plan.requirement
        || !call.has_receiver
        || call.service_reach != plan.checked_call_service_reach
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
        || plan.source_access != CheckedStructuralAccess::SharedBorrow
    {
        return unsupported("direct dynamic source must be an exact shared field subloan");
    }
    let [CheckedUnitStructuralPathSegment::Field(_)] = plan.source_path.as_slice() else {
        return unsupported("direct dynamic source must be one exact attachment field");
    };
    if let Some(store) = store
        && (store.statement_index != 0
            || store.destination_parameter_position != plan.source_parameter_position
            || store.carrier_path != plan.source_path
            || !checked_store_literal_matches(&store.value, store.primitive_type))
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

fn checked_store_literal_matches(
    value: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> bool {
    match (value, primitive_type) {
        (CheckedScalarExpression::IntegerLiteral { .. }, PrimitiveType::I32) => true,
        (CheckedScalarExpression::Boolean(boolean), PrimitiveType::Bool) => {
            matches!(boolean.as_ref(), CheckedBooleanExpression::Constant(_))
        }
        _ => false,
    }
}

fn validate_and_lower_source(
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDynamicScalarCallPlan,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    type_ids: &[(String, psi_core::StructuralTypeId)],
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
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    type_ids: &[(String, psi_core::StructuralTypeId)],
) -> Result<StructuralArgument, LoweringError> {
    if plan.source_parameter_position != caller_self.position
        || plan.caller_parameter_access
            != match caller_self.access {
                StructuralAccess::SharedBorrow => CheckedStructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow => CheckedStructuralAccess::MutableBorrow,
                _ => return unsupported("direct dynamic caller self access is unsupported"),
            }
        || !caller_self.is_self
        || caller_self.multiplicity != terminal_structural_multiplicity(plan.caller_multiplicity)
        || plan.source_access != CheckedStructuralAccess::SharedBorrow
    {
        return unsupported("direct dynamic caller self does not license a shared field subloan");
    }
    let attachment_id = lookup_type_id(type_ids, &plan.caller_attachment_type_identity)?;
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
    let psi_terminal::StructuralTypeShape::Record { fields } = &attachment.shape else {
        return unsupported("direct dynamic caller attachment must be a record");
    };
    let matching_fields = fields
        .iter()
        .filter(|field| {
            field.identity == *field_identity
                && field.field_type == psi_terminal::StructuralFieldType::Structural(source_type)
        })
        .count();
    if matching_fields != 1 {
        return unsupported("direct dynamic source field no longer matches its structural carrier");
    }
    Ok(StructuralArgument {
        place: caller_self.place,
        path: lower_structural_path(source_path),
        access: StructuralAccess::SharedBorrow,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_dynamic_call_custody(
    lane: DynamicLoweringLane<'_>,
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDynamicScalarCallPlan,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    type_ids: &[(String, psi_core::StructuralTypeId)],
    caller_machine: psi_core::MachineId,
    call_operation: psi_core::OperationId,
    latest_source: StructuralArgument,
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
    callable_identity: String,
    realization_machine: psi_core::MachineId,
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
    Ok(match lane {
        DynamicLoweringLane::Direct => (
            TerminalDynamicDispatchCatalog {
                selections: vec![latest_selection],
                rebound_descriptors: Vec::new(),
                direct_dispatches: vec![TerminalDirectDynamicDispatch {
                    owner: caller_machine,
                    operation: call_operation,
                    selection_ordinal: 0,
                    declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
                    public_requirement_identity: selected_row.public_requirement_identity.clone(),
                    requirement_identity: selected_row.requirement_identity.clone(),
                    realization_identity: selected_row.realization_identity.clone(),
                    realization_callable_identity: callable_identity,
                    realization: realization_machine,
                }],
                indirect_dispatches: Vec::new(),
            },
            OperationKind::CallStructuralScalar {
                callee: realization_machine,
                structural_arguments: vec![latest_source],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ),
        DynamicLoweringLane::Rebound(initial) => {
            let initial_source = validate_and_lower_selection_source(
                caller_self,
                plan,
                &initial.path,
                &initial.type_identity,
                structural_types,
                type_ids,
            )?;
            (
                TerminalDynamicDispatchCatalog {
                    selections: vec![
                        TerminalDynamicConformanceSelection {
                            owner: caller_machine,
                            ordinal: 0,
                            source: initial_source,
                            conformance_application_report_fingerprint: application
                                .report_fingerprint,
                            conformance_application_commitment: application.commitment,
                        },
                        latest_selection,
                    ],
                    rebound_descriptors: vec![TerminalReboundDynamicDescriptor {
                        owner: caller_machine,
                        ordinal: 0,
                        initial_selection_ordinal: 0,
                        rebound_selection_ordinal: 1,
                    }],
                    direct_dispatches: Vec::new(),
                    indirect_dispatches: vec![row_dispatch(0)],
                },
                OperationKind::CallDynamicScalar {
                    descriptor_ordinal: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            )
        }
    })
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
    match plan.caller_multiplicity {
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
        Vec<psi_terminal::StructuralTypeDeclaration>,
        Vec<(String, psi_core::StructuralTypeId)>,
    ),
    LoweringError,
> {
    if caller_attachment != plan.caller_attachment_type_identity {
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
    let [CheckedUnitStructuralPathSegment::Field(source_field)] = plan.source_path.as_slice()
    else {
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
                        type_identity: plan.source_type_identity.clone(),
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
            plan.source_type_identity.clone(),
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
    source_type: psi_core::StructuralTypeId,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
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
            let operation = operation_id(allocate_dense(next_operation)?);
            let operation_value = value_id(allocate_dense(next_value)?);
            let result_value = value_id(allocate_dense(next_value)?);
            let edge = edge_id(allocate_dense(next_edge)?);
            let parameter = StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: true,
                structural_type: source_type,
                multiplicity: terminal_projected_source_multiplicity(plan),
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            };
            let operations = lower_realization_operations(
                &callable.return_expression,
                scalar_type,
                &parameter,
                structural_types,
                operation,
                operation_value,
            )?;
            let returned = operations
                .last()
                .and_then(|operation| operation.result.scalar())
                .map(|value| value.id)
                .ok_or(LoweringError::Unsupported(
                    "dynamic realization did not emit one scalar result",
                ))?;
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

#[allow(clippy::too_many_arguments)]
fn lower_exact_application(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    owner: psi_core::MachineId,
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
    machine: psi_symbols::SymbolHandle,
    report_fingerprint: u64,
    commitment: psi_checked_trees::MachineContractCommitment,
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
    machine: psi_symbols::SymbolHandle,
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
    machine: psi_symbols::SymbolHandle,
    call: ServiceReachSummary,
) -> Result<Vec<psi_core::ServiceId>, LoweringError> {
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
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    type_ids: &[(String, psi_core::StructuralTypeId)],
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
    let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape else {
        return unsupported("direct dynamic store carrier must be a record");
    };
    let scalar_type = terminal_scalar_type(store.primitive_type)?;
    let matching = fields
        .iter()
        .filter(|field| {
            field.identity == store.field_identity
                && !field.relevance.is_erased()
                && field.field_type == psi_terminal::StructuralFieldType::Scalar(scalar_type)
        })
        .collect::<Vec<_>>();
    let [field] = matching.as_slice() else {
        return unsupported("direct dynamic store field is absent or ambiguous");
    };
    let constant = match &store.value {
        CheckedScalarExpression::IntegerLiteral { literal }
            if store.primitive_type == PrimitiveType::I32 =>
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
    expression: &CheckedScalarExpression,
    expected: psi_core::ScalarType,
    parameter: &StructuralParameterDeclaration,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    operation: psi_core::OperationId,
    value: psi_core::ValueId,
) -> Result<Vec<Operation>, LoweringError> {
    if let CheckedScalarExpression::Boolean(boolean) = expression
        && let CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } = boolean.as_ref()
    {
        let [CheckedStructuralPredicatePathSegment::Field(field_identity)] = path.as_slice() else {
            return unsupported("direct dynamic realization field path is unsupported");
        };
        if *parameter_position != 0 || expected != psi_core::ScalarType::Boolean {
            return unsupported("direct dynamic realization field result does not match self");
        }
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == parameter.structural_type)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic realization self type is absent",
            ))?;
        let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape else {
            return unsupported("direct dynamic realization self must be a record");
        };
        let matching = fields
            .iter()
            .filter(|field| {
                field.identity == *field_identity
                    && field.field_type
                        == psi_terminal::StructuralFieldType::Scalar(psi_core::ScalarType::Boolean)
            })
            .collect::<Vec<_>>();
        let [field] = matching.as_slice() else {
            return unsupported("direct dynamic realization Boolean field is absent or ambiguous");
        };
        return Ok(vec![Operation {
            id: operation,
            result: OperationResult::Scalar(ValueDeclaration {
                id: value,
                scalar_type: psi_core::ScalarType::Boolean,
            }),
            kind: OperationKind::BooleanStructuralField {
                source: parameter.place,
                field: field.id,
            },
        }]);
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
        let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape else {
            return unsupported("direct dynamic realization self must be a record");
        };
        let matching = fields
            .iter()
            .filter(|field| {
                field.identity == *field_identity
                    && field.field_type == psi_terminal::StructuralFieldType::Scalar(expected)
            })
            .collect::<Vec<_>>();
        let [field] = matching.as_slice() else {
            return unsupported("direct dynamic realization integer field is absent or ambiguous");
        };
        return Ok(vec![Operation {
            id: operation,
            result: OperationResult::Scalar(ValueDeclaration {
                id: value,
                scalar_type: expected,
            }),
            kind: OperationKind::IntegerStructuralField {
                source: parameter.place,
                field: field.id,
            },
        }]);
    }

    unsupported("direct dynamic realization must return one exact Boolean or i32 self field")
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
