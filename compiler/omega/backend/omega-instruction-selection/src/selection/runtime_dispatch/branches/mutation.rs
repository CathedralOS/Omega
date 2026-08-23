use crate::InstructionSelectionInput;
use crate::selection::lookups::state_parameters;
use omega_abstract_operations::{
    RuntimeBitFieldFragment, RuntimeValueOperand, RuntimeValueOperandHandle, SelectedInstruction,
    SelectedInstructionKind, StateGuardOperator,
};
use omega_control_flow::StateKey;
use psi_arena::Arena;
use psi_checked_trees::expression::{
    BinaryOperator, Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
    TableCallExpression, TableMemberExpression,
};
use psi_symbols::{BuiltinFunction, SymbolHandle};

use super::super::super::storage_places::{
    clamp_runtime_case_comparison_operands_in_table, classify_scalar_value_type_in_table,
    resolve_binary_operand_arithmetic_domain_in_table,
    resolve_binary_operation_arithmetic_domain_in_table, resolve_runtime_bit_field_place_in_table,
    resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_base_indexed_target_with_index_region_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_is_signed_in_table,
    resolve_runtime_storage_place_in_table, resolve_runtime_storage_primitive_type_in_table,
    static_integer_value_in_table,
};
use super::super::guards::static_guard_conjunct_summary_in_table;
use super::super::text_writes::{
    runtime_text_builder_write_in_table_emit, string_literal_data_handle,
};
use super::super::writes::mutation::{
    binary_value_operand_byte_width, binary_value_operands_are_float,
    resolve_runtime_stored_integer_operand_in_table,
};
use super::super::writes::{
    RuntimeStaticValues, case_payload_field_variant_tag, runtime_storage_copy_in_table,
    runtime_storage_fixed_indexed_source_copy_in_table,
    runtime_storage_indexed_source_copy_in_table, runtime_storage_indirect_copy_in_table,
    runtime_stored_integer_projection_copy_in_table, select_runtime_case_tag_write_in_table,
    select_runtime_convert_mutation_write_in_table, signedness_adjusted_operator,
    signedness_adjusted_operator_for_operands,
};
use crate::selection::instruction_sink::SelectedInstructionSink;
use psi_checked_trees::types::PrimitiveType;

fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 2 | 4 | 8)
}

fn supports_runtime_value_operand(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 2 | 4 | 8)
}

