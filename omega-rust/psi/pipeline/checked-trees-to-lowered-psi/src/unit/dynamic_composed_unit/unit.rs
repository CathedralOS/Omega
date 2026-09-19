//! Source-free Terminal custody for result-less dynamic requirement calls.
//!
//! This lane shares descriptor/application infrastructure with scalar dynamic
//! dispatch. The selected invocation is Unit-typed; other members in its complete
//! conformance table retain their own scalar or Unit bodies.
use super::realizations::{
    collect_dynamic_realizations, materialize_dynamic_realizations, retain_realizations_for_lane,
};
use super::{
    Block, CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralPathSegment,
    ClosedConformanceApplication, ClosedConformanceCallableResult, ClosedConformanceRow,
    LoweredPsi, LoweredSourceCallOccurrence, LoweringError, Multiplicity, Operation, OperationKind,
    OperationResult, ProofBundle, StructuralAccess, StructuralArgument,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralPlaceKind,
    TerminalDirectDynamicDispatch, TerminalDynamicConformanceSelection,
    TerminalDynamicDescriptorArgument, TerminalDynamicDescriptorParameter,
    TerminalDynamicDescriptorSource, TerminalDynamicDispatchCatalog,
    TerminalIndirectDynamicDispatch, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalParameterDynamicDispatch, TerminalReboundDynamicDescriptor, Terminator,
    VocabularyMarker, allocate_dense, block_id, edge_id, evidence_lowering, lookup_type_id,
    lower_installation_machine_service_ceiling, lower_root_service_reach, machine_id, operation_id,
    place_id, unsupported,
};
use crate::unit::dynamic_composed_unit::applications::{
    count_selected_family_rows, exact_machine_service_summary, lower_exact_application,
    lower_initial_rebound_application, validate_empty_contract, validate_empty_service_summary,
};
use crate::unit::dynamic_composed_unit::dynamic_lanes::{
    DynamicLoweringLane, LoweredDynamicRealization,
};
use crate::unit::dynamic_composed_unit::plan_validation::{
    validate_forwarded_dynamic_call_coordinates, validate_parameter_forwarding_call,
};
use crate::unit::dynamic_composed_unit::source_lowering::{
    dynamic_parameter_interface, validate_and_lower_dynamic_source,
};
use crate::unit::dynamic_composed_unit::store_operations::empty_terminal_contract;
use crate::unit::dynamic_composed_unit::structural_types::{
    lower_dynamic_structural_types_for_source, terminal_structural_multiplicity,
};
use checked_trees::{
    CheckedDynamicUnitCallOrigin, CheckedDynamicUnitCallPlan, CheckedReboundDynamicUnitCallPlan,
};

#[derive(Clone, Copy)]
pub(super) struct ForwardedUnitHelperIds {
    pub(super) machine: semantic_vocabulary::MachineId,
    pub(super) block: semantic_vocabulary::BlockId,
    pub(super) operation: semantic_vocabulary::OperationId,
    pub(super) edge: semantic_vocabulary::EdgeId,
}

pub(super) fn lower_direct_dynamic_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedDynamicUnitCallPlan,
) -> Result<LoweredPsi, LoweringError> {
    lower_dynamic_unit_machine(checked, plan, DynamicLoweringLane::Direct)
}

pub(super) fn lower_rebound_dynamic_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedReboundDynamicUnitCallPlan,
) -> Result<LoweredPsi, LoweringError> {
    lower_dynamic_unit_machine(
        checked,
        &plan.latest,
        DynamicLoweringLane::Rebound(&plan.initial),
    )
}

