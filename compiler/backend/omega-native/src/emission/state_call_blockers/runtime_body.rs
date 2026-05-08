use crate::control_flow::StateKey;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::runtime_dispatch::branching::{RuntimeBranchCallExpansion, RuntimeBranchingCall};
use crate::state_calls::StateCallLowering;
use omega_core::arena::Arena;

use super::super::{EmissionBlocker, blocker};

pub(super) fn collect_runtime_body_state_call_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    let mut grouped_blockers = Vec::<RuntimeBodyStateCallBlocker>::new();

    for (_, body) in native_plan.runtime_bodies.bodies.iter() {
        let Some(operations) = native_plan
            .runtime_bodies
            .operations
            .paged_span(body.operations)
        else {
            blockers.insert(blocker(
                "runtime bodies",
                &format!(
                    "#{} {}.{} has an invalid runtime body operation span",
                    body.dispatch_index, body.machine, body.state
                ),
            ));
            continue;
        };

        for operation in operations.iter() {
            let RuntimeDispatchBodyOperationKind::StateCall {
                target_key,
                target_machine,
                target_state,
                argument_count,
                lowering,
            } = &operation.kind
            else {
                continue;
            };

            push_runtime_body_state_call_blocker(
                &mut grouped_blockers,
                RuntimeBodyStateCallBlocker {
                    dispatch_index: body.dispatch_index,
                    source_key: operation.source_key,
                    source_machine: operation.source_machine.to_string(),
                    source_state: operation.source_state.to_string(),
                    first_statement_index: operation.statement_index,
                    target_key: *target_key,
                    target_machine: target_machine.to_string(),
                    target_state: target_state.to_string(),
                    argument_count: *argument_count,
                    lowering: *lowering,
                    count: 1,
                },
            );
        }
    }

    for grouped_blocker in grouped_blockers {
        if runtime_body_state_call_has_planned_expansion(native_plan, &grouped_blocker) {
            continue;
        }

        let expansion_reason =
            runtime_body_state_call_expansion_reason(native_plan, &grouped_blocker);
        blockers.insert(blocker(
            "state calls",
            &format!(
                "#{} {}.{} statement {} calls {}.{} with {} argument(s){}; runtime dispatch body needs {expansion_reason}",
                grouped_blocker.dispatch_index,
                grouped_blocker.source_machine,
                grouped_blocker.source_state,
                grouped_blocker.first_statement_index,
                grouped_blocker.target_machine,
                grouped_blocker.target_state,
                grouped_blocker.argument_count,
                repeated_count_suffix(grouped_blocker.count),
            ),
        ));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeBodyStateCallBlocker {
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: String,
    source_state: String,
    first_statement_index: usize,
    target_key: StateKey,
    target_machine: String,
    target_state: String,
    argument_count: usize,
    lowering: StateCallLowering,
    count: usize,
}

fn push_runtime_body_state_call_blocker(
    grouped_blockers: &mut Vec<RuntimeBodyStateCallBlocker>,
    blocker: RuntimeBodyStateCallBlocker,
) {
    if let Some(existing) = grouped_blockers.iter_mut().find(|existing| {
        existing.dispatch_index == blocker.dispatch_index
            && existing.source_key == blocker.source_key
            && existing.target_key == blocker.target_key
            && existing.argument_count == blocker.argument_count
            && existing.lowering == blocker.lowering
    }) {
        existing.count += 1;
        return;
    }

    grouped_blockers.push(blocker);
}

fn runtime_body_state_call_has_planned_expansion(
    native_plan: &NativePlan,
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

fn runtime_branching_call_has_planned_expansion(
    native_plan: &NativePlan,
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
    native_plan: &NativePlan,
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
                && expansion.branch_machine == call.target_machine
                && expansion.branch_state == call.target_state
        })
        .count()
}

fn runtime_branching_call_straight_line_expansion_count(
    native_plan: &NativePlan,
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
                && expansion.branch_machine == call.target_machine
                && expansion.branch_state == call.target_state
        })
        .count()
}

fn repeated_count_suffix(count: usize) -> String {
    if count <= 1 {
        String::new()
    } else {
        format!(" ({count} sites)")
    }
}

fn runtime_body_state_call_expansion_reason(
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

fn runtime_branching_call_matches_grouped_blocker(
    call: &RuntimeBranchingCall,
    grouped_blocker: &RuntimeBodyStateCallBlocker,
) -> bool {
    call.dispatch_index == grouped_blocker.dispatch_index
        && call.source_key == grouped_blocker.source_key
        && call.target_key == grouped_blocker.target_key
        && call.argument_count == grouped_blocker.argument_count
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
