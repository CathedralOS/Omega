//! Reads-from and modification-order coherence for normalized atomic
//! events under the bounded `happens_before` derivation.
//!
//! Every `AtomicEvent` operation carries retained ordering edges as
//! evidence — an observer's `reads_from` claim of which modification-order
//! member it read, and a write's `modification_after` claim of which
//! member it immediately follows. This check replays the axiom
//! independently of producer assertion across the function's whole block
//! graph: inside one activation `happens_before` is execution-path
//! sequencing, so a claim must resolve to a write sequenced before the
//! event in its own block or standing in a dominating block, and must be
//! the modification-order-latest write to the place on every execution
//! path — or the still-initial residency only when no write may precede.
//! Edges across activations do not resolve — they need the contract's
//! `synchronizes_with`/`global_sequential_order` reasoning that lands with
//! the concurrent-execution route.
use abstract_operations::{AbstractOperation, happens_before_atomic_coherence_violation};

use crate::OptimizationUnitValidationError;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::BlockId;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn validate_function_atomic_coherence(
    function: &PsiOptimizationFunction,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Result<(), OptimizationUnitValidationError> {
    let sequences: BTreeMap<BlockId, Vec<&AbstractOperation>> = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block.nodes.iter().map(|node| &node.operation).collect(),
            )
        })
        .collect();
    match happens_before_atomic_coherence_violation(
        &sequences,
        predecessors,
        dominators,
        function.entry,
        |predecessor, successor, root| {
            let predecessor = function.block(predecessor)?;
            let target = function.block(successor)?;
            let is_parameter = target.structural_parameter(root).is_some();
            let mut mapped = None;
            for edge in predecessor
                .successors()
                .filter(|edge| edge.target == successor)
            {
                // Case payloads are not whole-storage forwarding bindings. A
                // target-owned formal must have its exact binding on every edge.
                if is_parameter
                    && edge
                        .structural_bindings
                        .iter()
                        .filter(|binding| binding.parameter == root)
                        .count()
                        != 1
                {
                    return None;
                }
                let (source, path) =
                    crate::unit_validation::arrival_storage::projection_before_edge(edge, root);
                let before = (source, path.to_vec());
                if mapped.as_ref().is_some_and(|earlier| earlier != &before) {
                    return None;
                }
                mapped = Some(before);
            }
            mapped
        },
        |left, left_path, right, right_path| {
            abstract_operations::atomic::atomic_projections_may_alias(
                &function.structural_places,
                std::iter::once(function.structural_parameters.as_slice()).chain(
                    function
                        .blocks
                        .iter()
                        .map(|block| block.structural_parameters.as_slice()),
                ),
                left,
                left_path,
                right,
                right_path,
            )
        },
    ) {
        Some((block, node, violation)) => Err(
            OptimizationUnitValidationError::AtomicEventCoherenceMismatch {
                machine: function.machine,
                block,
                node: node as u32,
                violation,
            },
        ),
        None => Ok(()),
    }
}
