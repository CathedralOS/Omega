use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_checked_trees::statement::StatementNode;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;

mod branches;
mod edges;
mod guards;
mod operation_aliases;
mod text_writes;
mod writes;

use super::host_operations::{
    runtime_string_descriptor_place, runtime_text_literal_write_for_host_call, select_host_call,
};
use super::instruction_sink::SelectedInstructionSink;
use super::lookups::host_call_for_statement;
use crate::selection::bindings::{RuntimeAliasBuffer, RuntimeAliasResolutionContext};
use branches::{
    BranchPreludeSelectionScratch, LeafBranchSelectionScratch, StraightLineBranchSelectionScratch,
    select_runtime_branch_preludes_for_operation,
    select_runtime_leaf_branch_expansions_for_operation,
    select_runtime_straight_line_branch_expansions_for_operation,
};
use edges::select_runtime_dispatch_edge;
use omega_abstract_operations::{
    InstructionOperand, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
};
use operation_aliases::bind_runtime_operation_aliases;
use writes::select_runtime_storage_write_for_operation;
pub(crate) use writes::{RuntimeStaticValues, RuntimeStorageWriteScratch};

pub(crate) use branches::select_runtime_resolved_mutation_write;
pub(in crate::selection) use writes::runtime_frame_slot_target_expression;
pub(in crate::selection) use writes::select_runtime_frame_slot_value_write_in_table;
pub(in crate::selection) use writes::select_runtime_storage_resolved_mutation_write_in_table_with_scratch;

#[allow(clippy::too_many_arguments)]
pub(crate) fn select_runtime_unaliased_storage_mutation_write_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &mut writes::RuntimeStaticValues,
    scratch: &mut RuntimeStorageWriteScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    writes::select_runtime_storage_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        source_key,
        statement_index,
        target,
        value,
        static_values,
        scratch,
        runtime_value_operands,
        selected_instructions,
    )
}

