use crate::control_flow::StateKey;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation;
use crate::runtime_dispatch::branching::{
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion,
    RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchExpansion, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};
use omega_typed_program::expression::Expression;

use super::super::bindings::{
    resolve_leaf_binding_expression, resolve_straight_line_binding_expression,
};
use super::super::lookups::{
    state_call_for_statement, state_mutation_for_statement, state_operations, state_parameters,
};
use super::super::model::{SelectedInstruction, SelectedInstructionKind};
use super::super::storage_places::{resolve_machine_owned_place, static_integer_value};
use super::guards::select_runtime_leaf_branch_guard;
use super::writes::{runtime_storage_copy, runtime_text_builder_write_with_resolver};

pub(super) fn select_runtime_leaf_branch_expansions_for_operation(
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
            source_machine: expansion.source_machine.clone(),
            source_state: expansion.source_state.clone(),
            source_statement: expansion.statement_index,
        });
    } else {
        return;
    }
    selected_instructions.extend(mutation_writes);
}

pub(super) fn select_runtime_straight_line_branch_expansions_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, expansion) in native_plan
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == dispatch_index
                && expansion.source_key == operation.source_key
                && expansion.statement_index == operation.statement_index
        })
    {
        select_runtime_straight_line_branch_expansion(
            native_plan,
            expansion,
            selected_instructions,
        );
    }
}

fn select_runtime_straight_line_branch_expansion(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if expansion.resolved_guard != omega_typed_program::statement::TransitionGuard::Always {
        return;
    }

    select_runtime_straight_line_branch_writes(native_plan, expansion, selected_instructions);
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
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
                source_statement: operation.statement_index,
            });
            continue;
        }

        if let Some(instructions) = runtime_text_builder_write_with_resolver(
            native_plan,
            expansion.dispatch_index,
            operation.source_key,
            &operation.source_machine,
            &operation.source_state,
            operation.statement_index,
            &resolved_target,
            &|expression| resolve_leaf_binding_expression(expression, bindings),
        ) {
            for kind in instructions {
                selected_instructions.push(SelectedInstruction {
                    kind,
                    source_machine: operation.source_machine.clone(),
                    source_state: operation.source_state.clone(),
                    source_statement: operation.statement_index,
                });
            }
            continue;
        }

        if let Some(copy) = runtime_leaf_storage_copy(
            native_plan,
            expansion,
            &operation.source_machine,
            &operation.source_state,
            &resolved_target,
            &resolved_value,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: copy,
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
                source_statement: operation.statement_index,
            });
        }
    }
}

fn select_runtime_straight_line_branch_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(operations) = native_plan
        .runtime_branching_calls
        .straight_line_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = native_plan
        .runtime_branching_calls
        .straight_line_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    for operation in operations {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::Mutation { target, value, .. } => {
                let resolved_target = resolve_straight_line_binding_expression(target, bindings);
                let resolved_value = resolve_straight_line_binding_expression(value, bindings);
                select_runtime_resolved_mutation_write(
                    native_plan,
                    expansion.dispatch_index,
                    operation.source_key,
                    &expansion.source_machine,
                    &operation.source_machine,
                    &operation.source_state,
                    operation.statement_index,
                    &resolved_target,
                    &resolved_value,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                target_key,
                target_machine,
                target_state,
                lowering: crate::state_calls::StateCallLowering::InlineLeaf,
                ..
            } => select_runtime_straight_line_leaf_state_call_writes(
                native_plan,
                expansion,
                operation,
                bindings,
                *target_key,
                target_machine,
                target_state,
                selected_instructions,
            ),
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_leaf_state_call_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &RuntimeStraightLineBranchOperation,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    target_key: StateKey,
    target_machine: &str,
    target_state: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(state_call) =
        state_call_for_statement(native_plan, operation.source_key, operation.statement_index)
    else {
        return;
    };
    let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
        return;
    };
    let leaf_parameters = state_parameters(native_plan, target_key);
    let leaf_bindings = leaf_parameters
        .iter()
        .enumerate()
        .filter_map(|(parameter_index, parameter_name)| {
            let argument = arguments.get(parameter_index)?;
            Some(RuntimeLeafBranchBinding {
                parameter_name: parameter_name.clone(),
                expression: resolve_straight_line_binding_expression(
                    &argument.expression,
                    straight_line_bindings,
                ),
                kind: RuntimeLeafBranchBindingKind::LeafParameter,
            })
        })
        .collect::<Vec<_>>();

    let Some(operations) = state_operations(native_plan, target_key) else {
        return;
    };
    for leaf_operation in operations {
        let Some(mutation) =
            state_mutation_for_statement(native_plan, target_key, leaf_operation.statement_index)
        else {
            continue;
        };
        let resolved_target = resolve_leaf_binding_expression(&mutation.target, &leaf_bindings);
        let resolved_value = resolve_leaf_binding_expression(&mutation.value, &leaf_bindings);
        select_runtime_resolved_mutation_write(
            native_plan,
            expansion.dispatch_index,
            target_key,
            &expansion.source_machine,
            target_machine,
            target_state,
            leaf_operation.statement_index,
            &resolved_target,
            &resolved_value,
            selected_instructions,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_mutation_write(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation_key: StateKey,
    source_machine: &str,
    operation_machine: &str,
    operation_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolved_value: &Expression,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        resolved_target,
    ) && let Some(value) = static_integer_value(&native_plan.layouts, resolved_value)
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
                value,
            },
            source_machine: operation_machine.to_owned().into(),
            source_state: operation_state.to_owned().into(),
            source_statement: statement_index,
        });
        return;
    }

    if let Some(copy) = runtime_storage_copy(
        native_plan,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        resolved_target,
        resolved_value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_machine: operation_machine.to_owned().into(),
            source_state: operation_state.to_owned().into(),
            source_statement: statement_index,
        });
    }
}

fn runtime_leaf_machine_integer_write(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    target: &Expression,
    value_expression: &Expression,
) -> Option<(usize, usize, i64)> {
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        &expansion.source_machine,
        target,
    )?;
    let value = static_integer_value(&native_plan.layouts, value_expression)?;

    Some((byte_offset, byte_size, value))
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
