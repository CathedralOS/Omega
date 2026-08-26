use crate::StateCallPlanningContext;
use omega_control_flow::{OperationKind, StateKey};

use super::StateCallLowering;
use super::collection::CollectedStateCall;
use super::lookups::{state_flow_from_key, state_key_is_valid};

pub(crate) fn state_call_lowering(
    context: &StateCallPlanningContext,
    call: &CollectedStateCall,
    calling_states: &[StateKey],
) -> StateCallLowering {
    if call.dynamic_dispatch.is_some() && state_key_is_valid(call.target_key) {
        StateCallLowering::IndirectDynamic
    } else if !state_key_is_valid(call.target_key) {
        StateCallLowering::Unresolved
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
