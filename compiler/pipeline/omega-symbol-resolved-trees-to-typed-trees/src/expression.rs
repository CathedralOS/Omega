use crate::name::lower_name;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_expression_handle_from_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    match source.expression(expression) {
        resolved::expression::ExpressionNode::ArrayLiteral(values) => {
            let values = lower_expression_handle_span_from_table(source, target, *values)?;
            Ok(target.insert(typed::expression::ExpressionNode::ArrayLiteral(values)))
        }
        resolved::expression::ExpressionNode::Binary(binary) => {
            let left = lower_expression_handle_from_table(source, target, binary.left)?;
            let right = lower_expression_handle_from_table(source, target, binary.right)?;
            Ok(target.insert(typed::expression::ExpressionNode::Binary(
                typed::expression::TableBinaryExpression {
                    left,
                    operator: lower_binary_operator(binary.operator),
                    right,
                },
            )))
        }
        resolved::expression::ExpressionNode::Boolean(value) => {
            Ok(target.insert(typed::expression::ExpressionNode::Boolean(*value)))
        }
        resolved::expression::ExpressionNode::Cast(cast) => {
            let value = lower_expression_handle_from_table(source, target, cast.value)?;
            let target_type = lower_name_path_members_into_table(source, target, cast.target_type);
            Ok(target.insert(typed::expression::ExpressionNode::Cast(
                typed::expression::TableCastExpression { value, target_type },
            )))
        }
        resolved::expression::ExpressionNode::Call(call) => {
            let receiver = call
                .receiver
                .is_valid()
                .then(|| lower_expression_handle_from_table(source, target, call.receiver))
                .transpose()?
                .unwrap_or_else(typed::expression::ExpressionHandle::invalid);
            let arguments =
                lower_expression_handle_span_from_table(source, target, call.arguments)?;
            Ok(target.insert(typed::expression::ExpressionNode::Call(
                typed::expression::TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: lower_name(&call.target),
                    arguments,
                },
            )))
        }
        resolved::expression::ExpressionNode::Float(value) => {
            Ok(target.insert(typed::expression::ExpressionNode::Float(
                typed::expression::FloatLiteral::new(value.value()),
            )))
        }
        resolved::expression::ExpressionNode::Indexed(indexed) => {
            let collection =
                lower_expression_handle_from_table(source, target, indexed.collection)?;
            let index = lower_expression_handle_from_table(source, target, indexed.index)?;
            Ok(target.insert(typed::expression::ExpressionNode::Indexed(
                typed::expression::TableIndexedExpression { collection, index },
            )))
        }
        resolved::expression::ExpressionNode::Integer(value) => {
            Ok(target.insert(typed::expression::ExpressionNode::Integer(*value)))
        }
        resolved::expression::ExpressionNode::Member(member) => {
            let receiver = lower_expression_handle_from_table(source, target, member.receiver)?;
            Ok(target.insert(typed::expression::ExpressionNode::Member(
                typed::expression::TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: lower_name(&member.member),
                },
            )))
        }
        resolved::expression::ExpressionNode::Mutable(expression) => {
            let expression = lower_expression_handle_from_table(source, target, *expression)?;
            Ok(target.insert(typed::expression::ExpressionNode::Mutable(expression)))
        }
        resolved::expression::ExpressionNode::Name(path) => {
            let path = lower_table_name_path_node_into_table(source, target, path);
            Ok(target.insert(typed::expression::ExpressionNode::Name(path)))
        }
        resolved::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let fields =
                lower_struct_literal_field_span_from_table(source, target, struct_literal.fields)?;
            Ok(
                target.insert(typed::expression::ExpressionNode::StructLiteral(
                    typed::expression::TableStructLiteral {
                        type_name: lower_name(&struct_literal.type_name),
                        fields,
                    },
                )),
            )
        }
        resolved::expression::ExpressionNode::String(value) => Ok(target.insert(
            typed::expression::ExpressionNode::String(value.shared_text()),
        )),
    }
}

fn lower_expression_handle_span_from_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expressions: omega_core::arena::HandleSpan<resolved::expression::ExpressionHandle>,
) -> Result<omega_core::arena::HandleSpan<typed::expression::ExpressionHandle>, Diagnostic> {
    let lowered = source
        .expression_handles(expressions)
        .iter()
        .copied()
        .map(|expression| lower_expression_handle_from_table(source, target, expression))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(target.insert_expression_handles(lowered))
}

