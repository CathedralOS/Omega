//! Attached Unit closure assembly.
//!
//! `lower_unit_closure` retains exact transitive closure and publication
//! order. `call_catalog` closes operation, scalar-helper and provider
//! dependencies before allocation; every scalar helper is a Unit body with a
//! scalar completion; `admission` retains checked bodies and validates source/call
//! custody; `boundaries` retains and declares every boundary the closure calls;
//! `signatures` allocates each machine's formals, claims and requirements.
//! Emission borrows those records in the single shared namespace:
//! `ordinary_machine` emits each ordinary body and `composed_control::callable`
//! each composed one. Both lower their stores, scalar locals, borrowed-storage
//! windows, continuation cleanup, structural-value construction and calls
//! through one `operation_frame::OperationFrame`. `providers` checks each
//! provider candidate against the boundary it satisfies.
//!
//! The module's identity table interns output vocabulary; it does not change
//! invocation ownership or evaluation namespaces. Machine/state identities,
//! source/erased value prefixes and value/block/edge allocators retain their
//! distinct roles even where Rust gives them identical types. A new shared
//! context must not permute those operands or replace retained input with an
//! empty roster: doing so loses already-produced custody and cleanup work.
//!
//! Replacement has no separate source-shape recognizer. The checked operation
//! stream already orders production, old-value cleanup and establishment.
//! `OperationFrame` resolves the completed value in the same storage namespace
//! for stores and owned rebinding. Rebinding updates that local's current home;
//! it does not rewrite scalar values captured from the prior home. Cleanup
//! receipts settle an occurrence, so a later establishment at the same source
//! path withdraws its old receipt rather than suppressing the new value's death.
//!
//! The module list below groups the rest by that route: the closure's own
//! steps, body emission, the operations a body lowers with their source
//! custody, and the scalar or structural completion that ends a body.
use super::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, ClaimTransfer, LoweredPsi, LoweringError, MachineContract,
    MachineId, Multiplicity, Operation, OperationKind, OperationResult, PlaceId,
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, ScalarType, SemanticDomainId,
    ServiceReachSummary, StructuralDomainRequirement, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId, StructuralTypeShape,
    TERMINAL_UNIT_CALL_OBLIGATION_BASE, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker, allocate_dense, boundary_machine_id,
    content_conservation, contract_id, dense_identity, direct_expression_contains_short_circuit,
    edge_id, emit_direct_expression, lookup_claim_id, lookup_domain_id, lookup_machine_id,
    lookup_service_id, lookup_type_id, lower_boundary_content_guarantees,
    lower_boundary_crash_routes, lower_checked_crash_route_buckets, lower_placed_view_input,
    lower_structural_crash_route_buckets, machine_id, obligation_id, place_id,
    terminal_scalar_type, unsupported, validate_direct_parameter_types, value_id,
};
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::scalar_lowering::call_closure::callee::CheckedScalarCallee;
use crate::unit::attached_unit::call_closure::BoundaryIdentity;

// The closure `lower_unit_closure` assembles: body and callee discovery,
// admission, providers, catalogs, boundaries, claims and signatures.
mod admission;
mod boundaries;
mod call_catalog;
mod call_closure;
pub(crate) mod call_parameters;
pub(crate) mod call_results;
pub(crate) mod catalog;
mod claims;
pub(crate) mod completion_cleanup;
mod entry_requirements;
mod providers;
pub(crate) mod signatures;
pub(crate) mod target_families;
mod target_layouts;

// Body emission: each ordinary body and each composed state graph, through
// one operation frame and one argument schedule over borrowed body views,
// with the provider attachment roots a body establishes first.
pub(crate) mod argument_evaluation;
pub(crate) mod argument_schedule;
pub(crate) mod bodies;
mod composed_control;
mod operation_frame;
pub(crate) use operation_frame::BoundaryParameters;
mod normal_guarantees;
mod ordinary_machine;
mod provider_attachments;
mod quotients;

