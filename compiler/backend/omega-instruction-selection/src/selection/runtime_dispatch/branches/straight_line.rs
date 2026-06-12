use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::statement::StatementNode;
use omega_control_flow::{OperationKind, StateKey, StateParameterFlow};
use omega_core::arena::Arena;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_runtime_branching::{
    RuntimeStraightLineBranchBinding, RuntimeStraightLineBranchBindingKind,
    RuntimeStraightLineBranchExpansion, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};

use super::super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, RuntimeAliasResolutionContext,
    resolve_straight_line_binding_expression_handle, strip_mutable_expression_handle,
};
use super::super::super::lookups::{
    host_call_for_statement, state_assignment_value_call, state_assignment_value_call_by_ordinal,
    state_call_for_statement, state_mutation_for_statement, state_operations, state_parameters,
    state_transition_argument_call, state_transition_argument_call_by_ordinal,
};
use super::super::super::storage_places::{
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
};
use super::super::guards::select_runtime_straight_line_branch_guards;
use super::super::text_writes::runtime_text_builder_write_in_table_emit;
use super::super::writes::{
    RuntimeStaticValues, emit_runtime_frame_slot_slice_descriptor_write_in_table,
    runtime_frame_slot_target_expression, select_runtime_frame_slot_value_write_in_table,
};
use super::mutation::{
    select_runtime_resolved_mutation_write_in_table_with_scratch,
};
use super::prelude::{BranchPreludeSelectionScratch, select_runtime_branch_preludes_for_operation};
use crate::selection::host_operations::select_host_call;
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::state_bodies::{StateBodyVisitStack, select_state_body_instructions};
use omega_abstract_operations::{
    InstructionOperand, RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction,
    SelectedInstructionKind, StateGuardOperator,
};
use omega_state_calls::{StateCallArgument, StateCallLowering, StateCallRole};
use omega_state_guards::StateGuardLowering;

#[derive(Default)]
pub(in crate::selection) struct StraightLineBranchSelectionScratch {
    expressions: ExpressionTable,
    mutable_expressions: ExpressionTable,
    resolved_segment_expressions: ExpressionTable,
}

pub(in crate::selection) fn select_runtime_straight_line_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    _expansion_cursor: &mut usize,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let expansions = input
        .runtime_branching_calls
        .straight_line_expansions
        .storage_slice();

    for expansion in expansions {
        if straight_line_expansion_matches_operation(expansion, dispatch_index, operation) {
            select_runtime_straight_line_branch_expansion(
                input,
                expansion,
                scratch,
                operands,
                runtime_value_operands,
                selected_instructions,
            );
        }
    }
}