fn lower_dynamic_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedDynamicUnitCallPlan,
    lane: DynamicLoweringLane<'_>,
) -> Result<LoweredPsi, LoweringError> {
    validate_exact_unit_plan(checked, plan, lane)?;
    let (structural_types, type_ids) = lower_dynamic_structural_types_for_source(
        checked,
        &plan.caller_attachment_type_identity,
        &plan.caller_attachment_type_identity,
        &plan.source_path,
        &plan.source_type_identity,
    )?;
    let caller_attachment = lookup_type_id(&type_ids, &plan.caller_attachment_type_identity)?;
    let caller_self = StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: true,
        structural_type: caller_attachment,
        multiplicity: terminal_structural_multiplicity(plan.caller_multiplicity),
        access: match plan.caller_parameter_access {
            CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
            CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
            _ => return unsupported("dynamic Unit caller requires a borrowed self parameter"),
        },
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let latest_source = validate_and_lower_dynamic_source(
        &caller_self,
        plan.source_parameter_position,
        plan.caller_parameter_access,
        plan.caller_multiplicity,
        plan.source_access,
        &plan.caller_attachment_type_identity,
        &plan.source_path,
        &plan.source_type_identity,
        &structural_types,
        &type_ids,
    )?;
    let caller_machine = machine_id(1);
    let call_operation = operation_id(1);
    let source_type = lookup_type_id(&type_ids, &plan.source_type_identity)?;
    let callable_table = plan.into();
    let all_realizations = collect_dynamic_realizations(checked, &callable_table, 2)?;
    let lowered_realizations =
        retain_realizations_for_lane(&all_realizations, &callable_table, lane)?;
    let selected = lowered_realizations
        .iter()
        .filter(|candidate| {
            candidate.source_machine == plan.realization_machine
                && candidate.source_state == plan.realization_state
        })
        .collect::<Vec<_>>();
    let [selected] = selected.as_slice() else {
        return unsupported("dynamic Unit selected realization is absent or ambiguous");
    };
    if selected.result != ClosedConformanceCallableResult::Unit
        || selected.checked_identity != plan.realization_identity
    {
        return unsupported("dynamic Unit selected realization callable drifted");
    }
    let realization_machine = selected.machine;
    let callable_identity = selected.callable_identity.clone();
    let (application, selected_row) = lower_exact_application(
        checked,
        &callable_table,
        caller_machine,
        &lowered_realizations,
    )?;
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
    let mut next_operation = 2_u64;
    let mut next_edge = 2_u64;
    let mut next_value = 1_u64;
    let forwarded_helpers = forwarded_unit_helper_ids(
        plan,
        &lowered_realizations,
        &mut next_block,
        &mut next_operation,
        &mut next_edge,
    )?;
    let (dynamic_dispatch, call_kind) = lower_unit_call_custody(
        lane,
        &caller_self,
        plan,
        &structural_types,
        &type_ids,
        caller_machine,
        call_operation,
        latest_source,
        initial_application.as_ref(),
        &application,
        &selected_row,
        callable_identity,
        realization_machine,
        forwarded_helpers.first().copied(),
    )?;
    let mut dynamic_dispatch = dynamic_dispatch;
    if forwarded_helpers.len() > 1 {
        extend_unit_parameter_forwarding_catalog(&mut dynamic_dispatch, &forwarded_helpers)?;
    }
    let caller_reach = lower_installation_machine_service_ceiling(
        checked,
        plan.caller_machine,
        checked
            .facts
            .service_reaches
            .plan_for_machine(plan.caller_machine)
            .ok_or(LoweringError::Unsupported(
                "dynamic Unit caller has no checked service contract",
            ))?,
        exact_machine_service_summary(checked, plan.caller_machine)?,
        &[],
    )?;
    let root_service_reach = lower_root_service_reach(checked, plan.caller_machine, &[])?;
    let realization_machines = materialize_dynamic_realizations(
        checked,
        &callable_table,
        &lowered_realizations,
        source_type,
        &structural_types,
        &mut next_block,
        &mut next_place,
        &mut next_operation,
        &mut next_value,
        &mut next_edge,
    )?;
    let forwarded_helper_machines = materialize_forwarded_unit_helper_chain(
        checked,
        plan,
        &application,
        &selected_row,
        &forwarded_helpers,
    )?;
    let caller_block = block_id(1);

    Ok(LoweredPsi {
        semantic_module: TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
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
                    closed_reach_application: None,
                    declared_service_reach: Vec::new(),
                    id: caller_machine,
                    attachment: Some(caller_attachment),
                    parameters: Vec::new(),
                    structural_parameters: vec![caller_self.clone()],
                    ranked_scc: None,
                    result: TerminalMachineResult::Unit,
                    structural_places: vec![StructuralPlaceDeclaration {
                        id: caller_self.place,
                        kind: StructuralPlaceKind::Parameter {
                            position: caller_self.position,
                            is_self: caller_self.is_self,
                        },
                    }],
                    entry_claims: Vec::new(),
                    published_service_ceiling: caller_reach,
                    content_entry_claims: Vec::new(),
                    content_identity_reshuffles: Vec::new(),
                    content_partition_compositions: Vec::new(),
                    entry: caller_block,
                    blocks: vec![Block {
                        structural_parameters: Vec::new(),
                        id: caller_block,
                        parameters: Vec::new(),
                        erased_scalar_formals: Vec::new(),
                        operations: vec![Operation {
                            static_reach_binding: None,
                            id: call_operation,
                            result: OperationResult::Unit,
                            kind: call_kind,
                        }],
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
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: Vec::new(),
        },
        debug_map: None,
        source_call_occurrences: unit_source_call_occurrences_for_chain(
            plan,
            call_operation,
            &forwarded_helpers,
        )?,
        selected_ieee_float_fma_occurrences: Vec::new(),
        selected_ieee_float_comparison_occurrences: Vec::new(),
        selected_integer_comparison_occurrences: Vec::new(),
    })
}

pub(super) fn validate_exact_unit_plan(
    checked: &CheckedTrees,
    plan: &CheckedDynamicUnitCallPlan,
    lane: DynamicLoweringLane<'_>,
) -> Result<(), LoweringError> {
    let (selection_statement_index, call_statement_index, initial) = match lane {
        DynamicLoweringLane::Direct => (0, 1, None),
        DynamicLoweringLane::Rebound(initial) => (1, 2, Some(initial)),
        DynamicLoweringLane::Stored(_) => {
            return unsupported("stored descriptor cannot enter Unit dynamic lowering");
        }
    };
    if plan.selection.statement_index != selection_statement_index
        || plan.coordinate.statement_index != call_statement_index
        || plan.coordinate.call_ordinal != 0
        || plan.selection.machine != plan.caller_machine
        || plan.selection.state != plan.caller_state
        || plan.selection.binding != plan.receiver_binding
        || plan.selection.target_trait != plan.target_trait
        || plan.selection.conformance != Some(plan.selected_conformance)
        || plan.selection.source_symbol != plan.source_field
        || checked
            .facts
            .dynamic_conformances
            .binding_facts()
            .selections
            .into_iter()
            .filter(|selection| selection == &plan.selection)
            .count()
            != 1
    {
        return unsupported("dynamic Unit plan no longer matches its checked selection");
    }
    if let Some(initial) = initial
        && (initial.fact.statement_index != 0
            || initial.fact.binding != plan.receiver_binding
            || initial.fact.target_trait != plan.target_trait
            || initial.fact.conformance.is_none()
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
                != 1)
    {
        return unsupported("rebound dynamic Unit selection versions drifted from checking");
    }
    // The retained selection rows name the provider template; a finite-family
    // plan names the tuple's bare specialization instance. The join expands
    // each retained row's family roster and requires the plan's
    // `(family_tuple, realization_machine, realization_state)` to land on
    // exactly one expanded row; `realization_identity` must equal the bare
    // normalized identity of the instance the plan names.
    let realization_identity =
        evidence_lowering::checked_dynamic_machine_identity(checked, plan.realization_machine)?;
    if realization_identity != plan.realization_identity {
        return unsupported("dynamic Unit realization identity drifted from checking");
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
                && matches!(
                    callable.body,
                    checked_trees::CheckedDynamicRealizationBodyPlan::Unit
                )
        })
        .count();
    if selected_rows != 1 || selected_callables != 1 {
        return unsupported("dynamic Unit selected row or callable is absent or ambiguous");
    }
    let state = checked
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
    let [state] = state.as_slice() else {
        return unsupported("dynamic Unit caller has no exact checked flow state");
    };
    let calls = checked.facts.flow.control.calls.span_or_empty(state.calls);
    let forwarded_coordinates_match = match plan.origin {
        CheckedDynamicUnitCallOrigin::Local => true,
        CheckedDynamicUnitCallOrigin::Forwarded {
            machine,
            state,
            coordinate,
            parameter,
        } => validate_forwarded_dynamic_call_coordinates(
            checked,
            plan.requirement,
            plan.checked_call_service_reach,
            machine,
            state,
            coordinate,
            parameter,
        )?,
    };
    let forwarded_path_matches = match plan.origin {
        CheckedDynamicUnitCallOrigin::Local => plan.forwarding_transfers.is_empty(),
        CheckedDynamicUnitCallOrigin::Forwarded {
            machine,
            state,
            parameter,
            ..
        } => validate_unit_forwarding_transfer_path(checked, plan, machine, state, parameter)?,
    };
    let matching = calls
        .iter()
        .filter(|call| {
            call.statement_index == plan.coordinate.statement_index as usize
                && call.call_ordinal == 0
                && match plan.origin {
                    CheckedDynamicUnitCallOrigin::Local => {
                        call.has_receiver
                            && call.receiver_symbol == plan.receiver_binding
                            && call.target_symbol == plan.requirement
                            && call.service_reach == plan.checked_call_service_reach
                    }
                    CheckedDynamicUnitCallOrigin::Forwarded { state, .. } => {
                        let first_state = plan
                            .forwarding_transfers
                            .first()
                            .map(|transfer| transfer.caller_state)
                            .unwrap_or(state);
                        !call.has_receiver
                            && call.target_symbol == first_state
                            && forwarded_coordinates_match
                            && forwarded_path_matches
                    }
                }
        })
        .count();
    if matching != 1
        || checked
            .facts
            .flow
            .control
            .statements
            .span_or_empty(state.statements)
            .len()
            != plan.coordinate.statement_index as usize + 1
    {
        return unsupported("dynamic Unit call drifted from checked flow custody");
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
        || !matches!(
            plan.source_access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        )
        || (plan.source_access == CheckedStructuralAccess::MutableBorrow
            && plan.caller_parameter_access != CheckedStructuralAccess::MutableBorrow)
        || !matches!(
            plan.source_path.as_slice(),
            [CheckedUnitStructuralPathSegment::Field(_)]
        )
    {
        return unsupported("dynamic Unit source must be an exact borrowed field subloan");
    }
    validate_empty_service_summary(checked, plan.checked_call_service_reach)?;
    let caller_reach = exact_machine_service_summary(checked, plan.caller_machine)?;
    if caller_reach != plan.caller_service_reach {
        return unsupported("dynamic Unit caller service reach drifted from checking");
    }
    validate_empty_service_summary(checked, caller_reach)
}

#[allow(clippy::too_many_arguments)]
fn lower_unit_call_custody(
    lane: DynamicLoweringLane<'_>,
    caller_self: &StructuralParameterDeclaration,
    plan: &CheckedDynamicUnitCallPlan,
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
    forwarded_helper: Option<ForwardedUnitHelperIds>,
) -> Result<(TerminalDynamicDispatchCatalog, OperationKind), LoweringError> {
    let latest_selection = TerminalDynamicConformanceSelection {
        owner: caller_machine,
        ordinal: u32::from(matches!(lane, DynamicLoweringLane::Rebound(_))),
        source: latest_source.clone(),
        conformance_application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment,
    };
    Ok(match lane {
        DynamicLoweringLane::Direct => {
            if initial_application.is_some() {
                return unsupported("direct dynamic Unit dispatch retained a rebound application");
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
                OperationKind::CallUnit {
                    callee: helper.machine,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
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
                        family_tuple: selected_row.family_tuple.clone(),
                        requirement_identity: selected_row.requirement_identity.clone(),
                        realization_identity: selected_row.realization_identity.clone(),
                        realization_callable_identity: callable_identity,
                        realization: realization_machine,
                    });
                OperationKind::CallUnit {
                    callee: realization_machine,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    structural_arguments: vec![latest_source],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            };
            (catalog, call_kind)
        }
        DynamicLoweringLane::Rebound(initial) => {
            let initial_source = validate_and_lower_dynamic_source(
                caller_self,
                plan.source_parameter_position,
                plan.caller_parameter_access,
                plan.caller_multiplicity,
                plan.source_access,
                &plan.caller_attachment_type_identity,
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
                OperationKind::CallUnit {
                    callee: helper.machine,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            } else {
                catalog
                    .indirect_dispatches
                    .push(TerminalIndirectDynamicDispatch {
                        owner: caller_machine,
                        operation: call_operation,
                        descriptor_ordinal: 0,
                        declaring_trait_identity: selected_row.declaring_trait_identity.clone(),
                        public_requirement_identity: selected_row
                            .public_requirement_identity
                            .clone(),
                        family_tuple: selected_row.family_tuple.clone(),
                        requirement_identity: selected_row.requirement_identity.clone(),
                        realization_identity: selected_row.realization_identity.clone(),
                        realization_callable_identity: callable_identity,
                        realization: realization_machine,
                    });
                OperationKind::CallDynamicUnit {
                    descriptor_ordinal: 0,
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                }
            };
            (catalog, call_kind)
        }
        DynamicLoweringLane::Stored(_) => {
            return unsupported("stored descriptor cannot enter Unit dynamic lowering");
        }
    })
}

fn validate_unit_forwarding_transfer_path(
    checked: &CheckedTrees,
    plan: &CheckedDynamicUnitCallPlan,
    final_machine: symbols::SymbolHandle,
    final_state: symbols::SymbolHandle,
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

pub(super) fn extend_unit_parameter_forwarding_catalog(
    catalog: &mut TerminalDynamicDispatchCatalog,
    helpers: &[ForwardedUnitHelperIds],
) -> Result<(), LoweringError> {
    let [template] = catalog.parameters.as_slice() else {
        return unsupported("multi-hop dynamic Unit forwarding lost its first parameter interface");
    };
    let template = template.clone();
    let [dispatch] = catalog.parameter_dispatches.as_mut_slice() else {
        return unsupported("multi-hop dynamic Unit forwarding lost its final parameter dispatch");
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
        "multi-hop dynamic Unit forwarding has no final helper",
    ))?;
    dispatch.owner = final_helper.machine;
    dispatch.operation = final_helper.operation;
    Ok(())
}

pub(super) fn forwarded_unit_helper_ids(
    plan: &CheckedDynamicUnitCallPlan,
    realizations: &[LoweredDynamicRealization],
    next_block: &mut u64,
    next_operation: &mut u64,
    next_edge: &mut u64,
) -> Result<Vec<ForwardedUnitHelperIds>, LoweringError> {
    if !matches!(plan.origin, CheckedDynamicUnitCallOrigin::Forwarded { .. }) {
        if !plan.forwarding_transfers.is_empty() {
            return unsupported("local dynamic Unit call retained forwarding transfers");
        }
        return Ok(Vec::new());
    }
    let first_machine = realizations
        .iter()
        .map(|realization| realization.machine.get())
        .max()
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic Unit dispatch has no realization machine",
        ))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic Unit helper machine identity overflowed",
        ))?;
    (0..=plan.forwarding_transfers.len())
        .map(|ordinal| {
            let ordinal = u64::try_from(ordinal).map_err(|_| {
                LoweringError::Unsupported("forwarded dynamic Unit helper count exceeds u64")
            })?;
            Ok(ForwardedUnitHelperIds {
                machine: machine_id(first_machine.checked_add(ordinal).ok_or(
                    LoweringError::Unsupported(
                        "forwarded dynamic Unit helper machine identity overflowed",
                    ),
                )?),
                block: block_id(allocate_dense(next_block)?),
                operation: operation_id(allocate_dense(next_operation)?),
                edge: edge_id(allocate_dense(next_edge)?),
            })
        })
        .collect()
}

