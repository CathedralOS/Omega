use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::name::ProgramName;
use omega_control_flow::{StateKey, StateParameterFlow};
use omega_core::arena::Arena;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_branching::{
    RuntimeStraightLineBranchBinding, RuntimeStraightLineBranchExpansion,
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
};

use super::super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, resolve_straight_line_binding_expression_handle,
    strip_mutable_expression_handle,
};
use super::super::super::lookups::{
    state_assignment_value_call, state_assignment_value_call_by_ordinal, state_call_for_statement,
    state_mutation_for_statement, state_operations, state_parameters,
    state_transition_argument_call, state_transition_argument_call_by_ordinal,
};
use super::super::guards::select_runtime_straight_line_branch_guard;
use super::super::text_writes::runtime_text_builder_write_in_table_emit;
use super::mutation::{
    select_runtime_resolved_mutation_write,
    select_runtime_resolved_mutation_write_in_table_with_scratch,
};
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::state_bodies::{StateBodyVisitStack, select_state_body_instructions};
use omega_state_calls::{StateCallArgument, StateCallRole};
use omega_target_operations::{InstructionOperand, RuntimeValueOperand};

#[derive(Default)]
pub(in crate::selection::runtime_dispatch) struct StraightLineBranchSelectionScratch {
    expressions: ExpressionTable,
    mutable_expressions: ExpressionTable,
    resolved_segment_expressions: ExpressionTable,
}

pub(in crate::selection::runtime_dispatch) fn select_runtime_straight_line_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    expansion_cursor: &mut usize,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let expansions = input
        .runtime_branching_calls
        .straight_line_expansions
        .storage_slice();

    while let Some(expansion) = expansions.get(*expansion_cursor) {
        if !straight_line_expansion_matches_operation(expansion, dispatch_index, operation) {
            break;
        }

        select_runtime_straight_line_branch_expansion(
            input,
            expansion,
            scratch,
            operands,
            runtime_value_operands,
            selected_instructions,
        );
        *expansion_cursor += 1;
    }
}

fn straight_line_expansion_matches_operation(
    expansion: &RuntimeStraightLineBranchExpansion,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
) -> bool {
    expansion.dispatch_index == dispatch_index
        && expansion.source_key == operation.source_key
        && expansion.statement_index == operation.statement_index
}

