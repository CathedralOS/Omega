//! Optimizer module role: executable entrance. Provenance and custody accounting reconstructed independently of producers.
//!
//! Common edge-custody preservation and scalar-substitution contracts live at
//! this entrance. Exact transformation accounting descends into named family
//! leaves.
use crate::candidates::copy_propagation::rewrite_block_parameter_operation;
use crate::unit_validation::derived_metadata::expected_edges;
use abstract_operations::AbstractOperation as O;
use optimization_unit::{OptimizationEdge, RedundantBlockParameterRewrite, ScalarSubstitution};
use semantic_vocabulary::{BlockId, MachineId};

pub(super) mod adjacent_merge;
pub(super) mod common_subexpression;
pub(super) mod dead_scalar;
pub(super) mod non_adjacent_merge;
pub(super) mod scalar_identity;
pub(crate) mod substitutions;
pub(super) mod terminal_fusion;
pub(super) mod threading;

pub(crate) fn preserve_edge_custody(
    node: &optimization_unit::OptimizationNode,
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
