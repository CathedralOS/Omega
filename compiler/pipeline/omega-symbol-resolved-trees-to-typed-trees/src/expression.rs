use crate::name::lower_name;
use crate::program::Lowerer;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

mod domain_membership;
mod name_paths;

use domain_membership::lower_domain_membership_expression;
use name_paths::{lower_name_path_members_into_table, lower_table_name_path_node_into_table};

pub(crate) fn lower_expression_handle(
    lowerer: &mut Lowerer,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    let source = &lowerer.source_trees.tables.bodies.expressions;
    lower_expression_handle_from_table_with_self_substitution(
        Some(lowerer.source_trees),
        source,
        &mut lowerer.typed_trees.expression_table,
        expression,
        None,
    )
}

pub(crate) fn lower_expression_handle_from_table(
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    lower_expression_handle_from_table_with_self_substitution(
        None, source, target, expression, None,
    )
}

pub(crate) fn lower_expression_handle_from_table_in_program(
    program: &resolved::SymbolResolvedTrees,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    lower_expression_handle_from_table_with_self_substitution(
        Some(program),
        source,
        target,
        expression,
        None,
    )
}

pub(super) fn lower_expression_handle_from_table_with_self_substitution(
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
    self_substitution: Option<typed::expression::ExpressionHandle>,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    match source.expression(expression) {
        resolved::expression::ExpressionNode::ArrayLiteral(values) => {
            let values = lower_expression_handle_span_from_table(
                program,
                source,
                target,
                *values,
                self_substitution,
            )?;
            Ok(target.insert(typed::expression::ExpressionNode::ArrayLiteral(values)))
        }
        resolved::expression::ExpressionNode::Binary(binary) => {
            let left = lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                binary.left,
                self_substitution,
            )?;
            let right = lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                binary.right,
                self_substitution,
            )?;
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
            let value = lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                cast.value,
                self_substitution,
            )?;
            let target_type = lower_name_path_members_into_table(source, target, cast.target_type);
            Ok(target.insert(typed::expression::ExpressionNode::Cast(
                typed::expression::TableCastExpression { value, target_type },
            )))
        }
        resolved::expression::ExpressionNode::Call(call) => {
            let receiver = call
                .receiver
                .is_valid()
                .then(|| {
                    lower_expression_handle_from_table_with_self_substitution(
                        program,
                        source,
                        target,
                        call.receiver,
                        self_substitution,
                    )
                })
                .transpose()?
                .unwrap_or_else(typed::expression::ExpressionHandle::invalid);
            let arguments = lower_expression_handle_span_from_table(
                program,
                source,
                target,
                call.arguments,
                self_substitution,
            )?;
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
            let collection = lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                indexed.collection,
                self_substitution,
            )?;
            let index = lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                indexed.index,
                self_substitution,
            )?;
            Ok(target.insert(typed::expression::ExpressionNode::Indexed(
                typed::expression::TableIndexedExpression { collection, index },
            )))
        }
        resolved::expression::ExpressionNode::Integer(value) => {
            Ok(target.insert(typed::expression::ExpressionNode::Integer(*value)))
        }
        resolved::expression::ExpressionNode::Membership(membership) => {
            let Some(program) = program else {
                return Err(Diagnostic::error(
                    "cannot lower executable domain membership without a resolved program context",
                ));
            };
            if !membership.domain_symbol.is_valid() {
                let domain_name = resolved::expression::display_name_path(
                    source.name_path_members(membership.domain),
                    "::",
                );
                return Err(Diagnostic::error(format!(
                    "unknown domain `{domain_name}` in executable membership expression"
                )));
            }
            let value = lower_expression_handle_from_table_with_self_substitution(
                Some(program),
                source,
                target,
                membership.value,
                self_substitution,
            )?;
            lower_domain_membership_expression(program, target, value, membership.domain_symbol)
        }
        resolved::expression::ExpressionNode::Member(member) => {
            let receiver = lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                member.receiver,
                self_substitution,
            )?;
            Ok(target.insert(typed::expression::ExpressionNode::Member(
                typed::expression::TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: lower_name(&member.member),
                },
            )))
        }
        resolved::expression::ExpressionNode::Mutable(expression) => {
            let expression = lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                *expression,
                self_substitution,
            )?;
            Ok(target.insert(typed::expression::ExpressionNode::Mutable(expression)))
        }
        resolved::expression::ExpressionNode::Name(path) => {
            if path.is_self_value
                && path.members.count() == 1
                && let Some(substitution) = self_substitution
            {
                return Ok(substitution);
            }
            let path = lower_table_name_path_node_into_table(source, target, path);
            Ok(target.insert(typed::expression::ExpressionNode::Name(path)))
        }
        resolved::expression::ExpressionNode::Range(range) => {
            let start = range
                .start
                .is_valid()
                .then(|| {
                    lower_expression_handle_from_table_with_self_substitution(
                        program,
                        source,
                        target,
                        range.start,
                        self_substitution,
                    )
                })
                .transpose()?
                .unwrap_or_else(typed::expression::ExpressionHandle::invalid);
            let end = range
                .end
                .is_valid()
                .then(|| {
                    lower_expression_handle_from_table_with_self_substitution(
                        program,
                        source,
                        target,
                        range.end,
                        self_substitution,
                    )
                })
                .transpose()?
                .unwrap_or_else(typed::expression::ExpressionHandle::invalid);
            Ok(target.insert(typed::expression::ExpressionNode::Range(
                typed::expression::TableRangeExpression { start, end },
            )))
        }
        resolved::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let fields = lower_struct_literal_field_span_from_table(
                program,
                source,
                target,
                struct_literal.fields,
                self_substitution,
            )?;
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
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expressions: omega_core::arena::HandleSpan<resolved::expression::ExpressionHandle>,
    self_substitution: Option<typed::expression::ExpressionHandle>,
) -> Result<omega_core::arena::HandleSpan<typed::expression::ExpressionHandle>, Diagnostic> {
    let lowered = source
        .expression_handles(expressions)
        .iter()
        .copied()
        .map(|expression| {
            lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                expression,
                self_substitution,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(target.insert_expression_handles(lowered))
}

fn lower_struct_literal_field_span_from_table(
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    fields: omega_core::arena::HandleSpan<resolved::expression::TableStructLiteralField>,
    self_substitution: Option<typed::expression::ExpressionHandle>,
) -> Result<omega_core::arena::HandleSpan<typed::expression::TableStructLiteralField>, Diagnostic> {
    let lowered = source
        .struct_fields(fields)
        .iter()
        .map(|field| {
            let value = lower_expression_handle_from_table_with_self_substitution(
                program,
                source,
                target,
                field.value,
                self_substitution,
            )?;
            Ok(typed::expression::TableStructLiteralField {
                name: lower_name(&field.name),
                value,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;

    Ok(target.insert_struct_fields(lowered))
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
