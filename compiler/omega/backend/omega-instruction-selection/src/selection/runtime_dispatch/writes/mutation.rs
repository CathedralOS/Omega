mod binary_table_writes;
mod frame_slots;
mod normalization;
mod operators;
mod static_writes;
mod unary_table_writes;
mod value_operands;

pub(in crate::selection::runtime_dispatch::writes) use binary_table_writes::select_runtime_atomic_load_or_store_in_table;
pub(in crate::selection) use operators::{
    builtin_runtime_binary_float_call_operator_in_table, builtin_runtime_call_operator_in_table,
    builtin_runtime_ternary_float_call_operator_in_table,
    builtin_runtime_unary_call_operator_in_table, float_unary_result_is_bool,
};
pub(crate) use value_operands::resolve_runtime_value_operand_in_table;
pub(in crate::selection::runtime_dispatch) use value_operands::{
    binary_value_operand_byte_width, binary_value_operands_are_float,
    resolve_runtime_stored_integer_operand_in_table,
    select_runtime_stored_integer_mutation_write_in_table,
    select_runtime_stored_integer_projection_write_in_table,
};

use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::lookups::state_parameters;
use omega_abstract_operations::{
    RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind, StateGuardOperator,
};
use omega_control_flow::StateKey;
use omega_state_calls::{StateCallLowering, StateCallRole};
use psi_arena::Arena;
use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};

use super::super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, append_place_suffix,
    resolve_runtime_alias_binding_handle, strip_mutable_expression,
};
use super::super::super::storage_places::{
    resolve_binary_write_arithmetic_domain,
    resolve_runtime_frame_base_indexed_target_with_index_region,
    resolve_runtime_storage_arithmetic_domain, resolve_runtime_storage_is_signed,
    resolve_runtime_storage_place, resolve_runtime_storage_primitive_type,
    runtime_storage_target_is_atomic,
};
use super::super::super::storage_places::{
    resolve_runtime_assignment_value_call_result_place_by_ordinal,
    resolve_runtime_call_argument_call_result_place_by_ordinal,
    resolve_runtime_frame_base_double_indexed_source, resolve_runtime_frame_fixed_indexed_target,
    resolve_runtime_frame_indexed_target, resolve_runtime_machine_double_indexed_source,
    resolve_runtime_machine_indexed_target, resolve_runtime_pointee_double_indexed_target,
    resolve_runtime_pointee_slot_offset,
};
use super::super::guards::static_guard_conjunct_summary_in_table;
use super::super::text_writes::{
    resolve_bounded_buffer_target_place, runtime_text_builder_write_in_table_emit,
    runtime_text_builder_write_with_scratch_emit, select_runtime_string_descriptor_write,
    string_literal_data_handle,
};
use super::slice_descriptors::emit_runtime_frame_slot_slice_descriptor_write_in_table;
use super::static_values::{
    RuntimeStaticValues, invalidate_runtime_static_collection_for_indexed_write,
    resolve_runtime_static_integer, resolve_runtime_static_integer_value, set_runtime_static_value,
};
use super::storage_copy::runtime_storage_copy;
use super::storage_copy::runtime_storage_indexed_source_copy;
use super::storage_copy::runtime_storage_indirect_copy;
use super::subslice_copy::{
    runtime_fixed_array_subslice_descriptor_write, runtime_fixed_array_subslice_indexed_source_copy,
};
pub(in crate::selection::runtime_dispatch) use binary_table_writes::{
    build_runtime_convert_write, select_runtime_convert_mutation_write_in_table,
    signedness_adjusted_operator, signedness_adjusted_operator_for_operands,
};
pub(super) use binary_table_writes::{
    select_runtime_binary_mutation_write_in_table, select_runtime_frame_slot_convert_write_in_table,
};
pub(in crate::selection::runtime_dispatch) use binary_table_writes::{
    select_runtime_storage_binary_write_in_table,
    select_runtime_storage_binary_write_in_table_with_call_ordinal,
    select_runtime_storage_binary_write_in_table_with_evidence_source_key,
};
pub(in crate::selection) use frame_slots::{
    runtime_frame_slot_target_expression, select_runtime_frame_slot_value_write_in_table,
    select_runtime_frame_slot_value_write_in_table_with_call_ordinal,
    select_runtime_frame_slot_value_write_in_table_with_source_anchor,
};
pub(super) use normalization::simplify_runtime_expression_with_state_locals;
use normalization::{normalize_runtime_mutation_expression, resolve_runtime_mutation_target};
use operators::{
    builtin_runtime_call_operator, runtime_binary_operator, supports_scalar_integer_write,
};
use psi_checked_trees::types::PrimitiveType;
pub(super) use static_writes::select_runtime_static_mutation_write_in_table;
pub(in crate::selection) use unary_table_writes::select_runtime_logical_not_write_in_table;
pub(in crate::selection::runtime_dispatch) use value_operands::resolve_runtime_text_equals_operand_in_table;
use value_operands::resolve_runtime_value_operand;

fn resolve_direct_or_pointee_bounded_buffer_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    expression: &Expression,
) -> Option<omega_abstract_operations::Place> {
    if let Some(target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, expression)
    {
        return omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            target.pointer_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)?
        .with_step(omega_abstract_operations::PlaceStep::ConstOffset(
            target.field_byte_offset,
        ));
    }

    let place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        expression,
    )?;
    Some(omega_abstract_operations::Place::at(
        place.region,
        place.byte_offset,
    ))
}

fn append_tree_expression_path<'a>(expression: &'a Expression, path: &mut Vec<&'a str>) {
    match expression {
        Expression::Borrow(inner) => append_tree_expression_path(&inner.target, path),
        Expression::Name(name_path) => {
            if let Some(name) = name_path.last() {
                path.push(name.as_str());
            }
        }
        Expression::Member(member) => {
            append_tree_expression_path(&member.receiver, path);
            path.push(member.member.as_str());
        }
        _ => {}
    }
}

fn append_table_expression_path<'a>(
    expressions: &'a ExpressionTable,
    expression: ExpressionHandle,
) -> Vec<&'a str> {
    fn append<'a>(
        expressions: &'a ExpressionTable,
        expression: ExpressionHandle,
        path: &mut Vec<&'a str>,
    ) {
        if !expression.is_valid() {
            return;
        }
        match expressions.expression(expression) {
            psi_checked_trees::expression::ExpressionNode::Borrow(inner) => {
                append(expressions, inner.target, path);
            }
            psi_checked_trees::expression::ExpressionNode::Name(name_path) => {
                if let Some(name) = expressions.name_path_members(name_path.members).last() {
                    path.push(name.as_str());
                }
            }
            psi_checked_trees::expression::ExpressionNode::Member(member) => {
                append(expressions, member.receiver, path);
                path.push(member.member.as_str());
            }
            _ => {}
        }
    }

    let mut path = Vec::new();
    if !expression.is_valid() {
        return path;
    }
    append(expressions, expression, &mut path);
    path
}

fn state_call_matches_expression(
    input: &InstructionSelectionInput<'_>,
    state_call: &omega_state_calls::StateCall,
    target: &str,
    receiver_path: &[&str],
) -> bool {
    let (target_machine, target_state) = input
        .control_flow
        .state_names_by_key_cloned(state_call.target_key);
    let planned_receiver_path = input
        .state_calls
        .receiver_path_segments
        .span_or_empty(state_call.receiver_path);
    // Methods are named by their target state. A top-level named machine is
    // called by machine name and enters its canonical `entry` state.
    (target_state.as_str() == target
        || (target_state.as_str() == "entry" && target_machine.as_str() == target))
        && receiver_path.len() == planned_receiver_path.len()
        && receiver_path
            .iter()
            .zip(planned_receiver_path)
            .all(|(actual, planned)| *actual == planned.as_str())
}

fn resolve_matching_runtime_call_result_source_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    target: &str,
    receiver_path: &[&str],
    occurrence_rank: usize,
    minimum_call_ordinal: Option<usize>,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
    input
        .state_calls
        .calls
        .iter()
        .filter_map(|(_, state_call)| {
            (state_call_has_runtime_value_result(state_call.role)
                && super::super::state_key_matches_statement_source(
                    state_call.source_key,
                    source_key,
                )
                && state_call.statement_index == statement_index
                && minimum_call_ordinal.is_none_or(|minimum| state_call.call_ordinal >= minimum)
                && state_call_matches_expression(input, state_call, target, receiver_path))
            .then_some(state_call)
        })
        .nth(occurrence_rank)
        .and_then(|state_call| {
            resolve_runtime_call_result_source_place_by_ordinal(input, dispatch_index, state_call)
        })
}

