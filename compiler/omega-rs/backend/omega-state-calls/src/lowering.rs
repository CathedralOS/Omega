use crate::StateCallPlanningContext;
use crate::StateCallRole;
use omega_control_flow::{OperationKind, StateKey};

use super::StateCallLowering;
use super::collection::CollectedStateCall;
use super::lookups::{state_flow_from_key, state_key_is_valid};

pub(crate) fn state_call_lowering(
    context: &StateCallPlanningContext,
    call: &CollectedStateCall,
    calling_states: &[StateKey],
) -> StateCallLowering {
    if !state_key_is_valid(call.target_key) {
        StateCallLowering::Unresolved
    } else if tail_self_call(context, call) {
        // The tail-call-to-loop transform's plan half: qualified tail
        // self-calls lower as the entry-transition loop (see the variant
        // doc); every inline path skips them by construction.
        StateCallLowering::DispatchLoop
    } else if state_call_targets_leaf(context, call) && !calling_states.contains(&call.target_key) {
        // A leaf has no transitions AND makes no calls of its own. The
        // `calling_states` guard catches a target whose only call is in a `let`
        // initializer / argument expression (not an `OperationKind::Call` op), which
        // `state_call_targets_leaf` alone would miss -- such a target must be an
        // InlineExpansion so its nested call is delivered, not a leaf that drops it.
        StateCallLowering::InlineLeaf
    } else if state_call_targets_branching_state(context, call) {
        StateCallLowering::InlineBranching
    } else {
        StateCallLowering::InlineExpansion
    }
}

fn state_call_targets_branching_state(
    context: &StateCallPlanningContext,
    call: &CollectedStateCall,
) -> bool {
    state_flow_from_key(context, call.target_key)
        .and_then(|state| context.control_flow.transitions.span(state.transitions))
        .is_some_and(|transitions| !transitions.is_empty())
}

fn state_call_targets_leaf(context: &StateCallPlanningContext, call: &CollectedStateCall) -> bool {
    let Some(state) = state_flow_from_key(context, call.target_key) else {
        return false;
    };

    let transitions_are_empty = context
        .control_flow
        .transitions
        .span(state.transitions)
        .is_none_or(|transitions| transitions.is_empty());
    if !transitions_are_empty {
        return false;
    }

    context
        .control_flow
        .operations
        .span(state.operations)
        .is_none_or(|operations| {
            operations.iter().all(|operation| {
                !matches!(operation.kind, OperationKind::Call { .. })
                    || context.state_statement_has_host_call_by_key(
                        call.target_key,
                        operation.statement_index,
                    )
            })
        })
}

/// Whether a call qualifies for the tail-call-to-loop rewrite (the
/// runtime-flow builder re-checks the same legs and errors on drift). All
/// four legs are load-bearing -- see the flow builder's
/// `tail_self_call_qualifies` for the reasoning per leg.
fn tail_self_call(context: &StateCallPlanningContext, call: &CollectedStateCall) -> bool {
    use omega_checked_trees::expression::ExpressionNode;
    if std::env::var_os("OMEGA_DEBUG_TAILCALL").is_some()
        && call.target_key.machine == call.source_key.machine
    {
        eprintln!(
            "TAILCALL PLAN: same-machine call receiver_name={:?} role={:?} stmt {}",
            call.receiver_name.as_str(),
            call.role,
            call.statement_index,
        );
    }
    if call.receiver_name.as_str() != "self" {
        return false;
    }
    if call.role == StateCallRole::Statement {
        // Slice 1 is the VALUE spelling (`{ self.sum(..) }` as the state's
        // terminal). The statement spelling (unit machines) follows once
        // probed.
        return false;
    }
    if !state_key_is_valid(call.target_key) || call.target_key.machine != call.source_key.machine {
        return false;
    }
    let Some(state) = state_flow_from_key(context, call.source_key) else {
        return false;
    };
    // The bare-terminal spelling is TRANSITION-EMBEDDED: the call lives in
    // the terminal's target_value, and the state has NO operations at all.
    // Any operation means prior work (a let, a mutation, another call) the
    // rewrite would skip -- reject those shapes.
    let has_operations = context
        .control_flow
        .operations
        .span(state.operations)
        .is_some_and(|operations| !operations.is_empty());
    if has_operations {
        return false;
    }
    // Exactly one transition: an unguarded Terminal whose value IS the bare
    // recursive call (the loop's leaf terminal delivers the result; any
    // wrapper expression would be skipped work).
    let Some(transitions) = context.control_flow.transitions.span(state.transitions) else {
        return false;
    };
    let [transition] = transitions else {
        return false;
    };
    if !matches!(
        transition.target,
        omega_control_flow::PlannedTransitionTarget::Terminal
    ) {
        return false;
    }
    if transition.expressions.guard.is_valid() {
        return false;
    }
    if !transition.expressions.target_value.is_valid() {
        return false;
    }
    matches!(
        context
            .control_flow
            .expressions
            .expression(transition.expressions.target_value),
        ExpressionNode::Call(_)
    )
}
