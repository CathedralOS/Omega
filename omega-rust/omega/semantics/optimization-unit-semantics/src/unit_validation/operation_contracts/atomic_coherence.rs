//! Serial reads-from coherence for normalized atomic events.
//!
//! Every `AtomicEvent` operation carries a retained `reads_from` edge as
//! evidence — an observer's claim of which modification-order member it
//! read. This check replays the serial axiom independently of producer
//! assertion: within one block, node order is the `sequenced_before` and
//! `modification_order` source of truth, so the claim must resolve to the
//! latest write to the place sequenced before the observer, or to the
//! pre-block residency only when the block writes the place no earlier.
//! Edges across blocks do not resolve — they need the contract's
//! `synchronizes_with`/`happens_before` reasoning that lands with the
//! concurrent-execution route.
use abstract_operations::serial_atomic_coherence_violation;

use crate::OptimizationUnitValidationError;
use crate::PsiOptimizationFunction;

pub(crate) fn validate_block_atomic_coherence(
    function: &PsiOptimizationFunction,
    block: &optimization_unit::OptimizationBlock,
) -> Result<(), OptimizationUnitValidationError> {
    match serial_atomic_coherence_violation(block.nodes.iter().map(|node| &node.operation)) {
        Some((node, violation)) => Err(
            OptimizationUnitValidationError::AtomicEventCoherenceMismatch {
                machine: function.machine,
                block: block.id,
                node: node as u32,
                violation,
            },
        ),
        None => Ok(()),
    }
}