// The operations a body lowers, each with its exact source custody.
pub(crate) mod byte_locals;
mod byte_subslices;
pub(crate) mod ordinary_calls;
pub(crate) mod parameters;
pub(crate) mod primitive_locals;
pub(crate) mod reference_results;
pub(crate) mod scalar_arrays;
pub(crate) mod structural_values;
pub(crate) mod view_ranges;

// How a body completes: its scalar or structural result.
mod scalar_completion;

use bodies::{UnitBody, UnitPlans};
pub(crate) use boundaries::{lower_boundary_result, retain_unit_boundary};
pub(crate) use parameters::{lower_declared_service_reach, lower_fixed_boundary_service_reach};

use call_closure::{checked_scalar_call_closure_with_structural_roots, unique_unit_boundary};
pub(crate) use call_closure::{
    checked_unit_boundary_identity, checked_unit_call_closure_including,
};
pub(crate) use catalog::lower_root_service_reach;
use catalog::require_valid_service_row;
pub(crate) use composed_control::live as unit_graph_live_states;
pub(crate) use parameters::{
    literal_argument_places, lower_boundary_parameter_order,
    lower_installation_machine_service_ceiling, lower_published_service_ceiling,
    lower_structural_arguments, lower_unit_parameters,
};
use providers::checked_unit_provider_candidates;

/// The one closure assembly request. `plans` is the Unit roster the closure
/// resolves bodies and shapes from; `entry` is the exact machine the closure
/// begins at; `unit_roots` are its explicit Unit roots (the entry first for
/// an ordinary closure, empty for a scalar entry or an independent family
/// head). A family head closes every member through the callee catalog without
/// inventing an executable body. `scalar_entry` marks a scalar machine that borrows the same real callee
/// catalog without a synthetic Unit caller.
#[derive(Clone, Copy)]
pub(crate) struct UnitClosureRequest<'a> {
    pub(crate) plans: UnitPlans<'a>,
    pub(crate) entry: symbols::SymbolHandle,
    pub(crate) unit_roots: &'a [symbols::SymbolHandle],
    pub(crate) program_roots: crate::TerminalProgramRoots<'a>,
    pub(crate) exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    pub(crate) scalar_entry: bool,
}

impl<'a> UnitClosureRequest<'a> {
    /// An ordinary Unit closure over the published roster.
    pub(crate) fn unit(
        checked: &'a CheckedTrees,
        entry: symbols::SymbolHandle,
        roots: &'a [symbols::SymbolHandle],
        program_roots: crate::TerminalProgramRoots<'a>,
        exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    ) -> Self {
        Self {
            plans: UnitPlans::published(&checked.facts.flow),
            entry,
            unit_roots: roots,
            program_roots,
            exact_toolchain_sources,
            scalar_entry: false,
        }
    }
}

/// The dispatch-facing product of an ordinary Unit closure: the lowered
/// program with its machine source map. `machine_lowering` selects this for
/// attached and free Unit-effect machines.
pub(crate) fn lower_unit_effect_closure(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
    program_roots: crate::TerminalProgramRoots<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    lower_unit_closure(
        identities,
        checked,
        &UnitClosureRequest::unit(
            checked,
            entry,
            &[entry],
            program_roots,
            exact_toolchain_sources,
        ),
    )
}

/// The dispatch-facing product of a scalar entry in the common callee catalog.
/// The entrance finalizes proofs after the complete module's
/// contracts and installation entries have been retained.
pub(crate) fn lower_scalar_effect_closure(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    entry: symbols::SymbolHandle,
    program_roots: crate::TerminalProgramRoots<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    lower_unit_closure(
        identities,
        checked,
        &UnitClosureRequest {
            scalar_entry: true,
            ..UnitClosureRequest::unit(checked, entry, &[], program_roots, exact_toolchain_sources)
        },
    )
}

