use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_branching::{
    RuntimeStraightLineBranchBinding, RuntimeStraightLineBranchExpansion,
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
};
use omega_typed_program::expression::{Expression, NamePath};
use omega_typed_program::name::ProgramName;

use super::super::super::bindings::{
    append_place_suffix, resolve_straight_line_binding_expression,
};
use super::super::super::lookups::{
    state_call_for_statement, state_mutation_for_statement, state_operations, state_parameters,
};
use super::mutation::select_runtime_resolved_mutation_write;
use crate::selection::instruction_sink::SelectedInstructionSink;

pub(in crate::selection::runtime_dispatch) fn select_runtime_straight_line_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    selected_instructions: &mut SelectedInstructionSink,
) {
    for (_, expansion) in input
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == dispatch_index
                && expansion.source_key == operation.source_key
                && expansion.statement_index == operation.statement_index
        })
    {
        select_runtime_straight_line_branch_expansion(input, expansion, selected_instructions);
    }
}

fn select_runtime_straight_line_branch_expansion(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if expansion.resolved_guard != omega_typed_program::statement::TransitionGuard::Always {
        return;
    }

    select_runtime_straight_line_branch_writes(input, expansion, selected_instructions);
}

fn select_runtime_straight_line_branch_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(operations) = input
        .runtime_branching_calls
        .straight_line_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .straight_line_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    for operation in operations {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::Mutation { target, value, .. } => {
                let target = input.runtime_branching_calls.expressions.to_tree(*target);
                let value = input.runtime_branching_calls.expressions.to_tree(*value);
                let resolved_target = resolve_straight_line_binding_expression(
                    &input.runtime_branching_calls.expressions,
                    &target,
                    bindings,
                );
                let resolved_value = resolve_straight_line_binding_expression(
                    &input.runtime_branching_calls.expressions,
                    &value,
                    bindings,
                );
                let (operation_machine, operation_state) = state_names(input, operation.source_key);
                select_runtime_resolved_mutation_write(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    &source_machine_name(input, expansion.source_key),
                    &operation_machine,
                    &operation_state,
                    operation.statement_index,
                    &resolved_target,
                    &resolved_value,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                target_key,
                lowering: omega_state_calls::StateCallLowering::InlineLeaf,
                ..
            } => select_runtime_straight_line_leaf_state_call_writes(
                input,
                expansion,
                operation,
                bindings,
                *target_key,
                selected_instructions,
            ),
            _ => {}
        }
    }
}

fn state_names(input: &InstructionSelectionInput<'_>, key: StateKey) -> (ProgramName, ProgramName) {
    input.control_flow.state_names_by_key_cloned(key)
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_leaf_state_call_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &RuntimeStraightLineBranchOperation,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    target_key: StateKey,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(state_call) =
        state_call_for_statement(input, operation.source_key, operation.statement_index)
    else {
        return;
    };
    let Some(arguments) = input.state_calls.arguments.span(state_call.arguments) else {
        return;
    };
    let leaf_parameters = state_parameters(input, target_key);

    let Some(operations) = state_operations(input, target_key) else {
        return;
    };
    let (target_machine, target_state) = state_names(input, target_key);
    for leaf_operation in operations {
        let Some(mutation) =
            state_mutation_for_statement(input, target_key, leaf_operation.statement_index)
        else {
            continue;
        };
        let mutation_target = input.state_storage.expressions.to_tree(mutation.target);
        let mutation_value = input.state_storage.expressions.to_tree(mutation.value);
        let resolved_target = resolve_leaf_call_expression(
            input,
            &mutation_target,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        );
        let resolved_value = resolve_leaf_call_expression(
            input,
            &mutation_value,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        );
        select_runtime_resolved_mutation_write(
            input,
            expansion.dispatch_index,
            target_key,
            &source_machine_name(input, expansion.source_key),
            &target_machine,
            &target_state,
            leaf_operation.statement_index,
            &resolved_target,
            &resolved_value,
            selected_instructions,
        );
    }
}

fn resolve_leaf_call_expression(
    input: &InstructionSelectionInput<'_>,
    expression: &Expression,
    leaf_parameters: &[omega_control_flow::StateParameterFlow],
    arguments: &[omega_state_calls::StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_leaf_call_expression(
                input,
                target,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => resolve_leaf_call_name(
            input,
            path,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        )
        .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

fn resolve_leaf_call_name(
    input: &InstructionSelectionInput<'_>,
    path: &NamePath,
    leaf_parameters: &[omega_control_flow::StateParameterFlow],
    arguments: &[omega_state_calls::StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> Option<Expression> {
    let parameter_index = leaf_parameters.iter().position(|parameter| {
        parameter.symbol.is_valid()
            && path.head_symbol().is_valid()
            && parameter.symbol == path.head_symbol()
    })?;
    let argument = arguments.get(parameter_index)?;
    let argument_expression = input.state_calls.expressions.to_tree(argument.expression);
    let resolved_argument = resolve_straight_line_binding_expression(
        &input.runtime_branching_calls.expressions,
        &argument_expression,
        straight_line_bindings,
    );

    Some(append_place_suffix(&resolved_argument, &path[1..]))
}

fn source_machine_name(input: &InstructionSelectionInput<'_>, key: StateKey) -> ProgramName {
    input.control_flow.state_machine_name_by_key_cloned(key)
}
