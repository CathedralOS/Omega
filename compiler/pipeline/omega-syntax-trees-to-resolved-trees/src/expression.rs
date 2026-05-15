use crate::name::{lower_name, lower_name_members};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::expression::{
    ArrayLiteralExpression, ArrayLiteralExpressionStorage, BinaryExpression,
    BinaryExpressionStorage, BinaryOperator, CallExpression, CallExpressionStorage, CastExpression,
    CastExpressionStorage, Expression, FloatLiteral, IndexedExpression, IndexedExpressionStorage,
    MemberExpression, MemberExpressionStorage, StructLiteral, StructLiteralField,
    StructLiteralStorage,
};
use omega_syntax_trees as syntax;
use omega_syntax_trees::SyntaxTrees;

pub(crate) fn lower_expression_handle(
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
) -> Result<Expression, Diagnostic> {
    lower_expression_node(
        syntax_trees,
        syntax_trees.expressions.expression(expression),
    )
}

fn lower_expression_node(
    syntax_trees: &SyntaxTrees,
    expression: &syntax::expression::ExpressionNode,
) -> Result<Expression, Diagnostic> {
    match expression {
        syntax::expression::ExpressionNode::ArrayLiteral(values) => {
            Ok(Expression::ArrayLiteral(ArrayLiteralExpression {
                storage: ArrayLiteralExpressionStorage {
                    values: syntax_trees
                        .expressions
                        .expression_handles(*values)
                        .iter()
                        .map(|value| lower_expression_handle(syntax_trees, *value))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                },
            }))
        }
        syntax::expression::ExpressionNode::Binary(binary) => {
            Ok(Expression::Binary(Box::new(BinaryExpression {
                storage: BinaryExpressionStorage {
                    left: lower_expression_handle(syntax_trees, binary.left)?,
                    operator: lower_binary_operator(binary.operator),
                    right: lower_expression_handle(syntax_trees, binary.right)?,
                },
            })))
        }
        syntax::expression::ExpressionNode::Boolean(value) => Ok(Expression::Boolean(*value)),
        syntax::expression::ExpressionNode::Cast(cast) => {
            Ok(Expression::Cast(Box::new(CastExpression {
                storage: CastExpressionStorage {
                    value: lower_expression_handle(syntax_trees, cast.value)?,
                    target_type: lower_name_members(
                        syntax_trees
                            .expressions
                            .identifier_path_members(cast.target_type)
                            .iter(),
                    ),
                },
            })))
        }
        syntax::expression::ExpressionNode::Call(call) => {
            Ok(Expression::Call(Box::new(CallExpression {
                target_symbol: SymbolHandle::invalid(),
                target: lower_name(&call.target),
                storage: CallExpressionStorage {
                    receiver: if call.receiver.is_valid() {
                        Some(Box::new(lower_expression_handle(
                            syntax_trees,
                            call.receiver,
                        )?))
                    } else {
                        None
                    },
                    arguments: syntax_trees
                        .expressions
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|argument| lower_expression_handle(syntax_trees, *argument))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                },
            })))
        }
        syntax::expression::ExpressionNode::Float(value) => {
            let Some(value) = FloatLiteral::parse(value.as_str()) else {
                return Err(Diagnostic::error(format!(
                    "invalid float literal `{}`",
                    value.as_str()
                )));
            };
            Ok(Expression::Float(value))
        }
        syntax::expression::ExpressionNode::Indexed(indexed) => {
            Ok(Expression::Indexed(Box::new(IndexedExpression {
                storage: IndexedExpressionStorage {
                    collection: lower_expression_handle(syntax_trees, indexed.collection)?,
                    index: lower_expression_handle(syntax_trees, indexed.index)?,
                },
            })))
        }
        syntax::expression::ExpressionNode::Integer(value) => Ok(Expression::Integer(*value)),
        syntax::expression::ExpressionNode::Member(member) => {
            Ok(Expression::Member(Box::new(MemberExpression {
                storage: MemberExpressionStorage {
                    receiver: lower_expression_handle(syntax_trees, member.receiver)?,
                    member_symbol: SymbolHandle::invalid(),
                    member: lower_name(&member.member),
                },
            })))
        }
        syntax::expression::ExpressionNode::Mutable(expression) => Ok(Expression::Mutable(
            Box::new(lower_expression_handle(syntax_trees, *expression)?),
        )),
        syntax::expression::ExpressionNode::Name(path) => Ok(Expression::Name(lower_name_members(
            syntax_trees
                .expressions
                .identifier_path_members(*path)
                .iter(),
        ))),
        syntax::expression::ExpressionNode::SelfValue => Ok(Expression::Name(
            omega_resolved_trees::expression::NamePath::unresolved(vec![
                omega_resolved_trees::name::DiagnosticName::generated("self"),
            ]),
        )),
        syntax::expression::ExpressionNode::String(value) => {
            Ok(Expression::String(value.as_str().to_owned()))
        }
        syntax::expression::ExpressionNode::StructLiteral(struct_literal) => {
            Ok(Expression::StructLiteral(StructLiteral {
                storage: StructLiteralStorage {
                    type_name: lower_name(&struct_literal.type_name),
                    fields: syntax_trees
                        .expressions
                        .struct_fields(struct_literal.fields)
                        .iter()
                        .map(|field| {
                            Ok(StructLiteralField {
                                name: lower_name(&field.name),
                                value: lower_expression_handle(syntax_trees, field.value)?,
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?
                        .into_boxed_slice(),
                },
            }))
        }
    }
}

fn lower_binary_operator(operator: syntax::expression::BinaryOperator) -> BinaryOperator {
    match operator {
        syntax::expression::BinaryOperator::Add => BinaryOperator::Add,
        syntax::expression::BinaryOperator::And => BinaryOperator::And,
        syntax::expression::BinaryOperator::Divide => BinaryOperator::Divide,
        syntax::expression::BinaryOperator::Equal => BinaryOperator::Equal,
        syntax::expression::BinaryOperator::Greater => BinaryOperator::Greater,
        syntax::expression::BinaryOperator::GreaterOrEqual => BinaryOperator::GreaterOrEqual,
        syntax::expression::BinaryOperator::Less => BinaryOperator::Less,
        syntax::expression::BinaryOperator::LessOrEqual => BinaryOperator::LessOrEqual,
        syntax::expression::BinaryOperator::Modulo => BinaryOperator::Modulo,
        syntax::expression::BinaryOperator::Multiply => BinaryOperator::Multiply,
        syntax::expression::BinaryOperator::NotEqual => BinaryOperator::NotEqual,
        syntax::expression::BinaryOperator::Or => BinaryOperator::Or,
        syntax::expression::BinaryOperator::ShiftLeft => BinaryOperator::ShiftLeft,
        syntax::expression::BinaryOperator::ShiftRight => BinaryOperator::ShiftRight,
        syntax::expression::BinaryOperator::Subtract => BinaryOperator::Subtract,
    }
}
