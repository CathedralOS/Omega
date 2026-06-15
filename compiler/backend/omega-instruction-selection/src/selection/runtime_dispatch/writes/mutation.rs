mod binary_table_writes;
mod frame_slots;
mod normalization;
mod operators;
mod static_writes;
mod value_operands;

use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::lookups::state_parameters;
use omega_abstract_operations::{
    RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
};
use omega_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_state_calls::{StateCallLowering, StateCallRole};

use super::super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, append_place_suffix,
    resolve_runtime_alias_binding_handle, strip_mutable_expression,
};
use super::super::super::storage_places::{
    resolve_runtime_storage_arithmetic_domain, resolve_runtime_storage_is_signed,
    resolve_runtime_storage_place, resolve_runtime_storage_primitive_type,
};
use omega_checked_trees::types::PrimitiveType;
use super::super::super::storage_places::{
    resolve_runtime_assignment_value_call_result_place,
    resolve_runtime_assignment_value_call_result_place_by_ordinal,
    resolve_runtime_call_argument_call_result_place,
    resolve_runtime_call_argument_call_result_place_by_ordinal,
    resolve_runtime_frame_base_indexed_target, resolve_runtime_frame_fixed_indexed_target,
    resolve_runtime_frame_indexed_target, resolve_runtime_machine_indexed_target,
    resolve_runtime_pointee_slot_offset, resolve_runtime_transition_argument_call_result_place,
};
use super::super::guards::static_guard_conjunct_summary_in_table;
use super::super::text_writes::{
    runtime_text_builder_write_in_table_emit, runtime_text_builder_write_with_scratch_emit,
    select_runtime_string_descriptor_write, string_literal_data_handle,
};
use super::slice_descriptors::emit_runtime_frame_slot_slice_descriptor_write_in_table;
use super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value, set_runtime_static_value,
};
use super::storage_copy::runtime_storage_copy;
use super::storage_copy::runtime_storage_indexed_source_copy;
use super::storage_copy::runtime_storage_indirect_copy;
use super::subslice_copy::runtime_fixed_array_subslice_indexed_source_copy;
pub(super) use binary_table_writes::{
    select_runtime_binary_mutation_write_in_table, select_runtime_convert_mutation_write_in_table,
    select_runtime_frame_slot_convert_write_in_table, select_runtime_storage_binary_write_in_table,
};
pub(in crate::selection::runtime_dispatch) use binary_table_writes::{
    signedness_adjusted_operator, signedness_adjusted_operator_for_operands,
};
pub(in crate::selection) use frame_slots::{
    runtime_frame_slot_target_expression, select_runtime_frame_slot_value_write_in_table,
    select_runtime_frame_slot_value_write_in_table_with_source_anchor,
};
pub(super) use normalization::simplify_runtime_expression_with_state_locals;
use normalization::{normalize_runtime_mutation_expression, resolve_runtime_mutation_target};
use operators::{
    builtin_runtime_call_operator, runtime_binary_operator, supports_scalar_integer_write,
};
pub(super) use static_writes::select_runtime_static_mutation_write_in_table;
pub(in crate::selection::runtime_dispatch) use value_operands::resolve_runtime_text_equals_operand_in_table;
use value_operands::resolve_runtime_value_operand;

fn resolve_runtime_call_result_source_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
    resolve_runtime_assignment_value_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
    )
    .or_else(|| {
        resolve_runtime_call_argument_call_result_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
        )
    })
    .or_else(|| {
        resolve_runtime_transition_argument_call_result_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
        )
    })
}

