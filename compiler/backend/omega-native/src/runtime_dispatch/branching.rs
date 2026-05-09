use crate::plan::NativePlan;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use omega_state_calls::StateCallLowering;

mod aliases;
mod classify;
mod edges;
mod expansions;
mod lookups;
mod model;
mod operations;

use aliases::bind_runtime_branch_aliases;
use classify::classify_branch_call_expansion;
use edges::build_branch_edges;
use expansions::{append_leaf_branch_expansions, append_straight_line_branch_expansions};
use lookups::state_call_for_operation;
pub use model::{
    RuntimeBranchCallExpansion, RuntimeBranchTargetLowering, RuntimeBranchingCall,
    RuntimeBranchingCallEdge, RuntimeBranchingCallPlan, RuntimeLeafBranchBinding,
    RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion, RuntimeLeafBranchOperation,
    RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchBindingKind, RuntimeStraightLineBranchExpansion,
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
};

pub fn build_runtime_branching_call_plan(native_plan: &NativePlan) -> RuntimeBranchingCallPlan {
    let mut plan = RuntimeBranchingCallPlan::default();

    for (_, body) in native_plan.runtime_bodies.bodies.iter() {
        let Some(operations) = native_plan
            .runtime_bodies
            .operations
            .paged_span(body.operations)
        else {
            continue;
        };
        let mut aliases = Vec::new();

        for operation in operations.iter() {
            let state_call = state_call_for_operation(
                native_plan,
                operation.source_key,
                operation.statement_index,
            );
            let RuntimeDispatchBodyOperationKind::StateCall {
                target_key,
                argument_count,
                lowering: StateCallLowering::InlineBranching,
                ..
            } = &operation.kind
            else {
                if let Some(state_call) = state_call {
                    bind_runtime_branch_aliases(native_plan, &mut aliases, state_call);
                }
                continue;
            };

            let Some(state_call) = state_call else {
                continue;
            };
            let branch_edges = build_branch_edges(
                native_plan,
                state_call.target_key,
                &mut plan.target_arguments,
            );
            let expansion = classify_branch_call_expansion(&branch_edges);
            if matches!(
                expansion,
                RuntimeBranchCallExpansion::GuardedLeaf
                    | RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards
                    | RuntimeBranchCallExpansion::NeedsStraightLineTarget
            ) {
                append_leaf_branch_expansions(
                    native_plan,
                    &mut plan,
                    operation.source_key,
                    *target_key,
                    operation.statement_index,
                    body.dispatch_index,
                    &branch_edges,
                    state_call,
                    &aliases,
                );
            }
            if expansion == RuntimeBranchCallExpansion::NeedsStraightLineTarget {
                append_straight_line_branch_expansions(
                    native_plan,
                    &mut plan,
                    operation.source_key,
                    *target_key,
                    operation.statement_index,
                    body.dispatch_index,
                    &branch_edges,
                    state_call,
                    &aliases,
                );
            }
            let edges = plan.edges.insert_many(branch_edges);
            plan.calls.insert(RuntimeBranchingCall {
                dispatch_index: body.dispatch_index,
                source_key: operation.source_key,
                statement_index: operation.statement_index,
                target_key: state_call.target_key,
                argument_count: *argument_count,
                expansion,
                edges,
            });
            bind_runtime_branch_aliases(native_plan, &mut aliases, state_call);
        }
    }

    plan
}
