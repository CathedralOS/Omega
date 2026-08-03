use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, RuntimeAliasResolutionContext,
    resolve_branch_prelude_binding_expression_handle, strip_mutable_expression_handle,
};
use crate::selection::host_operations::select_host_call;
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::state_bodies::{StateBodyVisitStack, select_state_body_instructions};
use omega_abstract_operations::{InstructionOperand, RuntimeValueOperand};
use omega_control_flow::StateKey;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_branching::{
    RuntimeBranchPreludeBinding, RuntimeBranchPreludeExpansion, RuntimeBranchPreludeOperationKind,
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
};
use psi_arena::Arena;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use psi_checked_trees::statement::StatementNode;

use super::super::super::lookups::{host_call_for_statement, state_call_for_statement};
use super::super::text_writes::runtime_text_builder_write_in_table_emit;
use super::super::writes::{
    RuntimeStaticValues, emit_runtime_frame_slot_slice_descriptor_write_in_table,
    runtime_frame_slot_target_expression, select_runtime_frame_slot_value_write_in_table,
};
use super::mutation::select_runtime_resolved_mutation_write_in_table_with_scratch;
use super::straight_line::{
    StraightLineBranchSelectionScratch, select_assignment_value_call_result_local_copy,
    select_runtime_straight_line_nested_branch_expansions_for_operation,
};
use omega_abstract_operations::{SelectedInstruction, SelectedInstructionKind};

#[derive(Default)]
pub(in crate::selection) struct BranchPreludeSelectionScratch {
    expressions: ExpressionTable,
    mutable_expressions: ExpressionTable,
    resolved_segment_expressions: ExpressionTable,
}

pub(in crate::selection) fn select_runtime_branch_preludes_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    _expansion_cursor: &mut usize,
    scratch: &mut BranchPreludeSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let expansions = input
        .runtime_branching_calls
        .prelude_expansions
        .storage_slice();

    for expansion in expansions {
        if !prelude_expansion_matches_operation(expansion, dispatch_index, operation) {
            continue;
        }
        select_runtime_branch_prelude(
            input,
            expansion,
            scratch,
            operands,
            runtime_value_operands,
            selected_instructions,
        );
    }
}

