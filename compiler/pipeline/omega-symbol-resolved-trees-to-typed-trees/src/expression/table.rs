use crate::expression::domain_membership::lower_domain_membership_expression;
use crate::expression::name_paths::{
    lower_name_path_members_into_table, lower_table_name_path_node_into_table,
};
use crate::expression::operators::lower_binary_operator;
use crate::expression::spans::{
    lower_expression_handle_span_from_table, lower_struct_literal_field_span_from_table,
};
use crate::name::lower_name;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

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
            let receiver = lower_optional_expression_handle(
                program,
                source,
                target,
                call.receiver,
                self_substitution,
            )?;
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
            let start = lower_optional_expression_handle(
                program,
                source,
                target,
                range.start,
                self_substitution,
            )?;
            let end = lower_optional_expression_handle(
                program,
                source,
                target,
                range.end,
                self_substitution,
            )?;
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

fn lower_optional_expression_handle(
    program: Option<&resolved::SymbolResolvedTrees>,
    source: &resolved::expression::ExpressionTable,
    target: &mut typed::expression::ExpressionTable,
    expression: resolved::expression::ExpressionHandle,
    self_substitution: Option<typed::expression::ExpressionHandle>,
) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
    if !expression.is_valid() {
        return Ok(typed::expression::ExpressionHandle::invalid());
    }

    lower_expression_handle_from_table_with_self_substitution(
        program,
        source,
        target,
        expression,
        self_substitution,
    )
}
