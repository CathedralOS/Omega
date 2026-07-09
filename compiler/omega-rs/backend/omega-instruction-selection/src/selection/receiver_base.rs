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
    if std::env::var_os("OMEGA_RECEIVER_DISPATCH").is_none() {
        return None;
    }
    let state = input
        .runtime_flow
        .states
        .iter()
        .find(|(handle, _)| handle.arena_index() == dispatch_index)
        .map(|(_, state)| state)?;
    let (call_key, statement_index) = *input
        .runtime_flow
        .context_call_sites
        .get(state.context.0 as usize)?;
    if statement_index == usize::MAX {
        return None; // ROOT: the entry machine, no minting call
    }
    // Slice 1: the caller must BE the entry machine (its own base is 0);
    // deeper chains stay fenced.
    if call_key.machine != input.entry_key.machine {
        return None;
    }
    let state_call = input
        .state_calls
        .calls
        .iter()
        .map(|(_, call)| call)
        .find(|call| call.source_key == call_key && call.statement_index == statement_index)?;
    if state_call.receiver_name.as_str().is_empty() || state_call.receiver_name.as_str() == "self" {
        return None; // self/static receiver: entry storage, no override
    }
    let segments = input
        .state_calls
        .receiver_path_segments
        .span(state_call.receiver_path)
        .unwrap_or(&[]);
    let field_segments = match segments.first() {
        Some(root) if root.as_str() == "self" => &segments[1..],
        _ => segments,
    };
    let caller_layout = input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == call_key.machine)
        .map(|(_, machine_layout)| machine_layout)?;
    if field_segments.is_empty() {
        // Statement-position calls carry only the leaf name.
        return omega_layout::field_path_offset(
            input.layouts,
            caller_layout.fields,
            std::slice::from_ref(&state_call.receiver_name),
        );
    }
    omega_layout::field_path_offset(input.layouts, caller_layout.fields, field_segments)
}