fn materialize_forwarded_unit_helper(
    checked: &CheckedTrees,
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
    ids: ForwardedUnitHelperIds,
    source_machine: symbols::SymbolHandle,
    next_helper: Option<semantic_vocabulary::MachineId>,
) -> Result<TerminalMachine, LoweringError> {
    let checked_contract = checked
        .facts
        .contract_plans
        .for_machine(source_machine)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic Unit helper has no checked contract",
        ))?;
    validate_empty_contract(
        checked,
        source_machine,
        checked_contract.report_fingerprint,
        checked_contract.commitment,
    )?;
    let summary = exact_machine_service_summary(checked, source_machine)?;
    validate_empty_service_summary(checked, summary)?;
    let service_contract = checked
        .facts
        .service_reaches
        .plan_for_machine(source_machine)
        .ok_or(LoweringError::Unsupported(
            "forwarded dynamic Unit helper has no checked service contract",
        ))?;
    let published_service_ceiling = lower_installation_machine_service_ceiling(
        checked,
        source_machine,
        service_contract,
        summary,
        &[],
    )?;
    let (_, requirement_slot) = dynamic_parameter_interface(application, selected_row)?;
    Ok(TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: ids.machine,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: ids.block,
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: ids.block,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: ids.operation,
                result: OperationResult::Unit,
                kind: match next_helper {
                    Some(callee) => OperationKind::CallUnit {
                        callee,
                        arguments: Vec::new(),
                        erased_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    None => OperationKind::CallDynamicParameterUnit {
                        parameter_ordinal: 0,
                        requirement_slot,
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: ids.edge,
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: empty_terminal_contract(ids.machine.get()),
    })
}

pub(super) fn materialize_forwarded_unit_helper_chain(
    checked: &CheckedTrees,
    plan: &CheckedDynamicUnitCallPlan,
    application: &ClosedConformanceApplication,
    selected_row: &ClosedConformanceRow,
    helpers: &[ForwardedUnitHelperIds],
) -> Result<Vec<TerminalMachine>, LoweringError> {
    if helpers.is_empty() {
        return Ok(Vec::new());
    }
    let CheckedDynamicUnitCallOrigin::Forwarded {
        machine: final_source_machine,
        ..
    } = plan.origin
    else {
        return unsupported("forwarded Unit helper chain requires a forwarded checked origin");
    };
    if plan.forwarding_transfers.len() + 1 != helpers.len() {
        return unsupported("forwarded Unit helper chain length drifted from checked custody");
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
            materialize_forwarded_unit_helper(
                checked,
                application,
                selected_row,
                *ids,
                source_machine,
                next_helper,
            )
        })
        .collect()
}

