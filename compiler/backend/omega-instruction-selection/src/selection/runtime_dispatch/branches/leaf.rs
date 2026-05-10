use crate::InstructionSelectionInput;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_branching::{RuntimeLeafBranchExpansion, RuntimeLeafBranchOperationKind};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::super::super::bindings::resolve_leaf_binding_expression;
use super::super::super::storage_places::{resolve_machine_owned_place, static_integer_value};
use super::super::guards::select_runtime_leaf_branch_guard;
use super::super::text_writes::runtime_text_builder_write_with_resolver;
use super::super::writes::runtime_storage_copy;
use omega_target_program::{SelectedInstruction, SelectedInstructionKind};

pub(in crate::selection::runtime_dispatch) fn select_runtime_leaf_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    selected_instructions: &mut Vec<SelectedInstruction>,
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
        select_runtime_leaf_branch_expansion(input, expansion, selected_instructions);
    }
}

fn select_runtime_leaf_branch_expansion(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if let Some(guard) = select_runtime_leaf_branch_guard(input, expansion) {
        selected_instructions.push(SelectedInstruction {
            kind: guard,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    } else {
        return;
    }

    let write_start = selected_instructions.len();
    select_runtime_leaf_branch_mutation_writes(input, expansion, selected_instructions);
    if selected_instructions.len() == write_start {
        selected_instructions.pop();
    }
}

fn select_runtime_leaf_branch_mutation_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
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

    for operation in operations {
        let RuntimeLeafBranchOperationKind::Mutation { target, value, .. } = &operation.kind else {
            continue;
        };
        let target = input.runtime_branching_calls.expressions.to_tree(*target);
        let value = input.runtime_branching_calls.expressions.to_tree(*value);
        let resolved_target = resolve_leaf_binding_expression(
            &input.runtime_branching_calls.expressions,
            &target,
            bindings,
        );
        let resolved_value = resolve_leaf_binding_expression(
            &input.runtime_branching_calls.expressions,
            &value,
            bindings,
        );

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
        if let Some(instructions) = runtime_text_builder_write_with_resolver(
            input,
            expansion.dispatch_index,
            operation.source_key,
            &operation_machine,
            &operation_state,
            operation.statement_index,
            &resolved_target,
            &|expression| {
                resolve_leaf_binding_expression(
                    &input.runtime_branching_calls.expressions,
                    expression,
                    bindings,
                )
            },
        ) {
            for kind in instructions {
                selected_instructions.push(SelectedInstruction {
                    kind,
                    source_key: operation.source_key,
                    source_statement: operation.statement_index,
                });
            }
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
    let value = static_integer_value(&input.layouts, value_expression)?;

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
