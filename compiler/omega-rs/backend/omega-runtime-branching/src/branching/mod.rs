use crate::RuntimeBranchingContext;
use omega_control_flow::StateKey;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_state_calls::StateCallLowering;
use psi_arena::PagedSlice;

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
    let mut next_branch_scope_id = 1u32;

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
        // Transition-guard subject calls already EVALUATED in this body, per source
        // state: arms of one transition re-test the same subject, so only the first
        // arm appends any execution machinery (see the repeated-subject branch
        // below).
        let mut evaluated_guard_subjects: Vec<(
            StateKey,
            psi_checked_trees::expression::ExpressionHandle,
        )> = Vec::new();

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
            let root_scope_id = next_branch_scope_id;
            next_branch_scope_id += 1;
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
            // A transition subject evaluates ONCE per transition evaluation. The parser
            // lowers `transition self.f(x) { true -> a false -> b }` into one guard
            // call PER ARM, each holding a COPY of the subject call (lowering breaks
            // handle sharing, so compare structurally). The first arm's prelude runs
            // the callee and the runtime-storage plan parks the result in a guard
            // result slot SHARED by every arm of the transition; later arms emit no
            // prelude operations, no nested-callee expansions, and no value-selection
            // leafs -- their guard comparisons re-read the shared slot.
            let mut repeated_guard_subject = false;
            if has_prelude && state_call.role == omega_state_calls::StateCallRole::TransitionGuard {
                let subject = context
                    .control_flow
                    .transition_guard_subject_call(operation.source_key, operation.statement_index);
                if subject.is_valid() {
                    if evaluated_guard_subjects.iter().any(|(seen_key, seen)| {
                        *seen_key == operation.source_key
                            && context
                                .control_flow
                                .expressions
                                .expressions_structurally_equal(*seen, subject)
                    }) {
                        repeated_guard_subject = true;
                    } else {
                        evaluated_guard_subjects.push((operation.source_key, subject));
                    }
                }
            }
            if has_prelude {
                expansion = RuntimeBranchCallExpansion::NeedsBranchPrelude;
            }
            if has_prelude {
                // Who executes the callee's statements depends on the role -- see
                // PreludeStatementFilter. Guard calls: the prelude is the executor
                // (their runtime-bodies splice is skipped so the repeated-subject
                // dedupe above can suppress later arms). Every other role: the
                // splice executes mutations/host calls and flattens nested calls,
                // so the prelude keeps only the LOCAL INITIALIZERS the splice does
                // not cover -- carrying mutations or host calls here as well ran
                // the callee's side effects twice per evaluation.
                let statement_filter = if repeated_guard_subject {
                    operations::PreludeStatementFilter::None
                } else if state_call.role == omega_state_calls::StateCallRole::TransitionGuard {
                    operations::PreludeStatementFilter::All
                } else {
                    operations::PreludeStatementFilter::LocalDataOnly
                };
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
                    &mut next_branch_scope_id,
                    operation.source_key,
                    *target_key,
                    operation.statement_index,
                    body.dispatch_index,
                    state_call,
                    &aliases,
                    statement_filter,
                );
            }
            // A repeated guard subject appends NO leaf or straight-line expansions:
            // its first arm already wrote the shared guard result slot, and a leaf
            // value selection here would re-run the callee's nested calls once per
            // arm (the dungeon's 32-draws-for-1 RNG amplification).
            if !repeated_guard_subject
                && matches!(
                    classify_branch_call_expansion(branch_edges_slice),
                    RuntimeBranchCallExpansion::GuardedLeaf
                        | RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards
                        | RuntimeBranchCallExpansion::NeedsStraightLineTarget
                        | RuntimeBranchCallExpansion::NeedsNestedBranchTarget
                )
            {
                append_leaf_branch_expansions(
                    context,
                    &mut plan.expressions,
                    &plan.target_arguments,
                    &mut plan.leaf_expansions,
                    &mut plan.leaf_bindings,
                    &mut plan.leaf_operations,
                    root_scope_id,
                    operation.source_key,
                    *target_key,
                    operation.statement_index,
                    body.dispatch_index,
                    branch_edges_slice,
                    state_call,
                    &aliases,
                    psi_checked_trees::expression::ExpressionHandle::invalid(),
                    omega_state_guards::StateGuardKind::Always,
                );
            }
            if !repeated_guard_subject
                && matches!(
                    classify_branch_call_expansion(branch_edges_slice),
                    RuntimeBranchCallExpansion::NeedsStraightLineTarget
                        | RuntimeBranchCallExpansion::NeedsNestedBranchTarget
                )
            {
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
                    &mut next_branch_scope_id,
                    root_scope_id,
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
