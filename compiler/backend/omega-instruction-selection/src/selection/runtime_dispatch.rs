use crate::InstructionSelectionInput;
use omega_core::arena::Arena;
use omega_typed_program::expression::ExpressionTable;

mod branches;
mod edges;
mod guards;
mod operation_aliases;
mod text_writes;
mod writes;

use super::host_operations::{
    runtime_machine_string_descriptor_offset, runtime_text_literal_write_for_host_call,
    select_host_call,
};
use super::lookups::host_call_for_statement;
use branches::{
    select_runtime_leaf_branch_expansions_for_operation,
    select_runtime_straight_line_branch_expansions_for_operation,
};
use edges::select_runtime_dispatch_edge;
use omega_target_program::{InstructionOperand, SelectedInstruction, SelectedInstructionKind};
use operation_aliases::bind_runtime_operation_aliases;
use writes::select_runtime_storage_write_for_operation;

pub(super) fn select_runtime_dispatch_loop_instructions(
    input: &InstructionSelectionInput<'_>,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index: input.runtime_dispatch_loop.entry_dispatch_index,
            terminal_dispatch_index: input.runtime_dispatch_loop.terminal_dispatch_index,
        },
        source_key: input.entry_key,
        source_statement: 0,
    });

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
            let mut runtime_aliases = Vec::new();
            let mut runtime_alias_expressions = ExpressionTable::new();
            let mut runtime_static_values = Vec::new();

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
                    &runtime_aliases,
                    &runtime_alias_expressions,
                    &mut runtime_static_values,
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
                    if runtime_machine_string_descriptor_offset(input, host_call).is_none()
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
                    select_host_call(input, host_call, operands, selected_instructions);
                }
            }
        }

        if let Some(edges) = input.runtime_dispatch_loop.edges.span(dispatch_case.edges) {
            for edge in edges {
                select_runtime_dispatch_edge(edge, dispatch_case.key, selected_instructions);
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
