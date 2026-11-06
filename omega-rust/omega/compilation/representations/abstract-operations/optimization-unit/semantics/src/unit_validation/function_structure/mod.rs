//! Optimizer module role: executable entrance. Ordered per-function validation coordination.
//!
//! The entrance preserves the acceptance order across retained catalogs,
//! claims and parameters, CFG shape, results, structural roots, provenance,
//! facts, values, ownership, and service contracts. Each invariant family
//! descends into one named leaf.
use crate::unit_validation::catalog_mismatch;
use crate::unit_validation::derived_metadata::validate_places_and_claims;
use crate::unit_validation::operation_contracts::validate_values_and_bindings;
use crate::unit_validation::services::valid_service_ceiling;
use crate::unit_validation::structural_catalog::{
    validate_byte_sequence_literal_witnesses, validate_function_structural_catalog,
    validate_provider_attachment_specialization, validate_trivial_affine_local_witnesses,
};
use crate::{OptimizationUnitValidationError, current_ownership};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
};
use std::collections::BTreeMap;

mod byte_concatenation;
mod byte_field_freshness;
mod case_arguments;
pub(crate) use case_arguments::membership_on_every_path;
mod structural_freshness;
pub use byte_field_freshness::byte_field_root;
mod byte_predicates;
mod byte_views;
mod descriptor_sources;
mod fixed_byte_freshness;
pub(crate) use byte_predicates::{
    byte_field_predicates, byte_write_predicates, concatenation_preserves_predicates,
    copied_field_predicates_at_invocation, copied_field_predicates_at_transfer,
    replacement_preserves_predicates, whole_source_predicates,
};
pub(crate) use byte_views::byte_field_store_capacity;
pub use byte_views::declared_descriptor_home;
mod control_flow;
mod fact_index;
mod mutable_views;
pub(crate) use mutable_views::current_mutable_borrows;
mod parameters;
mod provenance;
mod results;
mod structural_roots;
pub(crate) use structural_roots::{
    ReferenceMembershipValidation, operation_place_accesses, operation_place_inputs,
};
mod structural_transfers;

pub(crate) use control_flow::{ControlCyclePolicy, FunctionControlFlow};
pub(crate) use fact_index::reconstruct_fact_index;

/// Borrow the catalogs already indexed for this exact unit. This is input to
/// function checking, not a receipt that any function has completed checking.
pub(crate) struct FunctionCatalogs<'catalog, 'unit> {
    pub(crate) unit: &'unit PsiOptimizationUnit,
    pub(crate) functions: &'catalog BTreeMap<MachineId, &'unit PsiOptimizationFunction>,
    pub(crate) boundary_machines:
        &'catalog BTreeMap<BoundaryMachineId, &'unit terminal_psi::BoundaryMachineDeclaration>,
    pub(crate) services: &'catalog BTreeMap<ServiceId, &'unit terminal_psi::ServiceDeclaration>,
    pub(crate) structural_types:
        &'catalog BTreeMap<StructuralTypeId, &'unit terminal_psi::StructuralTypeDeclaration>,
    pub(crate) structural_domains:
        &'catalog BTreeMap<StructuralDomainId, &'unit terminal_psi::StructuralDomainDeclaration>,
}

pub(crate) fn validate_function(
    catalogs: &FunctionCatalogs<'_, '_>,
    function: &PsiOptimizationFunction,
    cycle_policy: &control_flow::ControlCyclePolicy,
    call_accesses: &mut current_ownership::CallAccesses,
    descriptor_requirements: &mut Vec<
        terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement,
    >,
    ownership_frontiers: &mut Vec<optimization_unit::OwnershipFrontierFact>,
    reference_memberships: Option<&mut ReferenceMembershipValidation<'_>>,
) -> Result<(), OptimizationUnitValidationError> {
    let control_flow = prepare_function(
        catalogs,
        function,
        cycle_policy,
        call_accesses,
        reference_memberships,
    )?;
    let completion =
        control_flow.replay_ownership(catalogs, function, call_accesses, ownership_frontiers)?;
    descriptor_requirements.extend(completion.into_requirements());
    Ok(())
}

