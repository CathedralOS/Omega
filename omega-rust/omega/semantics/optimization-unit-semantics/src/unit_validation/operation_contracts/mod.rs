//! Optimizer module role: executable entrance. Independent operation-contract validation.
//!
//! Value flow, node contract families, bindings, structural access, claims,
//! payloadless cases, boundaries, and scalar typing descend into named leaves.
//! This entrance owns their exact per-node validation order.
use crate::BTreeMap;
use crate::BTreeSet;
use crate::BlockId;
use crate::BoundaryMachineId;
use crate::MachineId;
use crate::OptimizationUnitValidationError;
use crate::PsiOptimizationFunction;
use crate::ServiceId;
use crate::StructuralDomainId;
use crate::StructuralTypeId;
use crate::dominators;

mod affine_calls;
mod atomic_coherence;
mod boundaries;
mod claim_transfers;
mod node_contracts;
mod payloadless_cases;
mod records;
mod scalar_arrays;
mod scalar_types;
mod service_calls;
mod structural_access;
mod structural_cases;
mod values;

pub(crate) use affine_calls::*;
pub(crate) use boundaries::*;
pub(crate) use claim_transfers::*;
pub(crate) use payloadless_cases::*;
pub(crate) use records::*;
pub(crate) use scalar_arrays::*;
pub(crate) use scalar_types::*;
pub(crate) use service_calls::*;
pub(crate) use structural_access::*;

pub(crate) fn validate_values_and_bindings(
    function: &PsiOptimizationFunction,
    blocks: &BTreeMap<BlockId, &optimization_unit::OptimizationBlock>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &terminal_psi::ServiceDeclaration>,
    structural_types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let definitions = values::collect_value_definitions(function)?;
    let dominators = dominators(function.entry, blocks.keys().copied(), predecessors);
    atomic_coherence::validate_function_atomic_coherence(function, predecessors, &dominators)?;
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
