use crate::context::*;
use crate::lookup::first_valid_name_path_symbol;

pub(super) fn expression_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_symbol(program, *value, symbol)),
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_symbol(program, binary.left, symbol)
                || expression_uses_symbol(program, binary.right, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && expression_uses_symbol(program, call.receiver, symbol))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_symbol(program, cast.value, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_symbol(program, indexed.collection, symbol)
                || expression_uses_symbol(program, indexed.index, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_symbol(program, range.start, symbol))
                || (range.end.is_valid() && expression_uses_symbol(program, range.end, symbol))
        }
        omega_typed_trees::expression::ExpressionNode::Member(member) => {
            expression_uses_symbol(program, member.receiver, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Mutable(inner_expression) => {
            expression_uses_symbol(program, *inner_expression, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Unary(unary) => {
            expression_uses_symbol(program, unary.operand, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => {
            first_valid_name_path_symbol(path, &program.expression_table)
                .is_some_and(|path_symbol| path_symbol == symbol)
                || path.symbol == symbol
        }
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_symbol(program, field.value, symbol)),
        omega_typed_trees::expression::ExpressionNode::Boolean(_)
        | omega_typed_trees::expression::ExpressionNode::Float(_)
        | omega_typed_trees::expression::ExpressionNode::Integer(_)
        | omega_typed_trees::expression::ExpressionNode::String(_) => false,
    }
}

pub(super) fn expression_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    local_name: &str,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_local_name(program, *value, local_name)),
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_local_name(program, binary.left, local_name)
                || expression_uses_local_name(program, binary.right, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_uses_local_name(program, call.receiver, local_name))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_local_name(program, cast.value, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_local_name(program, indexed.collection, local_name)
                || expression_uses_local_name(program, indexed.index, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_local_name(program, range.start, local_name))
                || (range.end.is_valid()
                    && expression_uses_local_name(program, range.end, local_name))
        }
        omega_typed_trees::expression::ExpressionNode::Member(member) => {
            expression_uses_local_name(program, member.receiver, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Mutable(inner_expression) => {
            expression_uses_local_name(program, *inner_expression, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Unary(unary) => {
            expression_uses_local_name(program, unary.operand, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|member| member.as_str() == local_name),
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_local_name(program, field.value, local_name)),
        omega_typed_trees::expression::ExpressionNode::Boolean(_)
        | omega_typed_trees::expression::ExpressionNode::Float(_)
        | omega_typed_trees::expression::ExpressionNode::Integer(_)
        | omega_typed_trees::expression::ExpressionNode::String(_) => false,
    }
}
