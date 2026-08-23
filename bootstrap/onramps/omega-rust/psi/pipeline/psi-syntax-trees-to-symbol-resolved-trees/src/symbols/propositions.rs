use psi_arena::HandleSpan;
use psi_symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_symbol_resolved_trees::proposition::PropositionBody;
use psi_symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::lookup::{child_symbol_by_kinds, top_level_symbol_by_kinds};
use super::targets::{
    resolve_free_machine_entry_state_symbol, resolve_proposition_binder_argument_symbol,
};

pub(super) fn assign_proposition_expression_symbols(
    program: &mut psi_symbol_resolved_trees::SymbolResolvedTrees,
    symbols: &SymbolTable,
) {
    let expressions = &mut program.tables.bodies.expressions;
    let propositions = &program.roots.propositions;
    for proposition in propositions {
        let PropositionBody::Transparent { proposition: body } = proposition.body else {
            continue;
        };
        assign_expression_symbols(expressions, symbols, proposition.symbol, body);
    }
}

fn assign_expression_span_symbols(
    expressions: &mut ExpressionTable,
    symbols: &SymbolTable,
    proposition_symbol: SymbolHandle,
    span: HandleSpan<ExpressionHandle>,
) {
    let children = expressions.expression_handles(span).to_vec();
    for child in children {
        assign_expression_symbols(expressions, symbols, proposition_symbol, child);
    }
}

fn assign_expression_symbols(
    expressions: &mut ExpressionTable,
    symbols: &SymbolTable,
    proposition_symbol: SymbolHandle,
    expression: ExpressionHandle,
) {
    match expressions.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => {
            assign_expression_span_symbols(expressions, symbols, proposition_symbol, values)
        }
        ExpressionNode::Atomic(atomic) => {
            assign_expression_symbols(expressions, symbols, proposition_symbol, atomic.value);
            if atomic.result.is_valid() {
                assign_expression_symbols(expressions, symbols, proposition_symbol, atomic.result);
            }
        }
        ExpressionNode::Binary(binary) => {
            assign_expression_symbols(expressions, symbols, proposition_symbol, binary.left);
            assign_expression_symbols(expressions, symbols, proposition_symbol, binary.right);
        }
        ExpressionNode::Cast(cast) => {
            assign_expression_symbols(expressions, symbols, proposition_symbol, cast.value)
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_expression_symbols(expressions, symbols, proposition_symbol, call.receiver);
            }
            assign_expression_span_symbols(
                expressions,
                symbols,
                proposition_symbol,
                call.arguments,
            );
            let target_symbol = if call.receiver.is_valid() {
                SymbolHandle::invalid()
            } else {
                let proposition = top_level_symbol_by_kinds(
                    symbols,
                    &[SymbolKind::Proposition],
                    call.target.as_str(),
                );
                if proposition.is_valid() {
                    proposition
                } else {
                    let builtin = top_level_symbol_by_kinds(
                        symbols,
                        &[SymbolKind::BuiltinFunction],
                        call.target.as_str(),
                    );
                    if builtin.is_valid() {
                        builtin
                    } else {
                        resolve_free_machine_entry_state_symbol(symbols, call.target.as_str())
                    }
                }
            };
            if let ExpressionNode::Call(call) = expressions.expression_mut(expression) {
                call.target_symbol = target_symbol;
                for argument in &mut call.machine_arguments {
                    argument.symbol = resolve_proposition_binder_argument_symbol(
                        symbols,
                        proposition_symbol,
                        &argument.path,
                    );
                }
            }
        }
        ExpressionNode::Indexed(indexed) => {
            assign_expression_symbols(expressions, symbols, proposition_symbol, indexed.collection);
            assign_expression_symbols(expressions, symbols, proposition_symbol, indexed.index);
        }
        ExpressionNode::Membership(membership) => {
            assign_expression_symbols(expressions, symbols, proposition_symbol, membership.value)
        }
        ExpressionNode::Member(member) => {
            assign_expression_symbols(expressions, symbols, proposition_symbol, member.receiver)
        }
        ExpressionNode::Borrow(inner) => {
            assign_expression_symbols(expressions, symbols, proposition_symbol, inner.target)
        }
        ExpressionNode::Name(path) => {
            let members = expressions.name_path_members(path.members);
            let symbol = if let [name] = members {
                child_symbol_by_kinds(
                    symbols,
                    proposition_symbol,
                    &[
                        SymbolKind::Parameter,
                        SymbolKind::TypeParameter,
                        SymbolKind::PropositionMachineParameter,
                    ],
                    name.as_str(),
                )
            } else {
                SymbolHandle::invalid()
            };
            if let ExpressionNode::Name(path) = expressions.expression_mut(expression) {
                path.head_symbol = symbol;
                path.symbol = symbol;
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_expression_symbols(expressions, symbols, proposition_symbol, range.start);
            }
            if range.end.is_valid() {
                assign_expression_symbols(expressions, symbols, proposition_symbol, range.end);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            let fields = expressions.struct_fields(struct_literal.fields).to_vec();
            for field in fields {
                assign_expression_symbols(expressions, symbols, proposition_symbol, field.value);
            }
        }
        ExpressionNode::Unary(unary) => {
            assign_expression_symbols(expressions, symbols, proposition_symbol, unary.operand)
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
