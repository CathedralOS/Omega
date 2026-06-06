use crate::InstructionSelectionInput;
use crate::selection::bindings::{RuntimeAliasBinding, resolve_runtime_alias_binding};
use crate::selection::storage_places::{
    RuntimeStoragePlace, resolve_runtime_assignment_value_call_result_place_by_ordinal,
    resolve_runtime_frame_base_indexed_target, resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table, resolve_runtime_frame_indexed_target,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_pointee_slot_offset,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table, resolve_runtime_storage_primitive_type_in_table,
};
use omega_abstract_operations::{RuntimeValueOperand, RuntimeValueOperandHandle};
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_checked_trees::statement::StatementNode;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_state_calls::StateCallRole;

use super::super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_float_value_in_table,
    resolve_runtime_static_integer_value, resolve_runtime_static_integer_value_in_table,
};
use super::operators::{
    builtin_runtime_call_operator, builtin_runtime_call_operator_in_table, runtime_binary_operator,
    supports_runtime_value_operand,
};
use super::resolve_runtime_call_result_source_place;

/// Whether a binary value operand built from these two operand expressions is
/// floating-point, so the encoder uses the SSE unit (addsd/...) instead of an
/// integer add over the IEEE bits. A float operand is either a float-typed place
/// or a float literal; checking both operands covers `place OP literal` in any
/// order and `place OP place`.
pub(super) fn binary_value_operands_are_float(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    [left, right].into_iter().any(|operand| {
        resolve_runtime_storage_primitive_type_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            operand,
        )
        .is_some_and(|primitive| primitive.accepts_float_literal())
            || resolve_runtime_static_float_value_in_table(expressions, operand).is_some()
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_runtime_value_operand_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if let Some(value) =
        resolve_runtime_static_integer_value_in_table(input, expressions, expression, static_values)
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    // A float literal operand carries its IEEE-754 bits in the immediate; the
    // float binary write moves those bits into an XMM register for the SSE op.
    // First cut resolves the f64 bit pattern (the default float width); f32
    // arithmetic with a constant operand is gated out at the write site.
    if let Some(float_value) =
        resolve_runtime_static_float_value_in_table(expressions, expression)
    {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::Immediate(float_value.to_bits() as i64)),
        );
    }

    if let ExpressionNode::Binary(binary) = expressions.expression(expression) {
        let operator = runtime_binary_operator(binary.operator)?;
        let left_expr = binary.left;
        let right_expr = binary.right;
        let left = resolve_runtime_value_operand_in_table(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            left_expr,
            static_values,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand_in_table(
            input,
            dispatch_index,
            source_key,
            statement_index,
            expressions,
            right_expr,
            static_values,
            runtime_value_operands,
        )?;
        let is_float = binary_value_operands_are_float(
            input,
            dispatch_index,
            source_key,
            expressions,
            left_expr,
            right_expr,
        );
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            is_float,
        }));
    }

    if let ExpressionNode::Call(call) = expressions.expression(expression) {
        if let Some(operator) = builtin_runtime_call_operator_in_table(input, call) {
            let left_expr = expressions.expression_handle_at_offset(call.arguments, 0);
            let right_expr = expressions.expression_handle_at_offset(call.arguments, 1);
            let left = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                left_expr,
                static_values,
                runtime_value_operands,
            )?;
            let right = resolve_runtime_value_operand_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                right_expr,
                static_values,
                runtime_value_operands,
            )?;
            let is_float = binary_value_operands_are_float(
                input,
                dispatch_index,
                source_key,
                expressions,
                left_expr,
                right_expr,
            );
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
                left,
                operator,
                right,
                is_float,
            }));
        }

        if let Some(place) = resolve_runtime_call_result_source_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
        ) {
            return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
                region: place.region,
                byte_offset: place.byte_offset,
                byte_size: place.byte_count,
            }));
        }
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
                index_offset: indexed_target.index_offset,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_runtime_value_operand(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<RuntimeValueOperandHandle> {
    if let Some(value) = resolve_runtime_static_integer_value(
        input,
        source_key,
        expression,
        aliases,
        alias_expressions,
        static_values,
    ) {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Immediate(value)));
    }

    if let Expression::Binary(binary) = expression {
        let operator = runtime_binary_operator(binary.operator)?;
        let left = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            &binary.left,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            &binary.right,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            // Non-table value-operand path: float detection not wired here yet;
            // float arithmetic via this fallback stays a known gap.
            is_float: false,
        }));
    }

    if let Expression::Call(call) = expression
        && let Some(operator) = builtin_runtime_call_operator(input, call)
    {
        let [left, right] = &*call.arguments else {
            return None;
        };
        let left = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            left,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        let right = resolve_runtime_value_operand(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            right,
            aliases,
            alias_expressions,
            static_values,
            runtime_value_operands,
        )?;
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Binary {
            left,
            operator,
            right,
            // Non-table value-operand path: float detection not wired here yet;
            // float arithmetic via this fallback stays a known gap.
            is_float: false,
        }));
    }

    if matches!(expression, Expression::Call(_))
        && let Some(place) = resolve_prior_local_call_result_source_place(
            input,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            statement_index,
            expression,
            aliases,
            alias_expressions,
        )
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        }));
    }

    if matches!(expression, Expression::Call(_))
        && let Some(place) = resolve_runtime_call_result_source_place(
            input,
            dispatch_index,
            source_key,
            statement_index,
        )
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Storage {
            region: place.region,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
        }));
    }

    if let Some(pointer_target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, source_key, expression)
        && supports_runtime_value_operand(pointer_target.pointee_byte_size)
    {
        return Some(runtime_value_operands.insert(RuntimeValueOperand::Pointee {
            pointer_byte_offset: pointer_target.pointer_byte_offset,
            field_byte_offset: pointer_target.field_byte_offset,
            byte_size: pointer_target.pointee_byte_size,
        }));
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_indexed_target(input, dispatch_index, source_key, expression)
    {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameIndexed {
                descriptor_offset: indexed_target.descriptor_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_base_indexed_target(input, dispatch_index, source_key, expression)
    {
        return Some(
            runtime_value_operands.insert(RuntimeValueOperand::FrameBaseIndexed {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                byte_size: indexed_target.byte_count,
            }),
        );
    }

    let resolved =
        resolve_runtime_alias_binding(expression, source_key, aliases, alias_expressions);
    let place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        resolved.source_key,
        source_machine,
        source_state,
        &resolved.expression,
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

fn resolve_prior_local_call_result_source_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> Option<RuntimeStoragePlace> {
    let Expression::Call(_) = expression else {
        return None;
    };

    let mut candidate_states = Vec::new();
    if let Some(candidate) = program_state_statements_by_symbol(input, source_key) {
        candidate_states.push(candidate);
    }
    if let Some((machine_name, state_name)) = input.control_flow.state_names_by_key(source_key)
        && let Some(candidate) =
            program_state_statements_by_name(input, source_key, machine_name, state_name)
    {
        candidate_states.push(candidate);
    }
    if let Some(candidate) =
        program_state_statements_by_display_name(input, source_key, source_machine, source_state)
    {
        candidate_states.push(candidate);
    }

    for (local_source_key, statements) in candidate_states {
        for (local_statement_index, statement) in
            statements.iter().enumerate().take(statement_index)
        {
            let StatementNode::LocalData(local_data) = statement else {
                continue;
            };
            if !local_data.initial_value.is_valid() {
                continue;
            }

            let initializer = input
                .program
                .expression_table
                .to_tree(local_data.initial_value);
            let resolved_initializer =
                resolve_runtime_alias_binding(&initializer, source_key, aliases, alias_expressions);
            if !prior_local_call_initializer_matches(&resolved_initializer.expression, expression) {
                continue;
            }

            for state_call in input
                .state_calls
                .calls_for_statement(local_source_key, local_statement_index)
                .filter(|state_call| state_call.role == StateCallRole::AssignmentValue)
            {
                if let Some(place) = resolve_runtime_assignment_value_call_result_place_by_ordinal(
                    input,
                    dispatch_index,
                    local_source_key,
                    local_statement_index,
                    state_call.call_ordinal,
                ) {
                    return Some(place);
                }
            }
        }
    }

    None
}

fn prior_local_call_initializer_matches(initializer: &Expression, expression: &Expression) -> bool {
    if initializer == expression {
        return true;
    }

    let (Expression::Call(initializer), Expression::Call(expression)) = (initializer, expression)
    else {
        return false;
    };

    initializer.target_symbol == expression.target_symbol
        && initializer.target == expression.target
        && initializer.receiver == expression.receiver
        && initializer.arguments.len() == expression.arguments.len()
}

fn program_state_statements_by_name<'a>(
    input: &'a InstructionSelectionInput<'_>,
    source_key: StateKey,
    machine_name: &omega_checked_trees::name::Identifier,
    state_name: &omega_checked_trees::name::Identifier,
) -> Option<(StateKey, &'a [StatementNode])> {
    for machine in input.program.machines() {
        if machine.name != *machine_name {
            continue;
        }
        for state in input.program.machine_states(machine) {
            if state.name != *state_name {
                continue;
            }
            let local_source_key = StateKey {
                machine: machine.symbol,
                state: state.symbol,
                segment_index: source_key.segment_index,
            };
            let statements = input
                .program
                .statement_table
                .statements(state.statement_nodes);
            return Some((local_source_key, statements));
        }
    }

    None
}

fn program_state_statements_by_display_name<'a>(
    input: &'a InstructionSelectionInput<'_>,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
) -> Option<(StateKey, &'a [StatementNode])> {
    for machine in input.program.machines() {
        let machine_name = machine.name.to_string();
        if source_machine != machine_name
            && !source_machine.starts_with(&format!("{machine_name}::"))
        {
            continue;
        }

        for state in input.program.machine_states(machine) {
            let state_name = state.name.to_string();
            if state_name != source_state {
                continue;
            }
            let local_source_key = StateKey {
                machine: machine.symbol,
                state: state.symbol,
                segment_index: source_key.segment_index,
            };
            let statements = input
                .program
                .statement_table
                .statements(state.statement_nodes);
            return Some((local_source_key, statements));
        }
    }

    None
}

fn program_state_statements_by_symbol<'a>(
    input: &'a InstructionSelectionInput<'_>,
    source_key: StateKey,
) -> Option<(StateKey, &'a [StatementNode])> {
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
    let statements = input
        .program
        .statement_table
        .statements(state.statement_nodes);
    Some((source_key, statements))
}
