//! Reads-from coherence for normalized atomic events under the bounded
//! `happens_before` derivation.
//!
//! Every `AtomicEvent` operation carries a retained `reads_from` edge as
//! evidence — an observer's claim of which modification-order member it
//! read. This check replays the axiom independently of producer assertion
//! across the function's whole block graph: inside one activation
//! `happens_before` is execution-path sequencing, so the claim must
//! resolve to the write sequenced before the observer in its own block or
//! standing in a dominating block, and must be the
//! modification-order-latest write to the place on every execution path —
//! or the still-initial residency only when no write may precede. Edges
//! across activations do not resolve — they need the contract's
//! `synchronizes_with`/`global_sequential_order` reasoning that lands with
//! the concurrent-execution route.
use abstract_operations::{AbstractOperation, happens_before_atomic_coherence_violation};

use crate::BTreeMap;
use crate::BTreeSet;
use crate::BlockId;
use crate::OptimizationUnitValidationError;
use crate::PsiOptimizationFunction;

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
    match happens_before_atomic_coherence_violation(&sequences, predecessors, dominators) {
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
