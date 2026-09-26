//! Attached Unit closure assembly.
//!
//! `lower_unit_closure` retains exact transitive closure and publication
//! order. `call_catalog` closes operation/scalar/provider dependencies before
//! allocation; `admission` retains checked bodies and validates source/call
//! custody; `boundaries` retains and declares every boundary the closure calls;
//! `signatures` allocates each machine's formals, claims and requirements.
//! Emission borrows those records in the single shared namespace:
//! `ordinary_machine` emits each ordinary body and `composed_control::callable`
//! each composed one. Both lower their stores, scalar locals, borrowed-storage
//! windows, continuation cleanup, structural-value construction and calls
//! through one `operation_frame::OperationFrame`. `scalar_callees` then emits
//! the scalar machines those bodies call, and `providers` checks each provider
//! candidate against the boundary it satisfies.
//!
//! The module list below groups the rest by that route: the closure's own
//! steps, body emission, the operations a body lowers with their source
//! custody, and the scalar or structural completion that ends a body.
use super::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, ClaimTransfer, LoweredPsi, LoweringError, MachineContract,
    MachineId, Multiplicity, Operation, OperationKind, OperationResult, PlaceId, ProofBundle,
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, ScalarQualificationCatalog, ScalarType,
    SemanticDomainId, ServiceReachSummary, StructuralDomainId, StructuralDomainRequirement,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, TERMINAL_MACHINE_IDENTITY_STRIDE,
    TERMINAL_UNIT_CALL_OBLIGATION_BASE, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker, allocate_dense, boundary_machine_id,
    content_conservation, contract_id, dense_identity, direct_expression_contains_short_circuit,
    edge_id, emit_direct_expression, finalize_operation_proofs, lookup_claim_id, lookup_domain_id,
    lookup_machine_id, lookup_service_id, lookup_type_id, lower_boundary_content_guarantees,
    lower_boundary_crash_routes, lower_checked_crash_route_buckets, lower_placed_view_input,
    lower_structural_crash_route_buckets, machine_id, obligation_id, place_id,
    terminal_scalar_type, unsupported, validate_direct_parameter_types, value_id,
};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::expression_preparation::source_custody::flow_calls::retain_exact_flow_call;
use crate::scalar_graph::scalar_call_closure::callee::{CheckedScalarCallee, PreparedScalarCallee};
use typed_trees_to_checked_trees::checked_trees::CheckedUnitStructuralArgumentSourcePlan;

// The closure `lower_unit_closure` assembles: body and callee discovery,
// admission, providers, catalogs, boundaries, claims and signatures.
mod admission;
mod boundaries;
mod call_catalog;
mod call_closure;
pub(crate) mod catalog;
mod claims;
mod providers;
mod scalar_callees;
pub(crate) mod shared_closure;
mod signatures;

// Body emission: each ordinary body and each composed state graph, through
// one operation frame and one argument schedule over borrowed body views,
// with the provider attachment roots a body establishes first.
pub(crate) mod argument_evaluation;
mod argument_schedule;
pub(crate) mod bodies;
mod composed_control;
mod operation_frame;
mod ordinary_machine;
mod provider_attachments;

// The operations a body lowers, each with its exact source custody.
mod byte_subslices;
mod field_replacement;
mod ordinary_calls;
mod parameters;
pub(crate) mod primitive_locals;
mod reference_results;
pub(crate) mod scalar_arrays;
mod scalar_structural_calls;
mod structural_calls;
pub(crate) mod structural_values;
mod view_ranges;

// How a body completes: its scalar or structural result.
mod scalar_completion;
mod structural_completion;
pub(crate) use scalar_completion::control::validate_tail as validate_scalar_control_tail;

use bodies::{UnitBody, UnitPlans};
pub(crate) use boundaries::retain_exact_unit_boundary;
pub(crate) use parameters::validate_direct_unit_parameter_custody;
pub(crate) use parameters::{lower_declared_service_reach, lower_fixed_boundary_service_reach};

