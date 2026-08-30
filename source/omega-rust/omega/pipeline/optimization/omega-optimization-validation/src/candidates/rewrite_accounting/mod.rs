//! Optimizer module role: executable entrance. Provenance and custody accounting reconstructed independently of producers.
//!
//! Common edge-custody preservation and scalar-substitution contracts live at
//! this entrance. Exact transformation accounting descends into named family
//! leaves.

use super::*;

mod adjacent_merge;
mod common_subexpression;
mod dead_scalar;
mod non_adjacent_merge;
mod scalar_identity;
mod substitutions;
mod terminal_fusion;
mod threading;

pub(crate) use adjacent_merge::*;
pub(crate) use common_subexpression::*;
pub(crate) use dead_scalar::*;
pub(crate) use non_adjacent_merge::*;
pub(crate) use scalar_identity::*;
pub(crate) use substitutions::*;
pub(crate) use terminal_fusion::*;
pub(crate) use threading::*;

pub(crate) fn preserve_edge_custody(
    node: &omega_optimization_unit::OptimizationNode,
) -> Vec<OptimizationEdge> {
    let expected = expected_edges(&node.operation);
    expected
        .into_iter()
        .map(|mut edge| {
            if let Some(existing) = node
                .successors
                .iter()
                .find(|existing| existing.psi_edge == edge.psi_edge)
            {
                edge.provenance = existing.provenance.clone();
                edge.fuel = existing.fuel.clone();
            }
            edge
        })
        .collect()
}

pub(crate) fn rewrite_scalar_substitutions(
    operation: &mut O,
    substitutions: &[ScalarSubstitution],
    machine: MachineId,
    removed_block: BlockId,
) {
    for substitution in substitutions {
        rewrite_block_parameter_operation(
            operation,
            RedundantBlockParameterRewrite {
                machine,
                block: removed_block,
                position: 0,
                parameter: substitution.from,
                replacement: substitution.to,
                scalar_type: substitution.scalar_type,
            },
        );
    }
}
