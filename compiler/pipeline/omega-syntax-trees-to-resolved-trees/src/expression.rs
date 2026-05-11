use crate::name::{lower_name, lower_name_path};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_syntax_trees as syntax;
use omega_resolved_trees::expression::{
    BinaryExpression, BinaryOperator, CallExpression, CastExpression, Expression, FloatLiteral,
    IndexedExpression, MemberExpression, StructLiteral, StructLiteralField,
};

pub(crate) fn lower_expression(
    expression: &syntax::expression::Expression,
) -> Result<Expression, Diagnostic> {
    match expression {
        syntax::expression::Expression::ArrayLiteral(values) => Ok(Expression::ArrayLiteral(
            values
                .iter()
                .map(lower_expression)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        syntax::expression::Expression::Binary(binary) => Ok(Expression::Binary(Box::new(
            BinaryExpression {
                left: lower_expression(&binary.left)?,
                operator: lower_binary_operator(binary.operator),
                right: lower_expression(&binary.right)?,
            },
        ))),
        syntax::expression::Expression::Boolean(value) => Ok(Expression::Boolean(*value)),
        syntax::expression::Expression::Cast(cast) => Ok(Expression::Cast(Box::new(
            CastExpression {
                value: lower_expression(&cast.value)?,
                target_type: lower_name_path(&cast.target_type),
            },
        ))),
        syntax::expression::Expression::Call(call) => Ok(Expression::Call(Box::new(
            CallExpression {
                receiver: call
                    .receiver
                    .as_deref()
                    .map(lower_expression)
                    .transpose()?
                    .map(Box::new),
                target_symbol: SymbolHandle::invalid(),
                target: lower_name(&call.target),
                arguments: call
                    .arguments
                    .iter()
                    .map(lower_expression)
                    .collect::<Result<Vec<_>, _>>()?,
            },
        ))),
        syntax::expression::Expression::Float(value) => {
            let Some(value) = FloatLiteral::parse(value.as_str()) else {
                return Err(Diagnostic::error(format!(
                    "invalid float literal `{}`",
                    value.as_str()
                )));
            };
            Ok(Expression::Float(value))
        }
        syntax::expression::Expression::Indexed(indexed) => Ok(Expression::Indexed(Box::new(
            IndexedExpression {
                collection: lower_expression(&indexed.collection)?,
                index: lower_expression(&indexed.index)?,
            },
        ))),
        syntax::expression::Expression::Integer(value) => Ok(Expression::Integer(*value)),
        syntax::expression::Expression::Member(member) => Ok(Expression::Member(Box::new(
            MemberExpression {
                receiver: lower_expression(&member.receiver)?,
                member_symbol: SymbolHandle::invalid(),
                member: lower_name(&member.member),
            },
        ))),
        syntax::expression::Expression::Mutable(expression) => {
            Ok(Expression::Mutable(Box::new(lower_expression(expression)?)))
        }
        syntax::expression::Expression::Name(path) => Ok(Expression::Name(lower_name_path(path))),
        syntax::expression::Expression::String(value) => {
            Ok(Expression::String(value.as_str().to_owned()))
        }
        syntax::expression::Expression::StructLiteral(struct_literal) => {
            Ok(Expression::StructLiteral(StructLiteral {
                type_name: lower_name(&struct_literal.type_name),
                fields: struct_literal
                    .fields
                    .iter()
                    .map(|field| {
                        Ok(StructLiteralField {
                            name: lower_name(&field.name),
                            value: lower_expression(&field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?,
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
