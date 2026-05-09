use super::model::RuntimeBodyStateCallBlocker;
use super::planned::runtime_branching_call_matches_grouped_blocker;
use omega_native::plan::NativePlan;
use omega_native::runtime_dispatch::branching::RuntimeBranchCallExpansion;
use omega_state_calls::StateCallLowering;

pub(super) fn runtime_body_state_call_expansion_reason(
    native_plan: &NativePlan,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> String {
    match grouped_blocker.lowering {
        StateCallLowering::InlineLeaf => "leaf state-call expansion".to_owned(),
        StateCallLowering::InlineExpansion => "straight-line state-call expansion".to_owned(),
        StateCallLowering::Unresolved => "unresolved state-call expansion".to_owned(),
        StateCallLowering::InlineBranching => {
            runtime_branching_call_expansion_reason(native_plan, grouped_blocker)
        }
    }
}

fn runtime_branching_call_expansion_reason(
    native_plan: &NativePlan,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> String {
    let mut matching_calls = native_plan
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
        RuntimeBranchCallExpansion::NeedsStraightLineTarget => 2,
        RuntimeBranchCallExpansion::NeedsNestedBranchTarget => 3,
        RuntimeBranchCallExpansion::UnknownTarget => 4,
        RuntimeBranchCallExpansion::Unplanned => 5,
    }
}

fn runtime_branch_expansion_reason(expansion: RuntimeBranchCallExpansion) -> &'static str {
    match expansion {
        RuntimeBranchCallExpansion::GuardedLeaf => "guarded leaf branch expansion",
        RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards => {
            "guarded leaf branch expansion with complex guards"
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
