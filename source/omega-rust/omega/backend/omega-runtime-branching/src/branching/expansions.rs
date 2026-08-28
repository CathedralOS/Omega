use super::aliases::{
    BranchParameterBinding, BranchParameterBindings, RuntimeBranchAliasBuffer,
    branch_parameter_bindings, resolve_branch_expression_handle, resolve_branch_guard_handle,
};
use super::edges::build_branch_edges;
use super::lookups::state_parameters;
use super::operations::{
    leaf_local_initializer_operations, leaf_operations, prelude_operations,
    straight_line_operations,
};
use super::{
    RuntimeBranchPreludeBinding, RuntimeBranchPreludeExpansion, RuntimeBranchPreludeOperation,
    RuntimeBranchTargetLowering, RuntimeBranchingCallEdge, RuntimeLeafBranchBinding,
    RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion, RuntimeLeafBranchOperation,
    RuntimeStraightLineBranchBinding, RuntimeStraightLineBranchBindingKind,
    RuntimeStraightLineBranchExpansion, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};
use crate::RuntimeBranchingContext;
use omega_control_flow::StateKey;
use omega_state_calls::{StateCall, StateCallLowering, StateCallRole};
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_guards::StateGuardKind;
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, TableBinaryExpression,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VisitedNestedCall {
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    target_key: StateKey,
}

impl VisitedNestedCall {
    fn new(
        source_key: StateKey,
        statement_index: usize,
        role: StateCallRole,
        call_ordinal: usize,
        target_key: StateKey,
    ) -> Self {
        Self {
            source_key,
            statement_index,
            role,
            call_ordinal,
            target_key,
        }
    }
}