fn prelude_expansion_matches_operation(
    expansion: &RuntimeBranchPreludeExpansion,
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

fn select_runtime_branch_prelude(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeBranchPreludeExpansion,
    scratch: &mut BranchPreludeSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(operations) = input
        .runtime_branching_calls
        .prelude_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .prelude_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    let alias_bindings = prelude_alias_bindings(expansion.target_key, bindings);
    scratch.expressions.clear();
    scratch.resolved_segment_expressions.clear();

    for operation in operations {
        match &operation.kind {
            RuntimeBranchPreludeOperationKind::HostCall => {
                let Some(host_call) =
                    host_call_for_statement(input, operation.source_key, operation.statement_index)
                else {
                    continue;
                };
                select_host_call(
                    input,
                    host_call,
                    Some(expansion.dispatch_index),
                    Some(RuntimeAliasResolutionContext {
                        aliases: alias_bindings.bindings(),
                        alias_expressions: &input.runtime_branching_calls.expressions,
                    }),
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
            RuntimeBranchPreludeOperationKind::Mutation { target, value, .. } => {
                scratch.expressions.clear();
                let expressions = &mut scratch.expressions;
                let target =
                    expressions.copy_from(&input.runtime_branching_calls.expressions, *target);
                let value =
                    expressions.copy_from(&input.runtime_branching_calls.expressions, *value);
                let resolved_target = resolve_branch_prelude_binding_expression_handle(
                    &input.runtime_branching_calls.expressions,
                    expressions,
                    target,
                    bindings,
                );
                let resolved_value = resolve_branch_prelude_binding_expression_handle(
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
                        resolve_branch_prelude_binding_expression_handle(
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
            RuntimeBranchPreludeOperationKind::StateCall {
                target_key,
                lowering,
                role,
                call_ordinal,
                ..
            } => {
                if matches!(
                    lowering,
                    omega_state_calls::StateCallLowering::InlineExpansion
                        | omega_state_calls::StateCallLowering::InlineLeaf
                ) {
                    select_runtime_branch_prelude_inline_state_call(
                        input,
                        expansion,
                        operation.source_key,
                        operation.statement_index,
                        *target_key,
                        bindings,
                        operands,
                        runtime_value_operands,
                        selected_instructions,
                    );
                } else if matches!(
                    lowering,
                    omega_state_calls::StateCallLowering::InlineBranching
                ) {
                    let operation = RuntimeStraightLineBranchOperation {
                        source_key: operation.source_key,
                        statement_index: operation.statement_index,
                        kind: RuntimeStraightLineBranchOperationKind::StateCall {
                            role: *role,
                            call_ordinal: *call_ordinal,
                            target_key: *target_key,
                            argument_count: 0,
                            lowering: *lowering,
                        },
                    };
                    let mut straight_line_scratch = StraightLineBranchSelectionScratch::default();
                    select_runtime_straight_line_nested_branch_expansions_for_operation(
                        input,
                        expansion.dispatch_index,
                        &operation,
                        &mut straight_line_scratch,
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
            }
            RuntimeBranchPreludeOperationKind::LocalData => {
                select_runtime_branch_prelude_local_initializer_write(
                    input,
                    expansion,
                    operation.source_key,
                    operation.statement_index,
                    bindings,
                    scratch,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
            RuntimeBranchPreludeOperationKind::Other => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_branch_prelude_local_initializer_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeBranchPreludeExpansion,
    source_key: StateKey,
    statement_index: usize,
    bindings: &[RuntimeBranchPreludeBinding],
    scratch: &mut BranchPreludeSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == expansion.dispatch_index
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
    let Some(initializer) =
        local_initializer_handle(input, &mut scratch.expressions, source_key, statement_index)
    else {
        return;
    };

    let expressions = &mut scratch.expressions;
    let resolved_initializer = resolve_branch_prelude_binding_expression_handle(
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
        statement_index,
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
        statement_index,
        expressions,
        slot,
        resolved_initializer,
        &static_values,
        runtime_value_operands,
    ) {
        // A PLAIN-scalar initializer write (integer/binary/convert) is only
        // the PRELUDE's to emit for a GUARD-role expansion (the prelude is
        // that chain's sole executor). For every other role the
        // runtime-bodies splice emits the same write correctly TIMED (after
        // the callee's host calls) and in the callee's own resolution
        // context; the prelude duplicate ran before those host calls and
        // resolved same-named lets ACROSS callees -- two callees sharing
        // `freq` in one caller state read each other's still-ZII slots, and
        // a duplicated `x / freq` div-by-zero crashed before the correct op
        // (the cross-callee collision's internal-op flavor). Every OTHER
        // kind stays: indexed/descriptor reads (fixed_vec's
        // `cells[index]` through the prelude-bound descriptor) and plain
        // copies have no splice equivalent here, and none of them trap.
        let splice_covered_plain_write = match kind {
            SelectedInstructionKind::WriteRuntimeStorageConvert { .. }
            | SelectedInstructionKind::WritePlaceConvert { .. } => true,
            // Write rung 2b: the plain integer write rides WritePlaceInteger
            // now; only the DIRECT place shape is the splice-covered plain
            // write (deref/indexed shapes have no splice equivalent here).
            SelectedInstructionKind::WritePlaceInteger { target, .. } => {
                target.const_offset().is_some()
            }
            SelectedInstructionKind::WritePlaceBinary { target, .. } => {
                target.const_offset().is_some()
            }
            _ => false,
        };
        if expansion.role == omega_state_calls::StateCallRole::TransitionGuard
            || !splice_covered_plain_write
        {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: statement_index,
            });
        }
        return;
    }

    let target = runtime_frame_slot_target_expression(expressions, slot);
    scratch.resolved_segment_expressions.clear();
    if runtime_text_builder_write_in_table_emit(
        input,
        expansion.dispatch_index,
        source_key,
        expansion.source_key,
        statement_index,
        expressions,
        target,
        &mut scratch.resolved_segment_expressions,
        &|expressions, expression| {
            resolve_branch_prelude_binding_expression_handle(
                &input.runtime_branching_calls.expressions,
                expressions,
                expression,
                bindings,
            )
        },
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: statement_index,
            });
        },
    ) {}
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

#[allow(clippy::too_many_arguments)]
fn select_runtime_branch_prelude_inline_state_call(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeBranchPreludeExpansion,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    target_key: omega_control_flow::StateKey,
    bindings: &[RuntimeBranchPreludeBinding],
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
        let resolved_expression = resolve_branch_prelude_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            &mut child_alias_expressions,
            argument_expression,
            bindings,
        );
        let expression =
            strip_mutable_expression_handle(&mut child_alias_expressions, resolved_expression);
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

fn prelude_alias_bindings(
    target_key: omega_control_flow::StateKey,
    bindings: &[RuntimeBranchPreludeBinding],
) -> RuntimeAliasBuffer {
    RuntimeAliasBuffer::from_iter(bindings.iter().map(|binding| RuntimeAliasBinding {
        source_key: target_key,
        parameter_symbol: binding.parameter_symbol,
        parameter_name: binding.parameter_name.clone(),
        expression_source_key: target_key,
        expression: binding.expression,
    }))
}
