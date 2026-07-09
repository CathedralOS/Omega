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
    if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
        eprintln!(
            "RB: dispatch {} case m{} s{} vs source m{}",
            dispatch_index,
            state.key.machine.arena_index(),
            state.key.state.arena_index(),
            source_machine.arena_index(),
        );
    }
    if state.key.machine == source_machine {
        return dispatch_receiver_base(input, dispatch_index);
    }
    // Spliced callee: the unique inline call from the case's state into
    // `source_machine`, on the entry-machine caller (slice-1 scope).
    if state.key.machine != input.entry_key.machine {
        return None;
    }
    // Match callee machines by ATTACHED-DATA equivalence, not machine-symbol
    // equality: the resolution sweep may land in ANY machine layout attached
    // to the same data (`SystemTime::from_unix_seconds` vs the called
    // `SystemTime::duration_since`), and the receiver identity is a property
    // of the DATA instance, not the particular machine (2026-07-10y -- the
    // reversed-operand residual's final hop).
    let source_attached = attached_data_of(input, source_machine);
    let mut found: Option<&omega_state_calls::StateCall> = None;
    for (_, call) in input.state_calls.calls.iter() {
        if !call.reachable
            || call.source_key.machine != state.key.machine
            || call.source_key.state != state.key.state
        {
            continue;
        }
        let target_matches = call.target_key.machine == source_machine
            || (source_attached.is_some()
                && attached_data_of(input, call.target_key.machine) == source_attached);
        if !target_matches {
            continue;
        }
        if found.is_some() {
            if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
                eprintln!("RB: -> AMBIGUOUS (dispatch {dispatch_index})");
            }
            return None; // ambiguous: two calls into the same data family
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
    let resolved =
        omega_layout::field_path_offset(input.layouts, caller_layout.fields, field_segments);
    if std::env::var_os("OMEGA_DEBUG_RECEIVER").is_some() {
        eprintln!(
            "RB: -> inline base {:?} (dispatch {}, receiver {})",
            resolved,
            dispatch_index,
            call.receiver_name.as_str(),
        );
    }
    resolved
}

fn attached_data_of<'plan>(
    input: &'plan InstructionSelectionInput<'_>,
    machine: omega_core::symbols::SymbolHandle,
) -> Option<&'plan str> {
    input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, layout)| layout.symbol == machine)
        .and_then(|(_, layout)| layout.attached_data.as_deref())
}
