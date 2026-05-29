use crate::expression::domain_membership::lower_domain_membership_expression;
use crate::expression::name_paths::{
    lower_name_path_members_into_table, lower_table_name_path_node_into_table,
};
use crate::expression::operators::lower_binary_operator;
use crate::name::lower_name;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(super) struct ExpressionTableLowerer<'program, 'target> {
    program: Option<&'program resolved::SymbolResolvedTrees>,
    source: &'program resolved::expression::ExpressionTable,
    target: &'target mut typed::expression::ExpressionTable,
    self_substitution: Option<typed::expression::ExpressionHandle>,
}

impl<'program, 'target> ExpressionTableLowerer<'program, 'target> {
    pub(super) fn new(
        program: Option<&'program resolved::SymbolResolvedTrees>,
        source: &'program resolved::expression::ExpressionTable,
        target: &'target mut typed::expression::ExpressionTable,
        self_substitution: Option<typed::expression::ExpressionHandle>,
    ) -> Self {
        Self {
            program,
            source,
            target,
            self_substitution,
        }
    }

    pub(super) fn lower(
        &mut self,
        expression: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        match self.source.expression(expression) {
            resolved::expression::ExpressionNode::ArrayLiteral(values) => {
                let values = self.lower_expression_handle_span(*values)?;
                Ok(self
                    .target
                    .insert(typed::expression::ExpressionNode::ArrayLiteral(values)))
            }
            resolved::expression::ExpressionNode::Binary(binary) => {
                let left = self.lower(binary.left)?;
                let right = self.lower(binary.right)?;
                Ok(self
                    .target
                    .insert(typed::expression::ExpressionNode::Binary(
                        typed::expression::TableBinaryExpression {
                            left,
                            operator: lower_binary_operator(binary.operator),
                            right,
                        },
                    )))
            }
            resolved::expression::ExpressionNode::Boolean(value) => Ok(self
                .target
                .insert(typed::expression::ExpressionNode::Boolean(*value))),
            resolved::expression::ExpressionNode::Cast(cast) => {
                let value = self.lower(cast.value)?;
                let target_type =
                    lower_name_path_members_into_table(self.source, self.target, cast.target_type);
                Ok(self.target.insert(typed::expression::ExpressionNode::Cast(
                    typed::expression::TableCastExpression { value, target_type },
                )))
            }
            resolved::expression::ExpressionNode::Call(call) => {
                let receiver = self.lower_optional(call.receiver)?;
                let arguments = self.lower_expression_handle_span(call.arguments)?;
                Ok(self.target.insert(typed::expression::ExpressionNode::Call(
                    typed::expression::TableCallExpression {
                        receiver,
                        target_symbol: call.target_symbol,
                        target: lower_name(&call.target),
                        arguments,
                    },
                )))
            }
            resolved::expression::ExpressionNode::Float(value) => {
                Ok(self.target.insert(typed::expression::ExpressionNode::Float(
                    typed::expression::FloatLiteral::new(value.value()),
                )))
            }
            resolved::expression::ExpressionNode::Indexed(indexed) => {
                let collection = self.lower(indexed.collection)?;
                let index = self.lower(indexed.index)?;
                Ok(self
                    .target
                    .insert(typed::expression::ExpressionNode::Indexed(
                        typed::expression::TableIndexedExpression { collection, index },
                    )))
            }
            resolved::expression::ExpressionNode::Integer(value) => Ok(self
                .target
                .insert(typed::expression::ExpressionNode::Integer(*value))),
            resolved::expression::ExpressionNode::Membership(membership) => {
                self.lower_membership_expression(membership)
            }
            resolved::expression::ExpressionNode::Member(member) => {
                let receiver = self.lower(member.receiver)?;
                Ok(self
                    .target
                    .insert(typed::expression::ExpressionNode::Member(
                        typed::expression::TableMemberExpression {
                            receiver,
                            member_symbol: member.member_symbol,
                            member: lower_name(&member.member),
                        },
                    )))
            }
            resolved::expression::ExpressionNode::Mutable(expression) => {
                let expression = self.lower(*expression)?;
                Ok(self
                    .target
                    .insert(typed::expression::ExpressionNode::Mutable(expression)))
            }
            resolved::expression::ExpressionNode::Name(path) => {
                if path.is_self_value
                    && path.members.count() == 1
                    && let Some(substitution) = self.self_substitution
                {
                    return Ok(substitution);
                }
                let path = lower_table_name_path_node_into_table(self.source, self.target, path);
                Ok(self
                    .target
                    .insert(typed::expression::ExpressionNode::Name(path)))
            }
            resolved::expression::ExpressionNode::Range(range) => {
                let start = self.lower_optional(range.start)?;
                let end = self.lower_optional(range.end)?;
                Ok(self.target.insert(typed::expression::ExpressionNode::Range(
                    typed::expression::TableRangeExpression { start, end },
                )))
            }
            resolved::expression::ExpressionNode::StructLiteral(struct_literal) => {
                let fields = self.lower_struct_literal_field_span(struct_literal.fields)?;
                Ok(self
                    .target
                    .insert(typed::expression::ExpressionNode::StructLiteral(
                        typed::expression::TableStructLiteral {
                            type_name: lower_name(&struct_literal.type_name),
                            fields,
                        },
                    )))
            }
            resolved::expression::ExpressionNode::String(value) => {
                Ok(self
                    .target
                    .insert(typed::expression::ExpressionNode::String(
                        value.shared_text(),
                    )))
            }
        }
    }

    fn lower_optional(
        &mut self,
        expression: resolved::expression::ExpressionHandle,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        if !expression.is_valid() {
            return Ok(typed::expression::ExpressionHandle::invalid());
        }

        self.lower(expression)
    }

    fn lower_expression_handle_span(
        &mut self,
        expressions: omega_core::arena::HandleSpan<resolved::expression::ExpressionHandle>,
    ) -> Result<omega_core::arena::HandleSpan<typed::expression::ExpressionHandle>, Diagnostic>
    {
        let lowered = self
            .source
            .expression_handles(expressions)
            .iter()
            .copied()
            .map(|expression| self.lower(expression))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(self.target.insert_expression_handles(lowered))
    }

    fn lower_struct_literal_field_span(
        &mut self,
        fields: omega_core::arena::HandleSpan<resolved::expression::TableStructLiteralField>,
    ) -> Result<omega_core::arena::HandleSpan<typed::expression::TableStructLiteralField>, Diagnostic>
    {
        let lowered = self
            .source
            .struct_fields(fields)
            .iter()
            .map(|field| {
                let value = self.lower(field.value)?;
                Ok(typed::expression::TableStructLiteralField {
                    name: lower_name(&field.name),
                    value,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;

        Ok(self.target.insert_struct_fields(lowered))
    }

    fn lower_membership_expression(
        &mut self,
        membership: &resolved::expression::TableMembershipExpression,
    ) -> Result<typed::expression::ExpressionHandle, Diagnostic> {
        let Some(program) = self.program else {
            return Err(Diagnostic::error(
                "cannot lower executable domain membership without a resolved program context",
            ));
        };
        if !membership.domain_symbol.is_valid() {
            let domain_name = resolved::expression::display_name_path(
                self.source.name_path_members(membership.domain),
                "::",
            );
            return Err(Diagnostic::error(format!(
                "unknown domain `{domain_name}` in executable membership expression"
            )));
        }
        let value = self.lower(membership.value)?;
        lower_domain_membership_expression(program, self.target, value, membership.domain_symbol)
    }
}
