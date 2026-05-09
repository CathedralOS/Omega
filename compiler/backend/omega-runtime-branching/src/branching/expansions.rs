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
use omega_typed_program::expression::Expression;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_leaf_branch_expansions(
    native_plan: &RuntimeBranchingContext,
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
            statement_index,
            branch_key,
            edge_order: edge.order,
            guard: edge.guard.clone(),
            resolved_guard: resolve_branch_guard(&edge.guard, &branch_bindings),
            guard_kind: edge.guard_kind,
            leaf_key: *leaf_key,
            bindings,
            operations,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_straight_line_branch_expansions(
    native_plan: &RuntimeBranchingContext,
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
                statement_index,
                branch_key,
                target_key: *target_key,
                edge_order: edge.order,
                guard: edge.guard.clone(),
                resolved_guard: resolve_branch_guard(&edge.guard, &branch_bindings),
                guard_kind: edge.guard_kind,
                bindings,
                operations,
            });
    }
}

fn leaf_branch_bindings<'a>(
    branch_bindings: &'a [BranchParameterBinding],
    native_plan: &RuntimeBranchingContext,
    leaf_key: StateKey,
    leaf_arguments: &'a [Expression],
) -> impl Iterator<Item = RuntimeLeafBranchBinding> + 'a {
    let branch_parameter_bindings =
        branch_bindings
            .iter()
            .map(|binding| RuntimeLeafBranchBinding {
                parameter_symbol: binding.parameter_symbol,
                parameter_name: binding.parameter_name.clone(),
                expression: binding.expression.clone(),
                kind: RuntimeLeafBranchBindingKind::BranchParameter,
            });

    let leaf_parameters = state_parameters(native_plan, leaf_key);
    let leaf_parameter_bindings =
        leaf_parameters
            .into_iter()
            .enumerate()
            .filter_map(move |(parameter_index, parameter)| {
                let expression = leaf_arguments.get(parameter_index)?;
                Some(RuntimeLeafBranchBinding {
                    parameter_symbol: parameter.symbol,
                    parameter_name: parameter.name,
                    expression: resolve_branch_expression(expression, branch_bindings),
                    kind: RuntimeLeafBranchBindingKind::LeafParameter,
                })
            });

    branch_parameter_bindings.chain(leaf_parameter_bindings)
}

fn straight_line_branch_bindings<'a>(
    branch_bindings: &'a [BranchParameterBinding],
    native_plan: &RuntimeBranchingContext,
    target_key: StateKey,
    target_arguments: &'a [Expression],
) -> impl Iterator<Item = RuntimeStraightLineBranchBinding> + 'a {
    let branch_parameter_bindings =
        branch_bindings
            .iter()
            .map(|binding| RuntimeStraightLineBranchBinding {
                parameter_symbol: binding.parameter_symbol,
                parameter_name: binding.parameter_name.clone(),
                expression: binding.expression.clone(),
                kind: RuntimeStraightLineBranchBindingKind::BranchParameter,
            });

    let target_parameters = state_parameters(native_plan, target_key);
    let target_parameter_bindings = target_parameters.into_iter().enumerate().filter_map(
        move |(parameter_index, parameter)| {
            let expression = target_arguments.get(parameter_index)?;
            Some(RuntimeStraightLineBranchBinding {
                parameter_symbol: parameter.symbol,
                parameter_name: parameter.name,
                expression: resolve_branch_expression(expression, branch_bindings),
                kind: RuntimeStraightLineBranchBindingKind::TargetParameter,
            })
        },
    );

    branch_parameter_bindings.chain(target_parameter_bindings)
}
