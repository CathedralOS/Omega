//! Independent validation of the runtime-value spill.
//!
//! The validator never calls [`super::admission`]: it re-derives the spill's
//! legality from the source records — the named register's roster row, origin,
//! scalar payload and definition-site custody; the exact edge or boundary
//! definitions the contract demands; the use scan over instruction operands,
//! terminator operands, outgoing edge transports and case payloads; the
//! dominance the definition anchor must hold over every use block; the frame
//! rows' declared operand interface, the carrier class and any bit-preserving
//! conversion pair; the span policy's open/close decision and the structural
//! snapshot's chunk decomposition; the slot-reuse analysis over its own
//! derived positions; and the measured-step budget — then rebuilds the
//! expected fresh registers, stores and reload pairs itself and requires the
//! proposal to carry exactly them before restoring the complete source by
//! content. A producer admission error therefore fails validation even when
//! the proposal is exactly what that producer emitted.
//!
//! The shared `control`, `require_dominated_uses`, `slot::assign`,
//! `stored_transport_size`, `next_chunk` and `surviving_home_exists` helpers
//! stay shared: the audit reads the same contract, not the producer's record.
use selected_instructions::SelectedInstructionPlan;

use super::{RuntimeSpillError, RuntimeSpillReceipt, ValidatedRuntimeSpill};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;
/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_runtime_spill_with_span_policy(
    source: &impl ValidatedSelectedAnalysis,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    Ok(sealed!(
        ValidatedRuntimeSpill,
        RuntimeSpillReceipt,
        source,
        proposed
    ))
}
