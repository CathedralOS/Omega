use crate::name::lower_name;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_expression_from_table(
    table: &resolved::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::Expression, Diagnostic> {
    match table.expression(expression) {
        resolved::expression::ExpressionNode::ArrayLiteral(values) => {
            let mut lowered = Vec::new();
            for value in table.expression_handles(*values) {
                lowered.push(lower_expression_from_table(table, *value)?);
            }
            Ok(typed::expression::Expression::ArrayLiteral(lowered))
        }
        resolved::expression::ExpressionNode::Binary(binary) => Ok(
            typed::expression::Expression::Binary(Box::new(typed::expression::BinaryExpression {
                left: lower_expression_from_table(table, binary.left)?,
                operator: lower_binary_operator(binary.operator),
                right: lower_expression_from_table(table, binary.right)?,
            })),
        ),
        resolved::expression::ExpressionNode::Boolean(value) => {
            Ok(typed::expression::Expression::Boolean(*value))
        }
        resolved::expression::ExpressionNode::Cast(cast) => Ok(
            typed::expression::Expression::Cast(Box::new(typed::expression::CastExpression {
                value: lower_expression_from_table(table, cast.value)?,
                target_type: lower_table_name_path(table, cast.target_type),
            })),
        ),
        resolved::expression::ExpressionNode::Call(call) => Ok(
            typed::expression::Expression::Call(Box::new(typed::expression::CallExpression {
                receiver: call
                    .receiver
                    .is_valid()
                    .then(|| lower_expression_from_table(table, call.receiver))
                    .transpose()?
                    .map(Box::new),
                target_symbol: call.target_symbol,
                target: lower_name(&call.target),
                arguments: lower_expression_span_from_table(table, call.arguments)?,
            })),
        ),
        resolved::expression::ExpressionNode::Float(value) => {
            Ok(typed::expression::Expression::Float(
                typed::expression::FloatLiteral::new(value.value()),
            ))
        }
        resolved::expression::ExpressionNode::Indexed(indexed) => {
            Ok(typed::expression::Expression::Indexed(Box::new(
                typed::expression::IndexedExpression {
                    collection: lower_expression_from_table(table, indexed.collection)?,
                    index: lower_expression_from_table(table, indexed.index)?,
                },
            )))
        }
        resolved::expression::ExpressionNode::Integer(value) => {
            Ok(typed::expression::Expression::Integer(*value))
        }
        resolved::expression::ExpressionNode::Member(member) => Ok(
            typed::expression::Expression::Member(Box::new(typed::expression::MemberExpression {
                receiver: lower_expression_from_table(table, member.receiver)?,
                member_symbol: member.member_symbol,
                member: lower_name(&member.member),
            })),
        ),
        resolved::expression::ExpressionNode::Mutable(expression) => {
            Ok(typed::expression::Expression::Mutable(Box::new(
                lower_expression_from_table(table, *expression)?,
            )))
        }
        resolved::expression::ExpressionNode::Name(path) => Ok(
            typed::expression::Expression::Name(lower_table_name_path_node(table, path)),
        ),
        resolved::expression::ExpressionNode::StructLiteral(struct_literal) => Ok(
            typed::expression::Expression::StructLiteral(typed::expression::StructLiteral {
                type_name: lower_name(&struct_literal.type_name),
                fields: lower_struct_literal_fields_from_table(table, struct_literal.fields)?,
            }),
        ),
        resolved::expression::ExpressionNode::String(value) => Ok(
            typed::expression::Expression::String(value.as_str().to_owned()),
        ),
    }
}

fn lower_expression_span_from_table(
    table: &resolved::expression::ExpressionTable,
    expressions: omega_core::arena::HandleSpan<resolved::expression::ExpressionHandle>,
) -> Result<Vec<typed::expression::Expression>, Diagnostic> {
    let mut lowered = Vec::new();

    for expression in table.expression_handles(expressions) {
        lowered.push(lower_expression_from_table(table, *expression)?);
    }

    Ok(lowered)
}

fn lower_struct_literal_fields_from_table(
    table: &resolved::expression::ExpressionTable,
    fields: omega_core::arena::HandleSpan<resolved::expression::TableStructLiteralField>,
) -> Result<Vec<typed::expression::StructLiteralField>, Diagnostic> {
    let mut lowered = Vec::new();

    for field in table.struct_fields(fields) {
        lowered.push(typed::expression::StructLiteralField {
            name: lower_name(&field.name),
            value: lower_expression_from_table(table, field.value)?,
        });
    }

    Ok(lowered)
}

fn lower_table_name_path(
    table: &resolved::expression::ExpressionTable,
    members: omega_core::arena::HandleSpan<resolved::name::DiagnosticName>,
) -> typed::expression::NamePath {
    typed::expression::NamePath::unresolved(
        table
            .name_path_members(members)
            .iter()
            .map(lower_name)
            .collect(),
    )
}

fn lower_table_name_path_node(
    table: &resolved::expression::ExpressionTable,
    path: &resolved::expression::TableNamePath,
) -> typed::expression::NamePath {
    typed::expression::NamePath::resolved(
        table
            .name_path_members(path.members)
            .iter()
            .map(lower_name)
            .collect(),
        path.head_symbol,
        path.symbol,
    )
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