/// Check a function's structure before using its ownership result.
/// Return the checked graph for the later ownership replay.
pub(crate) fn prepare_function<'a>(
    catalogs: &FunctionCatalogs<'_, '_>,
    function: &'a PsiOptimizationFunction,
    cycle_policy: &control_flow::ControlCyclePolicy,
    call_accesses: &mut current_ownership::CallAccesses,
    reference_memberships: Option<&mut ReferenceMembershipValidation<'_>>,
) -> Result<FunctionControlFlow<'a>, OptimizationUnitValidationError> {
    let FunctionCatalogs {
        unit,
        functions,
        boundary_machines,
        services,
        structural_types,
        structural_domains,
    } = catalogs;
    if let Some(subject) = function.nominal_cleanup_subject
        && (function.attachment != Some(subject)
            || !structural_types.contains_key(&subject)
            || functions
                .values()
                .filter(|candidate| candidate.nominal_cleanup_subject == Some(subject))
                .count()
                != 1)
    {
        return Err(catalog_mismatch(function));
    }
    if !valid_service_ceiling(&function.published_service_ceiling, services) {
        return Err(
            OptimizationUnitValidationError::InvalidFunctionServiceCeiling(function.machine),
        );
    }
    let (byte_sequence_literals, trivial_affine_locals) = validate_function_structural_catalog(
        function,
        &unit.identities,
        structural_types,
        structural_domains,
    )?;
    validate_provider_attachment_specialization(function, boundary_machines, structural_types)?;
    structural_roots::validate_structural_root_uniqueness(function)?;

    parameters::validate_entry_claim_index(function)?;
    let blocks = control_flow::index_blocks(function)?;
    parameters::validate_parameter_metadata(function)?;
    structural_transfers::validate(function, structural_types, structural_domains)?;
    let control_flow = control_flow::validate_nodes_and_edges(function, blocks)?;
    control_flow::validate_total_cfg(
        function,
        &control_flow.blocks,
        &control_flow.successors,
        cycle_policy.admits(function.machine),
    )?;
    results::validate_function_results(function)?;

    validate_byte_sequence_literal_witnesses(function, &byte_sequence_literals)?;
    validate_trivial_affine_local_witnesses(function, &trivial_affine_locals)?;
    structural_roots::validate_structural_place_availability(
        function,
        &control_flow.blocks,
        &control_flow.predecessors,
    )?;
    structural_roots::validate_structural_root_operations(
        unit,
        function,
        structural_types,
        structural_domains,
        &control_flow.predecessors,
        reference_memberships,
        call_accesses.view_graph(),
    )?;
    crate::unit_validation::references::validate_function_references(
        function,
        functions,
        structural_types,
        &unit.dynamic_dispatch,
    )?;
    byte_concatenation::validate(
        function,
        structural_types,
        structural_domains,
        &control_flow.predecessors,
    )?;
    byte_field_freshness::validate(function, structural_types, &control_flow.predecessors)?;
    descriptor_sources::validate_whole_freshness(
        function,
        structural_types,
        &control_flow.predecessors,
    )?;
    case_arguments::validate(function, structural_types, &control_flow.predecessors)?;
    byte_predicates::validate_indexed_byte_stores(
        unit,
        function,
        structural_types,
        &control_flow.predecessors,
    )?;
    mutable_views::validate(function, structural_types)?;
    provenance::validate_provenance_fuel_effects(function)?;
    fact_index::validate_fact_index(function)?;
    validate_values_and_bindings(
        unit,
        function,
        &control_flow.blocks,
        &control_flow.predecessors,
        functions,
        boundary_machines,
        services,
        structural_types,
        structural_domains,
        call_accesses,
    )?;
    validate_places_and_claims(function)?;
    Ok(control_flow)
}