fn straight_line_expansion_matches_operation(
    expansion: &RuntimeStraightLineBranchExpansion,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
) -> bool {
    expansion.dispatch_index == dispatch_index
        && super::super::state_key_matches_statement_source(
            expansion.source_key,
            operation.source_key,
        )
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
    let guards =
        select_runtime_straight_line_branch_guards(input, expansion, runtime_value_operands);
    let emitted_guard = !guards.is_empty();
    let guard_start = selected_instructions.len();
    for guard in guards {
        selected_instructions.push(omega_abstract_operations::SelectedInstruction {
            kind: guard,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
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
    select_runtime_straight_line_branch_terminal_value_write(
        input,
        expansion,
        scratch,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_straight_line_assignment_value_target_copy(
        input,
        expansion,
        selected_instructions,
    );
    if emitted_guard && selected_instructions.len() == write_start {
        while selected_instructions.len() > guard_start {
            selected_instructions.pop();
        }
    } else if emitted_guard {
        selected_instructions.push(omega_abstract_operations::SelectedInstruction {
            kind: omega_abstract_operations::SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::NoOp,
                operator: StateGuardOperator::Equal,
                storage_region: RuntimeStorageRegion::Machine,
                byte_offset: 0,
                byte_size: 0,
                expected_value: 0,
                has_storage: false,
                is_float: false,
            },
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    }
}

fn select_runtime_straight_line_branch_terminal_value_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    scratch: &mut StraightLineBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if !expansion.target_value.is_valid() {
        return;
    }
    if !matches!(
        expansion.role,
        StateCallRole::AssignmentValue
            | StateCallRole::CallArgument
            | StateCallRole::TransitionArgument
            | StateCallRole::TransitionGuard
    ) {
        return;
    }
    let Some(slot) = input.runtime_storage.call_result_slot_by_ordinal(
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expansion.role,
        expansion.call_ordinal,
    ) else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .straight_line_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    scratch.expressions.clear();
    let expressions = &mut scratch.expressions;
    let value = expressions.copy_from(
        &input.runtime_branching_calls.expressions,
        expansion.target_value,
    );
    let resolved_value = resolve_straight_line_binding_expression_handle(
        &input.runtime_branching_calls.expressions,
        expressions,
        value,
        bindings,
    );
    let static_values = RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expressions,
        slot,
        resolved_value,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expressions,
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

    let target = runtime_frame_slot_target_expression(expressions, slot);
    if select_runtime_resolved_mutation_write_in_table_with_scratch(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.source_key,
        expansion.source_key,
        expansion.statement_index,
        expressions,
        target,
        resolved_value,
        &mut scratch.mutable_expressions,
        &mut scratch.resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }

    // Non-table mutation-write fallback removed (Phase 4): proven dead emitter — the
    // `_in_table` path above handles every case that lowers.
}

fn select_runtime_straight_line_assignment_value_target_copy(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if expansion.role != StateCallRole::AssignmentValue || !expansion.target_value.is_valid() {
        return;
    }
    let Some(source_slot) = input.runtime_storage.call_result_slot_by_ordinal(
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expansion.role,
        expansion.call_ordinal,
    ) else {
        return;
    };
    let Some(mutation) =
        state_mutation_for_statement(input, expansion.source_key, expansion.statement_index)
    else {
        return;
    };
    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.state_storage.expressions,
        mutation.target,
    ) && source_slot.byte_size == pointer_target.pointee_byte_size
        && source_slot.byte_size > 0
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                source_region: RuntimeStorageRegion::RuntimeFrame,
                source_offset: source_slot.byte_offset,
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_count: source_slot.byte_size,
            },
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
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

    // Statements whose assignment-value InlineBranching call this loop already
    // emitted. `straight_line_operations` lists a StateCall operation AND a
    // LocalData operation for `let x = self.f(...)`; the StateCall arm runs the
    // callee and copies its result into the local, so the LocalData arm must not
    // emit the call AGAIN (the callee's side effects ran twice per evaluation --
    // one doubling per nesting level of the dungeon's RNG chain).
    let mut emitted_assignment_calls: Vec<(StateKey, usize)> = Vec::new();

    for operation in operations {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::LocalData => {
                if emitted_assignment_calls
                    .contains(&(operation.source_key, operation.statement_index))
                {
                    continue;
                }
                select_runtime_straight_line_local_initializer_write(
                    input,
                    expansion,
                    operation,
                    bindings,
                    scratch,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::HostCall => {
                let Some(host_call) =
                    host_call_for_statement(input, operation.source_key, operation.statement_index)
                else {
                    continue;
                };
                let alias_bindings = straight_line_alias_bindings(expansion, bindings);
                let alias_context = (!alias_bindings.bindings().is_empty()).then_some(
                    RuntimeAliasResolutionContext {
                        aliases: alias_bindings.bindings(),
                        alias_expressions: &input.runtime_branching_calls.expressions,
                    },
                );
                select_host_call(
                    input,
                    host_call,
                    Some(expansion.dispatch_index),
                    alias_context,
                    operands,
                    selected_instructions,
                );
            }
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
                        selected_instructions.push(
                            omega_abstract_operations::SelectedInstruction {
                                kind,
                                source_key: operation.source_key,
                                source_statement: operation.statement_index,
                            },
                        );
                    },
                ) {
                    continue;
                }
                // Non-table mutation-write fallback removed (Phase 4): proven dead
                // emitter — the `_in_table` write + text-builder paths above cover
                // every case that lowers.
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                role,
                call_ordinal,
                target_key,
                lowering: StateCallLowering::InlineLeaf,
                ..
            } => {
                select_runtime_straight_line_leaf_state_call_writes(
                    input,
                    expansion,
                    operation,
                    *role,
                    *call_ordinal,
                    bindings,
                    *target_key,
                    scratch,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
                select_assignment_value_call_result_local_copy(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    *role,
                    *call_ordinal,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                role,
                call_ordinal,
                target_key,
                lowering: StateCallLowering::InlineExpansion,
                ..
            } => {
                select_runtime_straight_line_inline_state_call(
                    input,
                    expansion,
                    operation.source_key,
                    operation.statement_index,
                    *role,
                    *call_ordinal,
                    *target_key,
                    bindings,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
                select_assignment_value_call_result_local_copy(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    *role,
                    *call_ordinal,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                role,
                call_ordinal,
                lowering: StateCallLowering::InlineBranching,
                ..
            } => {
                select_runtime_straight_line_nested_branch_expansions_for_operation(
                    input,
                    expansion.dispatch_index,
                    operation,
                    scratch,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
                select_assignment_value_call_result_local_copy(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    *role,
                    *call_ordinal,
                    selected_instructions,
                );
                if *role == StateCallRole::AssignmentValue {
                    emitted_assignment_calls
                        .push((operation.source_key, operation.statement_index));
                }
            }
            _ => {}
        }
    }
}

fn select_runtime_straight_line_local_initializer_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &RuntimeStraightLineBranchOperation,
    bindings: &[RuntimeStraightLineBranchBinding],
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == expansion.dispatch_index
                && slot.source_key == operation.source_key
                && slot.statement_index == operation.statement_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ))
            .then_some(slot)
        })
    else {
        return;
    };
    if select_runtime_straight_line_assignment_value_local_initializer_call(
        input,
        expansion,
        operation,
        scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    let Some(initializer) = local_initializer_handle(
        input,
        &mut scratch.expressions,
        operation.source_key,
        operation.statement_index,
    ) else {
        return;
    };
    let expressions = &mut scratch.expressions;
    let resolved_initializer = resolve_straight_line_binding_expression_handle(
        &input.runtime_branching_calls.expressions,
        expressions,
        initializer,
        bindings,
    );
    let static_values = RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        operation.statement_index,
        expressions,
        slot,
        resolved_initializer,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        operation.statement_index,
        expressions,
        slot,
        resolved_initializer,
        &static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation.source_key,
            source_statement: operation.statement_index,
        });
        return;
    }

    let target = runtime_frame_slot_target_expression(expressions, slot);
    scratch.resolved_segment_expressions.clear();
    if runtime_text_builder_write_in_table_emit(
        input,
        expansion.dispatch_index,
        operation.source_key,
        expansion.source_key,
        operation.statement_index,
        expressions,
        target,
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
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
        },
    ) {}
}

