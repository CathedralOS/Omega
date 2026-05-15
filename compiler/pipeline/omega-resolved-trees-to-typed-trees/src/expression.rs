use crate::name::{lower_name, lower_name_path};
use omega_core::diagnostics::Diagnostic;
use omega_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_expression(
    expression: &resolved::expression::Expression,
) -> Result<typed::expression::Expression, Diagnostic> {
    match expression {
        resolved::expression::Expression::ArrayLiteral(array_literal) => {
            Ok(typed::expression::Expression::ArrayLiteral(
                array_literal
                    .values
                    .iter()
                    .map(lower_expression)
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        resolved::expression::Expression::Binary(binary) => Ok(
            typed::expression::Expression::Binary(Box::new(typed::expression::BinaryExpression {
                left: lower_expression(&binary.left)?,
                operator: lower_binary_operator(binary.operator),
                right: lower_expression(&binary.right)?,
            })),
        ),
        resolved::expression::Expression::Boolean(value) => {
            Ok(typed::expression::Expression::Boolean(*value))
        }
        resolved::expression::Expression::Cast(cast) => Ok(typed::expression::Expression::Cast(
            Box::new(typed::expression::CastExpression {
                value: lower_expression(&cast.value)?,
                target_type: lower_name_path(&cast.target_type),
            }),
        )),
        resolved::expression::Expression::Call(call) => Ok(typed::expression::Expression::Call(
            Box::new(typed::expression::CallExpression {
                receiver: call
                    .receiver
                    .as_deref()
                    .map(lower_expression)
                    .transpose()?
                    .map(Box::new),
                target_symbol: call.target_symbol,
                target: lower_name(&call.target),
                arguments: call
                    .arguments
                    .iter()
                    .map(lower_expression)
                    .collect::<Result<Vec<_>, _>>()?,
            }),
        )),
        resolved::expression::Expression::Float(value) => Ok(typed::expression::Expression::Float(
            typed::expression::FloatLiteral::new(value.value()),
        )),
        resolved::expression::Expression::Indexed(indexed) => {
            Ok(typed::expression::Expression::Indexed(Box::new(
                typed::expression::IndexedExpression {
                    collection: lower_expression(&indexed.collection)?,
                    index: lower_expression(&indexed.index)?,
                },
            )))
        }
        resolved::expression::Expression::Integer(value) => {
            Ok(typed::expression::Expression::Integer(*value))
        }
        resolved::expression::Expression::Member(member) => Ok(
            typed::expression::Expression::Member(Box::new(typed::expression::MemberExpression {
                receiver: lower_expression(&member.receiver)?,
                member_symbol: member.member_symbol,
                member: lower_name(&member.member),
            })),
        ),
        resolved::expression::Expression::Mutable(expression) => Ok(
            typed::expression::Expression::Mutable(Box::new(lower_expression(expression)?)),
        ),
        resolved::expression::Expression::Name(path) => {
            Ok(typed::expression::Expression::Name(lower_name_path(path)))
        }
        resolved::expression::Expression::StructLiteral(struct_literal) => Ok(
            typed::expression::Expression::StructLiteral(typed::expression::StructLiteral {
                type_name: lower_name(&struct_literal.type_name),
                fields: struct_literal
                    .fields
                    .iter()
                    .map(|field| {
                        Ok(typed::expression::StructLiteralField {
                            name: lower_name(&field.name),
                            value: lower_expression(&field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?,
            }),
        ),
        resolved::expression::Expression::String(value) => {
            Ok(typed::expression::Expression::String(value.clone()))
        }
    }
}

fn lower_binary_operator(
    operator: resolved::expression::BinaryOperator,
) -> typed::expression::BinaryOperator {
    match operator {
        resolved::expression::BinaryOperator::Add => typed::expression::BinaryOperator::Add,
        resolved::expression::BinaryOperator::And => typed::expression::BinaryOperator::And,
        resolved::expression::BinaryOperator::Divide => typed::expression::BinaryOperator::Divide,
        resolved::expression::BinaryOperator::Equal => typed::expression::BinaryOperator::Equal,
        resolved::expression::BinaryOperator::Greater => typed::expression::BinaryOperator::Greater,
        resolved::expression::BinaryOperator::GreaterOrEqual => {
            typed::expression::BinaryOperator::GreaterOrEqual
        }
        resolved::expression::BinaryOperator::Less => typed::expression::BinaryOperator::Less,
        resolved::expression::BinaryOperator::LessOrEqual => {
            typed::expression::BinaryOperator::LessOrEqual
        }
        resolved::expression::BinaryOperator::Modulo => typed::expression::BinaryOperator::Modulo,
        resolved::expression::BinaryOperator::Multiply => {
            typed::expression::BinaryOperator::Multiply
        }
        resolved::expression::BinaryOperator::NotEqual => {
            typed::expression::BinaryOperator::NotEqual
        }
        resolved::expression::BinaryOperator::Or => typed::expression::BinaryOperator::Or,
        resolved::expression::BinaryOperator::ShiftLeft => {
            typed::expression::BinaryOperator::ShiftLeft
        }
        resolved::expression::BinaryOperator::ShiftRight => {
            typed::expression::BinaryOperator::ShiftRight
        }
        resolved::expression::BinaryOperator::Subtract => {
            typed::expression::BinaryOperator::Subtract
        }
    }
}
