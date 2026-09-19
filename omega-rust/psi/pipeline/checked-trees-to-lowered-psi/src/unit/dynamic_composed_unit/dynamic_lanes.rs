//! Lane shapes for dynamic composed-unit lowering and the lowering shared
//! by every lane.

use crate::unit::dynamic_composed_unit::applications::{
    exact_machine_service_summary, lower_exact_application, lower_initial_rebound_application,
    terminal_callable_result,
};
use crate::unit::dynamic_composed_unit::continuation;
use crate::unit::dynamic_composed_unit::forwarded_helpers::{
    dynamic_source_call_occurrences_for_chain, extend_parameter_forwarding_catalog,
    forwarded_helper_chain_ids, materialize_forwarded_helper_chain,
};
use crate::unit::dynamic_composed_unit::plan_validation::{
    validate_exact_direct_plan, validate_exact_rebound_plan, validate_exact_stored_plan,
};
use crate::unit::dynamic_composed_unit::realizations::{
    collect_dynamic_realizations, materialize_dynamic_realizations, retain_realizations_for_lane,
};
use crate::unit::dynamic_composed_unit::source_lowering::{
    lower_dynamic_call_custody, validate_and_lower_source,
};
use crate::unit::dynamic_composed_unit::store_operations::{
    empty_terminal_contract, lower_caller_store_operations,
};
use crate::unit::dynamic_composed_unit::structural_types::{
    lower_dynamic_structural_types, terminal_structural_multiplicity,
};
use crate::unit::{
    CheckedTrees, LoweredPsi, LoweringError, MachineId, ProofBundle, block_id, edge_id,
    lookup_type_id, lower_installation_machine_service_ceiling, lower_root_service_reach,
    machine_id, operation_id, place_id, terminal_scalar_type, unsupported, value_id,
};
use checked_trees::{
    CheckedDynamicScalarCallPlan, CheckedDynamicSelectionPlan, CheckedStructuralAccess,
};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{
    Block, ClosedConformanceCallableResult, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralParameterDeclaration, StructuralPlaceDeclaration, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};

pub(crate) struct DynamicCallerShape {
    pub(crate) attachment_type_identity: String,
}

#[derive(Clone)]
pub(crate) struct LoweredDynamicRealization {
    pub(crate) source_machine: symbols::SymbolHandle,
    pub(crate) source_state: symbols::SymbolHandle,
    /// The checked plan's identity for this callable: the bare normalized
    /// overload identity `CheckedDynamic*CallPlan::realization_identity` and
    /// the callable roster retain. For a finite-family tuple instance this is
    /// the bare instance identity, not the commitment-wrapped Terminal one.
    pub(crate) checked_identity: String,
    /// The Terminal-side callable identity
    /// (`checked_evidence_machine_identity`): the bare identity for a
    /// nongeneric realization, or the specialization-application-wrapped
    /// identity for a tuple's instance. Registry entries, row references and
    /// dispatch rows all name this form.
    pub(crate) callable_identity: String,
    pub(crate) machine: semantic_vocabulary::MachineId,
    pub(crate) result: ClosedConformanceCallableResult,
}

#[derive(Clone, Copy)]
pub(crate) struct ForwardedHelperIds {
    pub(crate) machine: semantic_vocabulary::MachineId,
    pub(crate) block: semantic_vocabulary::BlockId,
    pub(crate) operation: semantic_vocabulary::OperationId,
    pub(crate) operation_value: semantic_vocabulary::ValueId,
    pub(crate) result_value: semantic_vocabulary::ValueId,
    pub(crate) edge: semantic_vocabulary::EdgeId,
}

#[derive(Clone, Copy)]
pub(crate) enum DynamicLoweringLane<'a> {
    Direct,
    Rebound(&'a CheckedDynamicSelectionPlan),
    Stored(&'a checked_trees::CheckedStoredDynamicScalarCallPlan),
}

pub(crate) fn lower_dynamic_composed_unit_machine(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    lane: DynamicLoweringLane<'_>,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
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
    let all_realizations = collect_dynamic_realizations(checked, plan, 2)?;
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
        || selected_realization.checked_identity != plan.realization_identity
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
        has_descriptor_store.then_some(operation_id(1)),
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
            static_reach_binding: None,
            id: operation_id(1),
            result: OperationResult::Unit,
            kind: OperationKind::StoreDynamicDescriptor {
                descriptor_ordinal: 0,
            },
        });
    }
    caller_operations.push(Operation {
        static_reach_binding: None,
        id: call_operation,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
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

    let lowered = LoweredPsi {
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
                        structural_parameters: Vec::new(),
                        id: caller_block,
                        parameters: Vec::new(),
                        erased_scalar_formals: Vec::new(),
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
            control_cycles: Vec::new(),
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
        selected_ieee_float_comparison_occurrences: Vec::new(),
        selected_integer_comparison_occurrences: Vec::new(),
    };
    retain_dynamic_source_owners(
        lowered,
        plan,
        &lowered_realizations,
        &forwarded_helpers,
        Vec::new(),
    )
}

pub(crate) fn retain_dynamic_source_owners(
    terminal: LoweredPsi,
    plan: &CheckedDynamicScalarCallPlan,
    realizations: &[LoweredDynamicRealization],
    helpers: &[ForwardedHelperIds],
    mut sources: Vec<(symbols::SymbolHandle, MachineId)>,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    if !sources
        .iter()
        .any(|(source, _)| *source == plan.caller_machine)
    {
        sources.push((plan.caller_machine, terminal.semantic_module.entry));
    }
    sources.extend(
        realizations
            .iter()
            .map(|realization| (realization.source_machine, realization.machine)),
    );
    for (index, helper) in helpers.iter().enumerate() {
        let checked_trees::CheckedDynamicScalarCallOrigin::Forwarded { machine, .. } = plan.origin
        else {
            return unsupported("dynamic helper has no forwarded source owner");
        };
        let source = plan
            .forwarding_transfers
            .get(index)
            .map_or(machine, |transfer| transfer.caller_machine);
        sources.push((source, helper.machine));
    }
    crate::producer_result::SourceMappedLowered::new(terminal, sources)
}