fn builtin_runtime_call_operator_in_table(
    input: &InstructionSelectionInput<'_>,
    call: &TableCallExpression,
) -> Option<StateGuardOperator> {
    if call.receiver.is_valid() || call.arguments.count() != 2 {
        return None;
    }

    let symbols = &input.program.symbols;
    if Some(call.target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Max) {
        return Some(StateGuardOperator::Max);
    }
    if Some(call.target_symbol) == symbols.builtin_function_symbol(BuiltinFunction::Min) {
        return Some(StateGuardOperator::Min);
    }

    None
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn select_runtime_resolved_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    resolved_target: ExpressionHandle,
    resolved_value: ExpressionHandle,
    mutable_expressions: &mut ExpressionTable,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if matches!(
        expressions.expression(resolved_value),
        ExpressionNode::StructLiteral(_)
    ) {
        let source_expressions = expressions;
        mutable_expressions.clear();
        let expressions = mutable_expressions;
        let resolved_target = expressions.copy_from(source_expressions, resolved_target);
        let resolved_value = expressions.copy_from(source_expressions, resolved_value);
        return select_runtime_resolved_mutation_write_in_mutable_table(
            input,
            dispatch_index,
            operation_key,
            target_source_key,
            value_source_key,
            statement_index,
            expressions,
            resolved_target,
            resolved_value,
            resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        );
    }

    select_runtime_resolved_scalar_mutation_write_in_table(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_value,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_mutation_write_in_mutable_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &mut ExpressionTable,
    resolved_target: ExpressionHandle,
    resolved_value: ExpressionHandle,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if let ExpressionNode::StructLiteral(struct_literal) =
        expressions.expression(resolved_value).clone()
    {
        let mut emitted = false;
        // Constructing a CASE writes the i32 tag prefix before the payload
        // fields (mirrors the dispatch-body struct-literal decomposition).
        // This path tracks no static values; pass a throwaway for the
        // invalidation bookkeeping.
        if let Some(case_name) = &struct_literal.case_name {
            let mut static_values = RuntimeStaticValues::new();
            emitted |= select_runtime_case_tag_write_in_table(
                input,
                dispatch_index,
                operation_key,
                target_source_key,
                statement_index,
                expressions,
                resolved_target,
                &struct_literal.type_name,
                case_name,
                &mut static_values,
                selected_instructions,
            );
        }
        for offset in 0..struct_literal.fields.count() {
            let field = expressions
                .struct_field_at_offset(struct_literal.fields, offset)
                .clone();
            // A case construction's PAYLOAD fields carry the constructed
            // variant, exactly as the destructure/read path tags them -- so
            // the write resolves the same variant-specific offset the read
            // does. Without this, a payload field whose NAME is shared with an
            // EARLIER variant (`amount` in Deposit(amount) vs Transfer(to,
            // amount)) resolved the FIRST variant's offset through a
            // value-call leaf terminal and the sibling field read garbage
            // (the #38 collision, previously fixed only on the mutation path).
            let case_variant = struct_literal.case_name.as_ref().and_then(|case_name| {
                case_payload_field_variant_tag(
                    input,
                    &struct_literal.type_name,
                    case_name,
                    &field.name,
                )
            });
            let field_target = expressions.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: resolved_target,
                member_symbol: SymbolHandle::invalid(),
                member: field.name.clone(),
                case_variant,
            }));
            let field_emitted = select_runtime_resolved_mutation_write_in_mutable_table(
                input,
                dispatch_index,
                operation_key,
                target_source_key,
                value_source_key,
                statement_index,
                expressions,
                field_target,
                field.value,
                resolved_segment_expressions,
                runtime_value_operands,
                selected_instructions,
            );
            if !field_emitted {
                if std::env::var_os("OMEGA_DEBUG_MUTATION_SELECTION").is_some() {
                    eprintln!(
                        "case-literal field write not selected: field `{}` value {:?}",
                        field.name.as_str(),
                        expressions.expression(field.value)
                    );
                }
                // A field no strategy served must NOT be OR'd away: the tag
                // and sibling writes already landed, so skipping just this
                // field compiles a partial construction that reads ZII 0 at
                // runtime (the cast-in-payload face, then text-equality
                // payloads). Poison instead -- emission planning rejects it
                // with the bind-to-a-`let` diagnostic; the poison never
                // encodes, so the partial writes never run.
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
                    source_key: operation_key,
                    source_statement: statement_index,
                });
                // Handled BY POISONING: the caller must not fall through to
                // another strategy on top of the marker.
                emitted = true;
            }
            emitted |= field_emitted;
        }
        return emitted;
    }

    select_runtime_resolved_scalar_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_value,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_scalar_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    resolved_target: ExpressionHandle,
    resolved_value: ExpressionHandle,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    select_runtime_resolved_scalar_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_value,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_scalar_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    resolved_target: ExpressionHandle,
    resolved_value: ExpressionHandle,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    resolved_segment_expressions.clear();
    if runtime_text_builder_write_in_table_emit(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_segment_expressions,
        &|_, expression| expression,
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_key,
                source_statement: statement_index,
            });
        },
    ) {
        return true;
    }

    if let Some(kind) = select_runtime_string_mutation_write_in_table(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        statement_index,
        expressions,
        resolved_target,
        resolved_value,
    )
    .or_else(|| {
        select_runtime_static_inline_branching_call_mutation_write_in_table(
            input,
            dispatch_index,
            operation_key,
            target_source_key,
            statement_index,
            expressions,
            resolved_target,
            resolved_value,
            runtime_value_operands,
        )
    })
    .or_else(|| {
        select_runtime_static_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            resolved_target,
            resolved_value,
        )
    })
    .or_else(|| {
        runtime_stored_integer_projection_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            resolved_target,
            resolved_value,
            runtime_value_operands,
        )
    })
    .or_else(|| {
        runtime_storage_indexed_source_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            resolved_target,
            resolved_value,
        )
    })
    .or_else(|| {
        runtime_storage_fixed_indexed_source_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            resolved_target,
            resolved_value,
        )
    })
    .or_else(|| {
        runtime_storage_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            resolved_target,
            resolved_value,
        )
    })
    .or_else(|| {
        runtime_storage_indirect_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            resolved_target,
            resolved_value,
        )
    })
    .or_else(|| {
        select_runtime_binary_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            statement_index,
            expressions,
            resolved_target,
            resolved_value,
            runtime_value_operands,
        )
    })
    .or_else(|| {
        // A CAST-valued field in a leaf terminal (`Metadata { mode: mode as
        // u32, .. }` delivered through a value-call result slot): no arm above
        // matches a Cast (not a place, not foldable, not Binary), so the field
        // write silently dropped while its siblings landed — the payload
        // arrived with ONE field ZII (the metadata_path `mode` bug,
        // TASKS_FS.md blocker #2A). Reuse the mutation-path convert write;
        // this path tracks no static values, so pass a throwaway.
        let mut convert_static_values = RuntimeStaticValues::new();
        select_runtime_convert_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            statement_index,
            expressions,
            resolved_target,
            resolved_value,
            &mut convert_static_values,
            runtime_value_operands,
        )
    }) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_key,
            source_statement: statement_index,
        });
        return true;
    }

    false
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_static_inline_branching_call_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let ExpressionNode::Call(call) = expressions.expression(value) else {
        return None;
    };
    let selected_value = static_inline_branching_call_value_in_table(input, expressions, call)?;

    let mut static_expressions = ExpressionTable::default();
    let target = static_expressions.copy_from(expressions, target);
    let value =
        static_expressions.copy_from(&input.runtime_branching_calls.expressions, selected_value);

    select_runtime_string_mutation_write_in_table(
        input,
        dispatch_index,
        operation_key,
        target_source_key,
        statement_index,
        &static_expressions,
        target,
        value,
    )
    .or_else(|| {
        select_runtime_static_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            &static_expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        select_runtime_binary_mutation_write_in_table(
            input,
            dispatch_index,
            target_source_key,
            target_source_key,
            statement_index,
            &static_expressions,
            target,
            value,
            runtime_value_operands,
        )
    })
}

