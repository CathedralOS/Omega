use super::aliases::{
    BranchParameterBindings, RuntimeBranchAliasBuffer, branch_parameter_bindings,
    resolve_branch_expression_handle, resolve_branch_guard_handle,
};
use super::lookups::state_parameters;
use super::operations::{leaf_operations, prelude_operations, straight_line_operations};
use super::{
    RuntimeBranchPreludeBinding, RuntimeBranchPreludeExpansion, RuntimeBranchPreludeOperation,
    RuntimeBranchTargetLowering, RuntimeBranchingCallEdge, RuntimeLeafBranchBinding,
    RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion, RuntimeLeafBranchOperation,
    RuntimeStraightLineBranchBinding, RuntimeStraightLineBranchBindingKind,
    RuntimeStraightLineBranchExpansion, RuntimeStraightLineBranchOperation,
};
use crate::RuntimeBranchingContext;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_state_calls::StateCall;
use omega_state_graph::RuntimeTransitionTarget;

pub(super) fn append_branch_prelude_expansion(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    prelude_expansions: &mut Arena<RuntimeBranchPreludeExpansion>,
    prelude_bindings: &mut Arena<RuntimeBranchPreludeBinding>,
    prelude_operations_arena: &mut Arena<RuntimeBranchPreludeOperation>,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    state_call: &StateCall,
    aliases: &RuntimeBranchAliasBuffer,
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
    );

    prelude_expansions.insert(RuntimeBranchPreludeExpansion {
        dispatch_index,
        source_key,
        statement_index,
        branch_key,
        target_key: state_call.target_key,
        bindings,
        operations,
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_leaf_branch_expansions(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    target_arguments: &Arena<ExpressionHandle>,
    leaf_expansions: &mut Arena<RuntimeLeafBranchExpansion>,
    leaf_bindings: &mut Arena<RuntimeLeafBranchBinding>,
    leaf_operations_arena: &mut Arena<RuntimeLeafBranchOperation>,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    aliases: &RuntimeBranchAliasBuffer,
) {
    let branch_bindings = branch_parameter_bindings(context, state_call, aliases, expressions);

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
                HandleSpan::empty(),
            ),
            _ => continue,
        };

        leaf_expansions.insert(RuntimeLeafBranchExpansion {
            dispatch_index,
            source_key,
            statement_index,
            branch_key,
            edge_order: edge.order,
            guard: edge.guard,
            resolved_guard: resolve_branch_guard_handle(edge.guard, &branch_bindings, expressions),
            guard_kind: edge.guard_kind,
            role: state_call.role,
            leaf_key,
            target_value: edge.target_value,
            bindings,
            operations,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_straight_line_branch_expansions(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    target_arguments: &Arena<ExpressionHandle>,
    straight_line_expansions: &mut Arena<RuntimeStraightLineBranchExpansion>,
    straight_line_bindings_arena: &mut Arena<RuntimeStraightLineBranchBinding>,
    straight_line_operations_arena: &mut Arena<RuntimeStraightLineBranchOperation>,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    aliases: &RuntimeBranchAliasBuffer,
) {
    let branch_bindings = branch_parameter_bindings(context, state_call, aliases, expressions);

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

        let target_arguments = target_arguments.span_or_empty(edge.target_arguments);
        let bindings = straight_line_branch_bindings(
            &branch_bindings,
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

        straight_line_expansions.insert(RuntimeStraightLineBranchExpansion {
            dispatch_index,
            source_key,
            statement_index,
            branch_key,
            target_key: *target_key,
            edge_order: edge.order,
            guard: edge.guard,
            resolved_guard: resolve_branch_guard_handle(edge.guard, &branch_bindings, expressions),
            guard_kind: edge.guard_kind,
            bindings,
            operations,
        });
    }
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