pub(super) fn append_branch_prelude_expansion(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    target_arguments_arena: &mut Arena<ExpressionHandle>,
    target_values: &mut Arena<ExpressionHandle>,
    output_edges: &mut Arena<RuntimeBranchingCallEdge>,
    prelude_expansions: &mut Arena<RuntimeBranchPreludeExpansion>,
    prelude_bindings: &mut Arena<RuntimeBranchPreludeBinding>,
    prelude_operations_arena: &mut Arena<RuntimeBranchPreludeOperation>,
    leaf_expansions: &mut Arena<RuntimeLeafBranchExpansion>,
    leaf_bindings: &mut Arena<RuntimeLeafBranchBinding>,
    leaf_operations_arena: &mut Arena<RuntimeLeafBranchOperation>,
    straight_line_expansions: &mut Arena<RuntimeStraightLineBranchExpansion>,
    straight_line_bindings_arena: &mut Arena<RuntimeStraightLineBranchBinding>,
    straight_line_operations_arena: &mut Arena<RuntimeStraightLineBranchOperation>,
    next_scope_id: &mut u32,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    state_call: &StateCall,
    aliases: &RuntimeBranchAliasBuffer,
    statement_filter: super::operations::PreludeStatementFilter,
) {
    let branch_bindings = branch_parameter_bindings(context, state_call, aliases, expressions);
    let bindings = prelude_bindings.insert_many(branch_bindings.iter().map(|binding| {
        RuntimeBranchPreludeBinding {
            parameter_symbol: binding.parameter_symbol,
            parameter_name: binding.parameter_name.clone(),
            expression: binding.expression,
        }
    }));
    let operations = prelude_operations(
        context,
        expressions,
        prelude_operations_arena,
        state_call.target_key,
        statement_filter,
    );

    prelude_expansions.insert(RuntimeBranchPreludeExpansion {
        dispatch_index,
        source_key,
        statement_index,
        branch_key,
        target_key: state_call.target_key,
        role: state_call.role,
        bindings,
        operations,
    });

    // Only a guard-role prelude (filter All) walks the callee's nested calls:
    // it is the chain's EXECUTOR, so the walk materializes the nested callees'
    // expansions it will emit. A repeated guard subject (filter None) keeps only
    // the empty prelude record above, and a NON-guard prelude (LocalDataOnly)
    // must not walk either -- the runtime-bodies splice already flattens every
    // nested call into the caller's body, where each gets its own directly
    // matched machinery; expansions appended here would be emitted ON TOP of
    // that (both through this prelude and through the flattened call's direct
    // match), re-running the leaf callee's side effects per executor.
    if statement_filter != super::operations::PreludeStatementFilter::All {
        return;
    }

    let operations = straight_line_operations(
        context,
        expressions,
        straight_line_operations_arena,
        state_call.target_key,
    );
    let operations = straight_line_operations_arena
        .span(operations)
        .map(|operations| operations.to_vec())
        .unwrap_or_default();
    let mut visited_states = Vec::new();
    let mut visited_nested_calls = Vec::new();
    append_nested_operation_branch_expansions_with_bindings(
        context,
        expressions,
        target_arguments_arena,
        target_values,
        output_edges,
        leaf_expansions,
        leaf_bindings,
        leaf_operations_arena,
        straight_line_expansions,
        straight_line_bindings_arena,
        straight_line_operations_arena,
        next_scope_id,
        dispatch_index,
        operations,
        &branch_bindings,
        ExpressionHandle::invalid(),
        StateGuardKind::Always,
        &mut visited_states,
        &mut visited_nested_calls,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_leaf_branch_expansions(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    target_arguments: &Arena<ExpressionHandle>,
    leaf_expansions: &mut Arena<RuntimeLeafBranchExpansion>,
    leaf_bindings: &mut Arena<RuntimeLeafBranchBinding>,
    leaf_operations_arena: &mut Arena<RuntimeLeafBranchOperation>,
    scope_id: u32,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    aliases: &RuntimeBranchAliasBuffer,
    inherited_guard: ExpressionHandle,
    inherited_guard_kind: StateGuardKind,
) {
    let branch_bindings = branch_parameter_bindings(context, state_call, aliases, expressions);
    append_leaf_branch_expansions_with_bindings(
        context,
        expressions,
        target_arguments,
        leaf_expansions,
        leaf_bindings,
        leaf_operations_arena,
        scope_id,
        source_key,
        branch_key,
        statement_index,
        dispatch_index,
        edges,
        state_call,
        &branch_bindings,
        inherited_guard,
        inherited_guard_kind,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_leaf_branch_expansions_with_bindings(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    target_arguments: &Arena<ExpressionHandle>,
    leaf_expansions: &mut Arena<RuntimeLeafBranchExpansion>,
    leaf_bindings: &mut Arena<RuntimeLeafBranchBinding>,
    leaf_operations_arena: &mut Arena<RuntimeLeafBranchOperation>,
    scope_id: u32,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    branch_bindings: &BranchParameterBindings,
    inherited_guard: ExpressionHandle,
    inherited_guard_kind: StateGuardKind,
) {
    for edge in edges {
        let (leaf_key, bindings, operations) = match &edge.target {
            RuntimeTransitionTarget::State { key: leaf_key, .. }
                if edge.lowering == RuntimeBranchTargetLowering::InlineLeaf =>
            {
                let leaf_arguments = target_arguments.span_or_empty(edge.target_arguments);
                let bindings = leaf_branch_bindings(
                    &branch_bindings,
                    context,
                    *leaf_key,
                    expressions,
                    leaf_bindings,
                    leaf_arguments,
                );
                let operations =
                    leaf_operations(context, expressions, leaf_operations_arena, *leaf_key);
                (*leaf_key, bindings, operations)
            }
            RuntimeTransitionTarget::Terminal if edge.target_value.is_valid() => (
                StateKey::default(),
                leaf_bindings.insert_many(branch_bindings.iter().map(|binding| {
                    RuntimeLeafBranchBinding {
                        parameter_symbol: binding.parameter_symbol,
                        parameter_name: binding.parameter_name.clone(),
                        expression: binding.expression,
                        kind: RuntimeLeafBranchBindingKind::BranchParameter,
                    }
                })),
                // SUB-STATE terminals carry their arm body's call-free
                // LocalData initializers -- nothing else emits a non-leaf
                // sub-state's body on this route. The callee ROOT is excluded:
                // its ops ride the prelude, and re-running them at terminal
                // time reset captured-before-mutation locals (the
                // trailing-local-return 7-red regression). See the fn's doc.
                if branch_key == state_call.target_key {
                    HandleSpan::empty()
                } else {
                    leaf_local_initializer_operations(
                        context,
                        expressions,
                        leaf_operations_arena,
                        branch_key,
                    )
                },
            ),
            _ => continue,
        };

        let edge_guard = resolve_branch_guard_handle(edge.guard, &branch_bindings, expressions);
        let resolved_guard =
            combine_runtime_branch_guards(expressions, inherited_guard, edge_guard);
        let guard_kind = combine_runtime_branch_guard_kinds(inherited_guard_kind, edge.guard_kind);

        leaf_expansions.insert(RuntimeLeafBranchExpansion {
            scope_id,
            dispatch_index,
            source_key,
            statement_index,
            branch_key,
            target_statement_index: edge.statement_index,
            edge_order: edge.order,
            guard: edge_guard,
            resolved_guard,
            guard_kind,
            local_guard_kind: edge.guard_kind,
            role: state_call.role,
            call_ordinal: state_call.call_ordinal,
            leaf_key,
            target_value: edge.target_value,
            is_default_target: edge.guard_kind == StateGuardKind::Always,
            bindings,
            operations,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_straight_line_branch_expansions(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    target_arguments_arena: &mut Arena<ExpressionHandle>,
    target_values: &mut Arena<ExpressionHandle>,
    output_edges: &mut Arena<RuntimeBranchingCallEdge>,
    leaf_expansions: &mut Arena<RuntimeLeafBranchExpansion>,
    leaf_bindings: &mut Arena<RuntimeLeafBranchBinding>,
    leaf_operations_arena: &mut Arena<RuntimeLeafBranchOperation>,
    straight_line_expansions: &mut Arena<RuntimeStraightLineBranchExpansion>,
    straight_line_bindings_arena: &mut Arena<RuntimeStraightLineBranchBinding>,
    straight_line_operations_arena: &mut Arena<RuntimeStraightLineBranchOperation>,
    next_scope_id: &mut u32,
    root_scope_id: u32,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    aliases: &RuntimeBranchAliasBuffer,
) {
    let branch_bindings = branch_parameter_bindings(context, state_call, aliases, expressions);
    let mut visited = Vec::new();
    let mut visited_nested_calls = Vec::new();
    append_straight_line_branch_expansions_for_edges(
        context,
        expressions,
        target_arguments_arena,
        target_values,
        output_edges,
        leaf_expansions,
        leaf_bindings,
        leaf_operations_arena,
        straight_line_expansions,
        straight_line_bindings_arena,
        straight_line_operations_arena,
        next_scope_id,
        source_key,
        branch_key,
        statement_index,
        dispatch_index,
        edges,
        state_call,
        &branch_bindings,
        root_scope_id,
        ExpressionHandle::invalid(),
        StateGuardKind::Always,
        &mut visited,
        &mut visited_nested_calls,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_straight_line_branch_expansions_for_edges(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    target_arguments_arena: &mut Arena<ExpressionHandle>,
    target_values: &mut Arena<ExpressionHandle>,
    output_edges: &mut Arena<RuntimeBranchingCallEdge>,
    leaf_expansions: &mut Arena<RuntimeLeafBranchExpansion>,
    leaf_bindings: &mut Arena<RuntimeLeafBranchBinding>,
    leaf_operations_arena: &mut Arena<RuntimeLeafBranchOperation>,
    straight_line_expansions: &mut Arena<RuntimeStraightLineBranchExpansion>,
    straight_line_bindings_arena: &mut Arena<RuntimeStraightLineBranchBinding>,
    straight_line_operations_arena: &mut Arena<RuntimeStraightLineBranchOperation>,
    next_scope_id: &mut u32,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    branch_bindings: &BranchParameterBindings,
    scope_id: u32,
    inherited_guard: ExpressionHandle,
    inherited_guard_kind: StateGuardKind,
    visited: &mut Vec<StateKey>,
    visited_nested_calls: &mut Vec<VisitedNestedCall>,
) {
    for edge in edges {
        let RuntimeTransitionTarget::State {
            key: target_key, ..
        } = &edge.target
        else {
            continue;
        };

        if !matches!(
            edge.lowering,
            RuntimeBranchTargetLowering::InlineStraightLine
                | RuntimeBranchTargetLowering::InlineBranching
        ) {
            continue;
        }

        let target_arguments = target_arguments_arena.span_or_empty(edge.target_arguments);
        let bindings = straight_line_branch_bindings(
            branch_bindings,
            context,
            *target_key,
            expressions,
            straight_line_bindings_arena,
            target_arguments,
        );
        let operations = straight_line_operations(
            context,
            expressions,
            straight_line_operations_arena,
            *target_key,
        );

        let edge_guard = resolve_branch_guard_handle(edge.guard, &branch_bindings, expressions);
        let resolved_guard =
            combine_runtime_branch_guards(expressions, inherited_guard, edge_guard);
        let guard_kind = combine_runtime_branch_guard_kinds(inherited_guard_kind, edge.guard_kind);

        let child_scope_id = *next_scope_id;
        *next_scope_id += 1;
        straight_line_expansions.insert(RuntimeStraightLineBranchExpansion {
            scope_id,
            child_scope_id,
            dispatch_index,
            source_key,
            statement_index,
            branch_key,
            target_key: *target_key,
            target_statement_index: edge.statement_index,
            edge_order: edge.order,
            guard: edge_guard,
            resolved_guard,
            guard_kind,
            local_guard_kind: edge.guard_kind,
            role: state_call.role,
            call_ordinal: state_call.call_ordinal,
            target_value: edge.target_value,
            bindings,
            operations,
        });

        let nested_branch_bindings = nested_branch_parameter_bindings(
            branch_bindings,
            context,
            *target_key,
            expressions,
            target_arguments,
        );
        let nested_edges = build_branch_edges(
            context,
            *target_key,
            expressions,
            target_arguments_arena,
            target_values,
            output_edges,
        );
        let nested_edges = output_edges.span_or_empty(nested_edges).to_vec();
        append_leaf_branch_expansions_with_bindings(
            context,
            expressions,
            target_arguments_arena,
            leaf_expansions,
            leaf_bindings,
            leaf_operations_arena,
            child_scope_id,
            source_key,
            *target_key,
            statement_index,
            dispatch_index,
            &nested_edges,
            state_call,
            &nested_branch_bindings,
            resolved_guard,
            guard_kind,
        );

        if edge.lowering == RuntimeBranchTargetLowering::InlineBranching
            && !visited.contains(target_key)
        {
            visited.push(*target_key);
            append_straight_line_branch_expansions_for_edges(
                context,
                expressions,
                target_arguments_arena,
                target_values,
                output_edges,
                leaf_expansions,
                leaf_bindings,
                leaf_operations_arena,
                straight_line_expansions,
                straight_line_bindings_arena,
                straight_line_operations_arena,
                next_scope_id,
                source_key,
                *target_key,
                statement_index,
                dispatch_index,
                &nested_edges,
                state_call,
                &nested_branch_bindings,
                child_scope_id,
                resolved_guard,
                guard_kind,
                visited,
                visited_nested_calls,
            );
            visited.pop();
        }

        append_nested_operation_branch_expansions(
            context,
            expressions,
            target_arguments_arena,
            target_values,
            output_edges,
            leaf_expansions,
            leaf_bindings,
            leaf_operations_arena,
            straight_line_expansions,
            straight_line_bindings_arena,
            straight_line_operations_arena,
            next_scope_id,
            dispatch_index,
            operations,
            &bindings,
            resolved_guard,
            guard_kind,
            visited,
            visited_nested_calls,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_nested_operation_branch_expansions(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    target_arguments_arena: &mut Arena<ExpressionHandle>,
    target_values: &mut Arena<ExpressionHandle>,
    output_edges: &mut Arena<RuntimeBranchingCallEdge>,
    leaf_expansions: &mut Arena<RuntimeLeafBranchExpansion>,
    leaf_bindings: &mut Arena<RuntimeLeafBranchBinding>,
    leaf_operations_arena: &mut Arena<RuntimeLeafBranchOperation>,
    straight_line_expansions: &mut Arena<RuntimeStraightLineBranchExpansion>,
    straight_line_bindings_arena: &mut Arena<RuntimeStraightLineBranchBinding>,
    straight_line_operations_arena: &mut Arena<RuntimeStraightLineBranchOperation>,
    next_scope_id: &mut u32,
    dispatch_index: u32,
    operations: HandleSpan<RuntimeStraightLineBranchOperation>,
    parent_bindings: &HandleSpan<RuntimeStraightLineBranchBinding>,
    inherited_guard: ExpressionHandle,
    inherited_guard_kind: StateGuardKind,
    visited: &mut Vec<StateKey>,
    visited_nested_calls: &mut Vec<VisitedNestedCall>,
) {
    let Some(operations) = straight_line_operations_arena.span(operations) else {
        return;
    };
    let operations = operations.to_vec();
    let parent_bindings = straight_line_bindings_arena.span_or_empty(*parent_bindings);
    let parent_branch_bindings = branch_parameter_bindings_from_straight_line(parent_bindings);

    append_nested_operation_branch_expansions_with_bindings(
        context,
        expressions,
        target_arguments_arena,
        target_values,
        output_edges,
        leaf_expansions,
        leaf_bindings,
        leaf_operations_arena,
        straight_line_expansions,
        straight_line_bindings_arena,
        straight_line_operations_arena,
        next_scope_id,
        dispatch_index,
        operations.into(),
        &parent_branch_bindings,
        inherited_guard,
        inherited_guard_kind,
        visited,
        visited_nested_calls,
    );
}

#[allow(clippy::too_many_arguments)]
fn append_nested_operation_branch_expansions_with_bindings(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    target_arguments_arena: &mut Arena<ExpressionHandle>,
    target_values: &mut Arena<ExpressionHandle>,
    output_edges: &mut Arena<RuntimeBranchingCallEdge>,
    leaf_expansions: &mut Arena<RuntimeLeafBranchExpansion>,
    leaf_bindings: &mut Arena<RuntimeLeafBranchBinding>,
    leaf_operations_arena: &mut Arena<RuntimeLeafBranchOperation>,
    straight_line_expansions: &mut Arena<RuntimeStraightLineBranchExpansion>,
    straight_line_bindings_arena: &mut Arena<RuntimeStraightLineBranchBinding>,
    straight_line_operations_arena: &mut Arena<RuntimeStraightLineBranchOperation>,
    next_scope_id: &mut u32,
    dispatch_index: u32,
    operations: Vec<RuntimeStraightLineBranchOperation>,
    parent_branch_bindings: &BranchParameterBindings,
    inherited_guard: ExpressionHandle,
    inherited_guard_kind: StateGuardKind,
    visited: &mut Vec<StateKey>,
    visited_nested_calls: &mut Vec<VisitedNestedCall>,
) {
    for operation in &operations {
        if let RuntimeStraightLineBranchOperationKind::StateCall {
            role,
            call_ordinal,
            target_key,
            lowering: StateCallLowering::InlineExpansion,
            ..
        } = operation.kind
        {
            let visited_call = VisitedNestedCall::new(
                operation.source_key,
                operation.statement_index,
                role,
                call_ordinal,
                target_key,
            );
            if visited_nested_calls.contains(&visited_call) || visited.contains(&target_key) {
                continue;
            }
            let Some(state_call) = state_call_for_straight_line_operation(
                context,
                operation.source_key,
                operation.statement_index,
                role,
                call_ordinal,
            ) else {
                continue;
            };
            let nested_branch_bindings = branch_parameter_bindings_for_nested_state_call(
                context,
                expressions,
                &parent_branch_bindings,
                state_call,
            );
            let target_operations = straight_line_operations(
                context,
                expressions,
                straight_line_operations_arena,
                target_key,
            );
            let Some(target_operations_slice) =
                straight_line_operations_arena.span(target_operations)
            else {
                continue;
            };
            if target_operations_slice.is_empty() {
                continue;
            }
            visited_nested_calls.push(visited_call);
            visited.push(target_key);
            append_nested_operation_branch_expansions_with_bindings(
                context,
                expressions,
                target_arguments_arena,
                target_values,
                output_edges,
                leaf_expansions,
                leaf_bindings,
                leaf_operations_arena,
                straight_line_expansions,
                straight_line_bindings_arena,
                straight_line_operations_arena,
                next_scope_id,
                dispatch_index,
                target_operations_slice.to_vec(),
                &nested_branch_bindings,
                inherited_guard,
                inherited_guard_kind,
                visited,
                visited_nested_calls,
            );
            visited.pop();
            continue;
        }

        let RuntimeStraightLineBranchOperationKind::StateCall {
            role,
            call_ordinal,
            target_key,
            lowering: StateCallLowering::InlineBranching,
            ..
        } = operation.kind
        else {
            continue;
        };
        let visited_call = VisitedNestedCall::new(
            operation.source_key,
            operation.statement_index,
            role,
            call_ordinal,
            target_key,
        );
        if visited_nested_calls.contains(&visited_call) || visited.contains(&target_key) {
            continue;
        }
        let Some(state_call) = state_call_for_straight_line_operation(
            context,
            operation.source_key,
            operation.statement_index,
            role,
            call_ordinal,
        ) else {
            continue;
        };
        visited_nested_calls.push(visited_call);
        let nested_scope_id = *next_scope_id;
        *next_scope_id += 1;

        let nested_branch_bindings = branch_parameter_bindings_for_nested_state_call(
            context,
            expressions,
            &parent_branch_bindings,
            state_call,
        );
        let target_prelude_operations = straight_line_operations(
            context,
            expressions,
            straight_line_operations_arena,
            target_key,
        );
        if let Some(target_prelude_operations_slice) =
            straight_line_operations_arena.span(target_prelude_operations)
            && !target_prelude_operations_slice.is_empty()
        {
            let target_prelude_bindings =
                straight_line_bindings_arena.insert_many(nested_branch_bindings.iter().map(
                    |binding| RuntimeStraightLineBranchBinding {
                        parameter_symbol: binding.parameter_symbol,
                        parameter_name: binding.parameter_name.clone(),
                        expression: binding.expression,
                        kind: RuntimeStraightLineBranchBindingKind::BranchParameter,
                    },
                ));
            straight_line_expansions.insert(RuntimeStraightLineBranchExpansion {
                scope_id: nested_scope_id,
                child_scope_id: nested_scope_id,
                dispatch_index,
                source_key: operation.source_key,
                statement_index: operation.statement_index,
                branch_key: target_key,
                target_key,
                target_statement_index: 0,
                edge_order: 0,
                guard: ExpressionHandle::invalid(),
                resolved_guard: inherited_guard,
                guard_kind: inherited_guard_kind,
                local_guard_kind: StateGuardKind::Always,
                role: state_call.role,
                call_ordinal: state_call.call_ordinal,
                target_value: ExpressionHandle::invalid(),
                bindings: target_prelude_bindings,
                operations: target_prelude_operations,
            });
            append_nested_operation_branch_expansions_with_bindings(
                context,
                expressions,
                target_arguments_arena,
                target_values,
                output_edges,
                leaf_expansions,
                leaf_bindings,
                leaf_operations_arena,
                straight_line_expansions,
                straight_line_bindings_arena,
                straight_line_operations_arena,
                next_scope_id,
                dispatch_index,
                target_prelude_operations_slice.to_vec(),
                &nested_branch_bindings,
                inherited_guard,
                inherited_guard_kind,
                visited,
                visited_nested_calls,
            );
        }
        let nested_edges = build_branch_edges(
            context,
            target_key,
            expressions,
            target_arguments_arena,
            target_values,
            output_edges,
        );
        let nested_edges = output_edges.span_or_empty(nested_edges).to_vec();
        append_leaf_branch_expansions_with_bindings(
            context,
            expressions,
            target_arguments_arena,
            leaf_expansions,
            leaf_bindings,
            leaf_operations_arena,
            nested_scope_id,
            operation.source_key,
            target_key,
            operation.statement_index,
            dispatch_index,
            &nested_edges,
            state_call,
            &nested_branch_bindings,
            inherited_guard,
            inherited_guard_kind,
        );
        visited.push(target_key);
        append_straight_line_branch_expansions_for_edges(
            context,
            expressions,
            target_arguments_arena,
            target_values,
            output_edges,
            leaf_expansions,
            leaf_bindings,
            leaf_operations_arena,
            straight_line_expansions,
            straight_line_bindings_arena,
            straight_line_operations_arena,
            next_scope_id,
            operation.source_key,
            target_key,
            operation.statement_index,
            dispatch_index,
            &nested_edges,
            state_call,
            &nested_branch_bindings,
            nested_scope_id,
            inherited_guard,
            inherited_guard_kind,
            visited,
            visited_nested_calls,
        );
        visited.pop();
    }
}

fn combine_runtime_branch_guards(
    expressions: &mut ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> ExpressionHandle {
    match (
        guard_is_always(expressions, left),
        guard_is_always(expressions, right),
    ) {
        (true, true) => ExpressionHandle::invalid(),
        (true, false) => right,
        (false, true) => left,
        (false, false) => expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator: BinaryOperator::And,
            right,
        })),
    }
}

fn guard_is_always(expressions: &ExpressionTable, guard: ExpressionHandle) -> bool {
    !guard.is_valid() || matches!(expressions.expression(guard), ExpressionNode::Boolean(true))
}

fn combine_runtime_branch_guard_kinds(
    left: StateGuardKind,
    right: StateGuardKind,
) -> StateGuardKind {
    match (left, right) {
        (StateGuardKind::Always, StateGuardKind::Always) => StateGuardKind::Always,
        (StateGuardKind::Always, right) => right,
        (left, StateGuardKind::Always) => left,
        _ => StateGuardKind::RuntimeExpression,
    }
}

fn branch_parameter_bindings_from_straight_line(
    bindings: &[RuntimeStraightLineBranchBinding],
) -> BranchParameterBindings {
    let mut output = BranchParameterBindings::with_capacity(bindings.len());
    for binding in bindings {
        output.push(BranchParameterBinding {
            parameter_symbol: binding.parameter_symbol,
            parameter_name: binding.parameter_name.clone(),
            expression: binding.expression,
        });
    }
    output
}

fn branch_parameter_bindings_for_nested_state_call(
    context: &RuntimeBranchingContext,
    expression_table: &mut ExpressionTable,
    parent_bindings: &BranchParameterBindings,
    state_call: &StateCall,
) -> BranchParameterBindings {
    let existing_count = parent_bindings.iter().count();
    let mut bindings = BranchParameterBindings::with_capacity(
        existing_count.saturating_add(state_call.arguments.len()),
    );

    for binding in parent_bindings.iter() {
        bindings.push(BranchParameterBinding {
            parameter_symbol: binding.parameter_symbol,
            parameter_name: binding.parameter_name.clone(),
            expression: binding.expression,
        });
    }

    let Some(arguments) = context.state_calls.arguments.span(state_call.arguments) else {
        return bindings;
    };
    for (parameter_index, parameter) in state_parameters(context, state_call.target_key)
        .iter()
        .enumerate()
    {
        let Some(argument) = arguments.get(parameter_index) else {
            continue;
        };
        let argument_expression =
            expression_table.copy_from(&context.state_calls.expressions, argument.expression);
        bindings.push(BranchParameterBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.clone(),
            expression: resolve_branch_expression_handle(
                argument_expression,
                parent_bindings,
                expression_table,
            ),
        });
    }

    bindings
}

fn state_call_for_straight_line_operation<'a>(
    context: &'a RuntimeBranchingContext<'a>,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
) -> Option<&'a StateCall> {
    match role {
        StateCallRole::Statement => context
            .state_calls
            .statement_call(source_key, statement_index),
        StateCallRole::AssignmentValue => context
            .state_calls
            .assignment_value_call_by_ordinal(source_key, statement_index, call_ordinal)
            .or_else(|| {
                context
                    .state_calls
                    .assignment_value_call(source_key, statement_index)
            }),
        StateCallRole::CallArgument => context.state_calls.call_argument_call_by_ordinal(
            source_key,
            statement_index,
            call_ordinal,
        ),
        StateCallRole::TransitionArgument => context
            .state_calls
            .transition_argument_call_by_ordinal(source_key, statement_index, call_ordinal)
            .or_else(|| {
                context
                    .state_calls
                    .transition_argument_call(source_key, statement_index)
            }),
        _ => None,
    }
}

fn nested_branch_parameter_bindings(
    branch_bindings: &BranchParameterBindings,
    context: &RuntimeBranchingContext,
    target_key: StateKey,
    expression_table: &mut ExpressionTable,
    target_arguments: &[ExpressionHandle],
) -> BranchParameterBindings {
    let existing_count = branch_bindings.iter().count();
    let mut bindings = BranchParameterBindings::with_capacity(
        existing_count.saturating_add(target_arguments.len()),
    );

    for binding in branch_bindings.iter() {
        bindings.push(BranchParameterBinding {
            parameter_symbol: binding.parameter_symbol,
            parameter_name: binding.parameter_name.clone(),
            expression: binding.expression,
        });
    }

    for (parameter_index, parameter) in state_parameters(context, target_key).iter().enumerate() {
        let Some(argument) = target_arguments.get(parameter_index) else {
            continue;
        };
        bindings.push(BranchParameterBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.clone(),
            expression: resolve_branch_expression_handle(
                *argument,
                branch_bindings,
                expression_table,
            ),
        });
    }

    bindings
}

fn leaf_branch_bindings<'a>(
    branch_bindings: &'a BranchParameterBindings,
    context: &'a RuntimeBranchingContext,
    leaf_key: StateKey,
    expression_table: &'a mut ExpressionTable,
    output_bindings: &mut Arena<RuntimeLeafBranchBinding>,
    leaf_arguments: &'a [ExpressionHandle],
) -> HandleSpan<RuntimeLeafBranchBinding> {
    output_bindings.insert_many(
        branch_bindings
            .iter()
            .map(|binding| RuntimeLeafBranchBinding {
                parameter_symbol: binding.parameter_symbol,
                parameter_name: binding.parameter_name.clone(),
                expression: binding.expression,
                kind: RuntimeLeafBranchBindingKind::BranchParameter,
            })
            .chain(leaf_argument_bindings(
                branch_bindings,
                context,
                leaf_key,
                expression_table,
                leaf_arguments,
            )),
    )
}

fn leaf_argument_bindings<'a>(
    branch_bindings: &'a BranchParameterBindings,
    context: &'a RuntimeBranchingContext,
    leaf_key: StateKey,
    expression_table: &'a mut ExpressionTable,
    leaf_arguments: &'a [ExpressionHandle],
) -> impl Iterator<Item = RuntimeLeafBranchBinding> + 'a {
    let leaf_parameters = state_parameters(context, leaf_key);
    leaf_parameters
        .iter()
        .enumerate()
        .filter_map(move |(parameter_index, parameter)| {
            let expression = leaf_arguments.get(parameter_index)?;
            let expression =
                resolve_branch_expression_handle(*expression, branch_bindings, expression_table);
            Some(RuntimeLeafBranchBinding {
                parameter_symbol: parameter.symbol,
                parameter_name: parameter.name.clone(),
                expression,
                kind: RuntimeLeafBranchBindingKind::LeafParameter,
            })
        })
}

fn straight_line_branch_bindings<'a>(
    branch_bindings: &'a BranchParameterBindings,
    context: &'a RuntimeBranchingContext,
    target_key: StateKey,
    expression_table: &'a mut ExpressionTable,
    output_bindings: &mut Arena<RuntimeStraightLineBranchBinding>,
    target_arguments: &'a [ExpressionHandle],
) -> HandleSpan<RuntimeStraightLineBranchBinding> {
    output_bindings.insert_many(
        branch_bindings
            .iter()
            .map(|binding| RuntimeStraightLineBranchBinding {
                parameter_symbol: binding.parameter_symbol,
                parameter_name: binding.parameter_name.clone(),
                expression: binding.expression,
                kind: RuntimeStraightLineBranchBindingKind::BranchParameter,
            })
            .chain(straight_line_argument_bindings(
                branch_bindings,
                context,
                target_key,
                expression_table,
                target_arguments,
            )),
    )
}

fn straight_line_argument_bindings<'a>(
    branch_bindings: &'a BranchParameterBindings,
    context: &'a RuntimeBranchingContext,
    target_key: StateKey,
    expression_table: &'a mut ExpressionTable,
    target_arguments: &'a [ExpressionHandle],
) -> impl Iterator<Item = RuntimeStraightLineBranchBinding> + 'a {
    let target_parameters = state_parameters(context, target_key);
    target_parameters
        .iter()
        .enumerate()
        .filter_map(move |(parameter_index, parameter)| {
            let expression = target_arguments.get(parameter_index)?;
            let expression =
                resolve_branch_expression_handle(*expression, branch_bindings, expression_table);
            Some(RuntimeStraightLineBranchBinding {
                parameter_symbol: parameter.symbol,
                parameter_name: parameter.name.clone(),
                expression,
                kind: RuntimeStraightLineBranchBindingKind::TargetParameter,
            })
        })
}