/// Rank `target_call` among calls with the same target and receiver by walking
/// the complete value expression in source order. State-call collection uses
/// this same pre-order, so the rank selects the corresponding ordinal slot.
fn runtime_tree_call_occurrence_rank(
    input: &InstructionSelectionInput<'_>,
    root: &Expression,
    target_call: &psi_checked_trees::expression::CallExpression,
) -> Option<usize> {
    fn visit(
        input: &InstructionSelectionInput<'_>,
        expression: &Expression,
        target_call: &psi_checked_trees::expression::CallExpression,
        target_receiver_path: &[&str],
        rank: &mut usize,
    ) -> bool {
        match expression {
            Expression::Atomic(atomic) => visit(
                input,
                &atomic.value,
                target_call,
                target_receiver_path,
                rank,
            ),
            Expression::ArrayLiteral(values) => values
                .iter()
                .any(|value| visit(input, value, target_call, target_receiver_path, rank)),
            Expression::Binary(binary) => {
                visit(input, &binary.left, target_call, target_receiver_path, rank)
                    || visit(
                        input,
                        &binary.right,
                        target_call,
                        target_receiver_path,
                        rank,
                    )
            }
            Expression::Call(call) => {
                if std::ptr::eq(call.as_ref(), target_call) {
                    return true;
                }
                let mut receiver_path = Vec::new();
                if let Some(receiver) = call.receiver.as_deref() {
                    append_tree_expression_path(receiver, &mut receiver_path);
                }
                if call.target == target_call.target
                    && receiver_path == target_receiver_path
                    && input.state_calls.calls.iter().any(|(_, state_call)| {
                        state_call_matches_expression(
                            input,
                            state_call,
                            call.target.as_str(),
                            &receiver_path,
                        )
                    })
                {
                    *rank += 1;
                }
                call.receiver.as_deref().is_some_and(|receiver| {
                    visit(input, receiver, target_call, target_receiver_path, rank)
                }) || call
                    .arguments
                    .iter()
                    .any(|argument| visit(input, argument, target_call, target_receiver_path, rank))
            }
            Expression::Cast(cast) => {
                visit(input, &cast.value, target_call, target_receiver_path, rank)
            }
            Expression::Indexed(indexed) => {
                visit(
                    input,
                    &indexed.collection,
                    target_call,
                    target_receiver_path,
                    rank,
                ) || visit(
                    input,
                    &indexed.index,
                    target_call,
                    target_receiver_path,
                    rank,
                )
            }
            Expression::Member(member) => visit(
                input,
                &member.receiver,
                target_call,
                target_receiver_path,
                rank,
            ),
            Expression::Borrow(inner) => visit(
                input,
                &inner.target,
                target_call,
                target_receiver_path,
                rank,
            ),
            Expression::Range(range) => {
                range.start.as_deref().is_some_and(|start| {
                    visit(input, start, target_call, target_receiver_path, rank)
                }) || range
                    .end
                    .as_deref()
                    .is_some_and(|end| visit(input, end, target_call, target_receiver_path, rank))
            }
            Expression::StructLiteral(struct_literal) => struct_literal
                .fields
                .iter()
                .any(|field| visit(input, &field.value, target_call, target_receiver_path, rank)),
            Expression::Unary(unary) => visit(
                input,
                &unary.operand,
                target_call,
                target_receiver_path,
                rank,
            ),
            Expression::Boolean(_)
            | Expression::Float(_)
            | Expression::Integer(_)
            | Expression::Name(_)
            | Expression::String(_)
            | Expression::ZeroValue(_) => false,
        }
    }

    let mut receiver_path = Vec::new();
    if let Some(receiver) = target_call.receiver.as_deref() {
        append_tree_expression_path(receiver, &mut receiver_path);
    }
    let mut rank = 0;
    visit(input, root, target_call, &receiver_path, &mut rank).then_some(rank)
}

fn resolve_runtime_tree_call_result_source_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call: &psi_checked_trees::expression::CallExpression,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
    resolve_runtime_tree_call_result_source_place_in_expression(
        input,
        dispatch_index,
        source_key,
        statement_index,
        None,
        call,
    )
}

fn resolve_runtime_tree_call_result_source_place_in_expression(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    root: Option<&Expression>,
    call: &psi_checked_trees::expression::CallExpression,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
    let mut receiver_path = Vec::new();
    if let Some(receiver) = call.receiver.as_deref() {
        append_tree_expression_path(receiver, &mut receiver_path);
    }
    let occurrence_rank = root
        .and_then(|root| runtime_tree_call_occurrence_rank(input, root, call))
        .unwrap_or(0);
    resolve_matching_runtime_call_result_source_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        call.target.as_str(),
        &receiver_path,
        occurrence_rank,
        None,
    )
}

/// Table-shaped counterpart of `runtime_tree_call_occurrence_rank`. Handles
/// cannot encode occurrence identity themselves after expression copying, so
/// walk from the owning value root and stop at the exact call handle.
#[allow(clippy::too_many_arguments)]
fn runtime_table_call_occurrence_rank(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
    root: ExpressionHandle,
    target_call: ExpressionHandle,
    target: &str,
    target_receiver_path: &[&str],
) -> Option<usize> {
    #[allow(clippy::too_many_arguments)]
    fn visit(
        input: &InstructionSelectionInput<'_>,
        expressions: &ExpressionTable,
        source_key: StateKey,
        statement_index: usize,
        expression: ExpressionHandle,
        target_call: ExpressionHandle,
        target: &str,
        target_receiver_path: &[&str],
        rank: &mut usize,
    ) -> bool {
        match expressions.expression(expression) {
            ExpressionNode::Atomic(atomic) => visit(
                input,
                expressions,
                source_key,
                statement_index,
                atomic.value,
                target_call,
                target,
                target_receiver_path,
                rank,
            ),
            ExpressionNode::ArrayLiteral(values) => {
                expressions.expression_handles(*values).iter().any(|value| {
                    visit(
                        input,
                        expressions,
                        source_key,
                        statement_index,
                        *value,
                        target_call,
                        target,
                        target_receiver_path,
                        rank,
                    )
                })
            }
            ExpressionNode::Binary(binary) => {
                visit(
                    input,
                    expressions,
                    source_key,
                    statement_index,
                    binary.left,
                    target_call,
                    target,
                    target_receiver_path,
                    rank,
                ) || visit(
                    input,
                    expressions,
                    source_key,
                    statement_index,
                    binary.right,
                    target_call,
                    target,
                    target_receiver_path,
                    rank,
                )
            }
            ExpressionNode::Call(call) => {
                if expression == target_call {
                    return true;
                }
                let receiver_path = append_table_expression_path(expressions, call.receiver);
                if call.target.as_str() == target
                    && receiver_path == target_receiver_path
                    && input
                        .state_calls
                        .calls_for_statement(source_key, statement_index)
                        .any(|state_call| {
                            state_call_has_runtime_value_result(state_call.role)
                                && state_call_matches_expression(
                                    input,
                                    state_call,
                                    call.target.as_str(),
                                    &receiver_path,
                                )
                        })
                {
                    *rank += 1;
                }
                (call.receiver.is_valid()
                    && visit(
                        input,
                        expressions,
                        source_key,
                        statement_index,
                        call.receiver,
                        target_call,
                        target,
                        target_receiver_path,
                        rank,
                    ))
                    || expressions
                        .expression_handles(call.arguments)
                        .iter()
                        .any(|argument| {
                            visit(
                                input,
                                expressions,
                                source_key,
                                statement_index,
                                *argument,
                                target_call,
                                target,
                                target_receiver_path,
                                rank,
                            )
                        })
            }
            ExpressionNode::Cast(cast) => visit(
                input,
                expressions,
                source_key,
                statement_index,
                cast.value,
                target_call,
                target,
                target_receiver_path,
                rank,
            ),
            ExpressionNode::Indexed(indexed) => {
                visit(
                    input,
                    expressions,
                    source_key,
                    statement_index,
                    indexed.collection,
                    target_call,
                    target,
                    target_receiver_path,
                    rank,
                ) || visit(
                    input,
                    expressions,
                    source_key,
                    statement_index,
                    indexed.index,
                    target_call,
                    target,
                    target_receiver_path,
                    rank,
                )
            }
            ExpressionNode::Member(member) => visit(
                input,
                expressions,
                source_key,
                statement_index,
                member.receiver,
                target_call,
                target,
                target_receiver_path,
                rank,
            ),
            ExpressionNode::Borrow(inner) => visit(
                input,
                expressions,
                source_key,
                statement_index,
                inner.target,
                target_call,
                target,
                target_receiver_path,
                rank,
            ),
            ExpressionNode::Range(range) => {
                (range.start.is_valid()
                    && visit(
                        input,
                        expressions,
                        source_key,
                        statement_index,
                        range.start,
                        target_call,
                        target,
                        target_receiver_path,
                        rank,
                    ))
                    || (range.end.is_valid()
                        && visit(
                            input,
                            expressions,
                            source_key,
                            statement_index,
                            range.end,
                            target_call,
                            target,
                            target_receiver_path,
                            rank,
                        ))
            }
            ExpressionNode::StructLiteral(struct_literal) => expressions
                .struct_fields(struct_literal.fields)
                .iter()
                .any(|field| {
                    visit(
                        input,
                        expressions,
                        source_key,
                        statement_index,
                        field.value,
                        target_call,
                        target,
                        target_receiver_path,
                        rank,
                    )
                }),
            ExpressionNode::Unary(unary) => visit(
                input,
                expressions,
                source_key,
                statement_index,
                unary.operand,
                target_call,
                target,
                target_receiver_path,
                rank,
            ),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => false,
        }
    }

    let mut rank = 0;
    visit(
        input,
        expressions,
        source_key,
        statement_index,
        root,
        target_call,
        target,
        target_receiver_path,
        &mut rank,
    )
    .then_some(rank)
}

pub(super) fn resolve_runtime_table_call_result_source_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    root: ExpressionHandle,
    call_expression: ExpressionHandle,
    call: &psi_checked_trees::expression::TableCallExpression,
    minimum_call_ordinal: Option<usize>,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
    let receiver_path = append_table_expression_path(expressions, call.receiver);
    let occurrence_rank = runtime_table_call_occurrence_rank(
        input,
        expressions,
        source_key,
        statement_index,
        root,
        call_expression,
        call.target.as_str(),
        &receiver_path,
    )?;
    resolve_matching_runtime_call_result_source_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        call.target.as_str(),
        &receiver_path,
        occurrence_rank,
        minimum_call_ordinal,
    )
}

fn resolve_runtime_call_expression_result_source_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call: &psi_checked_trees::expression::CallExpression,
    byte_count: usize,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
    resolve_runtime_tree_call_result_source_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        call,
    )
    .filter(|place| place.byte_count == byte_count)
}

fn state_call_has_runtime_value_result(role: StateCallRole) -> bool {
    matches!(
        role,
        StateCallRole::AssignmentValue
            | StateCallRole::CallArgument
            | StateCallRole::TransitionArgument
    )
}

fn resolve_runtime_call_result_source_place_by_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    state_call: &omega_state_calls::StateCall,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
    match state_call.role {
        StateCallRole::AssignmentValue => {
            resolve_runtime_assignment_value_call_result_place_by_ordinal(
                input,
                dispatch_index,
                state_call.source_key,
                state_call.statement_index,
                state_call.call_ordinal,
            )
        }
        StateCallRole::CallArgument => resolve_runtime_call_argument_call_result_place_by_ordinal(
            input,
            dispatch_index,
            state_call.source_key,
            state_call.statement_index,
            state_call.call_ordinal,
        ),
        _ => None,
    }
}

