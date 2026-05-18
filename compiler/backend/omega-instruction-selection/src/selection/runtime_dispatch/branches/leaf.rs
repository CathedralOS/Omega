use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{Expression, ExpressionTable};
use omega_checked_trees::name::ProgramName;
use omega_core::arena::Arena;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_branching::{RuntimeLeafBranchExpansion, RuntimeLeafBranchOperationKind};
use omega_state_calls::StateCallRole;

use super::super::super::bindings::resolve_leaf_binding_expression_handle;
use super::super::super::storage_places::{
    resolve_machine_owned_place, resolve_machine_owned_place_in_table, static_integer_value,
    static_integer_value_in_table,
};
use super::super::guards::select_runtime_leaf_branch_guard;
use super::super::text_writes::{
    runtime_text_builder_write_in_table_emit, runtime_text_builder_write_with_handle_resolver_emit,
};
use super::super::writes::{
    RuntimeStaticValues, runtime_frame_slot_target_expression, runtime_storage_copy,
    select_runtime_frame_slot_value_write_in_table,
};
use super::mutation::{
    select_runtime_resolved_mutation_write, select_runtime_resolved_mutation_write_in_table,
};
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_target_operations::{RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind};

fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
}

pub(in crate::selection::runtime_dispatch) fn select_runtime_leaf_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    for (_, expansion) in
        input
            .runtime_branching_calls
            .leaf_expansions
            .iter()
            .filter(|(_, expansion)| {
                expansion.dispatch_index == dispatch_index
                    && expansion.source_key == operation.source_key
                    && expansion.statement_index == operation.statement_index
            })
    {
        select_runtime_leaf_branch_expansion(
            input,
            expansion,
            runtime_value_operands,
            selected_instructions,
        );
    }
}

fn select_runtime_leaf_branch_expansion(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if let Some(guard) = select_runtime_leaf_branch_guard(input, expansion, runtime_value_operands)
    {
        selected_instructions.push(SelectedInstruction {
            kind: guard,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    } else {
        return;
    }

    let write_start = selected_instructions.len();
    select_runtime_leaf_branch_terminal_value_write(
        input,
        expansion,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_leaf_branch_mutation_writes(
        input,
        expansion,
        runtime_value_operands,
        selected_instructions,
    );
    if selected_instructions.len() == write_start {
        selected_instructions.pop();
    }
}

fn select_runtime_leaf_branch_terminal_value_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if !expansion.target_value.is_valid() {
        return;
    }
    let value = expansion.target_value;
    if !matches!(
        expansion.role,
        StateCallRole::AssignmentValue
            | StateCallRole::TransitionArgument
            | StateCallRole::TransitionGuard
    ) {
        return;
    }
    let Some(slot) = input.runtime_storage.call_result_slot(
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expansion.role,
    ) else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    let mut expressions = ExpressionTable::new();
    let value = expressions.copy_from(&input.runtime_branching_calls.expressions, value);
    let resolved_value = resolve_leaf_binding_expression_handle(
        &input.runtime_branching_calls.expressions,
        &mut expressions,
        value,
        bindings,
    );
    let static_values = RuntimeStaticValues::default();
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        &expressions,
        slot,
        resolved_value,
        &static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
        return;
    }

    let target = runtime_frame_slot_target_expression(&mut expressions, slot);
    if select_runtime_resolved_mutation_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.source_key,
        expansion.source_key,
        expansion.statement_index,
        &expressions,
        target,
        resolved_value,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }

    let target = expressions.to_tree(target);
    let resolved_value = expressions.to_tree(resolved_value);
    let (source_machine, source_state) = state_names(input, expansion.source_key);
    select_runtime_resolved_mutation_write(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &source_machine,
        &source_machine,
        &source_state,
        expansion.statement_index,
        &target,
        &resolved_value,
        runtime_value_operands,
        selected_instructions,
    );
}

