use crate::control_flow::{MachineFlow, OperationKind, PlannedTransitionTarget};
use crate::host_calls::HostCall;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::runtime_dispatch::guards::{StateGuardKind, classify_transition_guard};
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::state_calls::{StateCall, StateCallArgumentKind, StateCallLowering};
use crate::state_storage::{StateMutationKind, StateMutationLowering};
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeBranchingCallPlan {
    pub calls: Arena<RuntimeBranchingCall>,
    pub edges: Arena<RuntimeBranchingCallEdge>,
    pub target_arguments: Arena<Expression>,
    pub leaf_expansions: Arena<RuntimeLeafBranchExpansion>,
    pub leaf_operations: Arena<RuntimeLeafBranchOperation>,
    pub leaf_bindings: Arena<RuntimeLeafBranchBinding>,
    pub straight_line_expansions: Arena<RuntimeStraightLineBranchExpansion>,
    pub straight_line_operations: Arena<RuntimeStraightLineBranchOperation>,
    pub straight_line_bindings: Arena<RuntimeStraightLineBranchBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeBranchAlias {
    machine: String,
    state: String,
    parameter_name: String,
    expression: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchingCall {
    pub dispatch_index: u32,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub statement_index: usize,
    pub target_machine: ProgramName,
    pub target_state: ProgramName,
    pub argument_count: usize,
    pub expansion: RuntimeBranchCallExpansion,
    pub edges: HandleSpan<RuntimeBranchingCallEdge>,
}

impl Default for RuntimeBranchingCall {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            statement_index: 0,
            target_machine: ProgramName::default(),
            target_state: ProgramName::default(),
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
    pub target_arguments: HandleSpan<Expression>,
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
            target_arguments: HandleSpan::empty(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchExpansion {
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
    pub target_machine: String,
    pub target_state: String,
    pub bindings: HandleSpan<RuntimeStraightLineBranchBinding>,
    pub operations: HandleSpan<RuntimeStraightLineBranchOperation>,
}

impl Default for RuntimeStraightLineBranchExpansion {
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
            target_machine: String::new(),
            target_state: String::new(),
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchBinding {
    pub parameter_name: String,
    pub expression: Expression,
    pub kind: RuntimeStraightLineBranchBindingKind,
}

impl Default for RuntimeStraightLineBranchBinding {
    fn default() -> Self {
        Self {
            parameter_name: String::new(),
            expression: Expression::Integer(0),
            kind: RuntimeStraightLineBranchBindingKind::BranchParameter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeStraightLineBranchBindingKind {
    #[default]
    BranchParameter,
    TargetParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchOperation {
    pub source_machine: String,
    pub source_state: String,
    pub statement_index: usize,
    pub kind: RuntimeStraightLineBranchOperationKind,
}

impl Default for RuntimeStraightLineBranchOperation {
    fn default() -> Self {
        Self {
            source_machine: String::new(),
            source_state: String::new(),
            statement_index: 0,
            kind: RuntimeStraightLineBranchOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeStraightLineBranchOperationKind {
    HostCall {
        platform_call: String,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
        target: Expression,
        value: Expression,
    },
    StateCall {
        target_machine: String,
        target_state: String,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    LocalData,
    #[default]
    Other,
}

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
                &operation.source_machine,
                &operation.source_state,
                operation.statement_index,
            );
            let RuntimeDispatchBodyOperationKind::StateCall {
                target_machine,
                target_state,
                argument_count,
                lowering: StateCallLowering::InlineBranching,
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
                target_machine,
                target_state,
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
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
                statement_index: operation.statement_index,
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
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    dispatch_index: u32,
    branch_machine: &str,
    branch_state: &str,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    aliases: &[RuntimeBranchAlias],
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

        let branch_bindings = branch_parameter_bindings(native_plan, state_call, aliases);
        let bindings = plan.leaf_bindings.insert_many(leaf_branch_bindings(
            &branch_bindings,
            native_plan,
            branch_machine,
            branch_state,
            leaf_machine,
            leaf_state,
            plan.target_arguments.span_or_empty(edge.target_arguments),
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
            leaf_machine: leaf_machine.to_string(),
            leaf_state: leaf_state.to_string(),
            bindings,
            operations,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn append_straight_line_branch_expansions(
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
    aliases: &[RuntimeBranchAlias],
) {
    for edge in edges {
        if edge.lowering != RuntimeBranchTargetLowering::InlineStraightLine {
            continue;
        }

        let RuntimeTransitionTarget::State {
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
                target_machine,
                target_state,
                plan.target_arguments.span_or_empty(edge.target_arguments),
            ));
        let operations = plan
            .straight_line_operations
            .insert_many(straight_line_operations(
                native_plan,
                target_machine,
                target_state,
            ));

        plan.straight_line_expansions
            .insert(RuntimeStraightLineBranchExpansion {
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
                target_machine: target_machine.to_string(),
                target_state: target_state.to_string(),
                bindings,
                operations,
            });
    }
}

fn leaf_branch_bindings<'a>(
    branch_bindings: &'a [(String, Expression)],
    native_plan: &NativePlan,
    _branch_machine: &str,
    _branch_state: &str,
    leaf_machine: &str,
    leaf_state: &str,
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

    let leaf_parameters = state_parameters(native_plan, leaf_machine, leaf_state);
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
    branch_bindings: &'a [(String, Expression)],
    native_plan: &NativePlan,
    target_machine: &str,
    target_state: &str,
    target_arguments: &'a [Expression],
) -> impl Iterator<Item = RuntimeStraightLineBranchBinding> + 'a {
    let branch_parameter_bindings = branch_bindings.iter().map(|(parameter_name, expression)| {
        RuntimeStraightLineBranchBinding {
            parameter_name: parameter_name.clone(),
            expression: expression.clone(),
            kind: RuntimeStraightLineBranchBindingKind::BranchParameter,
        }
    });

    let target_parameters = state_parameters(native_plan, target_machine, target_state);
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
    aliases: &[RuntimeBranchAlias],
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
                    (
                        argument.parameter_name.to_string(),
                        resolve_runtime_branch_alias_expression(
                            &expression,
                            &state_call.source_machine,
                            &state_call.source_state,
                            aliases,
                        ),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn bind_runtime_branch_aliases(
    native_plan: &NativePlan,
    aliases: &mut Vec<RuntimeBranchAlias>,
    state_call: &StateCall,
) {
    let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        let expression = if argument.kind == StateCallArgumentKind::MutableAlias
            && !matches!(argument.expression, Expression::Mutable(_))
        {
            Expression::Mutable(Box::new(argument.expression.clone()))
        } else {
            argument.expression.clone()
        };
        set_runtime_branch_alias(
            aliases,
            RuntimeBranchAlias {
                machine: state_call.target_machine.to_string(),
                state: state_call.target_state.to_string(),
                parameter_name: argument.parameter_name.to_string(),
                expression: resolve_runtime_branch_alias_expression(
                    &expression,
                    &state_call.source_machine,
                    &state_call.source_state,
                    aliases,
                ),
            },
        );
    }
}

fn set_runtime_branch_alias(aliases: &mut Vec<RuntimeBranchAlias>, alias: RuntimeBranchAlias) {
    if let Some(existing_alias) = aliases.iter_mut().find(|existing_alias| {
        existing_alias.machine == alias.machine
            && existing_alias.state == alias.state
            && existing_alias.parameter_name == alias.parameter_name
    }) {
        *existing_alias = alias;
    } else {
        aliases.push(alias);
    }
}

fn resolve_runtime_branch_alias_expression(
    expression: &Expression,
    source_machine: &str,
    source_state: &str,
    aliases: &[RuntimeBranchAlias],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_runtime_branch_alias_expression(
                target,
                source_machine,
                source_state,
                aliases,
            );
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(
            omega_typed_program::expression::IndexedExpression {
                collection: resolve_runtime_branch_alias_expression(
                    &indexed.collection,
                    source_machine,
                    source_state,
                    aliases,
                ),
                index: resolve_runtime_branch_alias_expression(
                    &indexed.index,
                    source_machine,
                    source_state,
                    aliases,
                ),
            },
        )),
        Expression::Name(path) if !path.is_empty() => aliases
            .iter()
            .rev()
            .find(|alias| {
                alias.machine == source_machine
                    && alias.state == source_state
                    && alias.parameter_name == path[0]
            })
            .map(|alias| append_place_suffix(&alias.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
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
        Expression::Binary(binary) => Expression::Binary(Box::new(
            omega_typed_program::expression::BinaryExpression {
                left: resolve_branch_expression(&binary.left, branch_bindings),
                operator: binary.operator,
                right: resolve_branch_expression(&binary.right, branch_bindings),
            },
        )),
        _ => expression.clone(),
    }
}

fn append_place_suffix(expression: &Expression, suffix: &[ProgramName]) -> Expression {
    if suffix.is_empty() {
        return expression.clone();
    }

    match expression {
        Expression::Name(path) => {
            let mut resolved_path = path.clone();
            resolved_path.extend_from_slice(suffix);
            Expression::Name(resolved_path)
        }
        Expression::Indexed(indexed) => {
            if let Some(mut indexed_path) = indexed_expression_path(indexed) {
                indexed_path.extend_from_slice(suffix);
                Expression::Name(indexed_path)
            } else {
                expression.clone()
            }
        }
        Expression::Mutable(target) => {
            Expression::Mutable(Box::new(append_place_suffix(target, suffix)))
        }
        _ => expression.clone(),
    }
}

fn indexed_expression_path(
    indexed: &omega_typed_program::expression::IndexedExpression,
) -> Option<Vec<ProgramName>> {
    let Expression::Integer(index) = &indexed.index else {
        return None;
    };
    let mut path = match &indexed.collection {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(inner_indexed) => indexed_expression_path(inner_indexed)?,
        _ => return None,
    };
    let last_segment = path.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(path)
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
    target_arguments: &mut Arena<Expression>,
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

fn leaf_operations<'a>(
    native_plan: &'a NativePlan,
    machine_name: &'a str,
    state_name: &'a str,
) -> impl Iterator<Item = RuntimeLeafBranchOperation> + 'a {
    let operations = machine_flow(native_plan, machine_name)
        .and_then(|machine| {
            native_plan
                .control_flow
                .states
                .span(machine.states)
                .and_then(|states| states.iter().find(|state| state.name == state_name))
        })
        .and_then(|state| native_plan.control_flow.operations.span(state.operations));

    operations.into_iter().flat_map(move |operations| {
        operations
            .iter()
            .map(move |operation| RuntimeLeafBranchOperation {
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
    })
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

fn straight_line_operations<'a>(
    native_plan: &'a NativePlan,
    machine_name: &'a str,
    state_name: &'a str,
) -> impl Iterator<Item = RuntimeStraightLineBranchOperation> + 'a {
    let operations = machine_flow(native_plan, machine_name)
        .and_then(|machine| {
            native_plan
                .control_flow
                .states
                .span(machine.states)
                .and_then(|states| states.iter().find(|state| state.name == state_name))
        })
        .and_then(|state| native_plan.control_flow.operations.span(state.operations));

    operations.into_iter().flat_map(move |operations| {
        operations
            .iter()
            .map(move |operation| RuntimeStraightLineBranchOperation {
                source_machine: machine_name.to_owned(),
                source_state: state_name.to_owned(),
                statement_index: operation.statement_index,
                kind: straight_line_operation_kind(
                    native_plan,
                    machine_name,
                    state_name,
                    operation.statement_index,
                    &operation.kind,
                ),
            })
    })
}

fn straight_line_operation_kind(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
    operation_kind: &OperationKind,
) -> RuntimeStraightLineBranchOperationKind {
    if let Some(host_call) =
        host_call_for_statement(native_plan, machine_name, state_name, statement_index)
    {
        return RuntimeStraightLineBranchOperationKind::HostCall {
            platform_call: host_call.platform_call.clone(),
        };
    }

    if let Some(mutation) =
        mutation_for_statement(native_plan, machine_name, state_name, statement_index)
    {
        return RuntimeStraightLineBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: mutation.target.clone(),
            value: mutation.value.clone(),
        };
    }

    if let Some(state_call) =
        state_call_for_operation(native_plan, machine_name, state_name, statement_index)
    {
        return RuntimeStraightLineBranchOperationKind::StateCall {
            target_machine: state_call.target_machine.to_string(),
            target_state: state_call.target_state.to_string(),
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
) -> Option<&'plan crate::state_storage::StateMutation> {
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
        .map(|state| state.parameters.iter().map(ToString::to_string).collect())
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
            state: current_state.to_owned().into(),
        },
        PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
    }
}
