use super::*;
use crate::lookup::{expression_root_symbol, first_valid_name_path_symbol};

pub(crate) fn collect_call_argument_accesses(
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    arguments: &[ExpressionHandle],
    machine_symbol: SymbolHandle,
) -> omega_core::arena::HandleSpan<BorrowArgumentAccessFact> {
    let mut accesses = omega_core::arena::HandleSpan::empty();

    for argument in arguments {
        collect_argument_accesses(
            *argument,
            expressions,
            argument_accesses,
            &mut accesses,
            machine_symbol,
        );
    }

    accesses
}

fn collect_argument_accesses(
    expression: ExpressionHandle,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    machine_symbol: SymbolHandle,
) {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            if let Some(root_symbol) =
                expression_root_symbol(*inner_expression, expressions, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol,
                        kind: BorrowAccessKind::Mutable,
                    },
                );
            }
        }
        _ => collect_read_accesses(
            expression,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
    }
}

fn collect_read_accesses(
    expression: ExpressionHandle,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    machine_symbol: SymbolHandle,
) {
    match expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in expressions.expression_handles(*values) {
                collect_read_accesses(
                    *value,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_read_accesses(
                binary.left,
                expressions,
                argument_accesses,
                accesses,
                machine_symbol,
            );
            collect_read_accesses(
                binary.right,
                expressions,
                argument_accesses,
                accesses,
                machine_symbol,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_read_accesses(
                    call.receiver,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }

            for argument in expressions.expression_handles(call.arguments) {
                collect_read_accesses(
                    *argument,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Cast(cast) => collect_read_accesses(
            cast.value,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::Indexed(indexed) => {
            if let Some(root_symbol) =
                expression_root_symbol(indexed.collection, expressions, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol,
                        kind: BorrowAccessKind::Read,
                    },
                );
            }

            collect_read_accesses(
                indexed.index,
                expressions,
                argument_accesses,
                accesses,
                machine_symbol,
            );
        }
        ExpressionNode::Member(member) => collect_read_accesses(
            member.receiver,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::Name(path) => {
            if let Some(root_symbol) = first_valid_name_path_symbol(path, expressions) {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol,
                        kind: BorrowAccessKind::Read,
                    },
                );
            }
        }
        ExpressionNode::Mutable(inner_expression) => collect_read_accesses(
            *inner_expression,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in expressions.struct_fields(struct_literal.fields) {
                collect_read_accesses(
                    field.value,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}