fn select_runtime_leaf_branch_mutation_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(operations) = input
        .runtime_branching_calls
        .leaf_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    let mut expressions = ExpressionTable::new();
    let mut resolved_segment_expressions = ExpressionTable::new();
    let mut fallback_segment_expressions = ExpressionTable::new();

    for operation in operations {
        let RuntimeLeafBranchOperationKind::Mutation { target, value, .. } = &operation.kind else {
            continue;
        };
        expressions.clear();
        let target = expressions.copy_from(&input.runtime_branching_calls.expressions, *target);
        let value = expressions.copy_from(&input.runtime_branching_calls.expressions, *value);
        let resolved_target = resolve_leaf_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            &mut expressions,
            target,
            bindings,
        );
        let resolved_value = resolve_leaf_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            &mut expressions,
            value,
            bindings,
        );
        if let Some((byte_offset, byte_size, value)) = runtime_leaf_machine_integer_write_in_table(
            input,
            expansion,
            &expressions,
            resolved_target,
            resolved_value,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                    byte_offset,
                    byte_size,
                    value,
                },
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
            continue;
        }

        if select_runtime_resolved_mutation_write_in_table(
            input,
            expansion.dispatch_index,
            operation.source_key,
            operation.source_key,
            operation.source_key,
            operation.statement_index,
            &expressions,
            resolved_target,
            resolved_value,
            runtime_value_operands,
            selected_instructions,
        ) {
            continue;
        }

        resolved_segment_expressions.clear();
        if runtime_text_builder_write_in_table_emit(
            input,
            expansion.dispatch_index,
            operation.source_key,
            operation.source_key,
            operation.statement_index,
            &expressions,
            resolved_target,
            &mut resolved_segment_expressions,
            &|expressions, expression| {
                resolve_leaf_binding_expression_handle(
                    &input.runtime_branching_calls.expressions,
                    expressions,
                    expression,
                    bindings,
                )
            },
            &mut |kind| {
                selected_instructions.push(SelectedInstruction {
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
        if let Some((byte_offset, byte_size, value)) =
            runtime_leaf_machine_integer_write(input, expansion, &resolved_target, &resolved_value)
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                    byte_offset,
                    byte_size,
                    value,
                },
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
            continue;
        }

        let (operation_machine, operation_state) = state_names(input, operation.source_key);
        fallback_segment_expressions.clear();
        if runtime_text_builder_write_with_handle_resolver_emit(
            input,
            expansion.dispatch_index,
            operation.source_key,
            &operation_machine,
            &operation_state,
            operation.statement_index,
            &resolved_target,
            &mut fallback_segment_expressions,
            &|expressions, expression| {
                resolve_leaf_binding_expression_handle(
                    &input.runtime_branching_calls.expressions,
                    expressions,
                    expression,
                    bindings,
                )
            },
            &mut |kind| {
                selected_instructions.push(SelectedInstruction {
                    kind,
                    source_key: operation.source_key,
                    source_statement: operation.statement_index,
                });
            },
        ) {
            continue;
        }

        if let Some(copy) = runtime_leaf_storage_copy(
            input,
            expansion,
            &operation_machine,
            &operation_state,
            &resolved_target,
            &resolved_value,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: copy,
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
        }
    }
}

fn state_names(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> (ProgramName, ProgramName) {
    input.control_flow.state_names_by_key_cloned(key)
}

fn runtime_leaf_machine_integer_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    target: &Expression,
    value_expression: &Expression,
) -> Option<(usize, usize, i64)> {
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &input.layouts,
        input.entry_key.machine,
        expansion.source_key.machine,
        target,
    )?;
    if !supports_scalar_integer_write(byte_size) {
        return None;
    }
    let value = static_integer_value(&input.layouts, value_expression)?;

    Some((byte_offset, byte_size, value))
}

fn runtime_leaf_machine_integer_write_in_table(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    expressions: &ExpressionTable,
    target: omega_checked_trees::expression::ExpressionHandle,
    value_expression: omega_checked_trees::expression::ExpressionHandle,
) -> Option<(usize, usize, i64)> {
    let (byte_offset, byte_size) = resolve_machine_owned_place_in_table(
        &input.layouts,
        input.entry_key.machine,
        expansion.source_key.machine,
        expressions,
        target,
    )?;
    if !supports_scalar_integer_write(byte_size) {
        return None;
    }
    let value = static_integer_value_in_table(&input.layouts, expressions, value_expression)?;
    Some((byte_offset, byte_size, value))
}

fn runtime_leaf_storage_copy(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    operation_machine: &str,
    operation_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    runtime_storage_copy(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.source_key,
        operation_machine,
        operation_state,
        target,
        value,
    )
}
