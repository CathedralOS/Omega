use super::aliases::{
    BranchParameterBinding, RuntimeBranchAlias, branch_parameter_bindings,
    resolve_branch_expression_handle, resolve_branch_guard,
};
use super::lookups::state_parameters;
use super::operations::{leaf_operations, straight_line_operations};
use super::{
    RuntimeBranchTargetLowering, RuntimeBranchingCallEdge, RuntimeBranchingCallPlan,
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion,
    RuntimeStraightLineBranchBinding, RuntimeStraightLineBranchBindingKind,
    RuntimeStraightLineBranchExpansion,
};
use crate::RuntimeBranchingContext;
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_state_calls::StateCall;
use omega_state_graph::RuntimeTransitionTarget;
use omega_typed_program::expression::{ExpressionHandle, ExpressionTable};

#[allow(clippy::too_many_arguments)]
pub(super) fn append_leaf_branch_expansions(
    context: &RuntimeBranchingContext,
    plan: &mut RuntimeBranchingCallPlan,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    aliases: &[RuntimeBranchAlias],
) {
    for edge in edges {
        if edge.lowering != RuntimeBranchTargetLowering::InlineLeaf {
            continue;
        }

        let RuntimeTransitionTarget::State { key: leaf_key, .. } = &edge.target else {
            continue;
        };

        let branch_bindings =
            branch_parameter_bindings(context, state_call, aliases, &mut plan.expressions);
        let leaf_arguments = plan.target_arguments.span_or_empty(edge.target_arguments);
        let bindings = leaf_branch_bindings(
            &branch_bindings,
            context,
            *leaf_key,
            &mut plan.expressions,
            &mut plan.leaf_bindings,
            leaf_arguments,
        );
        let operations = leaf_operations(
            context,
            &mut plan.expressions,
            &mut plan.leaf_operations,
            *leaf_key,
        );

        plan.leaf_expansions.insert(RuntimeLeafBranchExpansion {
            dispatch_index,
            source_key,
            statement_index,
            branch_key,
            edge_order: edge.order,
            guard: edge.guard.clone(),
            resolved_guard: resolve_branch_guard(&edge.guard, &branch_bindings, &plan.expressions),
            guard_kind: edge.guard_kind,
            leaf_key: *leaf_key,
            bindings,
            operations,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_straight_line_branch_expansions(
    context: &RuntimeBranchingContext,
    plan: &mut RuntimeBranchingCallPlan,
    source_key: StateKey,
    branch_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    edges: &[RuntimeBranchingCallEdge],
    state_call: &StateCall,
    aliases: &[RuntimeBranchAlias],
) {
    for edge in edges {
        if edge.lowering != RuntimeBranchTargetLowering::InlineStraightLine {
            continue;
        }

        let RuntimeTransitionTarget::State {
            key: target_key, ..
        } = &edge.target
        else {
            continue;
        };

        let branch_bindings =
            branch_parameter_bindings(context, state_call, aliases, &mut plan.expressions);
        let target_arguments = plan.target_arguments.span_or_empty(edge.target_arguments);
        let bindings = straight_line_branch_bindings(
            &branch_bindings,
            context,
            *target_key,
            &mut plan.expressions,
            &mut plan.straight_line_bindings,
            target_arguments,
        );
        let operations = straight_line_operations(
            context,
            &mut plan.expressions,
            &mut plan.straight_line_operations,
            *target_key,
        );

        plan.straight_line_expansions
            .insert(RuntimeStraightLineBranchExpansion {
                dispatch_index,
                source_key,
                statement_index,
                branch_key,
                target_key: *target_key,
                edge_order: edge.order,
                guard: edge.guard.clone(),
                resolved_guard: resolve_branch_guard(
                    &edge.guard,
                    &branch_bindings,
                    &plan.expressions,
                ),
                guard_kind: edge.guard_kind,
                bindings,
                operations,
            });
    }
}

fn leaf_branch_bindings<'a>(
    branch_bindings: &'a [BranchParameterBinding],
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
    branch_bindings: &'a [BranchParameterBinding],
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
    branch_bindings: &'a [BranchParameterBinding],
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
    branch_bindings: &'a [BranchParameterBinding],
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