fn inline_branching_call_result_for_expression<'a>(
    input: &'a InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    call: &psi_checked_trees::expression::CallExpression,
) -> Option<&'a omega_state_calls::StateCall> {
    let mut receiver_path = Vec::new();
    if let Some(receiver) = call.receiver.as_deref() {
        append_tree_expression_path(receiver, &mut receiver_path);
    }
    input.state_calls.calls.iter().find_map(|(_, state_call)| {
        (state_call_has_runtime_value_result(state_call.role)
            && state_call.lowering == StateCallLowering::InlineBranching
            && super::super::state_key_matches_statement_source(state_call.source_key, source_key)
            && state_call.statement_index == statement_index
            && state_call_matches_expression(
                input,
                state_call,
                call.target.as_str(),
                &receiver_path,
            ))
        .then_some(state_call)
    })
}

fn inline_branching_call_argument_expansion_is_statically_selected(
    input: &InstructionSelectionInput<'_>,
    expansion: &omega_runtime_branching::RuntimeLeafBranchExpansion,
) -> bool {
    let summary = static_guard_conjunct_summary_in_table(
        input,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
    );
    expansion.target_value.is_valid()
        && !expansion.is_default_target
        && summary.has_true
        && !summary.has_false
}

fn inline_branching_call_argument_default_expansion_is_statically_selected(
    input: &InstructionSelectionInput<'_>,
    expansion: &omega_runtime_branching::RuntimeLeafBranchExpansion,
    siblings: &[&omega_runtime_branching::RuntimeLeafBranchExpansion],
) -> bool {
    if !expansion.target_value.is_valid() || !expansion.is_default_target {
        return false;
    }
    let summary = static_guard_conjunct_summary_in_table(
        input,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
    );
    if summary.has_false {
        return false;
    }

    siblings
        .iter()
        .filter(|sibling| sibling.target_value.is_valid() && !sibling.is_default_target)
        .all(|sibling| {
            static_guard_conjunct_summary_in_table(
                input,
                &input.runtime_branching_calls.expressions,
                sibling.resolved_guard,
            )
            .has_false
        })
}

fn materialize_static_inline_branching_call_argument_result(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call: &psi_checked_trees::expression::CallExpression,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(state_call) =
        inline_branching_call_result_for_expression(input, source_key, statement_index, call)
    else {
        return false;
    };
    materialize_static_inline_branching_state_call_argument_result(
        input,
        dispatch_index,
        state_call,
        static_values,
        runtime_value_operands,
        selected_instructions,
    )
}

fn materialize_static_inline_branching_call_argument_results_for_statement(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let mut emitted = false;
    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call_has_runtime_value_result(state_call.role)
            || state_call.lowering != StateCallLowering::InlineBranching
            || !super::super::state_key_matches_statement_source(state_call.source_key, source_key)
            || state_call.statement_index != statement_index
        {
            continue;
        }
        emitted |= materialize_static_inline_branching_state_call_argument_result(
            input,
            dispatch_index,
            state_call,
            static_values,
            runtime_value_operands,
            selected_instructions,
        );
    }
    emitted
}

fn materialize_static_inline_branching_state_call_argument_result(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    state_call: &omega_state_calls::StateCall,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(slot) = input.runtime_storage.call_result_slot_by_ordinal(
        dispatch_index,
        state_call.source_key,
        state_call.statement_index,
        state_call.role,
        state_call.call_ordinal,
    ) else {
        return false;
    };

    let siblings = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            expansion.dispatch_index == dispatch_index
                && super::super::state_key_matches_statement_source(
                    expansion.source_key,
                    state_call.source_key,
                )
                && expansion.statement_index == state_call.statement_index
                && expansion.role == state_call.role
                && expansion.call_ordinal == state_call.call_ordinal
        })
        .collect::<Vec<_>>();

    let selected = siblings
        .iter()
        .copied()
        .find(|expansion| {
            inline_branching_call_argument_expansion_is_statically_selected(input, expansion)
        })
        .or_else(|| {
            siblings.iter().copied().find(|expansion| {
                inline_branching_call_argument_default_expansion_is_statically_selected(
                    input, expansion, &siblings,
                )
            })
        });
    let Some(expansion) = selected else {
        return false;
    };

    let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        dispatch_index,
        expansion.branch_key,
        expansion.target_statement_index,
        &input.runtime_branching_calls.expressions,
        slot,
        expansion.target_value,
        static_values,
        runtime_value_operands,
    ) else {
        return false;
    };

    selected_instructions.push(SelectedInstruction {
        kind,
        source_key: expansion.branch_key,
        source_statement: expansion.target_statement_index,
    });
    true
}

fn resolve_static_inline_branching_call_expression_value(
    input: &InstructionSelectionInput<'_>,
    call: &psi_checked_trees::expression::CallExpression,
) -> Option<Expression> {
    resolve_static_inline_branching_call_expression_value_with_branch(input, call)
        .map(|(expression, _)| expression)
}

/// The selected expansion's terminal value AND its `branch_key` (the CALLEE
/// state). Despite the historical name, the terminal can be a RUNTIME
/// expression (a callee local like `shifted`, not just a foldable literal) --
/// any PLACE resolution of the returned expression must run in the returned
/// branch_key's context. Resolving it with the CALLER's source key sent the
/// bare name through the cross-source-key fallback ladder onto ANOTHER
/// callee's same-named local (the cross-callee let-name collision: the
/// Mutation fallback clobbered the first call's delivered result, TASKS.md).
fn resolve_static_inline_branching_call_expression_value_with_branch(
    input: &InstructionSelectionInput<'_>,
    call: &psi_checked_trees::expression::CallExpression,
) -> Option<(Expression, StateKey)> {
    // Candidate leaf expansions are matched by the call's TARGET STATE NAME, which
    // collides when two data types implement a same-named method (`Circle::code` /
    // `Square::code`): both impls' leafs answer to `code`, and the lexically-first
    // one would win for EVERY call site. Discriminate by the RECEIVER's static
    // type: `s.code()` through `s: &mut Circle` (or an alias-resolved
    // `(mut self.c).code()`) must only fold leafs of the machine attached to
    // Circle. When the receiver's type is not derivable (no filter), keep the full
    // candidate set (the pre-existing behavior).
    let receiver_machine = crate::selection::lookups::static_receiver_machine_for_call(
        input,
        call.receiver.as_deref(),
        call.target_symbol,
        &call.target,
    );
    let candidates = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            let (_, branch_state) = input
                .control_flow
                .state_names_by_key_cloned(expansion.branch_key);
            (expansion.branch_key.state == call.target_symbol
                || branch_state.as_str() == &*call.target)
                && receiver_machine.is_none_or(|machine| expansion.branch_key.machine == machine)
                && expansion.target_value.is_valid()
                && leaf_expansion_bindings_match_call_arguments(input, expansion, call)
        })
        .collect::<Vec<_>>();

    candidates
        .iter()
        .copied()
        .find(|expansion| {
            inline_branching_call_argument_expansion_is_statically_selected(input, expansion)
        })
        .or_else(|| {
            candidates.iter().copied().find(|expansion| {
                inline_branching_call_argument_default_expansion_is_statically_selected(
                    input,
                    expansion,
                    &candidates,
                )
            })
        })
        .map(|expansion| {
            (
                input
                    .runtime_branching_calls
                    .expressions
                    .to_tree(expansion.target_value),
                expansion.branch_key,
            )
        })
}

fn leaf_expansion_bindings_match_call_arguments(
    input: &InstructionSelectionInput<'_>,
    expansion: &omega_runtime_branching::RuntimeLeafBranchExpansion,
    call: &psi_checked_trees::expression::CallExpression,
) -> bool {
    let parameters = state_parameters(input, expansion.branch_key);
    let parameters = if parameters.len() == call.arguments.len().saturating_add(1)
        && call.receiver.is_some()
        && parameters
            .first()
            .is_some_and(|parameter| parameter.name.as_str() == "self")
    {
        &parameters[1..]
    } else {
        parameters
    };
    if parameters.len() != call.arguments.len() {
        return false;
    }
    let bindings = input
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    parameters
        .iter()
        .zip(call.arguments.iter())
        .all(|(parameter, argument)| {
            let Some(binding) = bindings.iter().rev().find(|binding| {
                binding.parameter_symbol == parameter.symbol
                    || binding.parameter_name == parameter.name
            }) else {
                return false;
            };
            let binding_expression = input
                .runtime_branching_calls
                .expressions
                .to_tree(binding.expression);
            static_inline_branching_argument_matches(input, bindings, &binding_expression, argument)
        })
}

fn static_inline_branching_argument_matches(
    input: &InstructionSelectionInput<'_>,
    bindings: &[omega_runtime_branching::RuntimeLeafBranchBinding],
    binding_expression: &Expression,
    argument: &Expression,
) -> bool {
    let binding_expression =
        resolve_static_inline_branching_binding_expression(input, bindings, binding_expression);
    let argument = resolve_static_inline_branching_binding_expression(input, bindings, argument);

    if binding_expression == argument {
        return true;
    }
    if static_inline_branching_expressions_match(input, bindings, &binding_expression, &argument) {
        return true;
    }

    let resolved_binding = match &binding_expression {
        Expression::Call(call) => {
            resolve_static_inline_branching_call_expression_value(input, call)
        }
        _ => None,
    };
    let resolved_argument = match &argument {
        Expression::Call(call) => {
            resolve_static_inline_branching_call_expression_value(input, call)
        }
        _ => None,
    };

    match (resolved_binding.as_ref(), resolved_argument.as_ref()) {
        (Some(binding), Some(argument)) => {
            binding == argument
                || static_inline_branching_expressions_match(input, bindings, binding, argument)
        }
        (Some(binding), None) => {
            binding == &argument
                || static_inline_branching_expressions_match(input, bindings, binding, &argument)
        }
        (None, Some(argument)) => {
            &binding_expression == argument
                || static_inline_branching_expressions_match(
                    input,
                    bindings,
                    &binding_expression,
                    argument,
                )
        }
        (None, None) => false,
    }
}