fn static_inline_branching_call_value_in_table(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    call: &TableCallExpression,
) -> Option<ExpressionHandle> {
    // Candidates are matched by target state NAME, which collides for same-named
    // methods on different data types; restrict to the machine the RECEIVER's
    // static type dispatches to when it is derivable (see
    // `static_receiver_machine_for_call`).
    let receiver_machine = crate::selection::lookups::static_receiver_machine_for_table_call(
        input,
        expressions,
        call.receiver,
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
                && leaf_expansion_bindings_match_table_call_arguments(
                    input,
                    expressions,
                    expansion,
                    call,
                )
        })
        .collect::<Vec<_>>();

    let selected = candidates
        .iter()
        .copied()
        .find(|expansion| inline_branching_expansion_is_statically_selected(input, expansion))
        .or_else(|| {
            candidates.iter().copied().find(|expansion| {
                inline_branching_default_expansion_is_statically_selected(
                    input,
                    expansion,
                    &candidates,
                )
            })
        })
        .map(|expansion| expansion.target_value);
    selected
}

fn static_inline_branching_call_value(
    input: &InstructionSelectionInput<'_>,
    call: &psi_checked_trees::expression::CallExpression,
) -> Option<Expression> {
    // Same receiver-type discrimination as the table variant above.
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
        .find(|expansion| inline_branching_expansion_is_statically_selected(input, expansion))
        .or_else(|| {
            candidates.iter().copied().find(|expansion| {
                inline_branching_default_expansion_is_statically_selected(
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

fn inline_branching_expansion_is_statically_selected(
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

fn inline_branching_default_expansion_is_statically_selected(
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

fn leaf_expansion_bindings_match_table_call_arguments(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    expansion: &omega_runtime_branching::RuntimeLeafBranchExpansion,
    call: &TableCallExpression,
) -> bool {
    let parameters = state_parameters(input, expansion.branch_key);
    let argument_count = call.arguments.count() as usize;
    let parameters = if parameters.len() == argument_count.saturating_add(1)
        && call.receiver.is_valid()
        && parameters
            .first()
            .is_some_and(|parameter| parameter.name.as_str() == "self")
    {
        &parameters[1..]
    } else {
        parameters
    };
    if parameters.len() != argument_count {
        return false;
    }
    let bindings = input
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    parameters.iter().enumerate().all(|(offset, parameter)| {
        let Some(binding) = bindings.iter().rev().find(|binding| {
            binding.parameter_symbol == parameter.symbol || binding.parameter_name == parameter.name
        }) else {
            return false;
        };
        let binding_expression = input
            .runtime_branching_calls
            .expressions
            .to_tree(binding.expression);
        let argument = expressions
            .to_tree(expressions.expression_handle_at_offset(call.arguments, offset as u32));
        static_inline_branching_argument_matches(input, bindings, &binding_expression, &argument)
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
        Expression::Call(call) => static_inline_branching_call_value(input, call),
        _ => None,
    };
    let resolved_argument = match &argument {
        Expression::Call(call) => static_inline_branching_call_value(input, call),
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

fn select_runtime_static_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let value = static_integer_value_in_table(&input.layouts, expressions, value)?;

    if let Some(bit_target) = resolve_runtime_bit_field_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    ) {
        return Some(SelectedInstructionKind::WriteStorageBitField {
            region: bit_target.region,
            base_byte_offset: bit_target.base_byte_offset,
            fragments: bit_target
                .fragments
                .into_iter()
                .map(|fragment| RuntimeBitFieldFragment {
                    container_byte_offset: fragment.container_byte_offset,
                    container_width_bits: fragment.container_width_bits,
                    destination_lsb: fragment.destination_lsb,
                    source_lsb: fragment.source_lsb,
                    width: fragment.width,
                })
                .collect(),
            value,
        });
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_frame_indexed(
                indexed_target.descriptor_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                value,
                indexed_target.byte_count,
            ),
        );
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            target,
        )
        && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_base_indexed(
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
        );
    }

    if let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    ) && supports_scalar_integer_write(indexed_target.byte_count)
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_base_indexed(
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
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    ) {
        return Some(
            crate::selection::runtime_dispatch::write_place_integer_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                value,
                pointer_target.pointee_byte_size,
            ),
        );
    }

    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        target,
    )?;
    if !supports_scalar_integer_write(target_place.byte_count) {
        return None;
    }

    Some(
        crate::selection::runtime_dispatch::write_place_integer_direct(
            target_place.region,
            target_place.byte_offset,
            value,
            target_place.byte_count,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_string_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_key: StateKey,
    target_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let value = expressions.string_literal_value(value)?;
    let data = string_literal_data_handle(input, operation_key, statement_index, &value);

    if data.is_valid()
        && let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                data,
                value.len(),
            ),
        );
    }

    if data.is_valid()
        && let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_pointee(
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                data,
                value.len(),
            ),
        );
    }

    if data.is_valid()
        && let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_frame_indexed(
                indexed_target.descriptor_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                data,
                value.len(),
            ),
        );
    }

    if data.is_valid()
        && let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && indexed_target.byte_count == input.runtime_abi.string_descriptor_size()
    {
        return Some(
            crate::selection::runtime_dispatch::write_place_string_frame_base_indexed(
                indexed_target.base_byte_offset,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                data,
                value.len(),
            ),
        );
    }

    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    if target_place.byte_count != input.runtime_abi.string_descriptor_size() || !data.is_valid() {
        return None;
    }

    // Honor the resolved region: a frame-resident place (e.g. a `let` local's
    // String field, common in inlined helpers) must be a frame write, not a
    // machine-storage write at the same numeric offset.
    match target_place.region {
        omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame => Some(
            crate::selection::runtime_dispatch::write_place_string_direct(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                target_place.byte_offset,
                data,
                value.len(),
            ),
        ),
        omega_abstract_operations::RuntimeStorageRegion::Machine => Some(
            crate::selection::runtime_dispatch::write_place_string_direct(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                target_place.byte_offset,
                data,
                value.len(),
            ),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_binary_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    // A TOP-LEVEL String `==`/`!=` as the written value (`z: self.name ==
    // "omega"` in a case-literal terminal, decomposed to this per-field
    // write): resolve the whole value to the `text_equals | 0` operand pair
    // and fall through to the SAME target arms below (frame-indexed staging /
    // pointee / plain place). An earlier attempt pre-empted those arms with
    // its own single plain-place target resolution and wrote the WRONG
    // region/offset (native 73 while the staged copy pattern expected the
    // frame staging slot) -- the target resolution must stay this function's.
    if let ExpressionNode::Binary(binary) = expressions.expression(value)
        && let Some(text_equals) =
            super::super::writes::resolve_runtime_text_equals_operand_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                binary.operator,
                binary.left,
                binary.right,
                runtime_value_operands,
            )
    {
        let zero = runtime_value_operands.insert(RuntimeValueOperand::Immediate(0));
        return select_runtime_binary_mutation_write_target_arms(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
            text_equals,
            StateGuardOperator::Or,
            zero,
        );
    }

    let (operator, left_expression, right_expression) = match expressions.expression(value) {
        ExpressionNode::Binary(binary) => (
            runtime_binary_operator(binary.operator)?,
            binary.left,
            binary.right,
        ),
        ExpressionNode::Call(call) => {
            let operator = builtin_runtime_call_operator_in_table(input, call)?;
            let left = expressions.expression_handle_at_offset(call.arguments, 0);
            let right = expressions.expression_handle_at_offset(call.arguments, 1);
            (operator, left, right)
        }
        _ => return None,
    };

    // Signedness matters for division/modulo/right-shift/min/max/comparisons:
    // a u32 argument value like `f(raw % 100)` must use the UNSIGNED encoding
    // (sdiv on raw >= 2^31 produces a negative remainder, which the unsigned
    // dispatch guards then read as a huge value -- the dungeon seed-7 extra
    // enemy draw). Same policy as the mutation-table write path.
    let operator = signedness_adjusted_operator(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        expressions,
        target,
        left_expression,
        right_expression,
        operator,
    );

    let left = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        left_expression,
        runtime_value_operands,
    )?;
    let right = resolve_runtime_value_operand_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        right_expression,
        runtime_value_operands,
    )?;

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
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

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
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

    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    // A float TARGET (e.g. `let c: f64 = a + b`) must use the SSE unit, not an
    // integer add over the IEEE bits. f64 (addsd) and f32 (addss) — the encoder
    // picks the scalar width from the target byte_size.
    let is_float = matches!(
        resolve_runtime_storage_primitive_type_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        ),
        Some(PrimitiveType::F64 | PrimitiveType::F32)
    );
    let domain = resolve_binary_operation_arithmetic_domain_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        value,
        left_expression,
        right_expression,
        is_float,
        target_place.byte_count,
    )?;
    let target_signed = resolve_runtime_storage_is_signed_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )
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

