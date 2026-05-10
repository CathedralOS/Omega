use crate::RuntimeBranchingContext;
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

pub fn build_runtime_branching_call_plan(
    context: &RuntimeBranchingContext,
) -> RuntimeBranchingCallPlan {
    let mut plan = RuntimeBranchingCallPlan::default();

    for (_, body) in context.runtime_bodies.bodies.iter() {
        let Some(operations) = context
            .runtime_bodies
            .operations
            .paged_span(body.operations)
        else {
            continue;
        };
        let mut aliases = Vec::new();

        for operation in operations.iter() {
            let state_call =
                state_call_for_operation(context, operation.source_key, operation.statement_index);
            let RuntimeDispatchBodyOperationKind::StateCall {
                target_key,
                argument_count,
                lowering: StateCallLowering::InlineBranching,
                ..
            } = &operation.kind
            else {
                if let Some(state_call) = state_call {
                    bind_runtime_branch_aliases(
                        context,
                        &mut plan.expressions,
                        &mut aliases,
                        state_call,
                    );
                }
                continue;
            };

            let Some(state_call) = state_call else {
                continue;
            };
            let branch_edges = build_branch_edges(
                context,
                state_call.target_key,
                &mut plan.expressions,
                &mut plan.target_arguments,
                &mut plan.edges,
            );
            let branch_edges_slice = plan.edges.span_or_empty(branch_edges);
            let expansion = classify_branch_call_expansion(branch_edges_slice);
            if matches!(
                expansion,
                RuntimeBranchCallExpansion::GuardedLeaf
                    | RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards
                    | RuntimeBranchCallExpansion::NeedsStraightLineTarget
            ) {
                append_leaf_branch_expansions(
                    context,
                    &mut plan.expressions,
                    &plan.target_arguments,
                    &mut plan.leaf_expansions,
                    &mut plan.leaf_bindings,
                    &mut plan.leaf_operations,
                    operation.source_key,
                    *target_key,
                    operation.statement_index,
                    body.dispatch_index,
                    branch_edges_slice,
                    state_call,
                    &aliases,
                );
            }
            if expansion == RuntimeBranchCallExpansion::NeedsStraightLineTarget {
                append_straight_line_branch_expansions(
                    context,
                    &mut plan.expressions,
                    &plan.target_arguments,
                    &mut plan.straight_line_expansions,
                    &mut plan.straight_line_bindings,
                    &mut plan.straight_line_operations,
                    operation.source_key,
                    *target_key,
                    operation.statement_index,
                    body.dispatch_index,
                    branch_edges_slice,
                    state_call,
                    &aliases,
                );
            }
            plan.calls.insert(RuntimeBranchingCall {
                dispatch_index: body.dispatch_index,
                source_key: operation.source_key,
                statement_index: operation.statement_index,
                target_key: state_call.target_key,
                argument_count: *argument_count,
                expansion,
                edges: branch_edges,
            });
            bind_runtime_branch_aliases(context, &mut plan.expressions, &mut aliases, state_call);
        }
    }

    plan
}
