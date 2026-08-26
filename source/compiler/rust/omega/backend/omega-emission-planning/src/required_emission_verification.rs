//! Post-instruction-selection invariant check.
//!
//! Every operation the plan marks `required` should produce at least one emitted
//! machine instruction (or be deliberately subsumed by control-flow / inlining).
//! When the backend silently DROPS a required cross-machine call or a required
//! mutation, the failure otherwise surfaces only as a far-away runtime SEGFAULT
//! in the emitted program (no symbols, debugged via cdb + RIP archaeology). This
//! pass turns that class of silent drop into a LOUD compile-time blocker that
//! names the machine::state, statement index, and the dropped operation.
//!
//! It runs AFTER instruction selection and correlates each `required` planned
//! item to the selected [`TargetOperation`] list by `(source_key,
//! source_statement)` — the same correlation the backend report uses to print
//! "required true" items alongside the selected instructions.
//!
//! ## Conservatism
//!
//! The structural blockers in this crate already gate most unplanned items by
//! inspecting plan-level structures (runtime bodies, branching-call expansions,
//! storage writes). This pass is deliberately a thin, additive belt-and-suspenders
//! layer that only fires for the highest-confidence class of drop.
//!
//! A required `Statement`-role state call from a dispatched state lowers to a
//! dispatch TRANSITION (a `SetDispatchState` / `TerminateDispatch` /
//! return-value write), but selection attributes that transition to the dispatch
//! EDGE's statement index — which is frequently NOT the call's own statement
//! index — and an inlined callee's body instructions carry the callee's source
//! key. Per-statement correlation therefore produces false positives on
//! perfectly-lowered programs (the dungeon crawler included). The only signal we
//! can trust without false positives is per-source-state: if a state contains a
//! `required` cross-machine `Statement` call yet its emitted instructions contain
//! NO dispatch-transition effect AT ALL — i.e. the entire continuation chain for
//! that state vanished — the call was genuinely dropped and the program would
//! fall through to a null/garbage continuation at runtime. Any state that emits
//! even one transition is treated as covered. It is far better to under-report
//! than to false-positive and break a currently-green program.

use crate::EmissionPlanningInput;
use crate::semantic_scope::{proof_scope_suffix, state_name};
use omega_control_flow::StateKey;
use omega_state_calls::{StateCall, StateCallRole};
use omega_target_operations::SelectedInstructionKind;
use psi_arena::Arena;

use super::{EmissionBlocker, blocker};

/// Adds blockers for `required` planned items that produced no emitted
/// instruction at their source site. Conservative by construction (see module
/// docs): only the dropped-Statement-state-call class is reported, and only when
/// the site is empty AND no dispatch transition covers it.
pub(super) fn verify_required_items_emitted(
    input: &EmissionPlanningInput<'_>,
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    // This invariant only applies to the runtime-dispatch lowering path, where a
    // required state call becomes a dispatch edge that MUST emit a transition. In
    // the straight-line schedule path, state calls are inlined inline and the
    // existing schedule/state-call blockers already cover gaps.
    if !needs_runtime_dispatch {
        return;
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if !required_dispatch_statement_call(input, state_call) {
            continue;
        }

        // Per-source-state coverage (see module docs): the call survives as long
        // as its source state emitted ANY dispatch transition. Only a state whose
        // entire continuation chain vanished is a confident drop.
        if source_state_emits_transition(input, state_call.source_key) {
            continue;
        }

        report_dropped_statement_call(input, state_call, blockers);
    }
}

/// True for a `required`, resolved `Statement`-role CROSS-MACHINE state call whose
/// source state participates in the dispatched runtime flow. These are the calls
/// that, if dropped, fall straight through to a null/garbage continuation at
/// runtime. Same-state recursion is excluded — a self-call's transition is
/// indistinguishable from the state's own dispatch entry.
fn required_dispatch_statement_call(
    input: &EmissionPlanningInput<'_>,
    state_call: &StateCall,
) -> bool {
    state_call.required
        && state_call.role == StateCallRole::Statement
        && state_call.target_key.is_valid()
        && !state_key_matches(state_call.source_key, state_call.target_key)
        && source_state_is_dispatched(input, state_call.source_key)
}

