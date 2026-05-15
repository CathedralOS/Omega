use crate::name::{lower_name, lower_name_members};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_resolved_trees::expression::{
    ArrayLiteralExpression, ArrayLiteralExpressionStorage, BinaryExpression,
    BinaryExpressionStorage, BinaryOperator, CallExpression, CallExpressionStorage, CastExpression,
    CastExpressionStorage, Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
    FloatLiteral, IndexedExpression, IndexedExpressionStorage, MemberExpression,
    MemberExpressionStorage, StructLiteral, StructLiteralField, StructLiteralStorage,
    TableBinaryExpression, TableCallExpression, TableCastExpression, TableIndexedExpression,
    TableMemberExpression, TableNamePath, TableStructLiteral, TableStructLiteralField,
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

pub(crate) fn lower_expression_into_table(
    syntax_trees: &SyntaxTrees,
    expressions: &mut ExpressionTable,
    expression: syntax::expression::ExpressionHandle,
) -> Result<ExpressionHandle, Diagnostic> {
    lower_expression_node_into_table(
        syntax_trees,
        expressions,
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
                        .collect::<Result<Box<[_]>, _>>()?,
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
                        .collect::<Result<Box<[_]>, _>>()?,
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
            omega_resolved_trees::expression::NamePath::single_unresolved(
                omega_resolved_trees::name::DiagnosticName::generated_static("self"),
            ),
        )),
        syntax::expression::ExpressionNode::String(value) => Ok(Expression::String(value.clone())),
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
                        .collect::<Result<Box<[_]>, Diagnostic>>()?,
                },
            }))
        }
    }
}

fn lower_expression_node_into_table(
    syntax_trees: &SyntaxTrees,
    expressions: &mut ExpressionTable,
    expression: &syntax::expression::ExpressionNode,
) -> Result<ExpressionHandle, Diagnostic> {
    match expression {
        syntax::expression::ExpressionNode::ArrayLiteral(values) => {
            let mut span = HandleSpan::empty();
            for value in syntax_trees.expressions.expression_handles(*values) {
                let value = lower_expression_into_table(syntax_trees, expressions, *value)?;
                expressions.push_expression_handle(&mut span, value);
            }
            Ok(expressions.insert(ExpressionNode::ArrayLiteral(span)))
        }
        syntax::expression::ExpressionNode::Binary(binary) => {
            let left = lower_expression_into_table(syntax_trees, expressions, binary.left)?;
            let right = lower_expression_into_table(syntax_trees, expressions, binary.right)?;
            Ok(
                expressions.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: lower_binary_operator(binary.operator),
                    right,
                })),
            )
        }
        syntax::expression::ExpressionNode::Boolean(value) => {
            Ok(expressions.insert(ExpressionNode::Boolean(*value)))
        }
        syntax::expression::ExpressionNode::Cast(cast) => {
            let value = lower_expression_into_table(syntax_trees, expressions, cast.value)?;
            let mut target_type = HandleSpan::empty();
            for member in syntax_trees
                .expressions
                .identifier_path_members(cast.target_type)
            {
                expressions.push_name_path_member(&mut target_type, lower_name(member));
            }
            Ok(
                expressions.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                })),
            )
        }
        syntax::expression::ExpressionNode::Call(call) => {
            let receiver = if call.receiver.is_valid() {
                lower_expression_into_table(syntax_trees, expressions, call.receiver)?
            } else {
                ExpressionHandle::invalid()
            };
            let mut arguments = HandleSpan::empty();
            for argument in syntax_trees.expressions.expression_handles(call.arguments) {
                let argument = lower_expression_into_table(syntax_trees, expressions, *argument)?;
                expressions.push_expression_handle(&mut arguments, argument);
            }
            Ok(
                expressions.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: SymbolHandle::invalid(),
                    target: lower_name(&call.target),
                    arguments,
                })),
            )
        }
        syntax::expression::ExpressionNode::Float(value) => {
            let Some(value) = FloatLiteral::parse(value.as_str()) else {
                return Err(Diagnostic::error(format!(
                    "invalid float literal `{}`",
                    value.as_str()
                )));
            };
            Ok(expressions.insert(ExpressionNode::Float(value)))
        }
        syntax::expression::ExpressionNode::Indexed(indexed) => {
            let collection =
                lower_expression_into_table(syntax_trees, expressions, indexed.collection)?;
            let index = lower_expression_into_table(syntax_trees, expressions, indexed.index)?;
            Ok(
                expressions.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                })),
            )
        }
        syntax::expression::ExpressionNode::Integer(value) => {
            Ok(expressions.insert(ExpressionNode::Integer(*value)))
        }
        syntax::expression::ExpressionNode::Member(member) => {
            let receiver = lower_expression_into_table(syntax_trees, expressions, member.receiver)?;
            Ok(
                expressions.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: SymbolHandle::invalid(),
                    member: lower_name(&member.member),
                })),
            )
        }
        syntax::expression::ExpressionNode::Mutable(expression) => {
            let expression = lower_expression_into_table(syntax_trees, expressions, *expression)?;
            Ok(expressions.insert(ExpressionNode::Mutable(expression)))
        }
        syntax::expression::ExpressionNode::Name(path) => {
            let mut members = HandleSpan::empty();
            for member in syntax_trees.expressions.identifier_path_members(*path) {
                expressions.push_name_path_member(&mut members, lower_name(member));
            }
            Ok(expressions.insert(ExpressionNode::Name(TableNamePath {
                members,
                head_symbol: SymbolHandle::invalid(),
                symbol: SymbolHandle::invalid(),
            })))
        }
        syntax::expression::ExpressionNode::SelfValue => {
            let mut members = HandleSpan::empty();
            expressions.push_name_path_member(
                &mut members,
                omega_resolved_trees::name::DiagnosticName::generated_static("self"),
            );
            Ok(expressions.insert(ExpressionNode::Name(TableNamePath {
                members,
                head_symbol: SymbolHandle::invalid(),
                symbol: SymbolHandle::invalid(),
            })))
        }
        syntax::expression::ExpressionNode::String(value) => {
            Ok(expressions.insert(ExpressionNode::String(value.clone())))
        }
        syntax::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let mut fields = HandleSpan::empty();
            for field in syntax_trees
                .expressions
                .struct_fields(struct_literal.fields)
            {
                let value = lower_expression_into_table(syntax_trees, expressions, field.value)?;
                expressions.push_struct_field(
                    &mut fields,
                    TableStructLiteralField {
                        name: lower_name(&field.name),
                        value,
                    },
                );
            }
            Ok(
                expressions.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: lower_name(&struct_literal.type_name),
                    fields,
                })),
            )
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
