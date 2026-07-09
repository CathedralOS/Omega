//! PER-INSTANCE receiver dispatch, the resolution half (TASKS_FS "Stolen
//! work #2"): a dispatched callee clone's machine-storage BASE, recovered
//! from the clone context's MINTING CALL SITE instead of the
//! first-type-match walk. `self.sum.checked_subtract(..)` with several
//! `Duration` fields must resolve `sum`'s storage (offset 56), not the
//! first `Duration` (offset 0).
//!
//! Chain: dispatch_index == the runtime-flow state's arena index (the
//! state-dispatch context assigns exactly that) -> RuntimeState { key,
//! context } -> RuntimeFlowPlan::context_call_sites[context] -> the minting
//! StateCall -> receiver_path -> omega_layout::field_path_offset (the SAME
//! walk the contained-receiver fence predicts with, by construction).
//!
//! SLICE 1 SCOPE: overrides only when the CALLER is the entry machine (a
//! nested caller's own base would need recursive context resolution; the
//! fence keeps guarding those chains) and only for a named non-`self`
//! receiver. `None` = no override; the by-type walk stays authoritative.
//! GATED behind OMEGA_RECEIVER_DISPATCH=1 while the fence still refuses
//! every affected program -- the mechanism is probeable but dormant.

use crate::InstructionSelectionInput;

pub(in crate::selection) fn dispatch_receiver_base(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
) -> Option<usize> {
    // The pipeline computes the table ONCE (compute_receiver_bases in the
    // backend-pipeline builder); empty when the gate is off.
    input
        .receiver_bases
        .get(dispatch_index as usize)
        .copied()
        .flatten()
}