fn unit_source_call_occurrences(
    plan: &CheckedDynamicUnitCallPlan,
    caller_operation: semantic_vocabulary::OperationId,
    forwarded_helper: Option<ForwardedUnitHelperIds>,
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    let mut occurrences = vec![LoweredSourceCallOccurrence {
        source_site: None,
        source_state: plan.caller_state,
        statement_index: usize::try_from(plan.coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("dynamic Unit statement coordinate exceeds usize")
        })?,
        call_ordinal: usize::try_from(plan.coordinate.call_ordinal)
            .map_err(|_| LoweringError::Unsupported("dynamic Unit call ordinal exceeds usize"))?,
        terminal_operation: caller_operation,
        source_target: match plan.origin {
            CheckedDynamicUnitCallOrigin::Local => plan.requirement,
            CheckedDynamicUnitCallOrigin::Forwarded { state, .. } => state,
        },
        source_values_before_call: Vec::new(),
    }];
    if let (
        Some(helper),
        CheckedDynamicUnitCallOrigin::Forwarded {
            state, coordinate, ..
        },
    ) = (forwarded_helper, plan.origin)
    {
        occurrences.push(LoweredSourceCallOccurrence {
            source_site: None,
            source_state: state,
            statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("forwarded Unit statement coordinate exceeds usize")
            })?,
            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("forwarded Unit call ordinal exceeds usize")
            })?,
            terminal_operation: helper.operation,
            source_target: plan.requirement,
            source_values_before_call: Vec::new(),
        });
    }
    Ok(occurrences)
}

