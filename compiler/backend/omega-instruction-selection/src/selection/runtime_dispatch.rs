use crate::InstructionSelectionInput;
use omega_checked_trees::data::DataMember;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::statement::StatementNode;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use omega_state_calls::StateCallRole;
use omega_state_values::{StateValueRole, simplify_state_expression_for_role};

mod argument_materialization;
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
pub(super) use branches::{
    BranchPreludeSelectionScratch, select_runtime_branch_preludes_for_operation,
};
use branches::{
    LeafBranchSelectionScratch, select_runtime_leaf_branch_expansions_for_operation,
    select_runtime_straight_line_branch_expansions_for_operation,
};
use edges::select_runtime_dispatch_edge;
use omega_abstract_operations::{
    InstructionOperand, RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction,
    SelectedInstructionKind,
};
use operation_aliases::bind_runtime_operation_aliases;
use writes::select_runtime_storage_write_for_operation;
pub(crate) use writes::{RuntimeStaticValues, RuntimeStorageWriteScratch};

pub(super) use branches::{
    StraightLineBranchSelectionScratch, select_assignment_value_call_result_local_copy,
    select_runtime_straight_line_nested_branch_expansions_for_operation,
};
pub(in crate::selection) use writes::emit_runtime_frame_slot_slice_descriptor_write_in_table;
pub(in crate::selection) use writes::runtime_frame_slot_target_expression;
pub(in crate::selection) use writes::select_runtime_frame_slot_value_write_in_table;
pub(in crate::selection) use writes::select_runtime_storage_resolved_mutation_write_in_table_with_scratch;

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

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

/// Emit writes that initialize the entry machine's data fields to their declared
/// default values (`data Main { x: i32 = 5 }`). Without this a field with a default
/// reads zero at runtime — the default is captured front-end (`DataField.initial_
/// value`) but otherwise never emitted. First cut: the ENTRY machine's own fields
/// (Machine region, offset == the field's storage offset) with a CONSTANT default
/// (integer / boolean / float literal). Non-constant defaults and nested-machine
/// fields are follow-ups.
fn select_entry_machine_field_default_writes(
    input: &InstructionSelectionInput<'_>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some((_, machine_layout)) = input
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, layout)| layout.symbol == input.entry_key.machine)
    else {
        return;
    };
    let Some(fields) = input.layouts.fields.span(machine_layout.fields) else {
        return;
    };
    for field in fields {
        let Some(initial_value) = entry_machine_field_initial_value(input, field.symbol) else {
            continue;
        };
        if !matches!(field.layout.size, 1 | 4 | 8) {
            continue;
        }
        let value = match input.program.expression_table.expression(initial_value) {
            ExpressionNode::Integer(value) => *value,
            ExpressionNode::Boolean(value) => i64::from(*value),
            ExpressionNode::Float(literal) => {
                if field.layout.size <= 4 {
                    i64::from((literal.value() as f32).to_bits())
                } else {
                    literal.value().to_bits() as i64
                }
            }
            // Non-constant defaults are not yet emitted (would need startup code that
            // evaluates the initializer expression).
            _ => continue,
        };
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                target_region: RuntimeStorageRegion::Machine,
                byte_offset: field.offset,
                byte_size: field.layout.size,
                value,
            },
            source_key: input.entry_key,
            source_statement: 0,
        });
    }
}

/// The declared default initializer for a data field, looked up by field symbol
/// across the program's data definitions.
fn entry_machine_field_initial_value(
    input: &InstructionSelectionInput<'_>,
    field_symbol: omega_core::symbols::SymbolHandle,
) -> Option<ExpressionHandle> {
    for data_definition in input.program.data_definitions() {
        for member in input.program.data_members(data_definition) {
            if let DataMember::Field(field) = member
                && field.symbol == field_symbol
                && field.initial_value.is_valid()
            {
                return Some(field.initial_value);
            }
        }
    }
    None
}

pub(super) fn select_runtime_dispatch_loop_instructions(
    input: &InstructionSelectionInput<'_>,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    // Initialize the entry machine's data-field defaults (`data Main { x: i32 = 5 }`)
    // before the dispatch loop, so a field starts at its declared value rather than
    // zero. Runs once at program entry.
    select_entry_machine_field_default_writes(input, selected_instructions);

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
                .paged_span(runtime_body.operations)
        {
            runtime_aliases.clear();
            runtime_alias_expressions.clear();
            runtime_static_values.clear();
            runtime_storage_write_scratch.clear();

            for operation in operations.iter() {
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
                    dispatch_case.dispatch_index,
                    runtime_aliases.bindings(),
                    &runtime_alias_expressions,
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
                && state_key_matches_statement_source(slot.source_key, source_key)
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
    let resolved_initializer_source_key = resolved_initializer.source_key;
    let resolved_initializer = simplify_runtime_local_initializer_handle(
        input,
        expressions,
        source_key,
        statement_index,
        resolved_initializer.expression,
    )
    .unwrap_or(resolved_initializer.expression);
    let wrote_slice = writes::emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        dispatch_index,
        resolved_initializer_source_key,
        statement_index,
        expressions,
        slot,
        resolved_initializer,
        selected_instructions,
    );
    if wrote_slice {
        return;
    }
    let wrote_text_comparison = writes::emit_runtime_frame_slot_text_comparison_write_in_table(
        input,
        dispatch_index,
        resolved_initializer_source_key,
        statement_index,
        expressions,
        slot,
        resolved_initializer,
        selected_instructions,
    );
    if wrote_text_comparison {
        return;
    }
    let direct_kind = writes::select_runtime_frame_slot_value_write_in_table(
        input,
        dispatch_index,
        resolved_initializer_source_key,
        statement_index,
        expressions,
        slot,
        resolved_initializer,
        static_values,
        runtime_value_operands,
    );
    if let Some(kind) = direct_kind {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key,
            source_statement: statement_index,
        });
        return;
    }

    if copy_assignment_value_call_result_into_local(
        input,
        dispatch_index,
        source_key,
        statement_index,
        slot,
        selected_instructions,
    ) {
        return;
    }

    let target = writes::runtime_frame_slot_target_expression(expressions, slot);
    let _ = writes::select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        source_key,
        source_key,
        resolved_initializer_source_key,
        statement_index,
        expressions,
        target,
        resolved_initializer,
        &[],
        static_values,
        mutable_expressions,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
}

fn copy_assignment_value_call_result_into_local(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    local_source_key: StateKey,
    statement_index: usize,
    local_slot: &omega_runtime_storage::RuntimeFrameSlot,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(call_result_slot) = input.runtime_storage.call_result_slot(
        dispatch_index,
        local_source_key,
        statement_index,
        StateCallRole::AssignmentValue,
    ) else {
        return false;
    };
    if call_result_slot.byte_size != local_slot.byte_size || local_slot.byte_size == 0 {
        return false;
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::CopyRuntimeStorage {
            source_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            source_offset: call_result_slot.byte_offset,
            target_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            target_offset: local_slot.byte_offset,
            byte_count: local_slot.byte_size,
        },
        source_key: local_source_key,
        source_statement: statement_index,
    });
    true
}

fn simplify_runtime_local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    expressions: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
    expression: ExpressionHandle,
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
    let simplified = simplify_state_expression_for_role(
        input.program,
        machine,
        state,
        statement_index,
        StateValueRole::CallArgument,
        &expressions.to_tree(expression),
    );
    Some(expressions.insert_tree(&simplified))
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
