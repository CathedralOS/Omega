//! Replays one legalized leaf, then seals its return and edge-fuel contract.

mod exact_arithmetic;
mod fuel;
mod immediate;
mod recipe;

pub(super) use exact_arithmetic::replay_active_resident_chain_shape;
use exact_arithmetic::{replay_exact_add_node, replay_exact_binary, replay_widened_exact_binary};
pub(super) use fuel::replay_edge_fuel;
use fuel::replay_operation_fuel;
use immediate::{replay_constant, replay_immediate};
use recipe::replay_leaf_value;

use super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_leaf(
    function: usize,
    recipe: LegalizationRecipe,
    arm_edge: EdgeId,
    target: &TargetIntegerControl,
    abstract_operations: &[AbstractOperation],
    nodes: &[omega_optimization_unit::OptimizationNode],
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[omega_optimization_unit::AcceptedObligationFact],
    proposed: &LegalizedLeaf,
    architecture: omega_target::Architecture,
    temporary_base: u32,
) -> Result<Vec<OperationId>, LegalizationError> {
    if nodes.len() != abstract_operations.len()
        || nodes
            .iter()
            .zip(abstract_operations)
            .any(|(node, operation)| node.operation != *operation)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let TargetIntegerControl::Return {
        psi_return_edge,
        source_value,
        expression,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if proposed.return_edge != *psi_return_edge || proposed.source_value != *source_value {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

    let u64_integer = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u64_type = ScalarType::Integer(u64_integer);
    let (return_node, operations) = replay_leaf_value(
        function,
        recipe,
        arm_edge,
        expression,
        source_value,
        nodes,
        abstracted,
        optimized,
        accepted_facts,
        proposed,
        architecture,
        temporary_base,
        u64_type,
    )?;

    let AbstractOperation::Return {
        psi_edge,
        value,
        scalar_type,
        cleanup_actions,
        ..
    } = &return_node.operation
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    if *psi_edge != *psi_return_edge
        || *value != *source_value
        || *scalar_type != u64_type
        || !cleanup_actions.is_empty()
        || return_node.provenance != vec![PsiProvenance::Edge(*psi_return_edge)]
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    replay_edge_fuel(
        function,
        *psi_return_edge,
        &return_node.fuel,
        &proposed.return_fuel,
    )?;
    Ok(operations)
}
