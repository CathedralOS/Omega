use super::model::RuntimeBodyStateCallBlocker;
use crate::EmissionPlanningInput;
use omega_runtime_branching::{RuntimeBranchCallExpansion, RuntimeBranchingCall};
use omega_state_calls::StateCallLowering;

pub(super) fn runtime_body_state_call_has_planned_expansion(
    native_plan: &EmissionPlanningInput<'_>,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> bool {
    if grouped_blocker.lowering != StateCallLowering::InlineBranching {
        return false;
    }

    let mut matching_calls = native_plan
        .runtime_branching_calls
        .calls
        .iter()
        .filter_map(|(_, call)| {
            runtime_branching_call_matches_grouped_blocker(call, grouped_blocker).then_some(call)
        })
        .peekable();

    if matching_calls.peek().is_none() {
        return false;
    }

    matching_calls.all(|call| runtime_branching_call_has_planned_expansion(native_plan, call))
}

pub(super) fn runtime_branching_call_matches_grouped_blocker(
    call: &RuntimeBranchingCall,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> bool {
    call.dispatch_index == grouped_blocker.dispatch_index
        && call.source_key == grouped_blocker.source_key
        && call.target_key == grouped_blocker.target_key
        && call.argument_count == grouped_blocker.argument_count
}

fn runtime_branching_call_has_planned_expansion(
    native_plan: &EmissionPlanningInput<'_>,
    call: &RuntimeBranchingCall,
) -> bool {
    match call.expansion {
        RuntimeBranchCallExpansion::GuardedLeaf => {
            runtime_branching_call_leaf_expansion_count(native_plan, call) > 0
        }
        RuntimeBranchCallExpansion::NeedsStraightLineTarget => {
            runtime_branching_call_leaf_expansion_count(native_plan, call) > 0
                && runtime_branching_call_straight_line_expansion_count(native_plan, call) > 0
        }
        RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards
        | RuntimeBranchCallExpansion::NeedsNestedBranchTarget
        | RuntimeBranchCallExpansion::UnknownTarget
        | RuntimeBranchCallExpansion::Unplanned => false,
    }
}

fn runtime_branching_call_leaf_expansion_count(
    native_plan: &EmissionPlanningInput<'_>,
    call: &RuntimeBranchingCall,
) -> usize {
    native_plan
        .runtime_branching_calls
        .leaf_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == call.dispatch_index
                && expansion.source_key == call.source_key
                && expansion.statement_index == call.statement_index
                && expansion.branch_key == call.target_key
        })
        .count()
}

fn runtime_branching_call_straight_line_expansion_count(
    native_plan: &EmissionPlanningInput<'_>,
    call: &RuntimeBranchingCall,
) -> usize {
    native_plan
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == call.dispatch_index
                && expansion.source_key == call.source_key
                && expansion.statement_index == call.statement_index
                && expansion.branch_key == call.target_key
        })
        .count()
}
