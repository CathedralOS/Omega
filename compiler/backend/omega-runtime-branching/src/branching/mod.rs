use crate::RuntimeBranchingContext;
use omega_control_flow::StateKey;
use omega_core::arena::PagedSlice;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_state_calls::StateCallLowering;

mod aliases;
mod classify;
mod edges;
mod expansions;
mod lookups;
mod model;
mod operations;

use aliases::{RuntimeBranchAliasBuffer, bind_runtime_branch_aliases};
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
    let capacity = estimate_runtime_branching_capacity(context);
    let mut plan = RuntimeBranchingCallPlan::with_capacity(
        capacity.calls,
        capacity.edges,
        capacity.arguments,
        capacity.expansions,
        capacity.bindings,
        capacity.operations,
    );

    for (_, body) in context.runtime_bodies.bodies.iter() {
        let Some(operations) = context
            .runtime_bodies
            .operations
            .paged_span(body.operations)
        else {
            continue;
        };
        let mut aliases = RuntimeBranchAliasBuffer::with_capacity(runtime_branch_alias_capacity(
            context,
            &operations,
        ));

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
            let branch_edges_vec = plan.edges.span_or_empty(branch_edges).to_vec();
            let branch_edges_slice = branch_edges_vec.as_slice();
            let mut expansion = classify_branch_call_expansion(branch_edges_slice);
            let has_prelude = branch_target_has_prelude(context, state_call.target_key);
            if has_prelude {
                expansion = RuntimeBranchCallExpansion::NeedsBranchPrelude;
            }
            if has_prelude {
                append_branch_prelude_expansion(
                    context,
                    &mut plan.expressions,
                    &mut plan.target_arguments,
                    &mut plan.target_values,
                    &mut plan.edges,
                    &mut plan.prelude_expansions,
                    &mut plan.prelude_bindings,
                    &mut plan.prelude_operations,
                    &mut plan.leaf_expansions,
                    &mut plan.leaf_bindings,
                    &mut plan.leaf_operations,
                    &mut plan.straight_line_expansions,
                    &mut plan.straight_line_bindings,
                    &mut plan.straight_line_operations,
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
                    omega_checked_trees::expression::ExpressionHandle::invalid(),
                    omega_state_guards::StateGuardKind::Always,
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
                    &mut plan.target_arguments,
                    &mut plan.target_values,
                    &mut plan.edges,
                    &mut plan.leaf_expansions,
                    &mut plan.leaf_bindings,
                    &mut plan.leaf_operations,
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

#[derive(Debug, Clone, Copy, Default)]
struct RuntimeBranchingCapacity {
    calls: usize,
    edges: usize,
    arguments: usize,
    expansions: usize,
    bindings: usize,
    operations: usize,
}

fn estimate_runtime_branching_capacity(
    context: &RuntimeBranchingContext,
) -> RuntimeBranchingCapacity {
    let mut capacity = RuntimeBranchingCapacity::default();

    for (_, body) in context.runtime_bodies.bodies.iter() {
        let Some(operations) = context
            .runtime_bodies
            .operations
            .paged_span(body.operations)
        else {
            continue;
        };

        for operation in operations.iter() {
            if !operation_is_branching_call(operation) {
                continue;
            }

            let RuntimeDispatchBodyOperationKind::StateCall {
                target_key,
                argument_count,
                ..
            } = &operation.kind
            else {
                continue;
            };

            let edge_count = context
                .control_flow
                .state_by_key(*target_key)
                .map(|state| state.transitions.len())
                .unwrap_or(0);

            capacity.calls = capacity.calls.saturating_add(1);
            capacity.edges = capacity.edges.saturating_add(edge_count);
            capacity.arguments = capacity
                .arguments
                .saturating_add(edge_count.saturating_mul(*argument_count));
            capacity.expansions = capacity.expansions.saturating_add(edge_count.max(1));
            capacity.bindings = capacity
                .bindings
                .saturating_add(edge_count.saturating_mul(*argument_count));
            capacity.operations = capacity
                .operations
                .saturating_add(edge_count.saturating_mul(operations.len()));
        }
    }

    capacity
}

fn operation_is_branching_call(operation: &RuntimeDispatchBodyOperation) -> bool {
    matches!(
        operation.kind,
        RuntimeDispatchBodyOperationKind::StateCall {
            lowering: StateCallLowering::InlineBranching,
            ..
        }
    )
}

fn runtime_branch_alias_capacity(
    context: &RuntimeBranchingContext,
    operations: &PagedSlice<'_, RuntimeDispatchBodyOperation>,
) -> usize {
    operations
        .iter()
        .filter_map(|operation| state_call_for_runtime_operation(context, operation))
        .map(|state_call| state_call.arguments.len())
        .sum()
}

fn branch_target_has_prelude(context: &RuntimeBranchingContext, target_key: StateKey) -> bool {
    context
        .control_flow
        .state_by_key(target_key)
        .and_then(|state| context.control_flow.operations.span(state.operations))
        .is_some_and(|operations| !operations.is_empty())
}