fn resolve_runtime_call_expression_result_source_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call: &omega_checked_trees::expression::CallExpression,
    byte_count: usize,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
    input
        .state_calls
        .calls
        .iter()
        .filter(|(_, state_call)| {
            let (_, target_state) = input
                .control_flow
                .state_names_by_key_cloned(state_call.target_key);
            state_call_has_runtime_value_result(state_call.role)
                && super::super::state_key_matches_statement_source(
                    state_call.source_key,
                    source_key,
                )
                && state_call.statement_index == statement_index
                && target_state.as_str() == &*call.target
        })
        .find_map(|(_, state_call)| {
            resolve_runtime_call_result_source_place_by_ordinal(input, dispatch_index, state_call)
                .filter(|place| place.byte_count == byte_count)
        })
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
    call: &omega_checked_trees::expression::CallExpression,
) -> Option<&'a omega_state_calls::StateCall> {
    input.state_calls.calls.iter().find_map(|(_, state_call)| {
        let (_, target_state) = input
            .control_flow
            .state_names_by_key_cloned(state_call.target_key);
        (state_call_has_runtime_value_result(state_call.role)
            && state_call.lowering == StateCallLowering::InlineBranching
            && super::super::state_key_matches_statement_source(state_call.source_key, source_key)
            && state_call.statement_index == statement_index
            && target_state.as_str() == &*call.target)
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
    call: &omega_checked_trees::expression::CallExpression,
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
    call: &omega_checked_trees::expression::CallExpression,
) -> Option<Expression> {
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
                && receiver_machine
                    .is_none_or(|machine| expansion.branch_key.machine == machine)
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
            input
                .runtime_branching_calls
                .expressions
                .to_tree(expansion.target_value)
        })
}

