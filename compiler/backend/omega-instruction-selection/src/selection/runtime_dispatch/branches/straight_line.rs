use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_branching::{
    RuntimeStraightLineBranchBinding, RuntimeStraightLineBranchExpansion,
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
};
use omega_checked_trees::expression::{Expression, NamePath};
use omega_checked_trees::name::ProgramName;

use super::super::super::bindings::{
    append_place_suffix, resolve_straight_line_binding_expression,
};
use super::super::super::lookups::{
    state_assignment_value_call, state_assignment_value_call_by_ordinal,
    state_call_for_statement, state_mutation_for_statement, state_operations, state_parameters,
};
use super::super::guards::select_runtime_straight_line_branch_guard;
use super::mutation::select_runtime_resolved_mutation_write;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_state_calls::StateCallRole;

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
    let mut emitted_guard = false;
    if let Some(guard) = select_runtime_straight_line_branch_guard(input, expansion) {
        selected_instructions.push(omega_target_operations::SelectedInstruction {
            kind: guard,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
        emitted_guard = true;
    }
    let write_start = selected_instructions.len();
    select_runtime_straight_line_branch_writes(input, expansion, selected_instructions);
    if emitted_guard && selected_instructions.len() == write_start {
        selected_instructions.pop();
    }
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
                role,
                call_ordinal,
                target_key,
                lowering: omega_state_calls::StateCallLowering::InlineLeaf,
                ..
            } => select_runtime_straight_line_leaf_state_call_writes(
                input,
                expansion,
                operation,
                *role,
                *call_ordinal,
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
    role: StateCallRole,
    call_ordinal: usize,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    target_key: StateKey,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let state_call = match role {
        StateCallRole::Statement => {
            state_call_for_statement(input, operation.source_key, operation.statement_index)
        }
        StateCallRole::AssignmentValue => {
            state_assignment_value_call_by_ordinal(
                input,
                operation.source_key,
                operation.statement_index,
                call_ordinal,
            ).or_else(|| {
                state_assignment_value_call(input, operation.source_key, operation.statement_index)
            })
        }
        _ => None,
    };
    let Some(state_call) = state_call
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
            target_key,
            &mutation_target,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        );
        let resolved_value = resolve_leaf_call_expression(
            input,
            target_key,
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
    target_key: StateKey,
    expression: &Expression,
    leaf_parameters: &[omega_control_flow::StateParameterFlow],
    arguments: &[omega_state_calls::StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_leaf_call_expression(
                input,
                target_key,
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
        Expression::Binary(binary) => Expression::Binary(Box::new(
            omega_checked_trees::expression::BinaryExpression {
                left: resolve_leaf_call_expression(
                    input,
                    target_key,
                    &binary.left,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                ),
                operator: binary.operator,
                right: resolve_leaf_call_expression(
                    input,
                    target_key,
                    &binary.right,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                ),
            },
        )),
        Expression::Call(call) => Expression::Call(Box::new(
            omega_checked_trees::expression::CallExpression {
                receiver: call.receiver.as_ref().map(|receiver| {
                    Box::new(resolve_leaf_call_expression(
                        input,
                        target_key,
                        receiver,
                        leaf_parameters,
                        arguments,
                        straight_line_bindings,
                    ))
                }),
                target_symbol: call.target_symbol,
                target: call.target.clone(),
                arguments: call
                    .arguments
                    .iter()
                    .map(|argument| {
                        resolve_leaf_call_expression(
                            input,
                            target_key,
                            argument,
                            leaf_parameters,
                            arguments,
                            straight_line_bindings,
                        )
                    })
                    .collect(),
            },
        )),
        Expression::Member(member) => Expression::Member(Box::new(
            omega_checked_trees::expression::MemberExpression {
                receiver: resolve_leaf_call_expression(
                    input,
                    target_key,
                    &member.receiver,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                ),
                member_symbol: member.member_symbol,
                member: member.member.clone(),
            },
        )),
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(
            omega_checked_trees::expression::IndexedExpression {
                collection: resolve_leaf_call_expression(
                    input,
                    target_key,
                    &indexed.collection,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                ),
                index: resolve_leaf_call_expression(
                    input,
                    target_key,
                    &indexed.index,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                ),
            },
        )),
        Expression::StructLiteral(struct_literal) => Expression::StructLiteral(
            omega_checked_trees::expression::StructLiteral {
                type_name: struct_literal.type_name.clone(),
                fields: struct_literal
                    .fields
                    .iter()
                    .map(|field| omega_checked_trees::expression::StructLiteralField {
                        name: field.name.clone(),
                        value: resolve_leaf_call_expression(
                            input,
                            target_key,
                            &field.value,
                            leaf_parameters,
                            arguments,
                            straight_line_bindings,
                        ),
                    })
                    .collect(),
            },
        ),
        Expression::Name(path) if !path.is_empty() => resolve_leaf_call_name(
            input,
            target_key,
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
    target_key: StateKey,
    path: &NamePath,
    leaf_parameters: &[omega_control_flow::StateParameterFlow],
    arguments: &[omega_state_calls::StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> Option<Expression> {
    if let Some(parameter_index) = leaf_parameters.iter().position(|parameter| {
        parameter.symbol.is_valid()
            && path.head_symbol().is_valid()
            && parameter.symbol == path.head_symbol()
    }) {
        let argument = arguments.get(parameter_index)?;
        let argument_expression = input.state_calls.expressions.to_tree(argument.expression);
        let resolved_argument = resolve_straight_line_binding_expression(
            &input.runtime_branching_calls.expressions,
            &argument_expression,
            straight_line_bindings,
        );

        return Some(append_place_suffix(&resolved_argument, &path[1..]));
    }

    let initializer = leaf_local_initializer(input, target_key, path)?;
    let resolved_initializer = resolve_leaf_call_expression(
        input,
        target_key,
        &initializer,
        leaf_parameters,
        arguments,
        straight_line_bindings,
    );
    Some(append_place_suffix(&resolved_initializer, &path[1..]))
}

fn leaf_local_initializer(
    input: &InstructionSelectionInput<'_>,
    target_key: StateKey,
    path: &NamePath,
) -> Option<Expression> {
    let machine = input
        .program
        .machines
        .iter()
        .find(|machine| machine.symbol == target_key.machine)?;
    let state = machine
        .states
        .iter()
        .find(|state| state.symbol == target_key.state)?;
    let statements = input.program.statement_table.statements(state.statement_nodes);
    statements.iter().find_map(|statement| {
        let omega_checked_trees::statement::StatementNode::LocalData(local_data) = statement else {
            return None;
        };
        let matches_symbol =
            path.head_symbol().is_valid() && local_data.symbol == path.head_symbol();
        let matches_name = path
            .first()
            .is_some_and(|name| local_data.name.as_str() == name.as_str());
        (local_data.initial_value.is_valid() && (matches_symbol || matches_name))
            .then(|| input.program.expression_table.to_tree(local_data.initial_value))
    })
}

fn source_machine_name(input: &InstructionSelectionInput<'_>, key: StateKey) -> ProgramName {
    input.control_flow.state_machine_name_by_key_cloned(key)
}
