use super::model::RuntimeBodyStateCallBlocker;
use super::planned::runtime_branching_call_matches_grouped_blocker;
use crate::EmissionPlanningInput;
use omega_runtime_branching::RuntimeBranchCallExpansion;
use omega_state_calls::StateCallLowering;

pub(super) fn runtime_body_state_call_expansion_reason(
    input: &EmissionPlanningInput<'_>,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> String {
    match grouped_blocker.lowering {
        StateCallLowering::InlineLeaf => "leaf state-call expansion".to_owned(),
        StateCallLowering::InlineExpansion => "straight-line state-call expansion".to_owned(),
        StateCallLowering::IndirectDynamic => "runtime dynamic table-slot call".to_owned(),
        StateCallLowering::Unresolved => "unresolved state-call expansion".to_owned(),
        StateCallLowering::InlineBranching => {
            runtime_branching_call_expansion_reason(input, grouped_blocker)
        }
    }
}

fn runtime_branching_call_expansion_reason(
    input: &EmissionPlanningInput<'_>,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> String {
    let mut matching_calls = input
        .runtime_branching_calls
        .calls
        .iter()
        .filter_map(|(_, call)| {
            runtime_branching_call_matches_grouped_blocker(call, grouped_blocker).then_some(call)
        })
        .peekable();

    if matching_calls.peek().is_none() {
        return "guarded state-call expansion".to_owned();
    }

    let mut expansion = RuntimeBranchCallExpansion::GuardedLeaf;

    for call in matching_calls {
        expansion = strongest_branch_expansion(expansion, call.expansion);
    }

    runtime_branch_expansion_reason(expansion).to_owned()
}

fn strongest_branch_expansion(
    current: RuntimeBranchCallExpansion,
    next: RuntimeBranchCallExpansion,
) -> RuntimeBranchCallExpansion {
    if branch_expansion_rank(next) > branch_expansion_rank(current) {
        next
    } else {
        current
    }
}

fn branch_expansion_rank(expansion: RuntimeBranchCallExpansion) -> u8 {
    match expansion {
        RuntimeBranchCallExpansion::GuardedLeaf => 0,
        RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards => 1,
        RuntimeBranchCallExpansion::NeedsBranchPrelude => 2,
        RuntimeBranchCallExpansion::NeedsStraightLineTarget => 3,
        RuntimeBranchCallExpansion::NeedsNestedBranchTarget => 4,
        RuntimeBranchCallExpansion::UnknownTarget => 5,
        RuntimeBranchCallExpansion::Unplanned => 6,
    }
}

fn runtime_branch_expansion_reason(expansion: RuntimeBranchCallExpansion) -> &'static str {
    match expansion {
        RuntimeBranchCallExpansion::GuardedLeaf => "guarded leaf branch expansion",
        RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards => {
            "guarded leaf branch expansion with complex guards"
        }
        RuntimeBranchCallExpansion::NeedsBranchPrelude => {
            "guarded branch expansion with branch-state prelude"
        }
        RuntimeBranchCallExpansion::NeedsStraightLineTarget => {
            "guarded branch expansion with straight-line target"
        }
        RuntimeBranchCallExpansion::NeedsNestedBranchTarget => "nested guarded branch expansion",
        RuntimeBranchCallExpansion::UnknownTarget => {
            "guarded branch expansion with unknown target lowering"
        }
        RuntimeBranchCallExpansion::Unplanned => "guarded state-call expansion",
    }
}