fn lower_struct_literal_field_span_from_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    fields: omega_core::arena::HandleSpan<resolved::expression::TableStructLiteralField>,
) -> Result<omega_core::arena::HandleSpan<typed::expression::TableStructLiteralField>, Diagnostic> {
    let lowered = source
        .struct_fields(fields)
        .iter()
        .map(|field| {
            let value = lower_expression_handle_from_table(source, target, field.value)?;
            Ok(typed::expression::TableStructLiteralField {
                name: lower_name(&field.name),
                value,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    Ok(target.insert_struct_fields(lowered))
}

fn lower_name_path_members_into_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    members: omega_core::arena::HandleSpan<resolved::name::DiagnosticName>,
) -> omega_core::arena::HandleSpan<typed::name::ProgramName> {
    let mut lowered = omega_core::arena::HandleSpan::empty();

    for member in source.name_path_members(members) {
        target.push_name_path_member(&mut lowered, lower_name(member));
    }

    lowered
}

fn lower_name_path_member_symbols_into_table(
    target: &mut typed::expression::ExpressionTable,
    member_count: u32,
    head_symbol: omega_core::symbols::SymbolHandle,
    symbol: omega_core::symbols::SymbolHandle,
) -> omega_core::arena::HandleSpan<omega_core::symbols::SymbolHandle> {
    let mut lowered = omega_core::arena::HandleSpan::empty();

    for offset in 0..member_count {
        let member_symbol = if offset == 0 {
            head_symbol
        } else if offset + 1 == member_count {
            symbol
        } else {
            omega_core::symbols::SymbolHandle::invalid()
        };
        target.push_name_path_member_symbol(&mut lowered, member_symbol);
    }

    lowered
}

fn lower_table_name_path_node_into_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    path: &resolved::expression::TableNamePath,
) -> typed::expression::TableNamePath {
    let members = lower_name_path_members_into_table(source, target, path.members);
    let member_symbols = lower_name_path_member_symbols_into_table(
        target,
        path.members.count(),
        path.head_symbol,
        path.symbol,
    );

    typed::expression::TableNamePath {
        members,
        member_symbols,
        head_symbol: path.head_symbol,
        symbol: path.symbol,
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

#[cfg(test)]
mod tests {
    use super::lower_expression_handle_from_table;
    use omega_core::arena::HandleSpan;
    use omega_symbol_resolved_trees as resolved;
    use omega_typed_trees as typed;

    #[test]
    fn lowers_binary_expression_directly_into_typed_table() {
        let mut source = resolved::expression::ExpressionTable::new();
        let left = source.insert(resolved::expression::ExpressionNode::Integer(1));
        let right = source.insert(resolved::expression::ExpressionNode::Integer(2));
        let expression = source.insert(resolved::expression::ExpressionNode::Binary(
            resolved::expression::TableBinaryExpression {
                left,
                operator: resolved::expression::BinaryOperator::Add,
                right,
            },
        ));

        let mut target = typed::expression::ExpressionTable::new();
        let lowered = lower_expression_handle_from_table(&source, &mut target, expression)
            .expect("direct lowering should succeed");

        assert_eq!(target.display_name(lowered), "1 + 2");
        assert_eq!(target.expression_count(), 3);
    }

    #[test]
    fn lowers_expression_spans_directly_into_typed_table() {
        let mut source = resolved::expression::ExpressionTable::new();
        let mut values = HandleSpan::empty();
        let one = source.insert(resolved::expression::ExpressionNode::Integer(1));
        let two = source.insert(resolved::expression::ExpressionNode::Integer(2));
        source.push_expression_handle(&mut values, one);
        source.push_expression_handle(&mut values, two);
        let expression = source.insert(resolved::expression::ExpressionNode::ArrayLiteral(values));

        let mut target = typed::expression::ExpressionTable::new();
        let lowered = lower_expression_handle_from_table(&source, &mut target, expression)
            .expect("direct lowering should succeed");

        let typed::expression::ExpressionNode::ArrayLiteral(values) = target.expression(lowered)
        else {
            panic!("root should lower to array literal");
        };

        assert_eq!(values.count(), 2);
        assert_eq!(target.display_name(lowered), "[1, 2]");
        assert_eq!(target.expression_count(), 3);
    }
}
