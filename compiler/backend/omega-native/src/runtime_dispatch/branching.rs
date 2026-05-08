use crate::control_flow::{MachineFlow, OperationKind, PlannedTransitionTarget, StateKey};
use crate::host_calls::HostCall;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::runtime_dispatch::guards::{StateGuardKind, classify_transition_guard};
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::state_calls::{StateCall, StateCallLowering};
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

mod aliases;
mod model;

use aliases::{
    RuntimeBranchAlias, bind_runtime_branch_aliases, branch_parameter_bindings,
    resolve_branch_expression, resolve_branch_guard,
};
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
                append_leaf_branch_expansions(
                    native_plan,
                    &mut plan,
                    operation.source_key,
                    &operation.source_machine,
                    &operation.source_state,
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
                append_straight_line_branch_expansions(
                    native_plan,
                    &mut plan,
                    operation.source_key,
                    &operation.source_machine,
                    &operation.source_state,
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
            plan.calls.insert(RuntimeBranchingCall {
                dispatch_index: body.dispatch_index,
                source_key: operation.source_key,
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
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

#[allow(clippy::too_many_arguments)]
fn append_leaf_branch_expansions(
    native_plan: &NativePlan,
    plan: &mut RuntimeBranchingCallPlan,
    source_key: StateKey,
    source_machine: &ProgramName,
    source_state: &ProgramName,
    statement_index: usize,
    dispatch_index: u32,
    branch_machine: &ProgramName,
    branch_state: &ProgramName,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    aliases: &[RuntimeBranchAlias],
) {
    for edge in edges {
        if edge.lowering != RuntimeBranchTargetLowering::InlineLeaf {
            continue;
        }

        let RuntimeTransitionTarget::State {
            key: leaf_key,
            machine: leaf_machine,
            state: leaf_state,
        } = &edge.target
        else {
            continue;
        };

        let branch_bindings = branch_parameter_bindings(native_plan, state_call, aliases);
        let bindings = plan.leaf_bindings.insert_many(leaf_branch_bindings(
            &branch_bindings,
            native_plan,
            *leaf_key,
            plan.target_arguments.span_or_empty(edge.target_arguments),
        ));
        let operations = plan
            .leaf_operations
            .insert_many(leaf_operations(native_plan, *leaf_key));

        plan.leaf_expansions.insert(RuntimeLeafBranchExpansion {
            dispatch_index,
            source_key,
            source_machine: source_machine.clone(),
            source_state: source_state.clone(),
            statement_index,
            branch_machine: branch_machine.clone(),
            branch_state: branch_state.clone(),
            edge_order: edge.order,
            guard: edge.guard.clone(),
            resolved_guard: resolve_branch_guard(&edge.guard, &branch_bindings),
            guard_kind: edge.guard_kind,
            leaf_machine: leaf_machine.clone(),
            leaf_state: leaf_state.clone(),
            bindings,
            operations,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn append_straight_line_branch_expansions(
    native_plan: &NativePlan,
    plan: &mut RuntimeBranchingCallPlan,
    source_key: StateKey,
    source_machine: &ProgramName,
    source_state: &ProgramName,
    statement_index: usize,
    dispatch_index: u32,
    branch_machine: &ProgramName,
    branch_state: &ProgramName,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    aliases: &[RuntimeBranchAlias],
) {
    for edge in edges {
        if edge.lowering != RuntimeBranchTargetLowering::InlineStraightLine {
            continue;
        }

        let RuntimeTransitionTarget::State {
            key: target_key,
            machine: target_machine,
            state: target_state,
        } = &edge.target
        else {
            continue;
        };

        let branch_bindings = branch_parameter_bindings(native_plan, state_call, aliases);
        let bindings = plan
            .straight_line_bindings
            .insert_many(straight_line_branch_bindings(
                &branch_bindings,
                native_plan,
                *target_key,
                plan.target_arguments.span_or_empty(edge.target_arguments),
            ));
        let operations = plan
            .straight_line_operations
            .insert_many(straight_line_operations(native_plan, *target_key));

        plan.straight_line_expansions
            .insert(RuntimeStraightLineBranchExpansion {
                dispatch_index,
                source_key,
                source_machine: source_machine.clone(),
                source_state: source_state.clone(),
                statement_index,
                branch_machine: branch_machine.clone(),
                branch_state: branch_state.clone(),
                edge_order: edge.order,
                guard: edge.guard.clone(),
                resolved_guard: resolve_branch_guard(&edge.guard, &branch_bindings),
                guard_kind: edge.guard_kind,
                target_machine: target_machine.clone(),
                target_state: target_state.clone(),
                bindings,
                operations,
            });
    }
}

fn leaf_branch_bindings<'a>(
    branch_bindings: &'a [(ProgramName, Expression)],
    native_plan: &NativePlan,
    leaf_key: StateKey,
    leaf_arguments: &'a [Expression],
) -> impl Iterator<Item = RuntimeLeafBranchBinding> + 'a {
    let branch_parameter_bindings =
        branch_bindings
            .iter()
            .map(|(parameter_name, expression)| RuntimeLeafBranchBinding {
                parameter_name: parameter_name.clone(),
                expression: expression.clone(),
                kind: RuntimeLeafBranchBindingKind::BranchParameter,
            });

    let leaf_parameters = state_parameters(native_plan, leaf_key);
    let leaf_parameter_bindings = leaf_parameters.into_iter().enumerate().filter_map(
        move |(parameter_index, parameter_name)| {
            let expression = leaf_arguments.get(parameter_index)?;
            Some(RuntimeLeafBranchBinding {
                parameter_name,
                expression: resolve_branch_expression(expression, &branch_bindings),
                kind: RuntimeLeafBranchBindingKind::LeafParameter,
            })
        },
    );

    branch_parameter_bindings.chain(leaf_parameter_bindings)
}

fn straight_line_branch_bindings<'a>(
    branch_bindings: &'a [(ProgramName, Expression)],
    native_plan: &NativePlan,
    target_key: StateKey,
    target_arguments: &'a [Expression],
) -> impl Iterator<Item = RuntimeStraightLineBranchBinding> + 'a {
    let branch_parameter_bindings = branch_bindings.iter().map(|(parameter_name, expression)| {
        RuntimeStraightLineBranchBinding {
            parameter_name: parameter_name.clone(),
            expression: expression.clone(),
            kind: RuntimeStraightLineBranchBindingKind::BranchParameter,
        }
    });

    let target_parameters = state_parameters(native_plan, target_key);
    let target_parameter_bindings = target_parameters.into_iter().enumerate().filter_map(
        move |(parameter_index, parameter_name)| {
            let expression = target_arguments.get(parameter_index)?;
            Some(RuntimeStraightLineBranchBinding {
                parameter_name,
                expression: resolve_branch_expression(expression, branch_bindings),
                kind: RuntimeStraightLineBranchBindingKind::TargetParameter,
            })
        },
    );

    branch_parameter_bindings.chain(target_parameter_bindings)
}

fn classify_branch_call_expansion(
    edges: &[RuntimeBranchingCallEdge],
) -> RuntimeBranchCallExpansion {
    if edges.is_empty() {
        return RuntimeBranchCallExpansion::Unplanned;
    }

    let mut has_unknown_target = false;
    let mut has_straight_line_target = false;
    let mut has_nested_branching_target = false;
    let mut has_complex_guard = false;

    for edge in edges {
        match edge.lowering {
            RuntimeBranchTargetLowering::Terminal | RuntimeBranchTargetLowering::InlineLeaf => {}
            RuntimeBranchTargetLowering::InlineStraightLine => has_straight_line_target = true,
            RuntimeBranchTargetLowering::InlineBranching => has_nested_branching_target = true,
            RuntimeBranchTargetLowering::Unknown => has_unknown_target = true,
        }

        if !matches!(
            edge.guard_kind,
            StateGuardKind::Always
                | StateGuardKind::RuntimeEquality
                | StateGuardKind::RuntimeInequality
        ) {
            has_complex_guard = true;
        }
    }

    if has_unknown_target {
        return RuntimeBranchCallExpansion::UnknownTarget;
    }

    if has_nested_branching_target {
        return RuntimeBranchCallExpansion::NeedsNestedBranchTarget;
    }

    if has_straight_line_target {
        return RuntimeBranchCallExpansion::NeedsStraightLineTarget;
    }

    if has_complex_guard {
        return RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards;
    }

    RuntimeBranchCallExpansion::GuardedLeaf
}

fn build_branch_edges(
    native_plan: &NativePlan,
    state_key: StateKey,
    target_arguments: &mut Arena<Expression>,
) -> Vec<RuntimeBranchingCallEdge> {
    let Some(machine) = native_plan
        .control_flow
        .machine_by_symbol(state_key.machine)
    else {
        return Vec::new();
    };
    let Some(state) = native_plan.control_flow.state_by_key(state_key) else {
        return Vec::new();
    };
    let Some(transitions) = native_plan.control_flow.transitions.span(state.transitions) else {
        return Vec::new();
    };

    transitions
        .iter()
        .enumerate()
        .map(|(order, transition)| {
            let target = runtime_transition_target(machine, &state.name, &transition.target);
            RuntimeBranchingCallEdge {
                order,
                lowering: branch_target_lowering(native_plan, &target),
                target,
                continuation: transition
                    .continuation
                    .as_ref()
                    .map(|continuation| {
                        runtime_transition_target(machine, &state.name, continuation)
                    })
                    .unwrap_or(RuntimeTransitionTarget::None),
                target_arguments: transition_target_arguments(&transition.target, target_arguments),
                guard_kind: classify_transition_guard(&transition.guard),
                guard: transition.guard.clone(),
            }
        })
        .collect()
}

fn transition_target_arguments(
    target: &PlannedTransitionTarget,
    arena: &mut Arena<Expression>,
) -> HandleSpan<Expression> {
    match target {
        PlannedTransitionTarget::State { arguments, .. }
        | PlannedTransitionTarget::Nested { arguments, .. } => arena.insert_many(arguments.clone()),
        PlannedTransitionTarget::SelfTarget | PlannedTransitionTarget::Terminal => {
            HandleSpan::empty()
        }
    }
}

fn branch_target_lowering(
    native_plan: &NativePlan,
    target: &RuntimeTransitionTarget,
) -> RuntimeBranchTargetLowering {
    let RuntimeTransitionTarget::State { key, .. } = target else {
        return match target {
            RuntimeTransitionTarget::Terminal | RuntimeTransitionTarget::None => {
                RuntimeBranchTargetLowering::Terminal
            }
            RuntimeTransitionTarget::Unknown { .. } => RuntimeBranchTargetLowering::Unknown,
            RuntimeTransitionTarget::State { .. } => unreachable!(),
        };
    };

    let Some(target_state) = native_plan.control_flow.state_by_key(*key) else {
        return RuntimeBranchTargetLowering::Unknown;
    };

    if native_plan
        .control_flow
        .transitions
        .span(target_state.transitions)
        .is_some_and(|transitions| !transitions.is_empty())
    {
        return RuntimeBranchTargetLowering::InlineBranching;
    }

    let has_state_call = native_plan
        .control_flow
        .operations
        .span(target_state.operations)
        .is_some_and(|operations| {
            operations.iter().any(|operation| {
                matches!(operation.kind, OperationKind::Call { .. })
                    && !state_statement_has_host_call(native_plan, *key, operation.statement_index)
            })
        });

    if has_state_call {
        RuntimeBranchTargetLowering::InlineStraightLine
    } else {
        RuntimeBranchTargetLowering::InlineLeaf
    }
}

fn leaf_operations(
    native_plan: &NativePlan,
    source_key: StateKey,
) -> Vec<RuntimeLeafBranchOperation> {
    let Some(machine) = native_plan
        .control_flow
        .machine_by_symbol(source_key.machine)
    else {
        return Vec::new();
    };
    let Some(state) = native_plan.control_flow.state_by_key(source_key) else {
        return Vec::new();
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return Vec::new();
    };

    operations
        .iter()
        .map(|operation| RuntimeLeafBranchOperation {
            source_key,
            source_machine: machine.name.clone(),
            source_state: state.name.clone(),
            statement_index: operation.statement_index,
            kind: leaf_operation_kind(native_plan, source_key, operation.statement_index),
        })
        .collect()
}

fn leaf_operation_kind(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> RuntimeLeafBranchOperationKind {
    if let Some(host_call) = host_call_for_statement(native_plan, source_key, statement_index) {
        return RuntimeLeafBranchOperationKind::HostCall {
            platform_call: host_call.platform_call.clone(),
        };
    }

    if let Some(mutation) = mutation_for_statement(native_plan, source_key, statement_index) {
        return RuntimeLeafBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: mutation.target.clone(),
            value: mutation.value.clone(),
        };
    }

    RuntimeLeafBranchOperationKind::Other
}

fn straight_line_operations(
    native_plan: &NativePlan,
    source_key: StateKey,
) -> Vec<RuntimeStraightLineBranchOperation> {
    let Some(machine) = native_plan
        .control_flow
        .machine_by_symbol(source_key.machine)
    else {
        return Vec::new();
    };
    let Some(state) = native_plan.control_flow.state_by_key(source_key) else {
        return Vec::new();
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return Vec::new();
    };

    operations
        .iter()
        .map(|operation| RuntimeStraightLineBranchOperation {
            source_key,
            source_machine: machine.name.clone(),
            source_state: state.name.clone(),
            statement_index: operation.statement_index,
            kind: straight_line_operation_kind(
                native_plan,
                source_key,
                operation.statement_index,
                &operation.kind,
            ),
        })
        .collect()
}

fn straight_line_operation_kind(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
    operation_kind: &OperationKind,
) -> RuntimeStraightLineBranchOperationKind {
    if let Some(host_call) = host_call_for_statement(native_plan, source_key, statement_index) {
        return RuntimeStraightLineBranchOperationKind::HostCall {
            platform_call: host_call.platform_call.clone(),
        };
    }

    if let Some(mutation) = mutation_for_statement(native_plan, source_key, statement_index) {
        return RuntimeStraightLineBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: mutation.target.clone(),
            value: mutation.value.clone(),
        };
    }

    if let Some(state_call) = state_call_for_operation(native_plan, source_key, statement_index) {
        return RuntimeStraightLineBranchOperationKind::StateCall {
            target_key: state_call.target_key,
            target_machine: state_call.target_machine.clone(),
            target_state: state_call.target_state.clone(),
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        };
    }

    if matches!(operation_kind, OperationKind::LocalData) {
        return RuntimeStraightLineBranchOperationKind::LocalData;
    }

    RuntimeStraightLineBranchOperationKind::Other
}

fn host_call_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    native_plan
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.source_key == source_key && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

fn mutation_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan crate::state_storage::StateMutation> {
    native_plan
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.source_key == source_key && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}

fn state_call_for_operation<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    native_plan
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| {
            state_call.source_key == source_key && state_call.statement_index == statement_index
        })
        .map(|(_, state_call)| state_call)
}

fn state_parameters(native_plan: &NativePlan, state_key: StateKey) -> Vec<ProgramName> {
    native_plan
        .control_flow
        .state_by_key(state_key)
        .map(|state| state.parameters.to_vec())
        .unwrap_or_default()
}

fn state_statement_has_host_call(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.source_key == source_key && host_call.statement_index == statement_index
    })
}

fn runtime_transition_target(
    machine: &MachineFlow,
    current_state: &str,
    target: &PlannedTransitionTarget,
) -> RuntimeTransitionTarget {
    match target {
        PlannedTransitionTarget::State { key, name, .. } => RuntimeTransitionTarget::State {
            key: *key,
            machine: machine.name.clone(),
            state: name.clone(),
        },
        PlannedTransitionTarget::Nested {
            receiver, state, ..
        } => machine
            .contains
            .iter()
            .find(|contained| contained.name == *receiver)
            .map(|contained| RuntimeTransitionTarget::State {
                key: Default::default(),
                machine: contained.type_name.clone(),
                state: state.clone(),
            })
            .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                name: format!("{receiver}.{state}"),
            }),
        PlannedTransitionTarget::SelfTarget => RuntimeTransitionTarget::State {
            key: Default::default(),
            machine: machine.name.clone(),
            state: current_state.to_owned().into(),
        },
        PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
    }
}
