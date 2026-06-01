use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{Expression, ExpressionTable};
use omega_checked_trees::name::Identifier;
use omega_core::arena::Arena;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use omega_runtime_branching::{RuntimeLeafBranchExpansion, RuntimeLeafBranchOperationKind};
use omega_state_calls::StateCallRole;

use super::super::super::bindings::resolve_leaf_binding_expression_handle;
use super::super::super::lookups::state_mutation_for_statement;
use super::super::super::storage_places::resolve_runtime_storage_place_in_table;
use super::super::super::storage_places::{
    resolve_machine_owned_place, resolve_machine_owned_place_in_table,
    resolve_runtime_pointee_slot_offset_in_table, static_integer_value, static_integer_value_in_table,
};
use super::super::guards::select_runtime_leaf_branch_guards;
use super::super::text_writes::{
    runtime_text_builder_write_in_table_emit, runtime_text_builder_write_with_handle_resolver_emit,
};
use super::super::writes::{
    RuntimeStaticValues, emit_runtime_frame_slot_slice_descriptor_write_in_table,
    runtime_frame_slot_target_expression, runtime_storage_copy,
    runtime_storage_fixed_indexed_source_copy, select_runtime_frame_slot_value_write_in_table,
};
use super::mutation::{
    select_runtime_resolved_mutation_write,
    select_runtime_resolved_mutation_write_in_table_with_scratch,
};
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
};
use omega_runtime_dispatch_loop::RuntimeDispatchLoopAction;

fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 4 | 8)
}

#[derive(Default)]
pub(in crate::selection::runtime_dispatch) struct LeafBranchSelectionScratch {
    expressions: ExpressionTable,
    mutable_expressions: ExpressionTable,
    resolved_segment_expressions: ExpressionTable,
    fallback_segment_expressions: ExpressionTable,
}

pub(in crate::selection::runtime_dispatch) fn select_runtime_leaf_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    _expansion_cursor: &mut usize,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let mut matching_expansions = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            leaf_expansion_matches_operation(expansion, dispatch_index, operation, false)
        })
        .collect::<Vec<_>>();

    order_return_value_fallbacks_first(&mut matching_expansions);

    for expansion in matching_expansions {
        select_runtime_leaf_branch_expansion(
            input,
            expansion,
            scratch,
            runtime_value_operands,
            selected_instructions,
        );
    }
}

pub(in crate::selection::runtime_dispatch) fn select_runtime_leaf_branch_expansions_matching_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let mut matching_expansions = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            leaf_expansion_matches_operation(expansion, dispatch_index, operation, true)
        })
        .collect::<Vec<_>>();

    order_return_value_fallbacks_first(&mut matching_expansions);

    for expansion in matching_expansions {
        select_runtime_leaf_branch_expansion(
            input,
            expansion,
            scratch,
            runtime_value_operands,
            selected_instructions,
        );
    }
}

fn leaf_expansion_matches_operation(
    expansion: &RuntimeLeafBranchExpansion,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    allow_synthetic_nested_operation: bool,
) -> bool {
    expansion.dispatch_index == dispatch_index
        && expansion.source_key == operation.source_key
        && expansion.statement_index == operation.statement_index
        && (matches!(
            operation.kind,
            RuntimeDispatchBodyOperationKind::StateCall { .. }
        ) || (allow_synthetic_nested_operation
            && matches!(operation.kind, RuntimeDispatchBodyOperationKind::Other)))
}

fn order_return_value_fallbacks_first(expansions: &mut [&RuntimeLeafBranchExpansion]) {
    expansions.sort_by_key(|expansion| {
        (
            !(expansion.target_value.is_valid() && expansion.is_default_target),
            expansion.edge_order,
        )
    });
}

