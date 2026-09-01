//! Optimizer module role: executable entrance. Ordered per-function validation coordination.
//!
//! The entrance preserves the acceptance order across retained catalogs,
//! claims and parameters, CFG shape, results, structural roots, provenance,
//! facts, values, ownership, and service contracts. Each invariant family
//! descends into one named leaf.

use super::*;

mod control_flow;
mod fact_index;
mod parameters;
mod provenance;
mod results;
mod structural_roots;

pub(crate) use control_flow::ControlCyclePolicy;
pub(crate) use fact_index::reconstruct_fact_index;

pub(crate) fn validate_function(
    function: &PsiOptimizationFunction,
    unit_entry: MachineId,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
    cycle_policy: &control_flow::ControlCyclePolicy,
) -> Result<(), OptimizationUnitValidationError> {
    if !valid_service_ceiling(&function.published_service_ceiling, services) {
        return Err(
            OptimizationUnitValidationError::InvalidFunctionServiceCeiling(function.machine),
        );
    }
    let (byte_sequence_literals, trivial_affine_locals) =
        validate_function_structural_catalog(function, structural_types, structural_domains)?;
    validate_provider_attachment_specialization(function, boundary_machines, structural_types)?;
    structural_roots::validate_structural_root_uniqueness(function)?;

    parameters::validate_entry_claim_index(function)?;
    let blocks = control_flow::index_blocks(function)?;
    parameters::validate_parameter_metadata(function)?;
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
    structural_roots::validate_structural_root_operations(function, unit_entry, structural_types)?;
    provenance::validate_provenance_fuel_effects(function)?;
    fact_index::validate_fact_index(function)?;
    validate_values_and_bindings(
        function,
        &control_flow.blocks,
        &control_flow.predecessors,
        functions,
        boundary_machines,
        services,
        structural_types,
        structural_domains,
    )?;
    validate_places_and_claims(function)?;
    current_ownership::validate_current_ownership_frontier(
        function,
        &control_flow.blocks,
        &control_flow.successors,
        functions,
        boundary_machines,
        structural_types,
    )
}