fn select_runtime_straight_line_assignment_value_local_initializer_call(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &RuntimeStraightLineBranchOperation,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(state_call) =
        state_assignment_value_call(input, operation.source_key, operation.statement_index)
    else {
        return false;
    };
    if state_call.lowering != StateCallLowering::InlineBranching {
        return false;
    }

    let branch_operation = RuntimeStraightLineBranchOperation {
        source_key: operation.source_key,
        statement_index: operation.statement_index,
        kind: RuntimeStraightLineBranchOperationKind::StateCall {
            role: StateCallRole::AssignmentValue,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    };
    let body_operation = RuntimeDispatchBodyOperation {
        source_key: operation.source_key,
        statement_index: operation.statement_index,
        kind: RuntimeDispatchBodyOperationKind::StateCall {
            role: StateCallRole::AssignmentValue,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    };

    let before = selected_instructions.len();
    let mut prelude_cursor = 0usize;
    let mut prelude_scratch = BranchPreludeSelectionScratch::default();
    select_runtime_branch_preludes_for_operation(
        input,
        expansion.dispatch_index,
        &body_operation,
        &mut prelude_cursor,
        &mut prelude_scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_straight_line_nested_branch_expansions_for_operation(
        input,
        expansion.dispatch_index,
        &branch_operation,
        scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    );
    if selected_instructions.len() == before {
        return false;
    }

    select_assignment_value_call_result_local_copy(
        input,
        expansion.dispatch_index,
        operation.source_key,
        operation.statement_index,
        StateCallRole::AssignmentValue,
        state_call.call_ordinal,
        selected_instructions,
    );
    true
}

fn local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
) -> Option<ExpressionHandle> {
    table.clear();
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)?;
    let statement = input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let StatementNode::LocalData(local_data) = statement else {
        return None;
    };
    local_data
        .initial_value
        .is_valid()
        .then(|| table.copy_from(&input.program.expression_table, local_data.initial_value))
}

pub(in crate::selection) fn select_runtime_straight_line_nested_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeStraightLineBranchOperation,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let body_operation = RuntimeDispatchBodyOperation {
        source_key: operation.source_key,
        statement_index: operation.statement_index,
        kind: omega_runtime_bodies::RuntimeDispatchBodyOperationKind::Other,
    };
    let expansions = input
        .runtime_branching_calls
        .straight_line_expansions
        .storage_slice();
    for expansion in expansions {
        if straight_line_expansion_matches_operation(expansion, dispatch_index, &body_operation) {
            select_runtime_straight_line_branch_expansion(
                input,
                expansion,
                scratch,
                operands,
                runtime_value_operands,
                selected_instructions,
            );
        }
    }

    let mut leaf_scratch = super::LeafBranchSelectionScratch::default();
    super::select_runtime_leaf_branch_expansions_matching_operation(
        input,
        dispatch_index,
        &body_operation,
        &mut leaf_scratch,
        runtime_value_operands,
        selected_instructions,
    );
}

pub(in crate::selection) fn select_assignment_value_call_result_local_copy(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if role != StateCallRole::AssignmentValue {
        return;
    }
    let Some(source_slot) = input.runtime_storage.call_result_slot_by_ordinal(
        dispatch_index,
        source_key,
        statement_index,
        role,
        call_ordinal,
    ) else {
        return;
    };
    let Some(target_slot) = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && slot.statement_index == statement_index
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
        source_key,
        source_statement: statement_index,
    });
}