/// The binary write's TARGET arms (frame-indexed staging / pointee / plain
/// place) for a PRE-RESOLVED operand triple. The text-equals write rides
/// exactly this resolution: an earlier attempt did its own single plain-place
/// lookup ahead of these arms and wrote the wrong region/offset (the
/// case-literal terminal's fields stage in the FRAME before the whole-value
/// copy). The flags are fixed for the 0/1 bool result: not float, Exact
/// domain (a bool OR cannot overflow), unsigned.
#[allow(clippy::too_many_arguments)]
fn select_runtime_binary_mutation_write_target_arms(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Option<SelectedInstructionKind> {
    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
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
    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
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
    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    Some(
        crate::selection::runtime_dispatch::write_place_binary_direct(
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
            left,
            operator,
            right,
            false,
            psi_numerics::arithmetic::ArithmeticDomain::Exact,
            false,
        ),
    )
}

fn resolve_runtime_value_operand_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if let Some(value) = static_integer_value_in_table(&input.layouts, expressions, expression) {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let operator = runtime_binary_operator(binary.operator)?;
            // Same signedness policy as the write paths: nested unsigned
            // operands pick the unsigned encoding.
            let operator = signedness_adjusted_operator_for_operands(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.left,
                binary.right,
                operator,
            );
            let left = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                binary.left,
                runtime_value_operands,
            )?;
            let right = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                binary.right,
                runtime_value_operands,
            )?;
            // A case-name equality (the lowered form of `in`) compares the
            // TAG only; the place operand must not read payload bytes.
            clamp_runtime_case_comparison_operands_in_table(
                &input.layouts,
                expressions,
                binary.operator,
                binary.left,
                binary.right,
                left,
                right,
                runtime_value_operands,
            );
            let domain_signedness = resolve_binary_operand_arithmetic_domain_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.left,
                binary.right,
            );
            let is_float = binary_value_operands_are_float(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.left,
                binary.right,
            );
            let byte_width = binary_value_operand_byte_width(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.left,
                binary.right,
            );
            let arithmetic_domain = resolve_binary_operation_arithmetic_domain_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                expression,
                binary.left,
                binary.right,
                is_float,
                byte_width,
            )?;
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
                left,
                operator,
                right,
                is_float,
                byte_width,
                // Recorded so the Saturating/Trapping operand-position
                // lowering picks its width-correct op + clamp/trap bounds.
                arithmetic_domain,
                operands_signed: domain_signedness.1,
            }));
        }
        ExpressionNode::Call(call) => {
            let operator = builtin_runtime_call_operator_in_table(input, call)?;
            // min/max builtins compare their operands -- same signedness policy.
            let operator = signedness_adjusted_operator_for_operands(
                input,
                dispatch_index,
                source_key,
                expressions,
                expressions.expression_handle_at_offset(call.arguments, 0),
                expressions.expression_handle_at_offset(call.arguments, 1),
                operator,
            );
            let left = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                expressions.expression_handle_at_offset(call.arguments, 0),
                runtime_value_operands,
            )?;
            let right = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                expressions.expression_handle_at_offset(call.arguments, 1),
                runtime_value_operands,
            )?;
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
                left,
                operator,
                right,
                // Branch-arm value operand: float detection not wired on this path yet.
                is_float: false,
                // Integer arm derives its own width; default 8 matches prior behavior.
                byte_width: 8,
                // min/max SELECTS one operand -- no overflow exists for a
                // domain to clamp/trap, so the fused compare-select is honest
                // under every domain. Signedness already rode the operator
                // (MinUnsigned/MaxUnsigned).
                arithmetic_domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
                operands_signed: true,
            }));
        }
        // A numeric `as` cast in operand position (`z: (x as i8) % 10` as a
        // case-payload field routed through the branch construction): resolve
        // the source and wrap it in a Convert, exactly like the mutation-table
        // resolver's cast arm. Before this arm existed the cast operand nulled
        // out, the binary write bailed, and the case-literal cascade dropped
        // JUST this field while the tag and sibling fields landed -- a silent
        // ZII read downstream (the cast-in-payload face; bare `x % 10` never
        // hit it because the Binary arm recurses fine over non-cast leaves).
        ExpressionNode::Cast(cast) => {
            let target_primitive = input.program.primitive_type_reference(cast.target_type)?;
            let source_primitive = classify_scalar_value_type_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                cast.value,
            )?;
            let target_byte_size = target_primitive.scalar_byte_size()?;
            let source_byte_size = source_primitive.scalar_byte_size()?;
            let source = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                cast.value,
                runtime_value_operands,
            )?;
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Convert {
                source,
                source_byte_size,
                target_byte_size,
                source_is_float: source_primitive.accepts_float_literal(),
                target_is_float: target_primitive.accepts_float_literal(),
                source_signed: source_primitive.is_signed_integer(),
                target_signed: target_primitive.is_signed_integer(),
                // F4: a Trapping float->int cast carries its trap guard.
                trapping: cast.domain == psi_numerics::arithmetic::ArithmeticDomain::Trapping
                    && source_primitive.accepts_float_literal()
                    && !target_primitive.accepts_float_literal(),
                saturating: cast.domain == psi_numerics::arithmetic::ArithmeticDomain::Saturating
                    && source_primitive.accepts_float_literal()
                    && !target_primitive.accepts_float_literal(),
            }));
        }
        _ => {}
    }

    if let Some(operand) = resolve_runtime_stored_integer_operand_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
        runtime_value_operands,
    ) {
        return Some(operand);
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameIndexed {
                descriptor_offset: indexed_target.descriptor_offset,
                index_region: indexed_target.index_region,
                index_offset: indexed_target.index_offset,
                index_byte_size: indexed_target.index_byte_size,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameBaseIndexed {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                index_byte_size: indexed_target.index_byte_size,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    if let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::MachineIndexed {
                base_byte_offset: indexed_target.base_byte_offset,
                index_region: indexed_target.index_region,
                index_offset: indexed_target.index_offset,
                index_byte_size: indexed_target.index_byte_size,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameFixedIndexed {
                descriptor_offset: indexed_target.descriptor_offset,
                element_index: indexed_target.element_index,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) && supports_runtime_value_operand(pointer_target.pointee_byte_size)
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Pointee {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
        }));
    }

    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    if !supports_runtime_value_operand(place.byte_count) {
        return None;
    }
    Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
        region: place.region,
        byte_offset: place.byte_offset,
        byte_size: place.byte_count,
    }))
}

