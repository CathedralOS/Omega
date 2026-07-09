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
//! LIVE for dispatch-routed calls (the fence relaxes exactly there);
//! inline-routed calls stay fenced until the inline half lands.

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

/// The receiver base for resolving a place whose expression belongs to
/// `source_machine`, under the dispatch case `dispatch_index`.
///
/// Two routes, one seam:
/// - the state IS the case (a dispatch clone or the caller's own state):
///   the precomputed per-dispatch table answers (per-instance DISPATCH).
/// - the state is a SPLICED CALLEE under a caller case (inline branching:
///   the prelude's `self.X` reads carry the callee machine while the case
///   is the caller's) -- recover the receiver from the UNIQUE inline call
///   in that state targeting the callee machine. Ambiguity (two calls to
///   the same callee machine in one state) returns None: the by-type walk
///   stays, and the fence keeps refusing that shape. Probed 2026-07-10t:
///   the prelude read a.seconds@0 instead of sum.seconds@56.
pub(in crate::selection) fn receiver_base_for(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_machine: omega_core::symbols::SymbolHandle,
) -> Option<usize> {
    let state = input
        .runtime_flow
        .states
        .iter()
        .find(|(handle, _)| handle.arena_index() == dispatch_index)
        .map(|(_, state)| state)?;
    if state.key.machine == source_machine {
        return dispatch_receiver_base(input, dispatch_index);
    }
    // Spliced callee: the unique inline call from the case's state into
    // `source_machine`, on the entry-machine caller (slice-1 scope).
    if state.key.machine != input.entry_key.machine {
        return None;
    }
    let mut found: Option<&omega_state_calls::StateCall> = None;
    for (_, call) in input.state_calls.calls.iter() {
        if !call.reachable
            || call.source_key.machine != state.key.machine
            || call.source_key.state != state.key.state
            || call.target_key.machine != source_machine
        {
            continue;
        }
        if found.is_some() {
            return None; // ambiguous: two calls to the same callee machine
        }
        found = Some(call);
    }
    let call = found?;
    let receiver_name = call.receiver_name.as_str();
    if receiver_name.is_empty() || receiver_name == "self" {
        return None;
    }
    let segments = input
        .state_calls
        .receiver_path_segments
        .span(call.receiver_path)
        .unwrap_or(&[]);
    let field_segments = match segments.first() {
        Some(root) if root.as_str() == "self" => &segments[1..],
        _ => segments,
    };
    let caller_layout = input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == state.key.machine)
        .map(|(_, machine_layout)| machine_layout)?;
    if field_segments.is_empty() {
        return omega_layout::field_path_offset(
            input.layouts,
            caller_layout.fields,
            std::slice::from_ref(&call.receiver_name),
        );
    }
    omega_layout::field_path_offset(input.layouts, caller_layout.fields, field_segments)
}