use call_closure::{
    checked_scalar_call_closure_with_structural_roots, checked_terminal_machine_name,
    reject_recursive_unit_closure, unique_unit_boundary, validate_unit_operation_sequence,
};
pub(crate) use call_closure::{
    checked_unit_boundary_identity, checked_unit_call_closure_including, unique_unit_machine,
};
#[cfg(test)]
pub(crate) use catalog::collect_contract_services;
use catalog::require_valid_service_row;
pub(crate) use catalog::{
    checked_unit_target_reach_matches, collect_installation_machine_contract_services,
    collect_published_contract_services, collect_service_summary, lower_root_service_reach,
    lower_unit_structural_type_roots,
};
pub(crate) use composed_control::dynamic_result::{
    emit_call_leaf as emit_dynamic_control_leaf,
    lower_control_catalogs as lower_dynamic_control_catalogs,
};
pub(crate) use composed_control::live as unit_graph_live_states;
pub(crate) use composed_control::lower_composed_unit_control_machine;
#[cfg(test)]
pub(crate) use parameters::lower_contract_service_ceiling;
pub(crate) use parameters::{
    checked_scalar_source_parameters, literal_argument_places, lower_boundary_parameter_order,
    lower_installation_machine_service_ceiling, lower_published_service_ceiling,
    lower_structural_arguments, lower_unit_parameters, structural_carrier_type,
    validate_transfer_shape,
};
pub(crate) use provider_attachments::lower_provider_attachment_places;
use providers::{ProviderBody, checked_unit_provider_candidates};

/// Which runtime-requirement roster a closure's machines answer to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeRequirementOwner {
    UnitClosure,
    /// Nominal cleanup assembles the final caller and cleanup contracts
    /// itself, after restoring their full scalar and structural namespaces.
    NominalCleanup,
}

/// The one closure assembly request. `plans` is the Unit roster the closure
/// resolves bodies and shapes from; `entry` is the exact machine the closure
/// begins at; `unit_roots` are its explicit Unit roots (the entry first for
/// an ordinary closure, empty for a scalar entry); `external` names the
/// boundary, type, service and scalar roots a composed caller supplies;
/// `scalar_entry` marks a scalar machine that borrows the same real callee
/// catalog without a synthetic Unit caller.
#[derive(Clone, Copy)]
pub(crate) struct UnitClosureRequest<'a> {
    pub(crate) plans: UnitPlans<'a>,
    pub(crate) entry: symbols::SymbolHandle,
    pub(crate) unit_roots: &'a [symbols::SymbolHandle],
    pub(crate) external: Option<shared_closure::ExternalUnitRoots<'a>>,
    pub(crate) requirements_owner: RuntimeRequirementOwner,
    pub(crate) scalar_entry: bool,
}

impl<'a> UnitClosureRequest<'a> {
    /// An ordinary Unit closure over the published roster.
    pub(crate) fn unit(
        checked: &'a CheckedTrees,
        entry: symbols::SymbolHandle,
        roots: &'a [symbols::SymbolHandle],
    ) -> Self {
        Self {
            plans: UnitPlans::published(&checked.facts.flow.terminal_unit_effects),
            entry,
            unit_roots: roots,
            external: None,
            requirements_owner: RuntimeRequirementOwner::UnitClosure,
            scalar_entry: false,
        }
    }
}

/// The dispatch-facing product of an ordinary Unit closure: the lowered
/// program with its machine source map. `machine_lowering` selects this for
/// attached and free Unit-effect machines.
pub(crate) fn lower_unit_effect_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    let closure = lower_unit_closure(checked, &UnitClosureRequest::unit(checked, entry, &[entry]))?;
    crate::producer_result::SourceMappedLowered::new(closure.lowered, closure.machine_ids)
}

/// The dispatch-facing product of a scalar entry that needs the shared
/// callee catalog; operation proofs are finalized here because no Unit
/// caller completes them later.
pub(crate) fn lower_scalar_effect_closure(
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    let mut closure = lower_unit_closure(
        checked,
        &UnitClosureRequest {
            scalar_entry: true,
            ..UnitClosureRequest::unit(checked, entry, &[])
        },
    )?;
    finalize_operation_proofs(&mut closure.lowered)?;
    crate::producer_result::SourceMappedLowered::new(closure.lowered, closure.machine_ids)
}