fn runtime_binary_operator(operator: BinaryOperator) -> Option<StateGuardOperator> {
    match operator {
        BinaryOperator::Add => Some(StateGuardOperator::Add),
        BinaryOperator::And => Some(StateGuardOperator::And),
        BinaryOperator::Equal => Some(StateGuardOperator::Equal),
        BinaryOperator::Greater => Some(StateGuardOperator::Greater),
        BinaryOperator::GreaterOrEqual => Some(StateGuardOperator::GreaterOrEqual),
        BinaryOperator::Less => Some(StateGuardOperator::Less),
        BinaryOperator::LessOrEqual => Some(StateGuardOperator::LessOrEqual),
        BinaryOperator::NotEqual => Some(StateGuardOperator::NotEqual),
        BinaryOperator::Multiply => Some(StateGuardOperator::Multiply),
        BinaryOperator::Divide => Some(StateGuardOperator::Divide),
        BinaryOperator::Modulo => Some(StateGuardOperator::Modulo),
        BinaryOperator::Or => Some(StateGuardOperator::Or),
        BinaryOperator::Subtract => Some(StateGuardOperator::Subtract),
        BinaryOperator::ShiftLeft => Some(StateGuardOperator::ShiftLeft),
        BinaryOperator::ShiftRight => Some(StateGuardOperator::ShiftRight),
        BinaryOperator::BitwiseAnd => Some(StateGuardOperator::BitwiseAnd),
        BinaryOperator::BitwiseOr => Some(StateGuardOperator::BitwiseOr),
        BinaryOperator::BitwiseXor => Some(StateGuardOperator::BitwiseXor),
    }
}
