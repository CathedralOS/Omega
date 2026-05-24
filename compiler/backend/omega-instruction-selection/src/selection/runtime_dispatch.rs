use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
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