fn straight_line_alias_bindings(
    expansion: &RuntimeStraightLineBranchExpansion,
    bindings: &[RuntimeStraightLineBranchBinding],
) -> RuntimeAliasBuffer {
    RuntimeAliasBuffer::from_iter(bindings.iter().map(|binding| {
        let source_key = match binding.kind {
            RuntimeStraightLineBranchBindingKind::BranchParameter => expansion.branch_key,
            RuntimeStraightLineBranchBindingKind::TargetParameter => expansion.target_key,
        };
        RuntimeAliasBinding {
            source_key,
            parameter_symbol: binding.parameter_symbol,
            parameter_name: binding.parameter_name.clone(),
            expression_source_key: expansion.source_key,
            expression: binding.expression,
        }
    }))
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_inline_state_call(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    target_key: StateKey,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let state_call = match role {
        StateCallRole::Statement => state_call_for_statement(input, source_key, statement_index),
        StateCallRole::AssignmentValue => {
            state_assignment_value_call_by_ordinal(input, source_key, statement_index, call_ordinal)
                .or_else(|| state_assignment_value_call(input, source_key, statement_index))
        }
        StateCallRole::CallArgument => {
            super::super::super::lookups::state_call_argument_call_by_ordinal(
                input,
                source_key,
                statement_index,
                call_ordinal,
            )
        }
        StateCallRole::TransitionArgument => state_transition_argument_call_by_ordinal(
            input,
            source_key,
            statement_index,
            call_ordinal,
        )
        .or_else(|| state_transition_argument_call(input, source_key, statement_index)),
        _ => None,
    };
    let Some(state_call) = state_call else { return };
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
    operands: &mut Arena<InstructionOperand>,
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
        StateCallRole::CallArgument => {
            super::super::super::lookups::state_call_argument_call_by_ordinal(
                input,
                operation.source_key,
                operation.statement_index,
                call_ordinal,
            )
        }
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
    let (child_aliases, child_alias_expressions) = leaf_call_alias_bindings(
        input,
        operation.source_key,
        target_key,
        arguments,
        straight_line_bindings,
    );
    scratch.expressions.clear();
    scratch.resolved_segment_expressions.clear();
    for leaf_operation in operations {
        if matches!(leaf_operation.kind, OperationKind::Call { .. }) {
            let Some(host_call) =
                host_call_for_statement(input, target_key, leaf_operation.statement_index)
            else {
                continue;
            };
            let alias_context =
                (!child_aliases.bindings().is_empty()).then_some(RuntimeAliasResolutionContext {
                    aliases: child_aliases.bindings(),
                    alias_expressions: &child_alias_expressions,
                });
            select_host_call(
                input,
                host_call,
                Some(expansion.dispatch_index),
                alias_context,
                operands,
                selected_instructions,
            );
            continue;
        }

        if matches!(leaf_operation.kind, OperationKind::LocalData) {
            select_runtime_leaf_state_call_local_initializer_write(
                input,
                expansion,
                target_key,
                leaf_operation.statement_index,
                leaf_parameters,
                arguments,
                straight_line_bindings,
                scratch,
                runtime_value_operands,
                selected_instructions,
            );
            continue;
        }

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
                selected_instructions.push(omega_abstract_operations::SelectedInstruction {
                    kind,
                    source_key: target_key,
                    source_statement: leaf_operation.statement_index,
                });
            },
        ) {
            continue;
        }
        // Non-table mutation-write fallback removed (Phase 4): proven dead emitter —
        // the `_in_table` write + text-builder paths above cover every lowering case.
    }
}

fn leaf_call_alias_bindings(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    target_key: StateKey,
    arguments: &[StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> (RuntimeAliasBuffer, ExpressionTable) {
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

    (child_aliases, child_alias_expressions)
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_leaf_state_call_local_initializer_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    target_key: StateKey,
    statement_index: usize,
    leaf_parameters: &[StateParameterFlow],
    arguments: &[StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    scratch: &mut StraightLineBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == expansion.dispatch_index
                && slot.source_key == target_key
                && slot.statement_index == statement_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ))
            .then_some(slot)
        })
    else {
        return;
    };

    let Some(initializer) =
        local_initializer_handle(input, &mut scratch.expressions, target_key, statement_index)
    else {
        return;
    };
    let resolved_initializer = resolve_leaf_call_expression_handle(
        input,
        &mut scratch.expressions,
        target_key,
        initializer,
        leaf_parameters,
        arguments,
        straight_line_bindings,
    );
    let static_values = RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        statement_index,
        &scratch.expressions,
        slot,
        resolved_initializer,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        statement_index,
        &scratch.expressions,
        slot,
        resolved_initializer,
        &static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: target_key,
            source_statement: statement_index,
        });
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
                    case_name: struct_literal.case_name,
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