fn leaf_expansion_bindings_match_call_arguments(
    input: &InstructionSelectionInput<'_>,
    expansion: &omega_runtime_branching::RuntimeLeafBranchExpansion,
    call: &omega_checked_trees::expression::CallExpression,
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
    if let Expression::StructLiteral(struct_literal) = value {
        for field in struct_literal.fields.iter() {
            let field_target =
                append_place_suffix(resolved_target, std::slice::from_ref(&field.name));
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
                kind: SelectedInstructionKind::WriteRuntimePointeeString {
                    pointer_byte_offset,
                    field_byte_offset,
                    data,
                    byte_length: value.len(),
                },
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
        && let Some(static_value) =
            resolve_static_inline_branching_call_expression_value(input, call)
    {
        select_runtime_resolved_target_value_source_mutation_writes(
            input,
            dispatch_index,
            operation_source_key,
            target_source_key,
            resolved_value.source_key,
            source_machine,
            source_state,
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
            kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                source_region: source_place.region,
                source_offset: source_place.byte_offset,
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_count: source_place.byte_count,
            },
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
    if matches!(&resolved_value.expression, Expression::Call(call)
            if builtin_runtime_call_operator(input, call).is_none())
        && let Some(source_place) = resolve_runtime_call_result_source_place(
            input,
            dispatch_index,
            value_source_key,
            statement_index,
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
                kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    pointer_byte_offset: pointer_target.pointer_byte_offset,
                    field_byte_offset: pointer_target.field_byte_offset,
                    byte_count: source_place.byte_count,
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
        ) && source_place.region == omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame
            && source_place.byte_count == indexed_target.byte_count
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    descriptor_offset: indexed_target.descriptor_offset,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_count: indexed_target.byte_count,
                },
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
                kind: SelectedInstructionKind::CopyRuntimeStorage {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    target_region: target_place.region,
                    target_offset: target_place.byte_offset,
                    byte_count: target_place.byte_count,
                },
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
                kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    pointer_byte_offset: indexed_target.descriptor_offset,
                    field_byte_offset,
                    byte_count: indexed_target.byte_count,
                },
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
                kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    pointer_byte_offset: pointer_target.pointer_byte_offset,
                    field_byte_offset: pointer_target.field_byte_offset,
                    byte_count: source_place.byte_count,
                },
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
                kind: SelectedInstructionKind::CopyRuntimeStorage {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    target_region: target_place.region,
                    target_offset: target_place.byte_offset,
                    byte_count: target_place.byte_count,
                },
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
                kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
                    source_region: source_place.region,
                    source_offset: source_place.byte_offset,
                    descriptor_offset: indexed_target.descriptor_offset,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_count: indexed_target.byte_count,
                },
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
                kind: SelectedInstructionKind::WriteRuntimeFrameIndexedInteger {
                    descriptor_offset: indexed_target.descriptor_offset,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_size: indexed_target.byte_count,
                    value,
                },
                source_key: operation_source_key,
                source_statement: statement_index,
            });
            return;
        }
    }

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target(
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
                kind: SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger {
                    base_byte_offset: indexed_target.base_byte_offset,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_size: indexed_target.byte_count,
                    value,
                },
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
                kind: SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
                    base_byte_offset: indexed_target.base_byte_offset,
                    index_region: indexed_target.index_region,
                    index_offset: indexed_target.index_offset,
                    element_byte_size: indexed_target.element_byte_size,
                    field_byte_offset: indexed_target.field_byte_offset,
                    byte_size: indexed_target.byte_count,
                    value,
                },
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
        && let Some(value) = resolve_runtime_static_integer_value(
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
            kind: SelectedInstructionKind::WriteRuntimePointeeInteger {
                pointer_byte_offset: indexed_target.descriptor_offset,
                field_byte_offset,
                byte_size: indexed_target.byte_count,
                value,
            },
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return;
    }

    let Some(value) = resolve_runtime_static_integer_value(
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
            kind: SelectedInstructionKind::WriteRuntimePointeeInteger {
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_size: pointer_target.pointee_byte_size,
                value,
            },
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

    set_runtime_static_value(
        static_values,
        strip_mutable_expression(resolved_target.clone()),
        value,
    );
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: target_place.region,
            byte_offset: target_place.byte_offset,
            byte_size: target_place.byte_count,
            value,
        },
        source_key: operation_source_key,
        source_statement: statement_index,
    });
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
    // Same signedness policy as the `_in_table` binary writes: unsigned operands
    // pick the unsigned division/modulo/shift/min/max/comparison encoding.
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

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(SelectedInstructionKind::WriteRuntimeFrameIndexedBinary {
            descriptor_offset: indexed_target.descriptor_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_size: indexed_target.byte_count,
            left,
            operator,
            right,
        });
    }

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameBaseIndexedBinary {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
                left,
                operator,
                right,
            },
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset(
        input,
        dispatch_index,
        target_source_key,
        resolved_target,
    ) {
        return Some(SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
            left,
            operator,
            right,
        });
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
        return Some(SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset: indexed_target.descriptor_offset,
            field_byte_offset,
            byte_size: indexed_target.byte_count,
            left,
            operator,
            right,
        });
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

    // Decision 17 (operand-driven): the arithmetic domain comes from the operands'
    // types (Exact neutral); signedness from whichever operand is an integer place.
    let domain = resolve_runtime_storage_arithmetic_domain(
        input,
        dispatch_index,
        value_source_key,
        left_expression,
    )
    .combine(resolve_runtime_storage_arithmetic_domain(
        input,
        dispatch_index,
        value_source_key,
        right_expression,
    ));
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

    Some(SelectedInstructionKind::WriteRuntimeStorageBinary {
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_size: target_place.byte_count,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    })
}

/// Fold an all-literal `+` tree to the single string it denotes; `None` when
/// any leaf is not a string literal. Mirrors the runtime-text planner's fold
/// (which classifies these writes as StaticText) and the data planner's
/// folded-literal object, so the descriptor write finds matching bytes.
fn fold_static_string_tree_value(value: &Expression) -> Option<String> {
    match value {
        Expression::String(value) => Some(value.to_string()),
        Expression::Binary(binary)
            if binary.operator == omega_checked_trees::expression::BinaryOperator::Add =>
        {
            let mut folded = fold_static_string_tree_value(&binary.left)?;
            folded.push_str(&fold_static_string_tree_value(&binary.right)?);
            Some(folded)
        }
        _ => None,
    }
}