/// The source state is realized as a dispatched runtime node (has a runtime body
/// / runtime-flow state). A call from a state that never enters the dispatch loop
/// is out of scope for this check.
fn source_state_is_dispatched(input: &EmissionPlanningInput<'_>, source_key: StateKey) -> bool {
    input
        .runtime_flow
        .states
        .iter()
        .any(|(_, state)| state_key_matches(state.key, source_key))
        || input
            .runtime_bodies
            .bodies
            .iter()
            .any(|(_, body)| state_key_matches(body.key, source_key))
}

/// A dispatched state's continuation chain survived selection: it emitted at
/// least one dispatch-transition effect somewhere in its body. A required
/// statement state call from such a state is treated as covered — its transition
/// is attributed to a dispatch edge whose statement index / source key we cannot
/// reliably tie back to the call site (see module docs). The absence of ANY
/// transition for a state that is supposed to hand off to a callee is the
/// unambiguous "the call produced no instructions" failure.
fn source_state_emits_transition(input: &EmissionPlanningInput<'_>, source_key: StateKey) -> bool {
    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            state_key_matches(instruction.source_key, source_key)
                && instruction_is_dispatch_transition(&instruction.kind)
        })
}

/// Selected instruction kinds that realize a hand-off to a continuation/callee:
/// a dispatch-state set, a terminal/return transition, or a return-value write.
/// These are exactly what a dispatched `Statement` state call lowers to.
fn instruction_is_dispatch_transition(kind: &SelectedInstructionKind) -> bool {
    matches!(
        kind,
        SelectedInstructionKind::SetDispatchState { .. }
            | SelectedInstructionKind::TerminateDispatch
            | SelectedInstructionKind::WriteReturnRegisterInteger { .. }
            | SelectedInstructionKind::CopyRuntimeStorageToReturnRegister { .. }
    )
}

fn report_dropped_statement_call(
    input: &EmissionPlanningInput<'_>,
    state_call: &StateCall,
    blockers: &mut Arena<EmissionBlocker>,
) {
    let source_name = state_name(input, state_call.source_key);
    let target_name = state_name(input, state_call.target_key);
    blockers.insert(blocker(
        "required emission",
        &format!(
            "required state call {} -> {} (statement {}) produced no instructions{}",
            source_name,
            target_name,
            state_call.statement_index,
            proof_scope_suffix(input, state_call.source_key)
        ),
    ));
}

fn state_key_matches(actual: StateKey, expected: StateKey) -> bool {
    actual == expected || (actual.machine == expected.machine && actual.state == expected.state)
}

#[cfg(test)]
mod tests {
    use super::instruction_is_dispatch_transition;
    use omega_target_operations::SelectedInstructionKind;

    #[test]
    fn dispatch_transitions_are_recognized_as_continuation_handoffs() {
        assert!(instruction_is_dispatch_transition(
            &SelectedInstructionKind::SetDispatchState { dispatch_index: 3 }
        ));
        assert!(instruction_is_dispatch_transition(
            &SelectedInstructionKind::TerminateDispatch
        ));
        assert!(instruction_is_dispatch_transition(
            &SelectedInstructionKind::WriteReturnRegisterInteger {
                register: omega_calling_conventions::MachineRegister::X86Rax,
                byte_size: 8,
                value: 0,
            }
        ));
    }

    #[test]
    fn inert_scaffolding_markers_do_not_prove_a_call_survived() {
        // These are emitted for every dispatched function regardless of whether
        // any required state call was actually lowered, so they must not count as
        // continuation hand-offs.
        assert!(!instruction_is_dispatch_transition(
            &SelectedInstructionKind::EnterFunction
        ));
        assert!(!instruction_is_dispatch_transition(
            &SelectedInstructionKind::LeaveFunction
        ));
        assert!(!instruction_is_dispatch_transition(
            &SelectedInstructionKind::LeaveDispatchCase
        ));
        assert!(!instruction_is_dispatch_transition(
            &SelectedInstructionKind::LeaveDispatchLoop
        ));
    }
}