fn unit_source_call_occurrences_for_chain(
    plan: &CheckedDynamicUnitCallPlan,
    caller_operation: semantic_vocabulary::OperationId,
    helpers: &[ForwardedUnitHelperIds],
) -> Result<Vec<LoweredSourceCallOccurrence>, LoweringError> {
    if helpers.len() <= 1 {
        return unit_source_call_occurrences(plan, caller_operation, helpers.first().copied());
    }
    let CheckedDynamicUnitCallOrigin::Forwarded {
        state: final_state,
        coordinate: final_coordinate,
        ..
    } = plan.origin
    else {
        return unsupported("forwarded Unit source-call chain requires a forwarded checked origin");
    };
    let first_state = plan
        .forwarding_transfers
        .first()
        .ok_or(LoweringError::Unsupported(
            "multi-hop Unit source-call chain lost its first transfer",
        ))?
        .caller_state;
    let mut occurrences = vec![LoweredSourceCallOccurrence {
        source_site: None,
        source_state: plan.caller_state,
        statement_index: usize::try_from(plan.coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("dynamic Unit statement coordinate exceeds usize")
        })?,
        call_ordinal: usize::try_from(plan.coordinate.call_ordinal)
            .map_err(|_| LoweringError::Unsupported("dynamic Unit call ordinal exceeds usize"))?,
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
                        "Unit parameter-forwarding statement coordinate exceeds usize",
                    )
                },
            )?,
            call_ordinal: usize::try_from(transfer.coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("Unit parameter-forwarding call ordinal exceeds usize")
            })?,
            terminal_operation: helper.operation,
            source_target: transfer.target_state,
            source_values_before_call: Vec::new(),
        });
    }
    let final_helper = helpers.last().ok_or(LoweringError::Unsupported(
        "multi-hop Unit source-call chain has no final helper",
    ))?;
    occurrences.push(LoweredSourceCallOccurrence {
        source_site: None,
        source_state: final_state,
        statement_index: usize::try_from(final_coordinate.statement_index).map_err(|_| {
            LoweringError::Unsupported("forwarded dynamic Unit statement coordinate exceeds usize")
        })?,
        call_ordinal: usize::try_from(final_coordinate.call_ordinal).map_err(|_| {
            LoweringError::Unsupported("forwarded dynamic Unit call ordinal exceeds usize")
        })?,
        terminal_operation: final_helper.operation,
        source_target: plan.requirement,
        source_values_before_call: Vec::new(),
    });
    Ok(occurrences)
}
