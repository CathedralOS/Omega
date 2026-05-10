use super::aliases::{
    BranchParameterBinding, RuntimeBranchAlias, branch_parameter_bindings,
    resolve_branch_expression, resolve_branch_guard,
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
            leaf_arguments,
        );
        let bindings = plan.leaf_bindings.insert_many(bindings);
        let operations = plan.leaf_operations.insert_many(leaf_operations(
            context,
            &mut plan.expressions,
            *leaf_key,
        ));

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
            target_arguments,
        );
        let bindings = plan.straight_line_bindings.insert_many(bindings);
        let operations = plan
            .straight_line_operations
            .insert_many(straight_line_operations(
                context,
                &mut plan.expressions,
                *target_key,
            ));

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
    context: &RuntimeBranchingContext,
    leaf_key: StateKey,
    expression_table: &mut ExpressionTable,
    leaf_arguments: &'a [ExpressionHandle],
) -> Vec<RuntimeLeafBranchBinding> {
    let mut bindings = Vec::new();

    for binding in branch_bindings {
        bindings.push(RuntimeLeafBranchBinding {
            parameter_symbol: binding.parameter_symbol,
            parameter_name: binding.parameter_name.clone(),
            expression: binding.expression,
            kind: RuntimeLeafBranchBindingKind::BranchParameter,
        });
    }

    let leaf_parameters = state_parameters(context, leaf_key);
    for (parameter_index, parameter) in leaf_parameters.into_iter().enumerate() {
        let Some(expression) = leaf_arguments.get(parameter_index) else {
            continue;
        };
        let expression = expression_table.to_tree(*expression);
        let expression = resolve_branch_expression(&expression, branch_bindings, expression_table);
        bindings.push(RuntimeLeafBranchBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.clone(),
            expression: expression_table.insert_tree(&expression),
            kind: RuntimeLeafBranchBindingKind::LeafParameter,
        });
    }

    bindings
}

fn straight_line_branch_bindings<'a>(
    branch_bindings: &'a [BranchParameterBinding],
    context: &RuntimeBranchingContext,
    target_key: StateKey,
    expression_table: &mut ExpressionTable,
    target_arguments: &'a [ExpressionHandle],
) -> Vec<RuntimeStraightLineBranchBinding> {
    let mut bindings = Vec::new();

    for binding in branch_bindings {
        bindings.push(RuntimeStraightLineBranchBinding {
            parameter_symbol: binding.parameter_symbol,
            parameter_name: binding.parameter_name.clone(),
            expression: binding.expression,
            kind: RuntimeStraightLineBranchBindingKind::BranchParameter,
        });
    }

    let target_parameters = state_parameters(context, target_key);
    for (parameter_index, parameter) in target_parameters.into_iter().enumerate() {
        let Some(expression) = target_arguments.get(parameter_index) else {
            continue;
        };
        let expression = expression_table.to_tree(*expression);
        let expression = resolve_branch_expression(&expression, branch_bindings, expression_table);
        bindings.push(RuntimeStraightLineBranchBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.clone(),
            expression: expression_table.insert_tree(&expression),
            kind: RuntimeStraightLineBranchBindingKind::TargetParameter,
        });
    }

    bindings
}
