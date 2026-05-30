use crate::name::lower_name;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable, FloatLiteral,
    TableBinaryExpression, TableCallExpression, TableCastExpression, TableIndexedExpression,
    TableMemberExpression, TableMembershipExpression, TableNamePath, TableRangeExpression,
    TableStructLiteral, TableStructLiteralField,
};
use omega_syntax_trees as syntax;
use omega_syntax_trees::SyntaxTrees;

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

fn lower_expression_node_into_table(
    syntax_trees: &SyntaxTrees,
    expressions: &mut ExpressionTable,
    expression: &syntax::expression::ExpressionNode,
) -> Result<ExpressionHandle, Diagnostic> {
    match expression {
        syntax::expression::ExpressionNode::ArrayLiteral(values) => {
            let span = expressions.reserve_expression_handles(values.count());
            for (offset, value) in syntax_trees
                .expressions
                .expression_handles(*values)
                .iter()
                .enumerate()
            {
                let value = lower_expression_into_table(syntax_trees, expressions, *value)?;
                expressions.set_expression_handle_at_offset(
                    span,
                    offset
                        .try_into()
                        .expect("expression handle span count overflow"),
                    value,
                );
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
            let arguments = expressions.reserve_expression_handles(call.arguments.count());
            for (offset, argument) in syntax_trees
                .expressions
                .expression_handles(call.arguments)
                .iter()
                .enumerate()
            {
                let argument = lower_expression_into_table(syntax_trees, expressions, *argument)?;
                expressions.set_expression_handle_at_offset(
                    arguments,
                    offset
                        .try_into()
                        .expect("expression handle span count overflow"),
                    argument,
                );
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
        syntax::expression::ExpressionNode::Membership(membership) => {
            let value = lower_expression_into_table(syntax_trees, expressions, membership.value)?;
            let mut domain = HandleSpan::empty();
            for member in syntax_trees
                .expressions
                .identifier_path_members(membership.domain)
            {
                expressions.push_name_path_member(&mut domain, lower_name(member));
            }
            Ok(
                expressions.insert(ExpressionNode::Membership(TableMembershipExpression {
                    value,
                    domain,
                    domain_symbol: SymbolHandle::invalid(),
                })),
            )
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
                is_self_value: false,
                head_symbol: SymbolHandle::invalid(),
                symbol: SymbolHandle::invalid(),
            })))
        }
        syntax::expression::ExpressionNode::Range(range) => {
            let start = range
                .start
                .is_valid()
                .then(|| lower_expression_into_table(syntax_trees, expressions, range.start))
                .transpose()?
                .unwrap_or_else(ExpressionHandle::invalid);
            let end = range
                .end
                .is_valid()
                .then(|| lower_expression_into_table(syntax_trees, expressions, range.end))
                .transpose()?
                .unwrap_or_else(ExpressionHandle::invalid);
            Ok(
                expressions.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                })),
            )
        }
        syntax::expression::ExpressionNode::SelfValue => {
            let mut members = HandleSpan::empty();
            expressions.push_name_path_member(
                &mut members,
                omega_symbol_resolved_trees::name::DiagnosticName::generated_static("self"),
            );
            Ok(expressions.insert(ExpressionNode::Name(TableNamePath {
                members,
                is_self_value: true,
                head_symbol: SymbolHandle::invalid(),
                symbol: SymbolHandle::invalid(),
            })))
        }
        syntax::expression::ExpressionNode::String(value) => {
            Ok(expressions.insert(ExpressionNode::String(value.clone())))
        }
        syntax::expression::ExpressionNode::StructLiteral(struct_literal) => {
            let fields = expressions.reserve_struct_fields(struct_literal.fields.count());
            for (offset, field) in syntax_trees
                .expressions
                .struct_fields(struct_literal.fields)
                .iter()
                .enumerate()
            {
                let value = lower_expression_into_table(syntax_trees, expressions, field.value)?;
                expressions.set_struct_field_at_offset(
                    fields,
                    offset
                        .try_into()
                        .expect("struct literal field span count overflow"),
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
