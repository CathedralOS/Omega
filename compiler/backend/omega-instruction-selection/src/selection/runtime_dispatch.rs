use crate::InstructionSelectionInput;
use omega_checked_trees::expression::ExpressionTable;
use omega_core::arena::Arena;

mod branches;
mod edges;
mod guards;
mod operation_aliases;
mod text_writes;
mod writes;

use super::host_operations::{
    runtime_string_descriptor_place, runtime_text_literal_write_for_host_call,
    select_host_call,
};
use super::instruction_sink::SelectedInstructionSink;
use super::lookups::host_call_for_statement;
use crate::selection::bindings::{RuntimeAliasBuffer, RuntimeAliasResolutionContext};
use branches::{
    select_runtime_branch_preludes_for_operation,
    select_runtime_leaf_branch_expansions_for_operation,
    select_runtime_straight_line_branch_expansions_for_operation,
};
use edges::select_runtime_dispatch_edge;
use omega_target_operations::{InstructionOperand, SelectedInstruction, SelectedInstructionKind};
use operation_aliases::bind_runtime_operation_aliases;
use writes::select_runtime_storage_write_for_operation;

pub(crate) use branches::select_runtime_resolved_mutation_write;

pub(super) fn select_runtime_dispatch_loop_instructions(
    input: &InstructionSelectionInput<'_>,
    operands: &mut Arena<InstructionOperand>,
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

    let mut runtime_aliases = RuntimeAliasBuffer::default();
    let mut runtime_alias_expressions = ExpressionTable::new();
    let mut runtime_static_values = Vec::new();

    for (_, dispatch_case) in input.runtime_dispatch_loop.cases.iter() {
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
            .iter()
            .find(|(_, body)| body.dispatch_index == dispatch_case.dispatch_index)
            .map(|(_, body)| body)
            && let Some(operations) = input
                .runtime_bodies
                .operations
                .span(runtime_body.operations)
        {
            runtime_aliases.clear();
            runtime_alias_expressions.clear();
            runtime_static_values.clear();

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
                    selected_instructions,
                );

                select_runtime_branch_preludes_for_operation(
                    input,
                    dispatch_case.dispatch_index,
                    operation,
                    operands,
                    selected_instructions,
                );
                select_runtime_leaf_branch_expansions_for_operation(
                    input,
                    dispatch_case.dispatch_index,
                    operation,
                    selected_instructions,
                );
                select_runtime_straight_line_branch_expansions_for_operation(
                    input,
                    dispatch_case.dispatch_index,
                    operation,
                    selected_instructions,
                );

                if let Some(host_call) =
                    host_call_for_statement(input, operation.source_key, operation.statement_index)
                {
                    let alias_context = Some(RuntimeAliasResolutionContext {
                        aliases: runtime_aliases.bindings(),
                        alias_expressions: &runtime_alias_expressions,
                    });

                    if runtime_string_descriptor_place(
                        input,
                        host_call,
                        Some(dispatch_case.dispatch_index),
                        alias_context,
                    )
                    .is_none()
                        && let Some((buffer, literal)) =
                            runtime_text_literal_write_for_host_call(input, host_call)
                    {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::WriteRuntimeTextLiteral {
                                buffer,
                                literal,
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

        if let Some(edges) = input.runtime_dispatch_loop.edges.span(dispatch_case.edges) {
            for edge in edges {
                select_runtime_dispatch_edge(input, edge, dispatch_case.key, selected_instructions);
            }
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