fn select_runtime_leaf_branch_expansion(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let guards = select_runtime_leaf_branch_guards(input, expansion, runtime_value_operands);
    if guards.is_empty() && expansion.guard_kind != omega_state_guards::StateGuardKind::Always {
        return;
    }
    let guards_were_empty = guards.is_empty();
    let guard_start = selected_instructions.len();
    for guard in guards {
        selected_instructions.push(SelectedInstruction {
            kind: guard,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    }

    let write_start = selected_instructions.len();
    select_runtime_leaf_branch_terminal_value_write(
        input,
        expansion,
        scratch,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_leaf_assignment_value_target_copy(input, expansion, selected_instructions);
    select_runtime_leaf_assignment_value_local_copy(input, expansion, selected_instructions);
    select_runtime_leaf_branch_mutation_writes(
        input,
        expansion,
        scratch,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_leaf_branch_completion_dispatch(input, expansion, selected_instructions);
    if selected_instructions.len() == write_start {
        while selected_instructions.len() > guard_start {
            selected_instructions.pop();
        }
    } else if !guards_were_empty {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: omega_abstract_operations::StateGuardLowering::NoOp,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
                storage_region: RuntimeStorageRegion::Machine,
                byte_offset: 0,
                byte_size: 0,
                expected_value: 0,
                has_storage: false,
            },
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    }
}

fn select_runtime_leaf_branch_completion_dispatch(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if expansion.role != StateCallRole::Statement || expansion.target_value.is_valid() {
        return;
    }
    if !leaf_state_is_empty(input, expansion.leaf_key) {
        return;
    }
    let Some(dispatch_index) = source_completion_dispatch_index(input, expansion) else {
        return;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::SetDispatchState { dispatch_index },
        source_key: expansion.source_key,
        source_statement: expansion.statement_index,
    });
}

fn leaf_state_is_empty(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> bool {
    let Some(state) = input.control_flow.state_by_key(key) else {
        return false;
    };
    input
        .control_flow
        .operations
        .span(state.operations)
        .map_or(true, <[_]>::is_empty)
        && input
            .control_flow
            .transitions
            .span(state.transitions)
            .map_or(true, <[_]>::is_empty)
}

fn source_completion_dispatch_index(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<u32> {
    let case = input
        .runtime_dispatch_loop
        .cases
        .iter()
        .find_map(|(_, case)| (case.key == expansion.source_key).then_some(case))?;
    let edges = input.runtime_dispatch_loop.edges.span(case.edges)?;
    let mut targets = edges.iter().filter_map(|edge| match edge.action {
        RuntimeDispatchLoopAction::EnterState | RuntimeDispatchLoopAction::Terminate => {
            Some(edge.target_dispatch_index)
        }
        RuntimeDispatchLoopAction::Unknown => None,
    });
    let first = targets.next()?;
    targets.next().is_none().then_some(first)
}

fn select_runtime_leaf_branch_terminal_value_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    scratch: &mut LeafBranchSelectionScratch,
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
    scratch.expressions.clear();
    let expressions = &mut scratch.expressions;
    let value = expressions.copy_from(&input.runtime_branching_calls.expressions, value);
    let resolved_value = resolve_leaf_binding_expression_handle(
        &input.runtime_branching_calls.expressions,
        expressions,
        value,
        bindings,
    );
    let static_values = RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.branch_key,
        expansion.target_statement_index,
        &expressions,
        slot,
        resolved_value,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.branch_key,
        expansion.target_statement_index,
        &expressions,
        slot,
        resolved_value,
        &static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: expansion.branch_key,
            source_statement: expansion.target_statement_index,
        });
        return;
    }

    let target = runtime_frame_slot_target_expression(expressions, slot);
    if select_runtime_resolved_mutation_write_in_table_with_scratch(
        input,
        expansion.dispatch_index,
        expansion.branch_key,
        expansion.source_key,
        expansion.branch_key,
        expansion.target_statement_index,
        &expressions,
        target,
        resolved_value,
        &mut scratch.mutable_expressions,
        &mut scratch.resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }

    let target = expressions.to_tree(target);
    let resolved_value = expressions.to_tree(resolved_value);
    let (source_machine, source_state) = state_names(input, expansion.branch_key);
    select_runtime_resolved_mutation_write(
        input,
        expansion.dispatch_index,
        expansion.branch_key,
        &source_machine,
        &source_machine,
        &source_state,
        expansion.target_statement_index,
        &target,
        &resolved_value,
        runtime_value_operands,
        selected_instructions,
    );
}

