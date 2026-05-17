use crate::RuntimeBranchingContext;
use omega_control_flow::StateKey;
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
use expansions::{
    append_branch_prelude_expansion, append_leaf_branch_expansions,
    append_straight_line_branch_expansions,
};
use lookups::state_call_for_runtime_operation;
pub use model::{
    RuntimeBranchCallExpansion, RuntimeBranchPreludeBinding, RuntimeBranchPreludeExpansion,
    RuntimeBranchPreludeOperation, RuntimeBranchPreludeOperationKind, RuntimeBranchTargetLowering,
    RuntimeBranchingCall, RuntimeBranchingCallEdge, RuntimeBranchingCallPlan,
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion,
    RuntimeLeafBranchOperation, RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchBinding,
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
            let state_call = state_call_for_runtime_operation(context, operation);
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
                &mut plan.target_values,
                &mut plan.edges,
            );
            let branch_edges_slice = plan.edges.span_or_empty(branch_edges);
            let mut expansion = classify_branch_call_expansion(branch_edges_slice);
            let has_prelude = branch_target_has_prelude(context, state_call.target_key);
            if has_prelude {
                expansion = RuntimeBranchCallExpansion::NeedsBranchPrelude;
            }
            if has_prelude {
                append_branch_prelude_expansion(
                    context,
                    &mut plan.expressions,
                    &mut plan.prelude_expansions,
                    &mut plan.prelude_bindings,
                    &mut plan.prelude_operations,
                    operation.source_key,
                    *target_key,
                    operation.statement_index,
                    body.dispatch_index,
                    state_call,
                    &aliases,
                );
            }
            if matches!(
                classify_branch_call_expansion(branch_edges_slice),
                RuntimeBranchCallExpansion::GuardedLeaf
                    | RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards
                    | RuntimeBranchCallExpansion::NeedsStraightLineTarget
                    | RuntimeBranchCallExpansion::NeedsNestedBranchTarget
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
            if matches!(
                classify_branch_call_expansion(branch_edges_slice),
                RuntimeBranchCallExpansion::NeedsStraightLineTarget
                    | RuntimeBranchCallExpansion::NeedsNestedBranchTarget
            ) {
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

fn branch_target_has_prelude(context: &RuntimeBranchingContext, target_key: StateKey) -> bool {
    context
        .control_flow
        .state_by_key(target_key)
        .and_then(|state| context.control_flow.operations.span(state.operations))
        .is_some_and(|operations| !operations.is_empty())
}