fn resolve_static_inline_branching_binding_expression(
    input: &InstructionSelectionInput<'_>,
    bindings: &[omega_runtime_branching::RuntimeLeafBranchBinding],
    expression: &Expression,
) -> Expression {
    let Expression::Name(name) = expression else {
        return expression.clone();
    };
    if name.len() != 1 {
        return expression.clone();
    }
    let Some(member) = name.first() else {
        return expression.clone();
    };
    let Some(binding) = bindings.iter().rev().find(|binding| {
        binding.parameter_symbol == name.symbol() || binding.parameter_name == *member
    }) else {
        return expression.clone();
    };
    let resolved = input
        .runtime_branching_calls
        .expressions
        .to_tree(binding.expression);
    if &resolved == expression {
        expression.clone()
    } else {
        resolved
    }
}

fn static_inline_branching_expressions_match(
    input: &InstructionSelectionInput<'_>,
    bindings: &[omega_runtime_branching::RuntimeLeafBranchBinding],
    left: &Expression,
    right: &Expression,
) -> bool {
    match (left, right) {
        (Expression::Name(left), Expression::Name(right)) => {
            left.symbol() == right.symbol()
                || (left.len() == right.len()
                    && left
                        .members()
                        .iter()
                        .zip(right.members().iter())
                        .all(|(left, right)| left == right))
        }
        (Expression::Call(left), Expression::Call(right)) => {
            left.target_symbol == right.target_symbol
                && left.target == right.target
                && left.arguments.len() == right.arguments.len()
                && left.arguments.iter().zip(right.arguments.iter()).all(
                    |(left_argument, right_argument)| {
                        static_inline_branching_argument_matches(
                            input,
                            bindings,
                            left_argument,
                            right_argument,
                        )
                    },
                )
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_mutation_writes(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let resolved_target = resolve_runtime_mutation_target(
        input,
        dispatch_index,
        source_key,
        target,
        aliases,
        alias_expressions,
    );
    select_runtime_resolved_target_value_source_mutation_writes(
        input,
        dispatch_index,
        source_key,
        resolved_target.source_key,
        value_source_key,
        source_machine,
        source_state,
        statement_index,
        &resolved_target.expression,
        value,
        aliases,
        alias_expressions,
        static_values,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_state_call_result_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    value_source_key: StateKey,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    scratch: &mut super::RuntimeStorageWriteScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input.runtime_storage.call_result_slot_by_ordinal(
        dispatch_index,
        operation_source_key,
        statement_index,
        role,
        call_ordinal,
    ) else {
        return;
    };
    scratch.expressions.clear();
    let value_expressions = &mut scratch.expressions;
    let copied_aliases =
        RuntimeAliasBuffer::copy_from_bindings(alias_expressions, aliases, value_expressions);
    let value_expression = value_expressions.copy_from(&input.runtime_bodies.expressions, value);
    let resolved_value = resolve_runtime_alias_binding_handle(
        value_expression,
        value_source_key,
        copied_aliases.bindings(),
        value_expressions,
    );
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        dispatch_index,
        resolved_value.source_key,
        statement_index,
        &value_expressions,
        slot,
        resolved_value.expression,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        dispatch_index,
        resolved_value.source_key,
        statement_index,
        &value_expressions,
        slot,
        resolved_value.expression,
        static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    let target = runtime_frame_slot_target_expression(value_expressions, slot);
    scratch.resolved_segment_expressions.clear();
    let copied_segment_aliases = RuntimeAliasBuffer::copy_from_bindings(
        value_expressions,
        copied_aliases.bindings(),
        &mut scratch.resolved_segment_expressions,
    );
    if runtime_text_builder_write_in_table_emit(
        input,
        dispatch_index,
        operation_source_key,
        operation_source_key,
        statement_index,
        value_expressions,
        target,
        &mut scratch.resolved_segment_expressions,
        &|expressions, expression| {
            resolve_runtime_alias_binding_handle(
                expression,
                operation_source_key,
                copied_segment_aliases.bindings(),
                expressions,
            )
            .expression
        },
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_source_key,
                source_statement: statement_index,
            });
        },
    ) {
        return;
    }

    let (value_machine, value_state) = input
        .control_flow
        .state_names_by_key_cloned(resolved_value.source_key);

    let target = value_expressions.to_tree(target);
    let value = value_expressions.to_tree(resolved_value.expression);
    scratch.resolved_segment_expressions.clear();

    select_runtime_resolved_target_value_source_mutation_writes(
        input,
        dispatch_index,
        operation_source_key,
        operation_source_key,
        resolved_value.source_key,
        &value_machine,
        &value_state,
        statement_index,
        &target,
        &value,
        aliases,
        alias_expressions,
        static_values,
        &mut scratch.resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
}

/// Flatten a left-associative `+` string-concat tree into its segments in source
/// order: `("== " + room.label) + " =="` -> `["== ", room.label, " =="]`. A
/// non-`Add` expression is a single segment.
fn flatten_string_concat_segments(value: &Expression) -> Vec<&Expression> {
    if let Expression::Binary(binary) = value
        && binary.operator == psi_checked_trees::expression::BinaryOperator::Add
    {
        let mut segments = flatten_string_concat_segments(&binary.left);
        segments.push(&binary.right);
        segments
    } else {
        vec![value]
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_runtime_resolved_target_value_source_mutation_writes(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    // A runtime-indexed write `arr[i] = ..` (non-constant index) can land on any
    // element of `arr`, so every folded constant for the whole collection is now
    // stale. Void it up front -- this covers every indexed sub-path below
    // (frame/machine copy, indexed-integer) in one place, and stays correct
    // because at runtime the RHS read precedes the write. No-op for non-indexed
    // and const-indexed targets, which keep precise per-place tracking.
    invalidate_runtime_static_collection_for_indexed_write(static_values, resolved_target);

    if let Expression::StructLiteral(struct_literal) = value {
        let literal_start = selected_instructions.len();
        let mut any_field_failed = false;
        for field in struct_literal.fields.iter() {
            let field_target =
                append_place_suffix(resolved_target, std::slice::from_ref(&field.name));
            let instructions_before = selected_instructions.len();
            select_runtime_resolved_target_value_source_mutation_writes(
                input,
                dispatch_index,
                operation_source_key,
                target_source_key,
                value_source_key,
                source_machine,
                source_state,
                statement_index,
                &field_target,
                &field.value,
                aliases,
                alias_expressions,
                static_values,
                resolved_segment_expressions,
                runtime_value_operands,
                selected_instructions,
            );
            if selected_instructions.len() == instructions_before {
                debug_unselected_runtime_mutation(
                    "case-literal field write emitted nothing",
                    source_machine,
                    source_state,
                    statement_index,
                    &field_target,
                    &field.value,
                );
                any_field_failed = true;
            }
        }
        // PARTIAL construction only (siblings landed, a field didn't): a
        // silent ZII field at runtime -- poison so emission planning rejects
        // it with the bind-to-a-`let` diagnostic; the partial writes never
        // encode. A FULLY-unserved literal emits nothing at all here, so the
        // caller's zero-growth check still falls through to its remaining
        // whole-value strategies.
        // Same no-aliases gate as the mod.rs decomposition: a call-SUBSTITUTED
        // literal (non-empty aliases -- the splice binds callee params) defers
        // computed members to the call's own delivery machinery, so partial
        // here is not final (constructor_computed_field's `a: n / 100`); the
        // call-terminal position is guarded by the branch cascade's poison.
        if any_field_failed && aliases.is_empty() && selected_instructions.len() > literal_start {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::EvaluateDispatchGuard {
                    guard_lowering:
                        omega_abstract_operations::StateGuardLowering::UnloweredCaseLiteralField,
                    operator: omega_abstract_operations::StateGuardOperator::Equal,
                    storage_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
                    byte_offset: 0,
                    byte_size: 0,
                    expected_value: 0,
                    has_storage: false,
                    is_float: false,
                },
                source_key: operation_source_key,
                source_statement: statement_index,
            });
        }
        return;
    }

    if let Expression::String(value) = value {
        let data = string_literal_data_handle(input, operation_source_key, statement_index, value);
        if data.is_valid()
            && let Some(target) = resolve_runtime_pointee_slot_offset(
                input,
                dispatch_index,
                target_source_key,
                resolved_target,
            )
        {
            let pointer_byte_offset = target.pointer_byte_offset;
            let field_byte_offset = target.field_byte_offset;
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::write_place_string_pointee(
                    pointer_byte_offset,
                    field_byte_offset,
                    data,
                    value.len(),
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
        select_runtime_string_descriptor_write(
            input,
            operation_source_key,
            target_source_key,
            source_machine,
            source_state,
            dispatch_index,
            statement_index,
            resolved_target,
            value,
            selected_instructions,
        );
        return;
    }

    // An all-literal concat (`"prefix " + "omega"`) writes its FOLDED value as
    // one descriptor write. The runtime-text planner folds these (no builder
    // is planned), so the builder path below cannot lower them; left to the
    // per-segment builder machinery they would also overwrite indexed targets
    // segment by segment instead of appending.
    if matches!(value, Expression::Binary(_))
        && let Some(folded) = fold_static_string_tree_value(value)
    {
        select_runtime_string_descriptor_write(
            input,
            operation_source_key,
            target_source_key,
            source_machine,
            source_state,
            dispatch_index,
            statement_index,
            resolved_target,
            &folded,
            selected_instructions,
        );
        return;
    }

    // A write value that is CONSTANT after value-call alias substitution: the
    // splice rewrites a callee's `options.write` through the caller's struct
    // LITERAL, leaving `(true as i32)` casts and `Member(StructLiteral, ..)`
    // reads under pure bitwise/shift arithmetic (open_with's per-target flag
    // word). Fold the whole tree to one integer and re-enter as a plain
    // constant store, which the existing paths lower. Restricted to the
    // sign-safe operator class (`| & ^ <<`; never `>> / %`, the known
    // const-fold signedness trap) and to bool-source casts (no possible
    // truncation), so a fold can never disagree with the interpreter's i64
    // evaluation of the same substituted tree.
    if matches!(value, Expression::Binary(_))
        && let Some(folded) = fold_substituted_constant_integer(value)
    {
        let folded_value =
            Expression::Integer(psi_numerics::literals::IntegerLiteral::from_value(folded));
        select_runtime_resolved_target_value_source_mutation_writes(
            input,
            dispatch_index,
            operation_source_key,
            target_source_key,
            value_source_key,
            source_machine,
            source_state,
            statement_index,
            resolved_target,
            &folded_value,
            aliases,
            alias_expressions,
            static_values,
            resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        );
        return;
    }

    // Owned `[u8; N]` carrier concat (`self.text = "== " + self.label + " =="`):
    // walk the left-associative `+` tree into segments. A first literal initializes
    // the target directly; a distinct first carrier initializes it through an empty
    // write followed by the normal source append. Each later segment is appended
    // onto the target's inline bytes at the running length. Both target and
    // source use the ordinary Place algebra, so direct fields and mutable
    // parameter pointees share one lowering.
    // The length-fits guard already proved the result fits the target's N. (Handles
    // the 2-segment `runtime_text_builder` shape as the n=2 special case.)
    let resolved_bounded_concat_value = if aliases.is_empty() {
        None
    } else {
        resolved_segment_expressions.clear();
        let copied_aliases = RuntimeAliasBuffer::copy_from_bindings(
            alias_expressions,
            aliases,
            resolved_segment_expressions,
        );
        let value_handle = resolved_segment_expressions.insert_tree(value);
        let resolved = resolve_runtime_alias_binding_handle(
            value_handle,
            operation_source_key,
            copied_aliases.bindings(),
            resolved_segment_expressions,
        );
        Some(resolved_segment_expressions.to_tree(resolved.expression))
    };
    let bounded_concat_value = resolved_bounded_concat_value.as_ref().unwrap_or(value);

    if let Expression::Binary(binary) = bounded_concat_value
        && binary.operator == psi_checked_trees::expression::BinaryOperator::Add
        && let Some(target_place) = resolve_bounded_buffer_target_place(
            input,
            dispatch_index,
            target_source_key,
            source_machine,
            source_state,
            resolved_target,
        )
    {
        let segments = flatten_string_concat_segments(bounded_concat_value);
        if let Some((first, rest)) = segments.split_first() {
            let mut kinds: Vec<SelectedInstructionKind> = Vec::with_capacity(segments.len());
            let mut all_segments_resolved = true;

            // Initialize the destination from the first segment. A literal can
            // write it directly. A distinct carrier source first establishes
            // the empty destination and then uses the common checked append;
            // an aliased source already supplies the running prefix in place.
            if let Expression::String(prefix) = first {
                kinds.push(SelectedInstructionKind::WritePlaceBoundedBuffer {
                    target: target_place,
                    literal: prefix.clone(),
                });
            } else if let Some(source_place) = resolve_bounded_buffer_target_place(
                input,
                dispatch_index,
                target_source_key,
                source_machine,
                source_state,
                first,
            ) {
                // `target = target + suffix` starts with the target's existing
                // content. A distinct source initializes through an empty value
                // before the common append operation.
                if source_place != target_place {
                    if resolve_direct_or_pointee_bounded_buffer_place(
                        input,
                        dispatch_index,
                        target_source_key,
                        source_machine,
                        source_state,
                        first,
                    ) == Some(source_place)
                    {
                        kinds.push(SelectedInstructionKind::WritePlaceBoundedBuffer {
                            target: target_place,
                            literal: std::sync::Arc::from(&b""[..]),
                        });
                        kinds.push(SelectedInstructionKind::AppendPlaceBoundedBufferSource {
                            target: target_place,
                            source: source_place,
                        });
                    } else {
                        all_segments_resolved = false;
                    }
                }
            } else {
                all_segments_resolved = false;
            }

            for segment in rest {
                if let Expression::String(literal) = segment {
                    kinds.push(SelectedInstructionKind::AppendPlaceBoundedBufferLiteral {
                        target: target_place,
                        literal: literal.clone(),
                    });
                } else if let Some(source_place) = resolve_bounded_buffer_target_place(
                    input,
                    dispatch_index,
                    target_source_key,
                    source_machine,
                    source_state,
                    segment,
                ) {
                    if resolve_direct_or_pointee_bounded_buffer_place(
                        input,
                        dispatch_index,
                        target_source_key,
                        source_machine,
                        source_state,
                        segment,
                    ) == Some(source_place)
                    {
                        kinds.push(SelectedInstructionKind::AppendPlaceBoundedBufferSource {
                            target: target_place,
                            source: source_place,
                        });
                    } else {
                        all_segments_resolved = false;
                        break;
                    }
                } else {
                    all_segments_resolved = false;
                    break;
                }
            }
            if all_segments_resolved {
                for kind in kinds {
                    selected_instructions.push(SelectedInstruction {
                        kind,
                        source_key: operation_source_key,
                        source_statement: statement_index,
                    });
                }
                return;
            }
        }
    }

    if runtime_text_builder_write_with_scratch_emit(
        input,
        dispatch_index,
        operation_source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        aliases,
        alias_expressions,
        resolved_segment_expressions,
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_source_key,
                source_statement: statement_index,
            });
        },
    ) {
        return;
    }

    let resolved_value = normalize_runtime_mutation_expression(
        input,
        value_source_key,
        statement_index,
        value,
        aliases,
        alias_expressions,
    );
    if let Expression::Call(call) = &resolved_value.expression
        && let Some((static_value, branch_key)) =
            resolve_static_inline_branching_call_expression_value_with_branch(input, call)
    {
        // The substituted terminal resolves in the CALLEE's context
        // (branch_key + its machine/state names), never the caller's: a bare
        // callee-local terminal (`shifted`) resolved with the caller's key
        // fell through the cross-source-key name ladder onto ANOTHER callee's
        // same-named local (the cross-callee let-name collision). Same-callee
        // multi-site stays correct: all sites share the callee's local slots
        // and splice contiguity keeps each site's values live at its own
        // Mutation op.
        let (value_machine, value_state) = input.control_flow.state_names_by_key_cloned(branch_key);
        select_runtime_resolved_target_value_source_mutation_writes(
            input,
            dispatch_index,
            operation_source_key,
            target_source_key,
            branch_key,
            &value_machine,
            &value_state,
            statement_index,
            resolved_target,
            &static_value,
            aliases,
            alias_expressions,
            static_values,
            resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        );
        return;
    }
    if let Expression::String(value) = &resolved_value.expression {
        select_runtime_string_descriptor_write(
            input,
            operation_source_key,
            target_source_key,
            source_machine,
            source_state,
            dispatch_index,
            statement_index,
            resolved_target,
            &value,
            selected_instructions,
        );
        return;
    }

    // A RANGE subslice (`arr[a..b]`) into a `&[u8]` view target materializes a
    // fat `{ptr, len}` descriptor, NOT a byte copy -- the copy paths below would
    // splat the bytes into the 16-byte descriptor slot. (Single-element copies
    // and array-into-array copies still fall through to the copies below.)
    if runtime_fixed_array_subslice_descriptor_write(
        input,
        dispatch_index,
        target_source_key,
        resolved_value.source_key,
        source_machine,
        source_state,
        operation_source_key,
        statement_index,
        resolved_target,
        &resolved_value.expression,
        selected_instructions,
    ) {
        return;
    }

    if let Some(copy) = runtime_fixed_array_subslice_indexed_source_copy(
        input,
        dispatch_index,
        target_source_key,
        resolved_value.source_key,
        source_machine,
        source_state,
        resolved_target,
        &resolved_value.expression,
    )
    .or_else(|| {
        runtime_storage_indexed_source_copy(
            input,
            dispatch_index,
            target_source_key,
            resolved_value.source_key,
            source_machine,
            source_state,
            resolved_target,
            &resolved_value.expression,
        )
    })
    .or_else(|| {
        runtime_storage_indirect_copy(
            input,
            dispatch_index,
            target_source_key,
            resolved_value.source_key,
            source_machine,
            source_state,
            resolved_target,
            &resolved_value.expression,
        )
    })
    .or_else(|| {
        runtime_storage_copy(
            input,
            dispatch_index,
            target_source_key,
            resolved_value.source_key,
            source_machine,
            source_state,
            resolved_target,
            &resolved_value.expression,
        )
    }) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && let Some(source_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        resolved_value.source_key,
        source_machine,
        source_state,
        &resolved_value.expression,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::copy_places_to_pointee(
                source_place.region,
                source_place.byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                source_place.byte_count,
            ),
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    // Only copy a call result DIRECTLY into the target when the value IS a bare,
    // non-builtin call (`target = self.f()`). When the call is a sub-expression of a
    // larger value (`target = self.f() + 1`, `target = max(x, self.f())`), this
    // statement-level copy would write just the call's result and silently drop the
    // surrounding operation; such values must fall through to the binary-write path
    // below, which resolves the call operand to its result slot AND applies the
    // operator. A builtin operator call (`max`/`min`) is itself an operator, not a
    // result-producing state call, so it must also fall through.
    if let Expression::Call(call) = &resolved_value.expression
        && builtin_runtime_call_operator(input, call).is_none()
        && let Some(source_place) = resolve_runtime_tree_call_result_source_place(
            input,
            dispatch_index,
            value_source_key,
            statement_index,
            call,
        )
    {
        materialize_static_inline_branching_call_argument_results_for_statement(
            input,
            dispatch_index,
            value_source_key,
            statement_index,
            static_values,
            runtime_value_operands,
            selected_instructions,
        );
        if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
            input,
            dispatch_index,
            target_source_key,
            resolved_target,
        ) && source_place.byte_count > 0
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_to_pointee(
                    source_place.region,
                    source_place.byte_offset,
                    pointer_target.pointer_byte_offset,
                    pointer_target.field_byte_offset,
                    source_place.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
            input,
            dispatch_index,
            target_source_key,
            resolved_target,
        ) && source_place.region == omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame
            && source_place.byte_count == indexed_target.byte_count
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_to_indexed(
                    source_place.region,
                    source_place.byte_offset,
                    indexed_target.descriptor_offset,
                    indexed_target.index_region,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    indexed_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        // A member read through a REFERENCE-typed slot on the RHS is a DEREF,
        // not a flat fold: `self.c = table.con_out` (or a `let` lifted to
        // machine storage) with `table: &EfiSystemTable` reads
        // [*(slot) + 64]. Tried BEFORE the flat resolver below, which would
        // read the frame bytes past the pointer slot (shared-ref-param-field-
        // read gap). The generalized pointee copy lands in the target region.
        if let Some(pointee) = resolve_runtime_pointee_slot_offset(
            input,
            dispatch_index,
            resolved_value.source_key,
            &resolved_value.expression,
        ) && let Some(target_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            target_source_key,
            source_machine,
            source_state,
            resolved_target,
        ) && matches!(
            target_place.region,
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame
                | omega_abstract_operations::RuntimeStorageRegion::Machine
        ) && pointee.pointee_byte_size == target_place.byte_count
            && pointee.pointee_byte_size > 0
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_from_pointee(
                    pointee.pointer_byte_offset,
                    pointee.field_byte_offset,
                    target_place.region,
                    target_place.byte_offset,
                    target_place.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        if let Some(target_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            target_source_key,
            source_machine,
            source_state,
            resolved_target,
        ) && target_place.byte_count == source_place.byte_count
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_direct(
                    source_place.region,
                    source_place.byte_offset,
                    target_place.region,
                    target_place.byte_offset,
                    target_place.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
    }

    if let Expression::Call(call) = &resolved_value.expression {
        if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target(
            input,
            dispatch_index,
            target_source_key,
            resolved_target,
        ) && let Some(field_byte_offset) = indexed_target.pointee_field_byte_offset()
            && let Some(source_place) = resolve_runtime_call_expression_result_source_place(
                input,
                dispatch_index,
                resolved_value.source_key,
                statement_index,
                call,
                indexed_target.byte_count,
            )
            && source_place.byte_count == indexed_target.byte_count
        {
            materialize_static_inline_branching_call_argument_result(
                input,
                dispatch_index,
                resolved_value.source_key,
                statement_index,
                call,
                static_values,
                runtime_value_operands,
                selected_instructions,
            );
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_to_pointee(
                    source_place.region,
                    source_place.byte_offset,
                    indexed_target.descriptor_offset,
                    field_byte_offset,
                    indexed_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
            input,
            dispatch_index,
            target_source_key,
            resolved_target,
        ) && let Some(source_place) = resolve_runtime_call_expression_result_source_place(
            input,
            dispatch_index,
            resolved_value.source_key,
            statement_index,
            call,
            pointer_target.pointee_byte_size,
        ) && source_place.byte_count > 0
        {
            materialize_static_inline_branching_call_argument_result(
                input,
                dispatch_index,
                resolved_value.source_key,
                statement_index,
                call,
                static_values,
                runtime_value_operands,
                selected_instructions,
            );
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_to_pointee(
                    source_place.region,
                    source_place.byte_offset,
                    pointer_target.pointer_byte_offset,
                    pointer_target.field_byte_offset,
                    source_place.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        if let Some(target_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            target_source_key,
            source_machine,
            source_state,
            resolved_target,
        ) && let Some(source_place) = resolve_runtime_call_expression_result_source_place(
            input,
            dispatch_index,
            resolved_value.source_key,
            statement_index,
            call,
            target_place.byte_count,
        ) && target_place.byte_count == source_place.byte_count
        {
            materialize_static_inline_branching_call_argument_result(
                input,
                dispatch_index,
                resolved_value.source_key,
                statement_index,
                call,
                static_values,
                runtime_value_operands,
                selected_instructions,
            );
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_direct(
                    source_place.region,
                    source_place.byte_offset,
                    target_place.region,
                    target_place.byte_offset,
                    target_place.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
    }

    if let Some(kind) = select_runtime_binary_mutation_write(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        resolved_value.source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        &resolved_value.expression,
        aliases,
        alias_expressions,
        static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    // Read a BOTH-RUNTIME element below a frame-held pointer into an ordinary
    // direct machine/frame place. The canonical copy keeps the full source
    // address algebra, so value capture and transition plumbing do not need a
    // pointee-double-indexed opcode variant.
    if let Some(double_source) = resolve_runtime_pointee_double_indexed_target(
        input,
        dispatch_index,
        resolved_value.source_key,
        &resolved_value.expression,
    ) && let Some(source) = double_source.place()
        && let Some(target) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            target_source_key,
            source_machine,
            source_state,
            resolved_target,
        )
        && target.byte_count == double_source.byte_count
        && target.byte_count > 0
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyPlaces {
                source,
                target: omega_abstract_operations::Place::at(target.region, target.byte_offset),
                byte_count: target.byte_count,
                role: omega_abstract_operations::CopyPlacesRole::Ordinary,
            },
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        if let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            resolved_value.source_key,
            source_machine,
            source_state,
            &resolved_value.expression,
        ) && source_place.region == omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_to_indexed(
                    source_place.region,
                    source_place.byte_offset,
                    indexed_target.descriptor_offset,
                    indexed_target.index_region,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    indexed_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        if supports_scalar_integer_write(indexed_target.byte_count)
            && let Some(value) = resolve_runtime_static_integer_value(
                input,
                operation_source_key,
                value,
                aliases,
                alias_expressions,
                static_values,
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::write_place_integer_frame_indexed(
                    indexed_target.descriptor_offset,
                    indexed_target.index_region,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    value,
                    indexed_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
    }

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_with_index_region(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        if supports_scalar_integer_write(indexed_target.byte_count)
            && let Some(value) = resolve_runtime_static_integer_value(
                input,
                operation_source_key,
                value,
                aliases,
                alias_expressions,
                static_values,
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::write_place_integer_base_indexed(
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    indexed_target.base_byte_offset,
                    indexed_target.index_region,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    value,
                    indexed_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
    }

    if let Some(indexed_target) = resolve_runtime_machine_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        // DUAL-indexed copy `arr[i] = arr[j]` (task #38): the VALUE is itself a
        // runtime-indexed machine element. This must be tried BEFORE the
        // storage-place source below -- `resolve_runtime_storage_place` on an
        // indexed read resolves to the array BASE (dropping the index), which
        // was the original silent miscompile this instruction closes.
        if let Some(indexed_source) = resolve_runtime_machine_indexed_target(
            input,
            dispatch_index,
            resolved_value.source_key,
            &resolved_value.expression,
        ) && indexed_source.byte_count == indexed_target.byte_count
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_machine_indexed_pair(
                    indexed_source.base_byte_offset,
                    indexed_source.index_region,
                    indexed_source.index_offset,
                    indexed_source.index_byte_size,
                    indexed_source.element_byte_size,
                    indexed_source.field_byte_offset,
                    indexed_target.base_byte_offset,
                    indexed_target.index_region,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    indexed_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        // Runtime-value source: `self.nums[self.j] = self.b`. The source field
        // resolves to a machine-resident storage place; the x86_64 encoder reads
        // the value, the index, and the target element all off the single shared
        // machine base, so it only handles a machine-region source.
        if let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            resolved_value.source_key,
            source_machine,
            source_state,
            &resolved_value.expression,
        ) && matches!(
            source_place.region,
            omega_abstract_operations::RuntimeStorageRegion::Machine
                | omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame
        ) && source_place.byte_count == indexed_target.byte_count
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_to_machine_indexed(
                    source_place.region,
                    source_place.byte_offset,
                    indexed_target.base_byte_offset,
                    indexed_target.index_region,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    indexed_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        if supports_scalar_integer_write(indexed_target.byte_count)
            && let Some(value) = resolve_runtime_static_integer_value(
                input,
                operation_source_key,
                value,
                aliases,
                alias_expressions,
                static_values,
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::write_place_integer_base_indexed(
                    omega_target_operations::RuntimeStorageRegion::Machine,
                    indexed_target.base_byte_offset,
                    indexed_target.index_region,
                    indexed_target.index_offset,
                    indexed_target.index_byte_size,
                    indexed_target.element_byte_size,
                    indexed_target.field_byte_offset,
                    value,
                    indexed_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
    }

    // BOTH-RUNTIME nested write below a frame-held recast/reference pointer.
    // Keep the pointer slot, both independent index slots, and both settled
    // strides in one composable Place; x86 walks it directly and AArch64
    // legalizes the same exact geometry.
    if let Some(double_target) = resolve_runtime_pointee_double_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && supports_scalar_integer_write(double_target.byte_count)
        && let Some(value) = resolve_runtime_static_integer_value(
            input,
            operation_source_key,
            value,
            aliases,
            alias_expressions,
            static_values,
        )
        && let Some(target) = double_target.place()
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WritePlaceInteger {
                target,
                value,
                byte_size: double_target.byte_count,
            },
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    // BOTH-RUNTIME nested write into a frame-resident inline 2D array
    // (`g[i][j] = 70`). The target and both index slots share one frame base;
    // runtime-place copies remain a separate CopyPlaces target-shape rung.
    if let Some(double_target) = resolve_runtime_frame_base_double_indexed_source(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && supports_scalar_integer_write(double_target.byte_count)
        && let Some(value) = resolve_runtime_static_integer_value(
            input,
            operation_source_key,
            value,
            aliases,
            alias_expressions,
            static_values,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_integer_double_indexed(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                double_target.base_byte_offset,
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                value,
                double_target.byte_count,
            ),
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    // Machine-resident double-indexed write twin. Tried after the single-index
    // block (mutually exclusive -- the double resolver requires BOTH indices
    // runtime).
    if let Some(double_target) = resolve_runtime_machine_double_indexed_source(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        // Runtime-place source: `grid[i][j] = self.v` / a param slot.
        if let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            resolved_value.source_key,
            source_machine,
            source_state,
            &resolved_value.expression,
        ) && matches!(
            source_place.region,
            omega_abstract_operations::RuntimeStorageRegion::Machine
                | omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame
        ) && source_place.byte_count == double_target.byte_count
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::copy_places_to_machine_double_indexed(
                    source_place.region,
                    source_place.byte_offset,
                    double_target.base_byte_offset,
                    double_target.outer_index_region,
                    double_target.outer_index_offset,
                    double_target.outer_index_byte_size,
                    double_target.outer_stride,
                    double_target.inner_index_region,
                    double_target.inner_index_offset,
                    double_target.inner_index_byte_size,
                    double_target.inner_stride,
                    double_target.field_byte_offset,
                    double_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }

        // Const-value source: `grid[i][j] = 70`.
        if supports_scalar_integer_write(double_target.byte_count)
            && let Some(value) = resolve_runtime_static_integer_value(
                input,
                operation_source_key,
                value,
                aliases,
                alias_expressions,
                static_values,
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind: crate::selection::runtime_dispatch::write_place_integer_double_indexed(
                    omega_abstract_operations::RuntimeStorageRegion::Machine,
                    double_target.base_byte_offset,
                    double_target.outer_index_region,
                    double_target.outer_index_offset,
                    double_target.outer_index_byte_size,
                    double_target.outer_stride,
                    double_target.inner_index_region,
                    double_target.inner_index_offset,
                    double_target.inner_index_byte_size,
                    double_target.inner_stride,
                    double_target.field_byte_offset,
                    value,
                    double_target.byte_count,
                ),
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
    }

    if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
        && let Some(value) = resolve_runtime_static_integer(
            input,
            operation_source_key,
            &value,
            aliases,
            alias_expressions,
            static_values,
        )
        && let Some(field_byte_offset) = indexed_target.pointee_field_byte_offset()
    {
        set_runtime_static_value(
            static_values,
            strip_mutable_expression(resolved_target.clone()),
            value,
        );
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_integer_pointee(
                indexed_target.descriptor_offset,
                field_byte_offset,
                value.bits(),
                indexed_target.byte_count,
            ),
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    let Some(value) = resolve_runtime_static_integer(
        input,
        operation_source_key,
        value,
        aliases,
        alias_expressions,
        static_values,
    ) else {
        debug_unselected_runtime_mutation(
            "static integer value did not resolve",
            source_machine,
            source_state,
            statement_index,
            resolved_target,
            &value,
        );
        return;
    };
    // Decision 17: a constant stored into a Saturating-domain target clamps to
    // the target type's range. The const-fold that produced this value already
    // dropped the operand domains (`self.v: u8 in Saturating = a * b`, a,b folded
    // to 100), so the target's declared domain is the authoritative signal -- the
    // store must write 255, not the wrapped low byte. See task #39.
    let value = value.with_bits(clamp_constant_to_target_domain(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
        value.bits(),
    ));
    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        set_runtime_static_value(
            static_values,
            strip_mutable_expression(resolved_target.clone()),
            value,
        );
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_integer_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                value.bits(),
                pointer_target.pointee_byte_size,
            ),
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }
    let Some(target_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        resolved_target,
    ) else {
        debug_unselected_runtime_mutation(
            "runtime storage target did not resolve",
            source_machine,
            source_state,
            statement_index,
            resolved_target,
            &value,
        );
        return;
    };
    if !supports_scalar_integer_write(target_place.byte_count) {
        debug_unselected_runtime_mutation(
            "runtime storage target is not scalar-write sized",
            source_machine,
            source_state,
            statement_index,
            resolved_target,
            &value,
        );
        return;
    }

    // Decision 17 (task #39): a constant stored into a Trapping target whose value
    // is out of the target type's range is a guaranteed overflow -- it MUST trap at
    // runtime (frozen decision: Trapping overflow traps at runtime and compiles
    // cleanly, matching the field-operand path; it is NOT a compile error). The
    // value was const-folded, losing the operands, so re-emit a guaranteed-
    // overflowing trapping op (`max + 1` / `min - 1` at the target width) and let
    // the binary-write encoder's existing trap (x86 ud2 / aarch64 brk) fire. No
    // standalone trap instruction exists; this reuses WriteRuntimeStorageBinary.
    if let Some(kind) = trapping_constant_overflow_write(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
        &target_place,
        value.bits(),
        runtime_value_operands,
    ) {
        // The program traps here, so nothing downstream observes the target; no
        // need to record a static value for it.
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    set_runtime_static_value(
        static_values,
        strip_mutable_expression(resolved_target.clone()),
        value,
    );
    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::write_place_integer_direct(
            target_place.region,
            target_place.byte_offset,
            value.bits(),
            target_place.byte_count,
        ),
        source_key: operation_source_key,
        source_statement: statement_index,
    });
}

/// Fold a write value that became CONSTANT through value-call alias
/// substitution (a callee's param member rewritten to the caller's struct
/// LITERAL). Leaves: integer literals, bool-source casts (`true as i32` -- a
/// 0/1, no truncation possible), and struct-literal member reads (an ABSENT
/// field folds to its ZII default 0, matching the interpreter). Operators:
/// ONLY the sign-safe bitwise class `| & ^` and `<<` with an in-range
/// amount -- the i64 fold agrees with any operand width for these (see the
/// const-fold MISCOMPILE-CLASS notes); `>>`/`/`/`%` are deliberately not
/// folded. Anything else (names, calls, indexes) -> None, and the caller
/// falls through to the existing paths.
fn fold_substituted_constant_integer(value: &Expression) -> Option<i64> {
    use psi_checked_trees::expression::BinaryOperator;
    match value {
        Expression::Integer(literal) => literal.value_i64(),
        Expression::Borrow(inner) => fold_substituted_constant_integer(&inner.target),
        Expression::Cast(cast) => {
            // Only a BOOL-source cast folds (0/1 into any integer width is
            // truncation-free). The bool may itself be a struct-literal
            // member read.
            match bool_leaf_value(&cast.value) {
                Some(value) => Some(i64::from(value)),
                None => None,
            }
        }
        Expression::Member(member) => {
            let Expression::StructLiteral(literal) = &member.receiver else {
                return None;
            };
            literal
                .fields
                .iter()
                .find(|field| field.name == member.member)
                // ZII: a field absent from the literal is its zero default.
                .map_or(Some(0), |field| {
                    fold_substituted_constant_integer(&field.value)
                })
        }
        Expression::Binary(binary) => {
            let left = fold_substituted_constant_integer(&binary.left)?;
            let right = fold_substituted_constant_integer(&binary.right)?;
            match binary.operator {
                BinaryOperator::BitwiseOr => Some(left | right),
                BinaryOperator::BitwiseAnd => Some(left & right),
                BinaryOperator::BitwiseXor => Some(left ^ right),
                BinaryOperator::ShiftLeft => {
                    let amount = u32::try_from(right).ok().filter(|amount| *amount < 32)?;
                    Some(left << amount)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// A bool LEAF under a cast: a literal `true`/`false`, or a struct-literal
/// member read whose field is a bool literal (absent field = ZII false).
fn bool_leaf_value(expression: &Expression) -> Option<bool> {
    match expression {
        Expression::Boolean(value) => Some(*value),
        Expression::Borrow(inner) => bool_leaf_value(&inner.target),
        Expression::Member(member) => {
            let Expression::StructLiteral(literal) = &member.receiver else {
                return None;
            };
            literal
                .fields
                .iter()
                .find(|field| field.name == member.member)
                .map_or(Some(false), |field| bool_leaf_value(&field.value))
        }
        _ => None,
    }
}

fn debug_unselected_runtime_mutation(
    reason: &str,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    target: &Expression,
    value: &impl std::fmt::Debug,
) {
    if std::env::var_os("OMEGA_DEBUG_MUTATION_SELECTION").is_none() {
        return;
    }

    eprintln!(
        "runtime mutation not selected: {source_machine}::{source_state} statement {statement_index}: {target:?} = {value:?}: {reason}"
    );
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_binary_mutation_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    _operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let (value, atomic_ordering) = match value {
        Expression::Atomic(atomic) => match atomic.ordering {
            psi_language_core::AtomicOrderingPlan::ReadModifyWrite(_) => {
                (&atomic.value, Some(atomic.ordering))
            }
            // Compare-exchange is selected from the authoritative table shape;
            // never reinterpret its synthetic arithmetic as an ordinary write.
            _ => return None,
        },
        value => (value, None),
    };
    let (operator, left_expression, right_expression) = match value {
        Expression::Binary(binary) => (
            runtime_binary_operator(binary.operator)?,
            &binary.left,
            &binary.right,
        ),
        Expression::Call(call) => {
            let operator = builtin_runtime_call_operator(input, call)?;
            let [left, right] = &*call.arguments else {
                return None;
            };
            (operator, left, right)
        }
        _ => return None,
    };
    // Same signedness policy as the `_in_table` binary writes: landed unsigned
    // operands pick the unsigned division/modulo/shift/min/max/comparison
    // encoding without being reinterpreted by the write target.
    let operator = binary_table_writes::signedness_adjusted_operator_for_tree_operands(
        input,
        dispatch_index,
        value_source_key,
        left_expression,
        right_expression,
        operator,
    );
    let left = resolve_runtime_value_operand(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        statement_index,
        left_expression,
        aliases,
        alias_expressions,
        static_values,
        runtime_value_operands,
    );
    let Some(left) = left else {
        debug_unselected_runtime_mutation(
            "left runtime value operand did not resolve",
            source_machine,
            source_state,
            statement_index,
            resolved_target,
            left_expression,
        );
        return None;
    };
    let right = resolve_runtime_value_operand(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        statement_index,
        right_expression,
        aliases,
        alias_expressions,
        static_values,
        runtime_value_operands,
    );
    let Some(right) = right else {
        debug_unselected_runtime_mutation(
            "right runtime value operand did not resolve",
            source_machine,
            source_state,
            statement_index,
            resolved_target,
            right_expression,
        );
        return None;
    };

    // The Atomic wrapper makes the left operand the destination for the prior
    // value returned by the RMW instruction. An unwrapped arithmetic assignment
    // remains an ordinary write even when the target's carrier is atomic.
    if let Some(ordering) = atomic_ordering
        && matches!(
            operator,
            StateGuardOperator::Add
                | StateGuardOperator::Subtract
                | StateGuardOperator::BitwiseXor
                | StateGuardOperator::BitwiseOr
                | StateGuardOperator::BitwiseAnd
        )
        && runtime_storage_target_is_atomic(
            input,
            dispatch_index,
            target_source_key,
            resolved_target,
        )
        && let Some(target_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            target_source_key,
            source_machine,
            source_state,
            resolved_target,
        )
        && let Some(result_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            value_source_key,
            source_machine,
            source_state,
            left_expression,
        )
        && target_place.byte_count > 0
    {
        return Some(match operator {
            StateGuardOperator::Add => SelectedInstructionKind::AtomicFetchAdd {
                target_region: target_place.region,
                target_offset: target_place.byte_offset,
                byte_size: target_place.byte_count,
                result_region: result_place.region,
                result_offset: result_place.byte_offset,
                delta: right,
                ordering,
            },
            StateGuardOperator::Subtract => SelectedInstructionKind::AtomicFetchSub {
                target_region: target_place.region,
                target_offset: target_place.byte_offset,
                byte_size: target_place.byte_count,
                result_region: result_place.region,
                result_offset: result_place.byte_offset,
                delta: right,
                ordering,
            },
            StateGuardOperator::BitwiseXor => SelectedInstructionKind::AtomicFetchXor {
                target_region: target_place.region,
                target_offset: target_place.byte_offset,
                byte_size: target_place.byte_count,
                result_region: result_place.region,
                result_offset: result_place.byte_offset,
                value: right,
                ordering,
            },
            StateGuardOperator::BitwiseOr => SelectedInstructionKind::AtomicFetchOr {
                target_region: target_place.region,
                target_offset: target_place.byte_offset,
                byte_size: target_place.byte_count,
                result_region: result_place.region,
                result_offset: result_place.byte_offset,
                value: right,
                ordering,
            },
            StateGuardOperator::BitwiseAnd => SelectedInstructionKind::AtomicFetchAnd {
                target_region: target_place.region,
                target_offset: target_place.byte_offset,
                byte_size: target_place.byte_count,
                result_region: result_place.region,
                result_offset: result_place.byte_offset,
                value: right,
                ordering,
            },
            _ => unreachable!("fetch arithmetic gate accepts add/sub/xor/or/and only"),
        });
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_frame_indexed(
                indexed_target.descriptor_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
                left,
                operator,
                right,
            ),
        );
    }

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_with_index_region(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_base_indexed(
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
                left,
                operator,
                right,
            ),
        );
    }

    // Machine-owned runtime-indexed array element: `self.arr[self.i] = a OP b`.
    // Sibling of the frame-base branch above, targeting the MACHINE region (like
    // `WriteRuntimeMachineIndexedInteger`). Placed after the frame-base branch and
    // before the pointee branch.
    if let Some(indexed_target) = resolve_runtime_machine_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_base_indexed(
                omega_target_operations::RuntimeStorageRegion::Machine,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
                left,
                operator,
                right,
            ),
        );
    }

    // BOTH-RUNTIME nested target (`grid[i][j] = a OP b`): the double-indexed
    // binary write. Ordinary operands already resolve here; frame-local
    // double-indexed RHS hoisting remains a separate frontend slotting gap.
    if let Some(double_target) = resolve_runtime_frame_base_double_indexed_source(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_double_indexed(
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                double_target.base_byte_offset,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
                left,
                operator,
                right,
            ),
        );
    }

    if let Some(double_target) = resolve_runtime_machine_double_indexed_source(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_double_indexed(
                omega_target_operations::RuntimeStorageRegion::Machine,
                double_target.base_byte_offset,
                double_target.outer_index_region,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                double_target.inner_index_region,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
                left,
                operator,
                right,
            ),
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                pointer_target.pointee_byte_size,
                left,
                operator,
                right,
            ),
        );
    }

    // `slice[const].field = left OP right` where `slice` is a {ptr,len} descriptor
    // in the frame: write through the descriptor's data pointer (deref + index*elem
    // + field), so a slice taken from a runtime &mut pointer reaches the real
    // referent rather than a constant-folded static place.
    if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) && let Some(field_byte_offset) = indexed_target.pointee_field_byte_offset()
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_binary_pointee(
                indexed_target.descriptor_offset,
                field_byte_offset,
                indexed_target.byte_count,
                left,
                operator,
                right,
            ),
        );
    }

    let target_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        resolved_target,
    )?;

    // A float TARGET performs the op on the SSE unit (matches the table path's
    // is_float keying). Without this, float arithmetic into a LOCAL (`let c: f64 =
    // a + b`, which routes through this non-table path) emits an integer add over
    // the IEEE bit patterns. f64 (8-byte, addsd) and f32 (4-byte, addss) — the
    // encoder selects the scalar width from the target byte_size.
    let is_float = matches!(
        resolve_runtime_storage_primitive_type(
            input,
            dispatch_index,
            target_source_key,
            resolved_target,
        ),
        Some(PrimitiveType::F64 | PrimitiveType::F32)
    );

    // Owned expression trees have no canonical checked-node identity. Never
    // reconstruct float semantics from their operand storage types; authored
    // float operations must lower through the provenance-preserving table path.
    if is_float {
        return None;
    }
    let domain = resolve_binary_write_arithmetic_domain(
        input,
        dispatch_index,
        value_source_key,
        left_expression,
        right_expression,
    );
    let target_signed =
        resolve_runtime_storage_is_signed(input, dispatch_index, value_source_key, left_expression)
            .or_else(|| {
                resolve_runtime_storage_is_signed(
                    input,
                    dispatch_index,
                    value_source_key,
                    right_expression,
                )
            })
            .unwrap_or(false);
    Some(
        crate::selection::runtime_dispatch::write_place_binary_direct(
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        ),
    )
}

/// Decision 17 (task #39): clamp a compile-time-constant store to a
/// Saturating-domain target's representable range. Constant folding collapses a
/// domained arithmetic expression (`a * b`, a,b `u8 in Saturating`) into a bare
/// literal that has lost its operands' domain, then this constant reaches the
/// store. The target field's declared domain is the surviving authoritative
/// signal, so a Saturating target clamps the value to its type range (matching
/// the runtime Saturating op). Exact (in-range by validation), Wrapping (the
/// truncating store wraps), and Trapping (constant overflow rejected upstream in
/// validation) are returned unchanged.
fn clamp_constant_to_target_domain(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    resolved_target: &Expression,
    value: i64,
) -> i64 {
    if resolve_runtime_storage_arithmetic_domain(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) != psi_numerics::arithmetic::ArithmeticDomain::Saturating
    {
        return value;
    }
    let Some((low, high)) = resolve_runtime_storage_primitive_type(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    )
    .and_then(saturating_integer_bounds) else {
        return value;
    };
    value.max(low).min(high)
}

/// Decision 17 (task #39): when a constant stored into a Trapping target is out
/// of the target type's range, return a guaranteed-overflowing trapping binary
/// write (`max + 1` for an over-range value, `min - 1` for under-range) so the
/// binary-write encoder's trap (x86 `ud2` / aarch64 `brk`) fires at runtime --
/// matching the frozen "Trapping overflow traps at runtime" semantics. The const
/// fold dropped the real operands, so any op that provably overflows the target
/// width reproduces the trap. `None` when the target is not Trapping, not a known
/// integer primitive, or the value is in range (then a normal store is correct).
fn trapping_constant_overflow_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    resolved_target: &Expression,
    target_place: &super::super::super::storage_places::RuntimeStoragePlace,
    value: i64,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    if resolve_runtime_storage_arithmetic_domain(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) != psi_numerics::arithmetic::ArithmeticDomain::Trapping
    {
        return None;
    }
    let primitive = resolve_runtime_storage_primitive_type(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    )?;
    let (min, max) = saturating_integer_bounds(primitive)?;
    if value >= min && value <= max {
        return None;
    }
    let (bound, operator) = if value > max {
        (max, StateGuardOperator::Add)
    } else {
        (min, StateGuardOperator::Subtract)
    };
    let left = runtime_value_operands.insert(RuntimeValueOperand::Immediate(bound));
    let right = runtime_value_operands.insert(RuntimeValueOperand::Immediate(1));
    Some(
        crate::selection::runtime_dispatch::write_place_binary_direct(
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
            left,
            operator,
            right,
            false,
            psi_numerics::arithmetic::ArithmeticDomain::Trapping,
            primitive.is_signed_integer(),
        ),
    )
}

/// Inclusive [min, max] of an integer primitive as `i64` (u64/usize high end
/// capped at `i64::MAX`, which is sufficient for clamping a folded `i64`
/// constant). `None` for non-integer primitives.
fn saturating_integer_bounds(primitive: PrimitiveType) -> Option<(i64, i64)> {
    match primitive {
        PrimitiveType::I8 => Some((i8::MIN as i64, i8::MAX as i64)),
        PrimitiveType::U8 => Some((0, u8::MAX as i64)),
        PrimitiveType::I16 => Some((i16::MIN as i64, i16::MAX as i64)),
        PrimitiveType::U16 => Some((0, u16::MAX as i64)),
        PrimitiveType::I32 => Some((i32::MIN as i64, i32::MAX as i64)),
        PrimitiveType::U32 => Some((0, u32::MAX as i64)),
        PrimitiveType::I64 => Some((i64::MIN, i64::MAX)),
        PrimitiveType::U64 | PrimitiveType::Addr => Some((0, i64::MAX)),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => None,
    }
}

/// Fold an all-literal `+` tree to the single string it denotes; `None` when
/// any leaf is not a string literal. Mirrors the runtime-text planner's fold
/// (which classifies these writes as StaticText) and the data planner's
/// folded-literal object, so the descriptor write finds matching bytes.
fn fold_static_string_tree_value(value: &Expression) -> Option<Vec<u8>> {
    match value {
        Expression::String(value) => Some(value.to_vec()),
        Expression::Binary(binary)
            if binary.operator == psi_checked_trees::expression::BinaryOperator::Add =>
        {
            let mut folded = fold_static_string_tree_value(&binary.left)?;
            folded.extend_from_slice(&fold_static_string_tree_value(&binary.right)?);
            Some(folded)
        }
        _ => None,
    }
}