fn select_runtime_leaf_assignment_value_target_copy(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if expansion.role != StateCallRole::AssignmentValue || !expansion.target_value.is_valid() {
        return;
    }
    let Some(source_slot) = input.runtime_storage.call_result_slot(
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expansion.role,
    ) else {
        return;
    };
    let Some(mutation) =
        state_mutation_for_statement(input, expansion.source_key, expansion.statement_index)
    else {
        return;
    };
    // A pointee target (a field reached through a reference, e.g. `room.label`
    // where `room: &mut Room`) is written through the pointer by the statement's
    // own mutation operation. The flat result-slot copy below resolves such a
    // target to the reference slot itself, so it would clobber the pointer rather
    // than the pointee -- skip it and let the mutation write through the pointer.
    if resolve_runtime_pointee_slot_offset_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.state_storage.expressions,
        mutation.target,
    )
    .is_some()
    {
        return;
    }
    let Some(target_place) = resolve_runtime_storage_place_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.state_storage.expressions,
        mutation.target,
    ) else {
        return;
    };
    if source_slot.byte_size != target_place.byte_count || source_slot.byte_size == 0 {
        return;
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::CopyRuntimeStorage {
            source_region: RuntimeStorageRegion::RuntimeFrame,
            source_offset: source_slot.byte_offset,
            target_region: target_place.region,
            target_offset: target_place.byte_offset,
            byte_count: source_slot.byte_size,
        },
        source_key: expansion.source_key,
        source_statement: expansion.statement_index,
    });
}

fn select_runtime_leaf_assignment_value_local_copy(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if expansion.role != StateCallRole::AssignmentValue || !expansion.target_value.is_valid() {
        return;
    }
    let Some(source_slot) = input.runtime_storage.call_result_slot(
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expansion.role,
    ) else {
        return;
    };
    let Some(target_slot) = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == expansion.dispatch_index
                && slot.source_key == expansion.source_key
                && slot.statement_index == expansion.statement_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ))
            .then_some(slot)
        })
    else {
        return;
    };
    if source_slot.byte_size != target_slot.byte_size || source_slot.byte_size == 0 {
        return;
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::CopyRuntimeStorage {
            source_region: RuntimeStorageRegion::RuntimeFrame,
            source_offset: source_slot.byte_offset,
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: target_slot.byte_offset,
            byte_count: source_slot.byte_size,
        },
        source_key: expansion.source_key,
        source_statement: expansion.statement_index,
    });
}

fn select_runtime_leaf_branch_mutation_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    scratch: &mut LeafBranchSelectionScratch,
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
    scratch.expressions.clear();
    scratch.resolved_segment_expressions.clear();
    scratch.fallback_segment_expressions.clear();

    for operation in operations {
        let RuntimeLeafBranchOperationKind::Mutation { target, value, .. } = &operation.kind else {
            continue;
        };
        scratch.expressions.clear();
        let expressions = &mut scratch.expressions;
        let target = expressions.copy_from(&input.runtime_branching_calls.expressions, *target);
        let value = expressions.copy_from(&input.runtime_branching_calls.expressions, *value);
        let resolved_target = resolve_leaf_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            expressions,
            target,
            bindings,
        );
        let resolved_value = resolve_leaf_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            expressions,
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

        if select_runtime_resolved_mutation_write_in_table_with_scratch(
            input,
            expansion.dispatch_index,
            operation.source_key,
            expansion.source_key,
            expansion.source_key,
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
            expansion.source_key,
            operation.statement_index,
            &expressions,
            resolved_target,
            &mut scratch.resolved_segment_expressions,
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
        scratch.fallback_segment_expressions.clear();
        if runtime_text_builder_write_with_handle_resolver_emit(
            input,
            expansion.dispatch_index,
            operation.source_key,
            &operation_machine,
            &operation_state,
            operation.statement_index,
            &resolved_target,
            &mut scratch.fallback_segment_expressions,
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
) -> (Identifier, Identifier) {
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
    runtime_storage_fixed_indexed_source_copy(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.source_key,
        operation_machine,
        operation_state,
        target,
        value,
    )
    .or_else(|| {
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
    })
}