/// Assemble one attached Unit closure: discover the transitive call catalog,
/// admit and validate every body, allocate the shared type/domain/service
/// namespaces, then emit each machine into one module.
pub(crate) fn lower_unit_closure(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    request: &UnitClosureRequest<'_>,
) -> Result<crate::producer_result::SourceMappedLowered, LoweringError> {
    let UnitClosureRequest {
        plans,
        entry,
        unit_roots,
        program_roots,
        exact_toolchain_sources,
        scalar_entry,
    } = *request;
    let ordinary_entry = unit_roots.first().copied();
    if !scalar_entry && ordinary_entry.is_some_and(|root| root != entry) {
        return unsupported("ordinary Unit closure must begin with its exact entry");
    }

    // Discovery: every machine the closure emits, grouped by the route that
    // emits it.
    let call_catalog = call_catalog::discover(
        checked,
        plans,
        entry,
        unit_roots,
        program_roots,
        scalar_entry,
    )?;
    let closure = call_catalog.operations;
    let provider_candidate_plans = call_catalog.providers;

    // Admission: retain every checked body and every boundary the closure
    // calls, then the catalog roots its callees and providers name.
    let mut boundaries = Vec::<(&CheckedBoundaryMachinePlan, BoundaryIdentity)>::new();
    for symbol in call_catalog.boundaries {
        retain_unit_boundary(checked, plans, &mut boundaries, symbol)?;
    }
    let admitted_bodies = admission::admit(checked, plans, &closure, &mut boundaries)?;
    boundaries.sort_by_key(|(_, identity)| identity.order_key());
    if let Some(pair) = boundaries.windows(2).find(|pair| pair[0].1 == pair[1].1) {
        return Err(LoweringError::InvalidUnitMachinePlan {
            machine: format!(
                "{} and {} ({:?}: {})",
                checked.symbols.display_path(pair[0].0.machine, "::"),
                checked.symbols.display_path(pair[1].0.machine, "::"),
                pair[0].1,
                pair[0].1.render(checked)?
            ),
            reason: "boundary Unit closure contains duplicate canonical identities",
            gap: None,
        });
    }

    // Shared namespaces: every emitted machine resolves types, domains, and
    // services against these one catalogs.
    let (mut structural_types, type_ids, structural_referents) =
        catalog::lower_unit_structural_types(identities, checked, plans, &closure, &boundaries)?;
    // Callable composed bodies borrow a complete shared catalog. Retain the
    // generated immutable carrier before cloning it into any callee emitter.
    for machine_symbol in &closure {
        for operation in UnitBody::find(plans, *machine_symbol)?.operations() {
            if operation.call().is_some_and(|call| {
                call.structural_arguments.iter().any(|argument| {
                    argument.byte_sequence_literal().is_some()
                        && argument.access == checked_trees::CheckedStructuralAccess::Owned
                })
            }) || crate::expression_preparation::structural_value_requirements::collect_operation_requirements(
                checked,
                *machine_symbol,
                operation,
                &mut Vec::new(),
            )? {
                crate::emission::structural_byte_sequence_store::literal_view_type(identities, &mut structural_types)?;
            }
            if let Some(value) = operation.stored_bytes() {
                crate::emission::structural_byte_sequence_store::prepare_types(
                    identities,
                    value,
                    &mut structural_types,
                )?;
            }
            // An owned bounded literal local copies its octets from a
            // generated literal view into its declared owner.
            if let CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ByteSequence { result, value, .. },
            ) = operation
                && let Ok(declared) = lookup_type_id(&type_ids, &result.type_identity)
                && structural_types.iter().any(|declaration| {
                    declaration.id == declared && declaration.shape.is_bounded_owned_byte_sequence()
                })
            {
                crate::emission::structural_byte_sequence_store::prepare_types(
                    identities,
                    value,
                    &mut structural_types,
                )?;
            }
        }
    }
    let (structural_domains, domain_ids) = catalog::lower_unit_structural_domains(
        identities,
        checked,
        plans,
        &closure,
        &boundaries,
        &type_ids,
    )?;
    let (services, service_ids) = catalog::lower_unit_services(
        identities,
        checked,
        plans,
        &closure,
        &boundaries,
        &provider_candidate_plans,
    )?;
    let root_service_reach = lower_root_service_reach(identities, checked, entry, &service_ids)?;

    // Signatures: boundary declarations, then each admitted body's formals,
    // claims and requirements, then every machine's Terminal identity.
    let mut next_place = 1_u64;
    let (boundary_machines, lowered_boundary_parameters) = boundaries::lower_declarations(
        identities,
        checked,
        exact_toolchain_sources,
        &boundaries,
        &type_ids,
        &structural_types,
        &domain_ids,
        &service_ids,
        &mut next_place,
    )?;
    // One scalar qualification namespace for every machine the closure
    // emits; every body's signature declares against it.
    let qualifications =
        crate::expression_preparation::qualifications::PreparedScalarQualifications::prepare(
            identities,
            checked,
            &[],
            &closure,
        )?;
    let mut next_value = 1_u64;
    let machine_signatures = signatures::lower(
        identities,
        checked,
        &admitted_bodies,
        &qualifications,
        &type_ids,
        &domain_ids,
        &structural_types,
        &mut next_place,
        &mut next_value,
    )?;
    let machine_ids = closure
        .iter()
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
                && checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .callable_entry_for_machine(input.machine)
                    .is_some_and(|entry| entry.state == input.state)
        })
        .map(|input| {
            lower_placed_view_input(
                identities,
                checked,
                exact_toolchain_sources,
                input,
                lookup_machine_id(&machine_ids, input.machine)?,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    placed_view_inputs.sort();
    // A call owes one obligation per row of its callee's Terminal `requires`
    // roster: a Unit or composed callee publishes one row per authored clause.
    let scalar_requirement_counts = machine_signatures
        .iter()
        .map(|signature| (signature.source, signature.requires.len()))
        .collect::<Vec<_>>();

    // Emission: each Unit body, ordinary or composed, into the shared
    // namespaces.
    let mut scalar_block_invariants = Vec::new();
    let mut next_operation = 1_u64;
    let mut next_edge = 1_u64;
    let mut next_block = 1_u64;
    let mut next_call_obligation = TERMINAL_UNIT_CALL_OBLIGATION_BASE;
    let mut machines = Vec::with_capacity(closure.len());
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
        prepared_scalar_qualifications: &qualifications,
    };
    let mut scalar_qualification_coercions = Vec::new();
    for (signature, admitted) in machine_signatures.iter().zip(admitted_bodies) {
        let body = admitted.source();
        let plan = body.entry()?;
        let terminal_machine = lookup_machine_id(&machine_ids, plan.machine)?;
        // A body's refusal names the machine it lowers: a closure emits every
        // callee, so the bare reason would not say which body stopped.
        let attributed = |error| match error {
            LoweringError::Unsupported(reason) => LoweringError::InvalidUnitMachinePlan {
                machine: checked.symbols.display_path(plan.machine, "::"),
                reason,
                gap: None,
            },
            error => error,
        };
        let parameters = &signature.parameters;
        let scalar_parameters = signature.scalar_parameters.clone();
        if let admission::AdmittedBody::Composed {
            source: source_plan,
            body: admitted,
        } = admitted
        {
            let (machine, composed, mut invariants, mut coercions) =
                composed_control::callable::emit(
                    identities,
                    checked,
                    source_plan,
                    admitted,
                    parameters.clone(),
                    scalar_parameters,
                    composed_control::callable::SharedCatalog {
                        structural_types: &structural_types,
                        type_ids: &type_ids,
                        domain_ids: &domain_ids,
                        service_ids: &service_ids,
                        boundary_parameters: &lowered_boundary_parameters,
                        machine_ids: &machine_ids,
                        signatures: &machine_signatures,
                        scalar_requirement_counts: &scalar_requirement_counts,
                        closure: &closure,
                        prepared_scalar_qualifications: &qualifications,
                    },
                    composed_control::callable::EmissionCounters {
                        place: &mut next_place,
                        value: &mut next_value,
                        block: &mut next_block,
                        operation: &mut next_operation,
                        edge: &mut next_edge,
                        call_obligation: &mut next_call_obligation,
                    },
                )
                .map_err(attributed)?;
            occurrences
                .quotient_adaptations
                .extend(composed.quotient_adaptations);
            occurrences.quotient_calls.extend(composed.quotient_calls);
            machines.push(machine);
            occurrences.retain(
                composed.source_calls,
                composed.selected_ieee_float_comparisons,
                composed.selected_integer_comparisons,
            );
            occurrences
                .source_checked_obligations
                .extend(composed.source_checked_obligations);
            occurrences.state_blocks.extend(composed.state_blocks);
            scalar_block_invariants.append(&mut invariants);
            scalar_qualification_coercions.append(&mut coercions);
            continue;
        }
        let plan = body.ordinary()?;
        let emitted = ordinary_machine::emit(
            identities,
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
        )
        .map_err(attributed)?;
        scalar_block_invariants.extend(emitted.scalar_block_invariants);
        scalar_qualification_coercions.extend(emitted.qualification_coercions);
        occurrences
            .quotient_adaptations
            .extend(emitted.quotient_adaptations);
        occurrences.quotient_calls.extend(emitted.quotient_calls);
        machines.push(emitted.machine);
        occurrences.retain(
            emitted.source_calls,
            emitted.selected_ieee_float_comparisons,
            emitted.selected_integer_comparisons,
        );
        occurrences
            .source_checked_obligations
            .extend(emitted.source_checked_obligations);
    }
    super::unit_cleanup::bind_nominal_cleanup_subjects(
        checked,
        &mut machines,
        &type_ids,
        &machine_ids,
    )?;
    super::unit_cleanup::finalize_partial_affine_residuals(
        identities,
        checked,
        &mut machines,
        &type_ids,
        &machine_ids,
    )?;
    let provider_candidates = providers::conformances(
        identities,
        checked,
        exact_toolchain_sources,
        &provider_candidate_plans,
        &lowered_boundary_parameters,
        &boundary_machines,
        &machine_ids,
        &machines,
    )?;

    // Publication: one module in canonical roster order.
    let target_machine_families =
        target_families::lower(checked, &machine_ids, &lowered_boundary_parameters)?;
    let target_layout_applications = target_layouts::lower(
        checked,
        plans,
        &structural_types,
        &type_ids,
        &machine_ids,
        &lowered_boundary_parameters,
        &boundary_machines,
        &target_machine_families,
    )?;
    let mut scalar_qualifications = qualifications.catalog().clone();
    scalar_qualifications
        .coercions
        .append(&mut scalar_qualification_coercions);
    scalar_qualifications
        .float_entry_ranges
        .sort_by_key(|range| (range.machine, range.parameter));
    scalar_qualifications
        .integer_entry_ranges
        .sort_by_key(|range| (range.machine, range.parameter));
    scalar_qualifications
        .coercions
        .sort_by_key(|coercion| (coercion.machine, coercion.edge, coercion.argument_ordinal));
    // The verifier reads the roster in exactly (machine, header) order.
    scalar_block_invariants.sort_by_key(|invariant| (invariant.machine, invariant.header));

    let mut retained_roots = Vec::with_capacity(program_roots.machines.len());
    for root in program_roots.machines {
        // A bodyless family head is retained in the interface catalog. Every
        // body root must resolve exactly; missing roots cannot disappear in DCE.
        if let terminal_psi::TargetMachineCallable::Machine(machine) =
            target_families::callable(*root, &machine_ids, &lowered_boundary_parameters)?
        {
            retained_roots.push(machine);
        }
    }
    let quotient_catalog = quotients::retain(
        checked,
        identities,
        exact_toolchain_sources,
        &structural_types,
        &occurrences.quotient_adaptations,
        &occurrences.quotient_calls,
        &machine_ids,
        &type_ids,
    )?;
    for representation in &quotient_catalog.representations {
        let declaration = structural_types
            .iter_mut()
            .find(|row| row.id == representation.quotient)
            .ok_or(LoweringError::Unsupported("quotient declaration is absent"))?;
        declaration.quotient_carrier = Some(representation.carrier);
    }
    let lowered = LoweredPsi {
        semantic_module: TerminalModule {
            callback_applications: Vec::new(),
            boundary_application_candidates: Vec::new(),
            boundary_application_demands: Vec::new(),
            scalar_qualifications,
            scalar_block_invariants,
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            identities: identities.snapshot(),
            // The root may be a bodyless portable contract. Every family body
            // stays in the catalog; no target member stands in for its head.
            entry: target_families::callable(entry, &machine_ids, &lowered_boundary_parameters)?,
            structural_types,
            structural_referents,
            structural_domains,
            services,
            root_service_reach,
            placed_view_inputs,
            placed_view_plans: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines,
            provider_candidates,
            dynamic_dispatch: Default::default(),
            quotients: quotient_catalog,
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            machines,
            target_machine_families,
            target_layout_applications,
            task_activation_demands: Default::default(),
        },
        debug_map: None,
        retained_roots,
        source_call_occurrences: occurrences.source_calls,
        selected_ieee_float_comparison_occurrences: occurrences.ieee_float_comparisons,
        selected_integer_comparison_occurrences: occurrences.integer_comparisons,
        state_blocks: occurrences.state_blocks,
    };
    crate::producer_result::SourceMappedLowered::new(
        lowered,
        machine_ids,
        lowered_boundary_parameters
            .iter()
            .map(|boundary| (boundary.source, boundary.id))
            .collect(),
    )
}

/// Occurrence rows every emitted machine publishes beside its Terminal
/// operations. Each selected comparison or source call keeps the row
/// that joins it to its checked application; the closure publishes all of
/// them.
#[derive(Default)]
struct ClosureOccurrences {
    quotient_adaptations:
        Vec<crate::emission::operation_emission::buffer::SourceQuotientAdaptation>,
    quotient_calls: Vec<crate::emission::operation_emission::buffer::SourceQuotientCall>,
    source_calls: Vec<lowered_psi::LoweredSourceCallOccurrence>,
    ieee_float_comparisons: Vec<lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence>,
    integer_comparisons: Vec<lowered_psi::LoweredSelectedIntegerComparisonOccurrence>,
    source_checked_obligations: Vec<semantic_vocabulary::ObligationId>,
    state_blocks: Vec<lowered_psi::LoweredStateBlock>,
}

impl ClosureOccurrences {
    fn retain(
        &mut self,
        source_calls: Vec<lowered_psi::LoweredSourceCallOccurrence>,
        ieee_float_comparisons: Vec<lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence>,
        integer_comparisons: Vec<lowered_psi::LoweredSelectedIntegerComparisonOccurrence>,
    ) {
        self.source_calls.extend(source_calls);
        self.ieee_float_comparisons.extend(ieee_float_comparisons);
        self.integer_comparisons.extend(integer_comparisons);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckedUnitProviderCandidate {
    provider: symbols::SymbolHandle,
    boundary: symbols::SymbolHandle,
    candidate: symbols::SymbolHandle,
    requirement_identity: typed_trees::type_identity::CallableIdentityId,
    provider_identity: String,
    candidate_identity: typed_trees::type_identity::CallableIdentityId,
}
