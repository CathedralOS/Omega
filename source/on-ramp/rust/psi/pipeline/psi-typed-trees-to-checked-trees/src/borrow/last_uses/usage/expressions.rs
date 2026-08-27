use crate::context::*;
use crate::lookup::first_valid_name_path_symbol;

pub(super) fn expression_uses_symbol(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        psi_typed_trees::expression::ExpressionNode::Atomic(atomic) => {
            expression_uses_symbol(program, atomic.value, symbol)
        }
        psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_symbol(program, *value, symbol)),
        psi_typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_symbol(program, binary.left, symbol)
                || expression_uses_symbol(program, binary.right, symbol)
        }
        psi_typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && expression_uses_symbol(program, call.receiver, symbol))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        psi_typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_symbol(program, cast.value, symbol)
        }
        psi_typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_symbol(program, indexed.collection, symbol)
                || expression_uses_symbol(program, indexed.index, symbol)
        }
        psi_typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_symbol(program, range.start, symbol))
                || (range.end.is_valid() && expression_uses_symbol(program, range.end, symbol))
        }
        psi_typed_trees::expression::ExpressionNode::Member(member) => {
            member.member_symbol == symbol
                || expression_uses_symbol(program, member.receiver, symbol)
        }
        psi_typed_trees::expression::ExpressionNode::Borrow(inner_expression) => {
            expression_uses_symbol(program, inner_expression.target, symbol)
        }
        psi_typed_trees::expression::ExpressionNode::Unary(unary) => {
            expression_uses_symbol(program, unary.operand, symbol)
        }
        psi_typed_trees::expression::ExpressionNode::Name(path) => {
            first_valid_name_path_symbol(path, &program.expression_table)
                .is_some_and(|path_symbol| path_symbol == symbol)
                || program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                    .iter()
                    .any(|member_symbol| *member_symbol == symbol)
                || path.symbol == symbol
        }
        psi_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_symbol(program, field.value, symbol)),
        psi_typed_trees::expression::ExpressionNode::Boolean(_)
        | psi_typed_trees::expression::ExpressionNode::Float(_)
        | psi_typed_trees::expression::ExpressionNode::Integer(_)
        | psi_typed_trees::expression::ExpressionNode::String(_)
        | psi_typed_trees::expression::ExpressionNode::ZeroValue(_) => false,
    }
}

pub(super) fn expression_uses_local_name(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    local_name: &str,
) -> bool {
    match program.expression_table.expression(expression) {
        psi_typed_trees::expression::ExpressionNode::Atomic(atomic) => {
            expression_uses_local_name(program, atomic.value, local_name)
        }
        psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_local_name(program, *value, local_name)),
        psi_typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_local_name(program, binary.left, local_name)
                || expression_uses_local_name(program, binary.right, local_name)
        }
        psi_typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_uses_local_name(program, call.receiver, local_name))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        psi_typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_local_name(program, cast.value, local_name)
        }
        psi_typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_local_name(program, indexed.collection, local_name)
                || expression_uses_local_name(program, indexed.index, local_name)
        }
        psi_typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_local_name(program, range.start, local_name))
                || (range.end.is_valid()
                    && expression_uses_local_name(program, range.end, local_name))
        }
        psi_typed_trees::expression::ExpressionNode::Member(member) => {
            expression_uses_local_name(program, member.receiver, local_name)
        }
        psi_typed_trees::expression::ExpressionNode::Borrow(inner_expression) => {
            expression_uses_local_name(program, inner_expression.target, local_name)
        }
        psi_typed_trees::expression::ExpressionNode::Unary(unary) => {
            expression_uses_local_name(program, unary.operand, local_name)
        }
        psi_typed_trees::expression::ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|member| member.as_str() == local_name),
        psi_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_local_name(program, field.value, local_name)),
        psi_typed_trees::expression::ExpressionNode::Boolean(_)
        | psi_typed_trees::expression::ExpressionNode::Float(_)
        | psi_typed_trees::expression::ExpressionNode::Integer(_)
        | psi_typed_trees::expression::ExpressionNode::String(_)
        | psi_typed_trees::expression::ExpressionNode::ZeroValue(_) => false,
    }
}

pub(super) fn expression_uses_place_symbol(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    let canonical_uses_symbol = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )
    .is_some_and(|place| {
        matches!(place.root, psi_facts::PlaceRoot::Symbol(root) if root == symbol)
            || place.segments.iter().any(|segment| {
                matches!(segment, psi_facts::PlaceSegment::Field { symbol: field } if *field == symbol)
            })
    });
    if canonical_uses_symbol {
        return true;
    }

    match program.expression_table.expression(expression) {
        psi_typed_trees::expression::ExpressionNode::Atomic(atomic) => {
            expression_uses_place_symbol(
                program,
                state_symbol,
                statement_index,
                atomic.value,
                symbol,
            )
        }
        psi_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| {
                expression_uses_place_symbol(program, state_symbol, statement_index, *value, symbol)
            }),
        psi_typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_place_symbol(
                program,
                state_symbol,
                statement_index,
                binary.left,
                symbol,
            ) || expression_uses_place_symbol(
                program,
                state_symbol,
                statement_index,
                binary.right,
                symbol,
            )
        }
        psi_typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_uses_place_symbol(
                    program,
                    state_symbol,
                    statement_index,
                    call.receiver,
                    symbol,
                ))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| {
                        expression_uses_place_symbol(
                            program,
                            state_symbol,
                            statement_index,
                            *argument,
                            symbol,
                        )
                    })
        }
        psi_typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_place_symbol(program, state_symbol, statement_index, cast.value, symbol)
        }
        psi_typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_place_symbol(
                program,
                state_symbol,
                statement_index,
                indexed.collection,
                symbol,
            ) || expression_uses_place_symbol(
                program,
                state_symbol,
                statement_index,
                indexed.index,
                symbol,
            )
        }
        psi_typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid()
                && expression_uses_place_symbol(
                    program,
                    state_symbol,
                    statement_index,
                    range.start,
                    symbol,
                ))
                || (range.end.is_valid()
                    && expression_uses_place_symbol(
                        program,
                        state_symbol,
                        statement_index,
                        range.end,
                        symbol,
                    ))
        }
        psi_typed_trees::expression::ExpressionNode::Member(member) => {
            expression_uses_place_symbol(
                program,
                state_symbol,
                statement_index,
                member.receiver,
                symbol,
            )
        }
        psi_typed_trees::expression::ExpressionNode::Borrow(inner_expression) => {
            expression_uses_place_symbol(
                program,
                state_symbol,
                statement_index,
                inner_expression.target,
                symbol,
            )
        }
        psi_typed_trees::expression::ExpressionNode::Unary(unary) => expression_uses_place_symbol(
            program,
            state_symbol,
            statement_index,
            unary.operand,
            symbol,
        ),
        psi_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| {
                expression_uses_place_symbol(
                    program,
                    state_symbol,
                    statement_index,
                    field.value,
                    symbol,
                )
            }),
        psi_typed_trees::expression::ExpressionNode::Boolean(_)
        | psi_typed_trees::expression::ExpressionNode::Float(_)
        | psi_typed_trees::expression::ExpressionNode::Integer(_)
        | psi_typed_trees::expression::ExpressionNode::Name(_)
        | psi_typed_trees::expression::ExpressionNode::String(_)
        | psi_typed_trees::expression::ExpressionNode::ZeroValue(_) => false,
    }
}
