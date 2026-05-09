use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::state_calls::StateCallLowering;
use omega_typed_program::name::ProgramName;

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
                target_machine,
                target_state,
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
                let (source_machine, source_state) = state_names(native_plan, operation.source_key);
                append_leaf_branch_expansions(
                    native_plan,
                    &mut plan,
                    operation.source_key,
                    &source_machine,
                    &source_state,
                    operation.statement_index,
                    body.dispatch_index,
                    target_machine,
                    target_state,
                    &branch_edges,
                    state_call,
                    &aliases,
                );
            }
            if expansion == RuntimeBranchCallExpansion::NeedsStraightLineTarget {
                let (source_machine, source_state) = state_names(native_plan, operation.source_key);
                append_straight_line_branch_expansions(
                    native_plan,
                    &mut plan,
                    operation.source_key,
                    &source_machine,
                    &source_state,
                    operation.statement_index,
                    body.dispatch_index,
                    target_machine,
                    target_state,
                    &branch_edges,
                    state_call,
                    &aliases,
                );
            }
            let edges = plan.edges.insert_many(branch_edges);
            let (source_machine, source_state) = state_names(native_plan, operation.source_key);
            plan.calls.insert(RuntimeBranchingCall {
                dispatch_index: body.dispatch_index,
                source_key: operation.source_key,
                source_machine,
                source_state,
                statement_index: operation.statement_index,
                target_key: state_call.target_key,
                target_machine: target_machine.clone(),
                target_state: target_state.clone(),
                argument_count: *argument_count,
                expansion,
                edges,
            });
            bind_runtime_branch_aliases(native_plan, &mut aliases, state_call);
        }
    }

    plan
}

fn state_names(
    native_plan: &NativePlan,
    key: crate::control_flow::StateKey,
) -> (ProgramName, ProgramName) {
    let machine = native_plan
        .control_flow
        .machine_by_symbol(key.machine)
        .map(|machine| machine.name.clone())
        .unwrap_or_default();
    let state = native_plan
        .control_flow
        .state_by_key(key)
        .map(|state| state.name.clone())
        .unwrap_or_default();
    (machine, state)
}