/// Assemble one attached Unit closure: discover the transitive call catalog,
/// admit and validate every body, allocate the shared type/domain/service
/// namespaces, then emit each machine into one module.
pub(crate) fn lower_unit_closure(
    checked: &CheckedTrees,
    request: &UnitClosureRequest<'_>,
) -> Result<shared_closure::SharedUnitClosure, LoweringError> {
    let UnitClosureRequest {
        plans,
        entry,
        unit_roots,
        external,
        requirements_owner,
        scalar_entry,
    } = *request;
    let reserved_prefix = usize::from(external.is_some());
    let ordinary_entry = unit_roots.first().copied();
    if external.is_none() && !scalar_entry && ordinary_entry != Some(entry) {
        return unsupported("ordinary Unit closure must begin with its exact entry");
    }

    // Discovery: every machine the closure emits, grouped by the route that
    // emits it.
    let call_catalog = call_catalog::discover(
        checked,
        plans,
        entry,
        unit_roots,
        external.as_ref(),
        scalar_entry,
    )?;
    let closure = call_catalog.operations;
    let provider_candidate_plans = call_catalog.providers;
    let scalar_closure = call_catalog.scalars;
    let checked_scalar_callees = scalar_closure
        .iter()
        .map(|machine| CheckedScalarCallee::find_for_unit_call(checked, *machine))
        .collect::<Result<Vec<_>, _>>()?;
    let realizations =
        RealizationRoots::collect(plans, &closure, &scalar_closure, &provider_candidate_plans)?;

    // Admission: retain every checked body and every boundary the closure
    // calls, then the catalog roots its callees and providers name.
    let mut boundaries = Vec::<(&CheckedBoundaryMachinePlan, String)>::new();
    if let Some(external) = &external {
        for symbol in external.boundary_roots {
            let boundary = unique_unit_boundary(plans, *symbol)?;
            retain_exact_unit_boundary(
                checked,
                plans,
                &mut boundaries,
                *symbol,
                boundary.state,
                boundary.contract_report_fingerprint,
                boundary.service_reach,
                boundary.result.clone(),
            )?;
        }
    }
    let admitted_bodies = admission::admit(
        checked,
        plans,
        entry,
        &closure,
        &scalar_closure,
        requirements_owner,
        &mut boundaries,
    )?;
    let mut additional_type_roots = external
        .as_ref()
        .map_or_else(Vec::new, |roots| roots.structural_type_roots.to_vec());
    for candidate in provider_candidate_plans
        .iter()
        .filter(|candidate| candidate.body == ProviderBody::AffineIdentity)
    {
        let plan = providers::affine_candidate(checked, candidate.candidate)?;
        additional_type_roots.extend(plan.attachment_type_identity.iter().cloned());
        additional_type_roots.push(plan.structural_parameter.type_identity.clone());
        additional_type_roots.push(plan.result.type_identity.clone());
    }
    let mut additional_service_roots = external
        .as_ref()
        .map_or_else(Vec::new, |roots| roots.service_roots.to_vec());
    scalar_callees::retain_catalog_roots(
        checked,
        &checked_scalar_callees,
        &mut boundaries,
        &mut additional_type_roots,
        &mut additional_service_roots,
    )?;
    boundaries.sort_by(|left, right| left.1.cmp(&right.1));
    if boundaries.windows(2).any(|pair| pair[0].1 == pair[1].1) {
        return unsupported("boundary Unit closure contains duplicate canonical identities");
    }

    // Shared namespaces: every emitted machine resolves types, domains, and
    // services against these one catalogs.
    let (mut structural_types, type_ids) = if !additional_type_roots.is_empty() {
        catalog::lower_unit_structural_types_including(
            checked,
            plans,
            &closure,
            &boundaries,
            &additional_type_roots,
        )?
    } else {
        catalog::lower_unit_structural_types(checked, plans, &closure, &boundaries)?
    };
    // Callable composed bodies borrow a complete shared catalog. Retain the
    // generated immutable carrier before cloning it into any callee emitter.
    for machine_symbol in &closure {
        if UnitBody::find(plans, *machine_symbol)?
            .operations()
            .any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                )
            })
        {
            crate::emission::structural_byte_sequence_store::literal_view_type(
                &mut structural_types,
            )?;
            break;
        }
    }
    let wrapper_domains = scalar_callees::wrapper_domains(&checked_scalar_callees);
    let (structural_domains, domain_ids) = catalog::lower_unit_structural_domains_including(
        checked,
        plans,
        &closure,
        &boundaries,
        &type_ids,
        &wrapper_domains,
    )?;
    let (services, service_ids) = if !additional_service_roots.is_empty() {
        catalog::lower_unit_services_including(
            checked,
            plans,
            &closure,
            &boundaries,
            &provider_candidate_plans,
            &additional_service_roots,
        )?
    } else {
        catalog::lower_unit_services(
            checked,
            plans,
            &closure,
            &boundaries,
            &provider_candidate_plans,
        )?
    };
    let root_service_reach = lower_root_service_reach(checked, entry, &service_ids)?;

    // Signatures: boundary declarations, then each admitted body's formals,
    // claims and requirements, then every machine's Terminal identity.
    let mut next_place = 1_u64;
    let (boundary_machines, lowered_boundary_parameters) = boundaries::lower_declarations(
        checked,
        &boundaries,
        &type_ids,
        &domain_ids,
        &service_ids,
        &mut next_place,
    )?;
    let mut next_value = 1_u64;
    let machine_signatures = signatures::lower(
        checked,
        &admitted_bodies,
        &type_ids,
        &domain_ids,
        &structural_types,
        requirements_owner,
        &mut next_place,
        &mut next_value,
    )?;
    let machine_ids = external
        .as_ref()
        .map(|_| &entry)
        .into_iter()
        .chain(closure.iter())
        .chain(&scalar_closure)
        .chain(&realizations.structural_results)
        .enumerate()
        .map(|(index, symbol)| Ok((*symbol, machine_id(dense_identity(index)?))))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut placed_view_inputs = checked
        .facts
        .placed_view_inputs
        .iter()
        .filter(|input| {
            machine_ids
                .iter()
                .any(|(symbol, _)| *symbol == input.machine)
        })
        .map(|input| {
            lower_placed_view_input(
                checked,
                input,
                lookup_machine_id(&machine_ids, input.machine)?,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    placed_view_inputs.sort();
    let prepared_scalar_callees = scalar_callees::prepare(
        checked,
        checked_scalar_callees,
        &type_ids,
        &domain_ids,
        &structural_types,
        &mut next_place,
    )?;
    // A call owes one obligation per row of its callee's Terminal `requires`
    // roster, and the two callee routes publish that roster differently. A
    // prepared scalar graph publishes its source clauses as one canonical
    // conjunction (`PreparedScalarContract::requirement_count`: 0 or 1),
    // while a Unit or composed callee publishes one row per authored clause.
    // The count therefore follows how the callee itself was lowered, never
    // the caller's route: every caller of one callee owes the same roster.
    let scalar_requirement_counts = prepared_scalar_callees
        .machines
        .iter()
        .map(|machine| (machine.source_machine(), machine.requirement_count()))
        .chain(machine_signatures.iter().filter_map(|signature| {
            UnitBody::find(plans, signature.source)
                .ok()
                .filter(|body| body.scalar_result_type().is_some())
                .map(|_| (signature.source, signature.requires.len()))
        }))
        .collect::<Vec<_>>();

    // Emission: each Unit body, ordinary or composed, into the shared
    // namespaces; then the scalar callees; then realization machines.
    let mut scalar_block_invariants = Vec::new();
    let mut next_operation = 1_u64;
    let mut next_edge = 1_u64;
    let mut next_block = 1_u64;
    let mut next_call_obligation = TERMINAL_UNIT_CALL_OBLIGATION_BASE;
    let mut call_evidence = Vec::new();
    let mut machines = Vec::with_capacity(
        closure.len() + scalar_closure.len() + realizations.structural_results.len(),
    );
    let mut occurrences = ClosureOccurrences::default();
    let catalog = ordinary_machine::ClosureCatalog {
        type_ids: &type_ids,
        domain_ids: &domain_ids,
        service_ids: &service_ids,
        boundary_parameters: &lowered_boundary_parameters,
        machine_ids: &machine_ids,
        signatures: &machine_signatures,
        scalar_requirement_counts: &scalar_requirement_counts,
        plans,
        closure: &closure,
        provider_candidate_plans: &provider_candidate_plans,
        prepared_scalar_machines: &prepared_scalar_callees.machines,
    };
    for (signature, admitted) in machine_signatures.iter().zip(admitted_bodies) {
        let body = admitted.source();
        let plan = body.entry()?;
        let terminal_machine = lookup_machine_id(&machine_ids, plan.machine)?;
        let parameters = &signature.parameters;
        let scalar_parameters = signature.scalar_parameters.clone();
        if let admission::AdmittedBody::Composed {
            source: source_plan,
            body: admitted,
        } = admitted
        {
            let (machine, composed, mut invariants) = composed_control::callable::emit(
                checked,
                source_plan,
                admitted,
                parameters.clone(),
                scalar_parameters,
                composed_control::callable::SharedCatalog {
                    structural_types: &structural_types,
                    type_ids: &type_ids,
                    structural_domains: &structural_domains,
                    domain_ids: &domain_ids,
                    services: &services,
                    service_ids: &service_ids,
                    root_service_reach: &root_service_reach,
                    boundaries: &boundary_machines,
                    boundary_parameters: &lowered_boundary_parameters,
                    machine_ids: &machine_ids,
                    signatures: &machine_signatures,
                    scalar_requirement_counts: &scalar_requirement_counts,
                    closure: &closure,
                    prepared_scalar_machines: &prepared_scalar_callees.machines,
                },
                composed_control::callable::EmissionCounters {
                    place: &mut next_place,
                    value: &mut next_value,
                    block: &mut next_block,
                    operation: &mut next_operation,
                    edge: &mut next_edge,
                    call_obligation: &mut next_call_obligation,
                },
            )?;
            machines.push(machine);
            occurrences.retain(
                composed.source_calls,
                composed.selected_ieee_float_comparisons,
                composed.selected_integer_comparisons,
            );
            scalar_block_invariants.append(&mut invariants);
            continue;
        }
        let plan = body.ordinary()?;
        let emitted = ordinary_machine::emit(
            checked,
            signature,
            plan,
            terminal_machine,
            &mut structural_types,
            catalog,
            composed_control::callable::EmissionCounters {
                place: &mut next_place,
                value: &mut next_value,
                block: &mut next_block,
                operation: &mut next_operation,
                edge: &mut next_edge,
                call_obligation: &mut next_call_obligation,
            },
        )?;
        machines.push(emitted.machine);
        occurrences.retain(
            emitted.source_calls,
            emitted.selected_ieee_float_comparisons,
            emitted.selected_integer_comparisons,
        );
    }
    super::unit_cleanup::patch_nominal_cleanup_members(
        checked,
        entry,
        &mut machines,
        &type_ids,
        &machine_ids,
    )?;
    let first_scalar_callee_index =
        closure
            .len()
            .checked_add(reserved_prefix)
            .ok_or(LoweringError::Unsupported(
                "shared Unit reserved machine count overflows usize",
            ))?;
    let mut scalar_output = scalar_callees::emit(
        checked,
        prepared_scalar_callees,
        scalar_callees::EmissionCatalog {
            machine_ids: &machine_ids,
            requirement_counts: &scalar_requirement_counts,
            boundary_parameters: &lowered_boundary_parameters,
            structural_types: &structural_types,
            type_ids: &type_ids,
            domain_ids: &domain_ids,
            service_ids: &service_ids,
            first_machine_index: first_scalar_callee_index,
        },
        &mut next_place,
        &mut next_call_obligation,
        &mut machines,
        &mut occurrences,
    )?;
    let mut lowered_structural_result_realizations =
        crate::returns::affine_return::lower_claim_free_affine_return_machines(
            checked,
            &realizations.structural_results,
            &structural_types,
            &type_ids,
            &machine_ids,
            reserved_prefix + closure.len() + scalar_closure.len(),
        )?;
    machines.append(&mut lowered_structural_result_realizations);
    let provider_candidates = providers::conformances(
        &provider_candidate_plans,
        &lowered_boundary_parameters,
        &boundary_machines,
        &machine_ids,
        &machines,
    )?;

    // Publication: one module in canonical roster order.
    call_evidence.append(&mut scalar_output.evidence);
    let mut float_entry_ranges = scalar_output.float_entry_ranges;
    let mut integer_entry_ranges = scalar_output.integer_entry_ranges;
    float_entry_ranges.sort_by_key(|range| (range.machine, range.parameter));
    integer_entry_ranges.sort_by_key(|range| (range.machine, range.parameter));
    // The verifier reads the roster in exactly (machine, header) order.
    scalar_block_invariants.sort_by_key(|invariant| (invariant.machine, invariant.header));

    let lowered = LoweredPsi {
        semantic_module: TerminalModule {
            scalar_qualifications: ScalarQualificationCatalog {
                float_entry_ranges,
                integer_entry_ranges,
                ..Default::default()
            },
            scalar_block_invariants,
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            // Operation bodies are emitted first; a scalar entry may follow
            // its helpers. Preserve the selected source owner, not roster order.
            entry: lookup_machine_id(&machine_ids, entry)?,
            structural_types,
            structural_domains,
            services,
            root_service_reach,
            placed_view_inputs,
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines,
            provider_candidates,
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle {
            crash_obligations: Vec::new(),
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: call_evidence,
        },
        debug_map: None,
        source_call_occurrences: occurrences.source_calls,
        selected_ieee_float_comparison_occurrences: occurrences.ieee_float_comparisons,
        selected_integer_comparison_occurrences: occurrences.integer_comparisons,
    };
    Ok(shared_closure::SharedUnitClosure {
        lowered,
        machine_ids,
        type_ids,
        domain_ids,
        service_ids,
        boundary_parameters: lowered_boundary_parameters,
        signatures: machine_signatures,
        scalar_requirement_counts,
        next_place,
        next_value,
        next_block,
        next_operation,
        next_edge,
        next_call_obligation,
    })
}

/// Occurrence rows every emitted machine publishes beside its Terminal
/// operations. Each selected comparison or source call keeps the row
/// that joins it to its checked application; the closure publishes all of
/// them.
#[derive(Default)]
struct ClosureOccurrences {
    source_calls: Vec<crate::lowered_psi::LoweredSourceCallOccurrence>,
    ieee_float_comparisons: Vec<crate::lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence>,
    integer_comparisons: Vec<crate::lowered_psi::LoweredSelectedIntegerComparisonOccurrence>,
}

impl ClosureOccurrences {
    fn retain(
        &mut self,
        source_calls: Vec<crate::lowered_psi::LoweredSourceCallOccurrence>,
        ieee_float_comparisons: Vec<
            crate::lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence,
        >,
        integer_comparisons: Vec<crate::lowered_psi::LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        self.source_calls.extend(source_calls);
        self.ieee_float_comparisons.extend(ieee_float_comparisons);
        self.integer_comparisons.extend(integer_comparisons);
    }

    /// Take the rows a separately lowered helper module recorded.
    fn retain_lowered(&mut self, lowered: &mut LoweredPsi) {
        self.retain(
            std::mem::take(&mut lowered.source_call_occurrences),
            std::mem::take(&mut lowered.selected_ieee_float_comparison_occurrences),
            std::mem::take(&mut lowered.selected_integer_comparison_occurrences),
        );
    }
}

/// Machines emitted only as realizations of an operation the closure's bodies
/// select: a structural-result callee without a Unit body. Each is its own Terminal machine, never also a
/// body of the closure.
struct RealizationRoots {
    structural_results: Vec<symbols::SymbolHandle>,
}

impl RealizationRoots {
    fn collect(
        plans: UnitPlans<'_>,
        closure: &[symbols::SymbolHandle],
        scalar_closure: &[symbols::SymbolHandle],
        provider_candidate_plans: &[CheckedUnitProviderCandidate],
    ) -> Result<Self, LoweringError> {
        let mut structural_results = Vec::new();
        for candidate in provider_candidate_plans
            .iter()
            .filter(|candidate| candidate.body == ProviderBody::AffineIdentity)
        {
            if !structural_results.contains(&candidate.candidate) {
                structural_results.push(candidate.candidate);
            }
        }
        for machine_symbol in closure {
            for operation in UnitBody::find(plans, *machine_symbol)?.operations() {
                let target =
                    match operation {
                        CheckedUnitEffectOperationPlan::StructuralCall {
                            target_machine, ..
                        } if !UnitBody::contains(plans, *target_machine) => Some(*target_machine),
                        _ => None,
                    };
                if let Some(target) = target
                    && !structural_results.contains(&target)
                {
                    structural_results.push(target);
                }
            }
        }
        if structural_results
            .iter()
            .any(|machine| closure.contains(machine) || scalar_closure.contains(machine))
        {
            return unsupported("structural-result machine overlaps another attached Unit closure");
        }
        Ok(Self { structural_results })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedUnitProviderCandidate {
    body: ProviderBody,
    boundary: symbols::SymbolHandle,
    candidate: symbols::SymbolHandle,
    requirement_identity: String,
    provider_identity: String,
    candidate_identity: String,
}