pub(super) fn select_runtime_dispatch_loop_instructions(
    input: &InstructionSelectionInput<'_>,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index: input.runtime_dispatch_loop.entry_dispatch_index,
            terminal_dispatch_index: input.runtime_dispatch_loop.terminal_dispatch_index,
        },
        source_key: input.entry_key,
        source_statement: 0,
    });

    let mut runtime_aliases = RuntimeAliasBuffer::with_capacity(input.state_calls.arguments.len());
    let mut runtime_alias_expressions =
        ExpressionTable::with_expression_capacity(input.state_calls.arguments.len());
    let mut local_initializer_expressions = ExpressionTable::with_expression_capacity(
        input.state_calls.arguments.len().saturating_add(4),
    );
    let mut local_initializer_mutable_expressions = ExpressionTable::with_expression_capacity(4);
    let mut local_initializer_segment_expressions = ExpressionTable::with_expression_capacity(4);
    let mut runtime_static_values =
        writes::RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    let mut runtime_storage_write_scratch = RuntimeStorageWriteScratch::default();
    let mut prelude_expansion_cursor = 0usize;
    let mut leaf_expansion_cursor = 0usize;
    let mut straight_line_expansion_cursor = 0usize;
    let mut prelude_selection_scratch = BranchPreludeSelectionScratch::default();
    let mut leaf_selection_scratch = LeafBranchSelectionScratch::default();
    let mut straight_line_selection_scratch = StraightLineBranchSelectionScratch::default();

    for (dispatch_case_index, (_, dispatch_case)) in
        input.runtime_dispatch_loop.cases.iter().enumerate()
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EnterDispatchCase {
                dispatch_index: dispatch_case.dispatch_index,
            },
            source_key: dispatch_case.key,
            source_statement: 0,
        });

        if let Some(runtime_body) = input
            .runtime_bodies
            .bodies
            .storage_slice()
            .get(dispatch_case_index)
            .filter(|body| body.dispatch_index == dispatch_case.dispatch_index)
            .or_else(|| {
                input
                    .runtime_bodies
                    .bodies
                    .iter()
                    .find(|(_, body)| body.dispatch_index == dispatch_case.dispatch_index)
                    .map(|(_, body)| body)
            })
            && let Some(operations) = input
                .runtime_bodies
                .operations
                .span(runtime_body.operations)
        {
            runtime_aliases.clear();
            runtime_alias_expressions.clear();
            runtime_static_values.clear();
            runtime_storage_write_scratch.clear();

            for operation in operations {
                bind_runtime_operation_aliases(
                    input,
                    operation,
                    &mut runtime_aliases,
                    &mut runtime_alias_expressions,
                );

                if matches!(
                    operation.kind,
                    RuntimeDispatchBodyOperationKind::LocalStorage { .. }
                ) {
                    select_runtime_dispatch_local_initializer_write(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        runtime_aliases.bindings(),
                        &runtime_alias_expressions,
                        &mut local_initializer_expressions,
                        &mut local_initializer_mutable_expressions,
                        &mut local_initializer_segment_expressions,
                        &mut runtime_static_values,
                        runtime_value_operands,
                        selected_instructions,
                    );
                    continue;
                }

                select_runtime_storage_write_for_operation(
                    input,
                    dispatch_case.dispatch_index,
                    operation,
                    runtime_aliases.bindings(),
                    &runtime_alias_expressions,
                    &mut runtime_static_values,
                    &mut runtime_storage_write_scratch,
                    runtime_value_operands,
                    selected_instructions,
                );

                select_runtime_branch_preludes_for_operation(
                    input,
                    dispatch_case.dispatch_index,
                    operation,
                    &mut prelude_expansion_cursor,
                    &mut prelude_selection_scratch,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
                select_runtime_straight_line_branch_expansions_for_operation(
                    input,
                    dispatch_case.dispatch_index,
                    operation,
                    &mut straight_line_expansion_cursor,
                    &mut straight_line_selection_scratch,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
                select_runtime_leaf_branch_expansions_for_operation(
                    input,
                    dispatch_case.dispatch_index,
                    operation,
                    &mut leaf_expansion_cursor,
                    &mut leaf_selection_scratch,
                    runtime_value_operands,
                    selected_instructions,
                );

                if matches!(operation.kind, RuntimeDispatchBodyOperationKind::HostCall)
                    && let Some(host_call) = host_call_for_statement(
                        input,
                        operation.source_key,
                        operation.statement_index,
                    )
                {
                    let alias_bindings = runtime_aliases.bindings();
                    let alias_context =
                        (!alias_bindings.is_empty()).then_some(RuntimeAliasResolutionContext {
                            aliases: alias_bindings,
                            alias_expressions: &runtime_alias_expressions,
                        });

                    if runtime_string_descriptor_place(
                        input,
                        host_call,
                        Some(dispatch_case.dispatch_index),
                        alias_context,
                    )
                    .is_none()
                        && let Some(literal_write) =
                            runtime_text_literal_write_for_host_call(input, host_call)
                    {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::WriteRuntimeTextLiteral {
                                buffer: literal_write.buffer,
                                literal: literal_write.literal,
                            },
                            source_key: host_call.source_key,
                            source_statement: host_call.statement_index,
                        });
                    }
                    select_host_call(
                        input,
                        host_call,
                        Some(dispatch_case.dispatch_index),
                        alias_context,
                        operands,
                        selected_instructions,
                    );
                }
            }
        }

        let case_edges = input.runtime_dispatch_loop.edges.span(dispatch_case.edges);
        if let Some(edges) = case_edges {
            for edge in edges {
                select_runtime_dispatch_edge(
                    input,
                    edge,
                    dispatch_case.key,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
        }
        if case_edges.map_or(true, <[_]>::is_empty) {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_key: dispatch_case.key,
                source_statement: 0,
            });
        }

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::LeaveDispatchCase,
            source_key: dispatch_case.key,
            source_statement: 0,
        });
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::LeaveDispatchLoop,
        source_key: input.entry_key,
        source_statement: 0,
    });
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_dispatch_local_initializer_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    aliases: &[crate::selection::bindings::RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    expressions: &mut ExpressionTable,
    mutable_expressions: &mut ExpressionTable,
    resolved_segment_expressions: &mut ExpressionTable,
    static_values: &mut writes::RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input
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

    expressions.clear();
    let Some(initializer) =
        local_initializer_handle(input, expressions, source_key, statement_index)
    else {
        return;
    };
    let copied_aliases =
        RuntimeAliasBuffer::copy_from_bindings(alias_expressions, aliases, expressions);
    let resolved_initializer = crate::selection::bindings::resolve_runtime_alias_binding_handle(
        initializer,
        source_key,
        copied_aliases.bindings(),
        expressions,
    );
    if let Some(kind) = writes::select_runtime_frame_slot_value_write_in_table(
        input,
        dispatch_index,
        resolved_initializer.source_key,
        statement_index,
        expressions,
        slot,
        resolved_initializer.expression,
        static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key,
            source_statement: statement_index,
        });
        return;
    }

    let target = writes::runtime_frame_slot_target_expression(expressions, slot);
    let _ = writes::select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        source_key,
        source_key,
        resolved_initializer.source_key,
        statement_index,
        expressions,
        target,
        resolved_initializer.expression,
        &[],
        static_values,
        mutable_expressions,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
}

fn local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
) -> Option<ExpressionHandle> {
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
