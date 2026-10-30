//! Optimizer module role: executable entrance. Independent operation-contract validation.
//!
//! Value flow, node contract families, bindings, structural access, claims,
//! payloadless cases, boundaries, and scalar typing descend into named leaves.
//! This entrance owns their exact per-node validation order.
use crate::OptimizationUnitValidationError;
use crate::unit_validation::derived_metadata::dominators;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, MachineId, ServiceId, StructuralDomainId, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};

mod affine_calls;
mod atomic_coherence;
mod boundaries;
mod claim_transfers;
pub(crate) use claim_transfers::{function_entry_claim_paths, validate_slot_claim_transfers};
mod node_contracts;
mod payloadless_cases;
mod plain_calls;
mod records;
mod scalar_arrays;
mod scalar_types;
mod service_calls;
mod structural_access;
mod structural_cases;
mod values;

pub(crate) use records::constructible;
pub(crate) use structural_access::{
    owned_bounded_owner_loan, structural_access_can_supply, structural_operation_result_contract,
    structural_source_contract,
};

pub(crate) fn validate_values_and_bindings(
    unit: &optimization_unit::PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
    call_accesses: &crate::current_ownership::CallAccesses,
) -> Result<(), OptimizationUnitValidationError> {
    let definitions = values::collect_value_definitions(function)?;
    let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
    atomic_coherence::validate_function_atomic_coherence(
        function,
        structural_types,
        predecessors,
        &dominators,
    )?;
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            values::validate_node_uses(
                function,
                block,
                node_index,
                node,
                &definitions,
                &dominators,
            )?;
            node_contracts::validate_node_operation_contracts(
                unit,
                function,
                block,
                node_index,
                node,
                &definitions,
                functions,
                boundary_machines,
                services,
                structural_types,
                structural_domains,
                call_accesses,
            )?;
            values::validate_successor_bindings(
                function,
                node,
                &definitions,
                blocks,
                structural_types,
            )?;
        }
    }
    Ok(())
}
