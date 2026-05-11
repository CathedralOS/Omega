use super::model::RuntimeBodyStateCallBlocker;
use crate::EmissionPlanningInput;
use omega_runtime_branching::{RuntimeBranchCallExpansion, RuntimeBranchingCall};
use omega_state_calls::StateCallLowering;

pub(super) fn runtime_body_state_call_has_planned_expansion(
    input: &EmissionPlanningInput<'_>,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> bool {
    if grouped_blocker.lowering != StateCallLowering::InlineBranching {
        return false;
    }

    let mut matching_calls = input
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

    matching_calls.all(|call| runtime_branching_call_has_planned_expansion(input, call))
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
    input: &EmissionPlanningInput<'_>,
    call: &RuntimeBranchingCall,
) -> bool {
    let prelude_count = runtime_branching_call_prelude_expansion_count(input, call);
    let leaf_count = runtime_branching_call_leaf_expansion_count(input, call);
    let straight_line_count = runtime_branching_call_straight_line_expansion_count(input, call);
    let Some(edges) = input.runtime_branching_calls.edges.span(call.edges) else {
        return false;
    };
    let needs_leaf = edges.iter().any(|edge| {
        matches!(
            edge.lowering,
            omega_runtime_branching::RuntimeBranchTargetLowering::InlineLeaf
        )
    });
    let needs_straight_line = edges.iter().any(|edge| {
        matches!(
            edge.lowering,
            omega_runtime_branching::RuntimeBranchTargetLowering::InlineStraightLine
                | omega_runtime_branching::RuntimeBranchTargetLowering::InlineBranching
        )
    });

    match call.expansion {
        RuntimeBranchCallExpansion::GuardedLeaf
        | RuntimeBranchCallExpansion::NeedsStraightLineTarget
        | RuntimeBranchCallExpansion::NeedsNestedBranchTarget => {
            (!needs_leaf || leaf_count > 0) && (!needs_straight_line || straight_line_count > 0)
        }
        RuntimeBranchCallExpansion::NeedsBranchPrelude => {
            prelude_count > 0
                && (!needs_leaf || leaf_count > 0)
                && (!needs_straight_line || straight_line_count > 0)
        }
        RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards
        | RuntimeBranchCallExpansion::UnknownTarget
        | RuntimeBranchCallExpansion::Unplanned => false,
    }
}

fn runtime_branching_call_prelude_expansion_count(
    input: &EmissionPlanningInput<'_>,
    call: &RuntimeBranchingCall,
) -> usize {
    input
        .runtime_branching_calls
        .prelude_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == call.dispatch_index
                && expansion.source_key == call.source_key
                && expansion.statement_index == call.statement_index
                && expansion.branch_key == call.target_key
        })
        .count()
}

fn runtime_branching_call_leaf_expansion_count(
    input: &EmissionPlanningInput<'_>,
    call: &RuntimeBranchingCall,
) -> usize {
    input
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
    input: &EmissionPlanningInput<'_>,
    call: &RuntimeBranchingCall,
) -> usize {
    input
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
