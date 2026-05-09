use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation;
use crate::runtime_dispatch::branching::{
    RuntimeLeafBranchExpansion, RuntimeLeafBranchOperationKind,
};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::super::super::bindings::resolve_leaf_binding_expression;
use super::super::super::model::{SelectedInstruction, SelectedInstructionKind};
use super::super::super::storage_places::{resolve_machine_owned_place, static_integer_value};
use super::super::guards::select_runtime_leaf_branch_guard;
use super::super::text_writes::runtime_text_builder_write_with_resolver;
use super::super::writes::runtime_storage_copy;

pub(in crate::instructions::runtime_dispatch) fn select_runtime_leaf_branch_expansions_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, expansion) in native_plan
        .runtime_branching_calls
        .leaf_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == dispatch_index
                && expansion.source_key == operation.source_key
                && expansion.statement_index == operation.statement_index
        })
    {
        select_runtime_leaf_branch_expansion(native_plan, expansion, selected_instructions);
    }
}

fn select_runtime_leaf_branch_expansion(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let mut mutation_writes = Vec::new();
    select_runtime_leaf_branch_mutation_writes(native_plan, expansion, &mut mutation_writes);
    if mutation_writes.is_empty() {
        return;
    }

    if let Some(guard) = select_runtime_leaf_branch_guard(native_plan, expansion) {
        selected_instructions.push(SelectedInstruction {
            kind: guard,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    } else {
        return;
    }
    selected_instructions.extend(mutation_writes);
}

fn select_runtime_leaf_branch_mutation_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(operations) = native_plan
        .runtime_branching_calls
        .leaf_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = native_plan
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    for operation in operations {
        let RuntimeLeafBranchOperationKind::Mutation { target, value, .. } = &operation.kind else {
            continue;
        };
        let resolved_target = resolve_leaf_binding_expression(target, bindings);
        let resolved_value = resolve_leaf_binding_expression(value, bindings);

        if let Some((byte_offset, byte_size, value)) = runtime_leaf_machine_integer_write(
            native_plan,
            expansion,
            &resolved_target,
            &resolved_value,
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

        let (operation_machine, operation_state) = state_names(native_plan, operation.source_key);
        if let Some(instructions) = runtime_text_builder_write_with_resolver(
            native_plan,
            expansion.dispatch_index,
            operation.source_key,
            &operation_machine,
            &operation_state,
            operation.statement_index,
            &resolved_target,
            &|expression| resolve_leaf_binding_expression(expression, bindings),
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
            native_plan,
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
    native_plan: &NativePlan,
    key: crate::control_flow::StateKey,
) -> (ProgramName, ProgramName) {
    native_plan.control_flow.state_names_by_key_cloned(key)
}

fn runtime_leaf_machine_integer_write(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    target: &Expression,
    value_expression: &Expression,
) -> Option<(usize, usize, i64)> {
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &native_plan.layouts,
        native_plan.entry_machine_name(),
        &source_machine_name(native_plan, expansion.source_key),
        target,
    )?;
    let value = static_integer_value(&native_plan.layouts, value_expression)?;

    Some((byte_offset, byte_size, value))
}

fn source_machine_name(
    native_plan: &NativePlan,
    key: crate::control_flow::StateKey,
) -> ProgramName {
    native_plan
        .control_flow
        .state_machine_name_by_key_cloned(key)
}

fn runtime_leaf_storage_copy(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    operation_machine: &str,
    operation_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    runtime_storage_copy(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
        operation_machine,
        operation_state,
        target,
        value,
    )
}
