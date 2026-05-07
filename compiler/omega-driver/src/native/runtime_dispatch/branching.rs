use crate::ir::expression::Expression;
use crate::ir::statement::TransitionGuard;
use crate::native::control_flow::{MachineFlow, OperationKind, PlannedTransitionTarget};
use crate::native::host_calls::HostCall;
use crate::native::plan::NativePlan;
use crate::native::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::native::runtime_dispatch::guards::{StateGuardKind, classify_transition_guard};
use crate::native::runtime_flow::RuntimeTransitionTarget;
use crate::native::state_calls::{StateCall, StateCallArgumentKind, StateCallLowering};
use crate::native::state_storage::{StateMutationKind, StateMutationLowering};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeBranchingCallPlan {
    pub calls: Arena<RuntimeBranchingCall>,
    pub edges: Arena<RuntimeBranchingCallEdge>,
    pub leaf_expansions: Arena<RuntimeLeafBranchExpansion>,
    pub leaf_operations: Arena<RuntimeLeafBranchOperation>,
    pub leaf_bindings: Arena<RuntimeLeafBranchBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchingCall {
    pub dispatch_index: u32,
    pub source_machine: String,
    pub source_state: String,
    pub statement_index: usize,
    pub target_machine: String,
    pub target_state: String,
    pub argument_count: usize,
    pub expansion: RuntimeBranchCallExpansion,
    pub edges: HandleSpan<RuntimeBranchingCallEdge>,
}

impl Default for RuntimeBranchingCall {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_machine: String::new(),
            source_state: String::new(),
            statement_index: 0,
            target_machine: String::new(),
            target_state: String::new(),
            argument_count: 0,
            expansion: RuntimeBranchCallExpansion::Unplanned,
            edges: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchingCallEdge {
    pub order: usize,
    pub target: RuntimeTransitionTarget,
    pub continuation: RuntimeTransitionTarget,
    pub guard: TransitionGuard,
    pub target_arguments: Vec<Expression>,
    pub guard_kind: StateGuardKind,
    pub lowering: RuntimeBranchTargetLowering,
}

impl Default for RuntimeBranchingCallEdge {
    fn default() -> Self {
        Self {
            order: 0,
            target: RuntimeTransitionTarget::None,
            continuation: RuntimeTransitionTarget::None,
            guard: TransitionGuard::Always,
            target_arguments: Vec::new(),
            guard_kind: StateGuardKind::Always,
            lowering: RuntimeBranchTargetLowering::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeBranchTargetLowering {
    Terminal,
    InlineLeaf,
    InlineStraightLine,
    InlineBranching,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeBranchCallExpansion {
    GuardedLeaf,
    GuardedLeafWithComplexGuards,
    NeedsStraightLineTarget,
    NeedsNestedBranchTarget,
    UnknownTarget,
    #[default]
    Unplanned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchExpansion {
    pub dispatch_index: u32,
    pub source_machine: String,
    pub source_state: String,
    pub statement_index: usize,
    pub branch_machine: String,
    pub branch_state: String,
    pub edge_order: usize,
    pub guard: TransitionGuard,
    pub resolved_guard: TransitionGuard,
    pub guard_kind: StateGuardKind,
    pub leaf_machine: String,
    pub leaf_state: String,
    pub bindings: HandleSpan<RuntimeLeafBranchBinding>,
    pub operations: HandleSpan<RuntimeLeafBranchOperation>,
}

impl Default for RuntimeLeafBranchExpansion {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_machine: String::new(),
            source_state: String::new(),
            statement_index: 0,
            branch_machine: String::new(),
            branch_state: String::new(),
            edge_order: 0,
            guard: TransitionGuard::Always,
            resolved_guard: TransitionGuard::Always,
            guard_kind: StateGuardKind::Always,
            leaf_machine: String::new(),
            leaf_state: String::new(),
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchBinding {
    pub parameter_name: String,
    pub expression: Expression,
    pub kind: RuntimeLeafBranchBindingKind,
}

impl Default for RuntimeLeafBranchBinding {
    fn default() -> Self {
        Self {
            parameter_name: String::new(),
            expression: Expression::Integer(0),
            kind: RuntimeLeafBranchBindingKind::BranchParameter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeLeafBranchBindingKind {
    #[default]
    BranchParameter,
    LeafParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchOperation {
    pub source_machine: String,
    pub source_state: String,
    pub statement_index: usize,
    pub kind: RuntimeLeafBranchOperationKind,
}

impl Default for RuntimeLeafBranchOperation {
    fn default() -> Self {
        Self {
            source_machine: String::new(),
            source_state: String::new(),
            statement_index: 0,
            kind: RuntimeLeafBranchOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeLeafBranchOperationKind {
    HostCall {
        platform_call: String,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
        target: Expression,
        value: Expression,
    },
    #[default]
    Other,
}

pub fn build_runtime_branching_call_plan(native_plan: &NativePlan) -> RuntimeBranchingCallPlan {
    let mut plan = RuntimeBranchingCallPlan::default();

    for (_, body) in native_plan.runtime_bodies.bodies.iter() {
        let Some(operations) = native_plan.runtime_bodies.operations.span(body.operations) else {
            continue;
        };

        for operation in operations {
            let RuntimeDispatchBodyOperationKind::StateCall {
                target_machine,
                target_state,
                argument_count,
                lowering: StateCallLowering::InlineBranching,
            } = &operation.kind
            else {
                continue;
            };

            let Some(state_call) = state_call_for_operation(
                native_plan,
                &operation.source_machine,
                &operation.source_state,
                operation.statement_index,
            ) else {
                continue;
            };
            let branch_edges = build_branch_edges(native_plan, target_machine, target_state);
            let expansion = classify_branch_call_expansion(&branch_edges);
            if matches!(
                expansion,
                RuntimeBranchCallExpansion::GuardedLeaf
                    | RuntimeBranchCallExpansion::GuardedLeafWithComplexGuards
            ) {
                append_leaf_branch_expansions(
                    native_plan,
                    &mut plan,
                    &operation.source_machine,
                    &operation.source_state,
                    operation.statement_index,
                    body.dispatch_index,
                    target_machine,
                    target_state,
                    &branch_edges,
                    state_call,
                );
            }
            let edges = plan.edges.insert_many(branch_edges);
            plan.calls.insert(RuntimeBranchingCall {
                dispatch_index: body.dispatch_index,
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
                statement_index: operation.statement_index,
                target_machine: target_machine.clone(),
                target_state: target_state.clone(),
                argument_count: *argument_count,
                expansion,
                edges,
            });
        }
    }

    plan
}

#[allow(clippy::too_many_arguments)]
fn append_leaf_branch_expansions(
    native_plan: &NativePlan,
    plan: &mut RuntimeBranchingCallPlan,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    dispatch_index: u32,
    branch_machine: &str,
    branch_state: &str,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
) {
    for edge in edges {
        if edge.lowering != RuntimeBranchTargetLowering::InlineLeaf {
            continue;
        }

        let RuntimeTransitionTarget::State {
            machine: leaf_machine,
            state: leaf_state,
        } = &edge.target
        else {
            continue;
        };

        let branch_bindings = branch_parameter_bindings(native_plan, state_call);
        let bindings = plan.leaf_bindings.insert_many(leaf_branch_bindings(
            &branch_bindings,
            native_plan,
            branch_machine,
            branch_state,
            leaf_machine,
            leaf_state,
            &edge.target_arguments,
        ));
        let operations = plan.leaf_operations.insert_many(leaf_operations(
            native_plan,
            leaf_machine,
            leaf_state,
        ));

        plan.leaf_expansions.insert(RuntimeLeafBranchExpansion {
            dispatch_index,
            source_machine: source_machine.to_owned(),
            source_state: source_state.to_owned(),
            statement_index,
            branch_machine: branch_machine.to_owned(),
            branch_state: branch_state.to_owned(),
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

fn leaf_branch_bindings(
    branch_bindings: &[(String, Expression)],
    native_plan: &NativePlan,
    branch_machine: &str,
    branch_state: &str,
    leaf_machine: &str,
    leaf_state: &str,
    leaf_arguments: &[Expression],
) -> Vec<RuntimeLeafBranchBinding> {
    let mut bindings = branch_bindings
        .iter()
        .map(|(parameter_name, expression)| RuntimeLeafBranchBinding {
            parameter_name: parameter_name.clone(),
            expression: expression.clone(),
            kind: RuntimeLeafBranchBindingKind::BranchParameter,
        })
        .collect::<Vec<_>>();

    let leaf_parameters = state_parameters(native_plan, leaf_machine, leaf_state);
    bindings.extend(leaf_parameters.iter().enumerate().filter_map(
        |(parameter_index, parameter_name)| {
            let expression = leaf_arguments.get(parameter_index)?;
            Some(RuntimeLeafBranchBinding {
                parameter_name: parameter_name.clone(),
                expression: resolve_branch_expression(expression, &branch_bindings),
                kind: RuntimeLeafBranchBindingKind::LeafParameter,
            })
        },
    ));

    if state_parameters(native_plan, branch_machine, branch_state).is_empty() {
        return bindings;
    }

    bindings
}

fn resolve_branch_guard(
    guard: &TransitionGuard,
    branch_bindings: &[(String, Expression)],
) -> TransitionGuard {
    match guard {
        TransitionGuard::Always => TransitionGuard::Always,
        TransitionGuard::When(expression) => {
            TransitionGuard::When(resolve_branch_expression(expression, branch_bindings))
        }
    }
}

fn branch_parameter_bindings(
    native_plan: &NativePlan,
    state_call: &StateCall,
) -> Vec<(String, Expression)> {
    native_plan
        .state_calls
        .arguments
        .span(state_call.arguments)
        .map(|arguments| {
            arguments
                .iter()
                .map(|argument| {
                    let expression = if argument.kind == StateCallArgumentKind::MutableAlias
                        && !matches!(argument.expression, Expression::Mutable(_))
                    {
                        Expression::Mutable(Box::new(argument.expression.clone()))
                    } else {
                        argument.expression.clone()
                    };
                    (argument.parameter_name.clone(), expression)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_branch_expression(
    expression: &Expression,
    branch_bindings: &[(String, Expression)],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_branch_expression(target, branch_bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => branch_bindings
            .iter()
            .find(|(parameter_name, _)| parameter_name == &path[0])
            .map(|(_, bound_expression)| append_place_suffix(bound_expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        Expression::Binary(binary) => {
            Expression::Binary(Box::new(crate::ir::expression::BinaryExpression {
                left: resolve_branch_expression(&binary.left, branch_bindings),
                operator: binary.operator,
                right: resolve_branch_expression(&binary.right, branch_bindings),
            }))
        }
        _ => expression.clone(),
    }
}

fn append_place_suffix(expression: &Expression, suffix: &[String]) -> Expression {
    if suffix.is_empty() {
        return expression.clone();
    }

    match expression {
        Expression::Name(path) => {
            let mut resolved_path = path.clone();
            resolved_path.extend_from_slice(suffix);
            Expression::Name(resolved_path)
        }
        Expression::Mutable(target) => {
            Expression::Mutable(Box::new(append_place_suffix(target, suffix)))
        }
        _ => expression.clone(),
    }
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
    machine_name: &str,
    state_name: &str,
) -> Vec<RuntimeBranchingCallEdge> {
    let Some(machine) = machine_flow(native_plan, machine_name) else {
        return Vec::new();
    };
    let Some(state) = native_plan
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.name == state_name))
    else {
        return Vec::new();
    };
    let Some(transitions) = native_plan.control_flow.transitions.span(state.transitions) else {
        return Vec::new();
    };

    transitions
        .iter()
        .enumerate()
        .map(|(order, transition)| {
            let target = runtime_transition_target(machine, state_name, &transition.target);
            RuntimeBranchingCallEdge {
                order,
                lowering: branch_target_lowering(native_plan, &target),
                target,
                continuation: transition
                    .continuation
                    .as_ref()
                    .map(|continuation| {
                        runtime_transition_target(machine, state_name, continuation)
                    })
                    .unwrap_or(RuntimeTransitionTarget::None),
                target_arguments: transition_target_arguments(&transition.target),
                guard_kind: classify_transition_guard(&transition.guard),
                guard: transition.guard.clone(),
            }
        })
        .collect()
}

fn transition_target_arguments(target: &PlannedTransitionTarget) -> Vec<Expression> {
    match target {
        PlannedTransitionTarget::State { arguments, .. }
        | PlannedTransitionTarget::Nested { arguments, .. } => arguments.clone(),
        PlannedTransitionTarget::SelfTarget | PlannedTransitionTarget::Terminal => Vec::new(),
    }
}

fn branch_target_lowering(
    native_plan: &NativePlan,
    target: &RuntimeTransitionTarget,
) -> RuntimeBranchTargetLowering {
    let RuntimeTransitionTarget::State { machine, state } = target else {
        return match target {
            RuntimeTransitionTarget::Terminal | RuntimeTransitionTarget::None => {
                RuntimeBranchTargetLowering::Terminal
            }
            RuntimeTransitionTarget::Unknown { .. } => RuntimeBranchTargetLowering::Unknown,
            RuntimeTransitionTarget::State { .. } => unreachable!(),
        };
    };

    let Some(target_machine) = machine_flow(native_plan, machine) else {
        return RuntimeBranchTargetLowering::Unknown;
    };
    let Some(target_state) = native_plan
        .control_flow
        .states
        .span(target_machine.states)
        .and_then(|states| states.iter().find(|candidate| candidate.name == *state))
    else {
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
                    && !state_statement_has_host_call(
                        native_plan,
                        machine,
                        state,
                        operation.statement_index,
                    )
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
    machine_name: &str,
    state_name: &str,
) -> Vec<RuntimeLeafBranchOperation> {
    let Some(machine) = machine_flow(native_plan, machine_name) else {
        return Vec::new();
    };
    let Some(state) = native_plan
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.name == state_name))
    else {
        return Vec::new();
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return Vec::new();
    };

    operations
        .iter()
        .map(|operation| RuntimeLeafBranchOperation {
            source_machine: machine_name.to_owned(),
            source_state: state_name.to_owned(),
            statement_index: operation.statement_index,
            kind: leaf_operation_kind(
                native_plan,
                machine_name,
                state_name,
                operation.statement_index,
            ),
        })
        .collect()
}

fn leaf_operation_kind(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> RuntimeLeafBranchOperationKind {
    if let Some(host_call) =
        host_call_for_statement(native_plan, machine_name, state_name, statement_index)
    {
        return RuntimeLeafBranchOperationKind::HostCall {
            platform_call: host_call.platform_call.clone(),
        };
    }

    if let Some(mutation) =
        mutation_for_statement(native_plan, machine_name, state_name, statement_index)
    {
        return RuntimeLeafBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: mutation.target.clone(),
            value: mutation.value.clone(),
        };
    }

    RuntimeLeafBranchOperationKind::Other
}

fn host_call_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    native_plan
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.machine == machine_name
                && host_call.state == state_name
                && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

fn mutation_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan crate::native::state_storage::StateMutation> {
    native_plan
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.machine == machine_name
                && mutation.state == state_name
                && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}

fn machine_flow<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
) -> Option<&'plan MachineFlow> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .map(|(_, machine)| machine)
}

fn state_call_for_operation<'plan>(
    native_plan: &'plan NativePlan,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    native_plan
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| {
            state_call.source_machine == source_machine
                && state_call.source_state == source_state
                && state_call.statement_index == statement_index
        })
        .map(|(_, state_call)| state_call)
}

fn state_parameters(native_plan: &NativePlan, machine_name: &str, state_name: &str) -> Vec<String> {
    machine_flow(native_plan, machine_name)
        .and_then(|machine| native_plan.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == state_name))
        .map(|state| state.parameters.clone())
        .unwrap_or_default()
}

fn state_statement_has_host_call(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.machine == machine_name
            && host_call.state == state_name
            && host_call.statement_index == statement_index
    })
}

fn runtime_transition_target(
    machine: &MachineFlow,
    current_state: &str,
    target: &PlannedTransitionTarget,
) -> RuntimeTransitionTarget {
    match target {
        PlannedTransitionTarget::State { name, .. } => RuntimeTransitionTarget::State {
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
                machine: contained.type_name.clone(),
                state: state.clone(),
            })
            .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                name: format!("{receiver}.{state}"),
            }),
        PlannedTransitionTarget::SelfTarget => RuntimeTransitionTarget::State {
            machine: machine.name.clone(),
            state: current_state.to_owned(),
        },
        PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
    }
}