fn select_runtime_straight_line_branch_expansion(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let mut emitted_guard = false;
    if let Some(guard) =
        select_runtime_straight_line_branch_guard(input, expansion, runtime_value_operands)
    {
        selected_instructions.push(omega_target_operations::SelectedInstruction {
            kind: guard,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
        emitted_guard = true;
    }
    let write_start = selected_instructions.len();
    select_runtime_straight_line_branch_writes(
        input,
        expansion,
        scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    );
    if emitted_guard && selected_instructions.len() == write_start {
        selected_instructions.pop();
    }
}

fn select_runtime_straight_line_branch_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
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
    scratch.expressions.clear();
    scratch.resolved_segment_expressions.clear();

    for operation in operations {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::Mutation { target, value, .. } => {
                scratch.expressions.clear();
                let expressions = &mut scratch.expressions;
                let target =
                    expressions.copy_from(&input.runtime_branching_calls.expressions, *target);
                let value =
                    expressions.copy_from(&input.runtime_branching_calls.expressions, *value);
                let resolved_target = resolve_straight_line_binding_expression_handle(
                    &input.runtime_branching_calls.expressions,
                    expressions,
                    target,
                    bindings,
                );
                let resolved_value = resolve_straight_line_binding_expression_handle(
                    &input.runtime_branching_calls.expressions,
                    expressions,
                    value,
                    bindings,
                );
                if select_runtime_resolved_mutation_write_in_table_with_scratch(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.source_key,
                    operation.source_key,
                    operation.statement_index,
                    &expressions,
                    resolved_target,
                    resolved_value,
                    &mut scratch.mutable_expressions,
                    &mut scratch.resolved_segment_expressions,
                    runtime_value_operands,
                    selected_instructions,
                ) {
                    continue;
                }
                scratch.resolved_segment_expressions.clear();
                if runtime_text_builder_write_in_table_emit(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.source_key,
                    operation.statement_index,
                    &expressions,
                    resolved_target,
                    &mut scratch.resolved_segment_expressions,
                    &|expressions, expression| {
                        resolve_straight_line_binding_expression_handle(
                            &input.runtime_branching_calls.expressions,
                            expressions,
                            expression,
                            bindings,
                        )
                    },
                    &mut |kind| {
                        selected_instructions.push(omega_target_operations::SelectedInstruction {
                            kind,
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    },
                ) {
                    continue;
                }
                let resolved_target = expressions.to_tree(resolved_target);
                let resolved_value = expressions.to_tree(resolved_value);
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
                    runtime_value_operands,
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
                scratch,
                runtime_value_operands,
                selected_instructions,
            ),
            RuntimeStraightLineBranchOperationKind::StateCall {
                role: StateCallRole::Statement,
                target_key,
                lowering: omega_state_calls::StateCallLowering::InlineExpansion,
                ..
            } => select_runtime_straight_line_inline_state_call(
                input,
                expansion,
                operation.source_key,
                operation.statement_index,
                *target_key,
                bindings,
                operands,
                runtime_value_operands,
                selected_instructions,
            ),
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_inline_state_call(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    source_key: StateKey,
    statement_index: usize,
    target_key: StateKey,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(state_call) = state_call_for_statement(input, source_key, statement_index) else {
        return;
    };
    let Some(arguments) = input.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    let mut child_alias_expressions =
        ExpressionTable::with_expression_capacity(arguments.len().saturating_mul(2));
    let mut child_aliases = RuntimeAliasBuffer::with_capacity(arguments.len());

    for argument in arguments {
        let argument_expression =
            child_alias_expressions.copy_from(&input.state_calls.expressions, argument.expression);
        let resolved_expression = resolve_straight_line_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            &mut child_alias_expressions,
            argument_expression,
            straight_line_bindings,
        );
        let expression =
            strip_mutable_expression_handle(&child_alias_expressions, resolved_expression);
        child_aliases.set_alias(RuntimeAliasBinding {
            source_key: target_key,
            parameter_symbol: argument.parameter_symbol,
            parameter_name: argument.parameter_name.clone(),
            expression_source_key: source_key,
            expression,
        });
    }

    select_state_body_instructions(
        input,
        target_key,
        Some(expansion.dispatch_index),
        &child_aliases,
        &child_alias_expressions,
        operands,
        runtime_value_operands,
        selected_instructions,
        &mut StateBodyVisitStack::with_capacity(input.control_flow.states.len()),
    );
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
    scratch: &mut StraightLineBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let state_call = match role {
        StateCallRole::Statement => {
            state_call_for_statement(input, operation.source_key, operation.statement_index)
        }
        StateCallRole::AssignmentValue => state_assignment_value_call_by_ordinal(
            input,
            operation.source_key,
            operation.statement_index,
            call_ordinal,
        )
        .or_else(|| {
            state_assignment_value_call(input, operation.source_key, operation.statement_index)
        }),
        StateCallRole::TransitionArgument => state_transition_argument_call_by_ordinal(
            input,
            operation.source_key,
            operation.statement_index,
            call_ordinal,
        )
        .or_else(|| {
            state_transition_argument_call(input, operation.source_key, operation.statement_index)
        }),
        _ => None,
    };
    let Some(state_call) = state_call else {
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
    scratch.expressions.clear();
    scratch.resolved_segment_expressions.clear();
    for leaf_operation in operations {
        let Some(mutation) =
            state_mutation_for_statement(input, target_key, leaf_operation.statement_index)
        else {
            continue;
        };

        scratch.expressions.clear();
        let expressions = &mut scratch.expressions;
        let mutation_target =
            expressions.copy_from(&input.state_storage.expressions, mutation.target);
        let mutation_value =
            expressions.copy_from(&input.state_storage.expressions, mutation.value);
        let resolved_target = resolve_leaf_call_expression_handle(
            input,
            expressions,
            target_key,
            mutation_target,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        );
        let resolved_value = resolve_leaf_call_expression_handle(
            input,
            expressions,
            target_key,
            mutation_value,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        );
        if select_runtime_resolved_mutation_write_in_table_with_scratch(
            input,
            expansion.dispatch_index,
            target_key,
            target_key,
            target_key,
            leaf_operation.statement_index,
            &expressions,
            resolved_target,
            resolved_value,
            &mut scratch.mutable_expressions,
            &mut scratch.resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        ) {
            continue;
        }
        scratch.resolved_segment_expressions.clear();
        if runtime_text_builder_write_in_table_emit(
            input,
            expansion.dispatch_index,
            target_key,
            target_key,
            leaf_operation.statement_index,
            &expressions,
            resolved_target,
            &mut scratch.resolved_segment_expressions,
            &|expressions, expression| {
                resolve_leaf_call_expression_handle(
                    input,
                    expressions,
                    target_key,
                    expression,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                )
            },
            &mut |kind| {
                selected_instructions.push(omega_target_operations::SelectedInstruction {
                    kind,
                    source_key: target_key,
                    source_statement: leaf_operation.statement_index,
                });
            },
        ) {
            continue;
        }
        let resolved_target = expressions.to_tree(resolved_target);
        let resolved_value = expressions.to_tree(resolved_value);
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
            runtime_value_operands,
            selected_instructions,
        );
    }
}

fn resolve_leaf_call_expression_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    target_key: StateKey,
    expression: ExpressionHandle,
    leaf_parameters: &[StateParameterFlow],
    arguments: &[StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> ExpressionHandle {
    match table.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => {
            let copied_values = table.reserve_expression_handles(values.count());
            for offset in 0..values.count() {
                let value = table.expression_handle_at_offset(values, offset);
                let resolved = resolve_leaf_call_expression_handle(
                    input,
                    table,
                    target_key,
                    value,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                );
                table.set_expression_handle_at_offset(copied_values, offset, resolved);
            }
            table.insert(ExpressionNode::ArrayLiteral(copied_values))
        }
        ExpressionNode::Binary(binary) => {
            let left = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                binary.left,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            let right = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                binary.right,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            table.insert(ExpressionNode::Binary(
                omega_checked_trees::expression::TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                },
            ))
        }
        ExpressionNode::Cast(cast) => {
            let value = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                cast.value,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            table.insert(ExpressionNode::Cast(
                omega_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                },
            ))
        }
        ExpressionNode::Call(call) => {
            let receiver = call.receiver.is_valid().then(|| {
                resolve_leaf_call_expression_handle(
                    input,
                    table,
                    target_key,
                    call.receiver,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                )
            });
            let copied_arguments = table.reserve_expression_handles(call.arguments.count());
            for offset in 0..call.arguments.count() {
                let argument = table.expression_handle_at_offset(call.arguments, offset);
                let resolved = resolve_leaf_call_expression_handle(
                    input,
                    table,
                    target_key,
                    argument,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                );
                table.set_expression_handle_at_offset(copied_arguments, offset, resolved);
            }
            table.insert(ExpressionNode::Call(
                omega_checked_trees::expression::TableCallExpression {
                    receiver: receiver.unwrap_or_else(ExpressionHandle::invalid),
                    target_symbol: call.target_symbol,
                    target: call.target,
                    arguments: copied_arguments,
                },
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                indexed.collection,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            let index = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                indexed.index,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            table.insert(ExpressionNode::Indexed(
                omega_checked_trees::expression::TableIndexedExpression { collection, index },
            ))
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                member.receiver,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            table.insert(ExpressionNode::Member(
                omega_checked_trees::expression::TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                },
            ))
        }
        ExpressionNode::Mutable(target) => {
            let resolved_target = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                target,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            if matches!(
                table.expression(resolved_target),
                ExpressionNode::Mutable(_)
            ) {
                resolved_target
            } else {
                table.insert(ExpressionNode::Mutable(resolved_target))
            }
        }
        ExpressionNode::Name(path) if path.members.count() > 0 => resolve_leaf_call_name_handle(
            input,
            table,
            target_key,
            &path,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        )
        .unwrap_or(expression),
        ExpressionNode::StructLiteral(struct_literal) => {
            let copied_fields = table.reserve_struct_fields(struct_literal.fields.count());
            for offset in 0..struct_literal.fields.count() {
                let field = table
                    .struct_field_at_offset(struct_literal.fields, offset)
                    .clone();
                let value = resolve_leaf_call_expression_handle(
                    input,
                    table,
                    target_key,
                    field.value,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                );
                table.set_struct_field_at_offset(
                    copied_fields,
                    offset,
                    omega_checked_trees::expression::TableStructLiteralField {
                        name: field.name,
                        value,
                    },
                );
            }
            table.insert(ExpressionNode::StructLiteral(
                omega_checked_trees::expression::TableStructLiteral {
                    type_name: struct_literal.type_name,
                    fields: copied_fields,
                },
            ))
        }
        _ => expression,
    }
}

fn resolve_leaf_call_name_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    target_key: StateKey,
    path: &omega_checked_trees::expression::TableNamePath,
    leaf_parameters: &[StateParameterFlow],
    arguments: &[StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> Option<ExpressionHandle> {
    if let Some(parameter_index) = leaf_parameters.iter().position(|parameter| {
        parameter.symbol.is_valid()
            && path.head_symbol.is_valid()
            && parameter.symbol == path.head_symbol
    }) {
        let argument = arguments.get(parameter_index)?;
        let argument_expression =
            table.copy_from(&input.state_calls.expressions, argument.expression);
        let resolved_argument = resolve_straight_line_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            table,
            argument_expression,
            straight_line_bindings,
        );

        return Some(table.insert_copy_with_member_suffix(
            resolved_argument,
            path.members,
            path.member_symbols,
            1,
        ));
    }

    let initializer = leaf_local_initializer_handle(input, table, target_key, path)?;
    let resolved_initializer = resolve_leaf_call_expression_handle(
        input,
        table,
        target_key,
        initializer,
        leaf_parameters,
        arguments,
        straight_line_bindings,
    );
    Some(table.insert_copy_with_member_suffix(
        resolved_initializer,
        path.members,
        path.member_symbols,
        1,
    ))
}

fn leaf_local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    target_key: StateKey,
    path: &omega_checked_trees::expression::TableNamePath,
) -> Option<ExpressionHandle> {
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == target_key.state)?;
    let statements = input
        .program
        .statement_table
        .statements(state.statement_nodes);
    statements.iter().find_map(|statement| {
        let omega_checked_trees::statement::StatementNode::LocalData(local_data) = statement else {
            return None;
        };
        let matches_symbol = path.head_symbol.is_valid() && local_data.symbol == path.head_symbol;
        (local_data.initial_value.is_valid() && matches_symbol)
            .then(|| table.copy_from(&input.program.expression_table, local_data.initial_value))
    })
}

fn source_machine_name(input: &InstructionSelectionInput<'_>, key: StateKey) -> ProgramName {
    input.control_flow.state_machine_name_by_key_cloned(key)
}
