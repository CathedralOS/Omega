//! Conversion between owned `Expression` trees and expression table rows.

use crate::name::Identifier;
use crate::typed_trees::values::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern, NamePath,
    OwnedMatchPattern, StructLiteralField, TableAtomicExpression, TableBinaryExpression,
    TableBorrowExpression, TableCallExpression, TableCastExpression, TableIndexedExpression,
    TableMatchArm, TableMatchExpression, TableMemberExpression, TableNamePath,
    TableRangeExpression, TableStructLiteral, TableStructLiteralField, TableUnaryExpression,
};
use arena::HandleSpan;
use symbols::SymbolHandle;

impl ExpressionTable {
    fn insert_name_path_members(&mut self, path: &NamePath) -> HandleSpan<Identifier> {
        let mut members = HandleSpan::empty();

        for member in path.members() {
            self.name_path_members
                .append_to_span(&mut members, member.clone());
        }

        members
    }

    fn insert_name_path_member_symbols(&mut self, path: &NamePath) -> HandleSpan<SymbolHandle> {
        let mut member_symbols = HandleSpan::empty();

        for member_symbol in path.member_symbols() {
            self.name_path_member_symbols
                .append_to_span(&mut member_symbols, *member_symbol);
        }

        member_symbols
    }

    fn insert_expression_handle_span_from_trees(
        &mut self,
        expressions: &[Expression],
    ) -> HandleSpan<ExpressionHandle> {
        let handles = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression = self.insert_tree(expression);
            self.set_expression_handle_at_offset(
                handles,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        handles
    }

    fn insert_struct_field_span_from_tree(
        &mut self,
        fields: &[StructLiteralField],
    ) -> HandleSpan<TableStructLiteralField> {
        let field_span = self.reserve_struct_fields(
            fields
                .len()
                .try_into()
                .expect("struct literal field span count overflow"),
        );

        for (offset, field) in fields.iter().enumerate() {
            let value = self.insert_tree(&field.value);
            self.set_struct_field_at_offset(
                field_span,
                offset
                    .try_into()
                    .expect("struct literal field span count overflow"),
                TableStructLiteralField {
                    name: field.name.clone(),
                    field_symbol: SymbolHandle::invalid(),
                    value,
                },
            );
        }

        field_span
    }

    pub fn insert_tree(&mut self, expression: &Expression) -> ExpressionHandle {
        match expression {
            Expression::Match(dispatch) => {
                let subject = self.insert_tree(&dispatch.subject);
                let mut arms = Vec::with_capacity(dispatch.arms.len());
                for arm in dispatch.arms.iter() {
                    let pattern = match &arm.pattern {
                        OwnedMatchPattern::Value(value) => {
                            MatchPattern::Value(self.insert_tree(value))
                        }
                        OwnedMatchPattern::Wildcard => MatchPattern::Wildcard,
                    };
                    let value = self.insert_tree(&arm.value);
                    arms.push(TableMatchArm {
                        pattern,
                        value,
                        source_span: arm.source_span,
                    });
                }
                let arms = self.insert_match_arms(arms);
                self.insert(ExpressionNode::Match(TableMatchExpression {
                    subject,
                    arms,
                    ..Default::default()
                }))
            }
            Expression::ArrayLiteral(values) => {
                let values = self.insert_expression_handle_span_from_trees(values);
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            Expression::Atomic(atomic) => {
                let value = self.insert_tree(&atomic.value);
                let result = atomic
                    .result
                    .as_ref()
                    .map(|result| self.insert_tree(result))
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }))
            }
            Expression::Binary(binary) => {
                let left = self.insert_tree(&binary.left);
                let right = self.insert_tree(&binary.right);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            Expression::Boolean(value) => self.insert(ExpressionNode::Boolean(*value)),
            Expression::Cast(cast) => {
                let value = self.insert_tree(&cast.value);
                let target_label = self.insert_name_path_members(&cast.target_label);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    result_type: crate::types::TypeReferenceHandle::invalid(),
                    target_label,
                    domain: cast.domain,
                    // Tree-built casts are compiler-internal (tests/builders)
                    // and never carry the qualification suffix.
                    semantic_domain: HandleSpan::empty(),
                    semantic_domain_arguments: HandleSpan::empty(),
                    semantic_domain_symbol: SymbolHandle::invalid(),
                    semantic_domain_id: language_semantics::SemanticDomainId::NULL,
                    form: cast.form,
                }))
            }
            Expression::Call(call) => {
                let receiver = call
                    .receiver
                    .as_ref()
                    .map(|receiver| self.insert_tree(receiver))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let arguments = self.insert_expression_handle_span_from_trees(&call.arguments);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    authored_machine_application: Default::default(),
                    requirement_application: call.requirement_application,
                    exact_coercion_target: call.exact_coercion_target,
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    static_requirement_dispatch: None,
                    static_machine_parameter: SymbolHandle::invalid(),
                    machine_arguments: Box::default(),
                    quotient_operation: None,
                    private_layout_operation: None,
                    layout_query: None,
                    native_code_layout: None,
                    build_selection: Default::default(),
                    arguments,
                    evidence_arguments: call.evidence_arguments.to_vec().into_boxed_slice(),
                    operational_acknowledgement: call.operational_acknowledgement,
                }))
            }
            Expression::Float(value) => self.insert(ExpressionNode::Float(value.clone())),
            Expression::UndeterminedNaN(format) => {
                self.insert(ExpressionNode::UndeterminedNaN(*format))
            }
            Expression::Indexed(indexed) => {
                let collection = self.insert_tree(&indexed.collection);
                let index = self.insert_tree(&indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            Expression::Integer(value) => self.insert(ExpressionNode::Integer(value.clone())),
            Expression::Member(member) => {
                let receiver = self.insert_tree(&member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                }))
            }
            Expression::Borrow(inner_expression) => {
                let target = self.insert_tree(&inner_expression.target);
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: inner_expression.access,
                }))
            }
            Expression::Name(path) => {
                let members = self.insert_name_path_members(path);
                let member_symbols = self.insert_name_path_member_symbols(path);
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: path.head_symbol(),
                    symbol: path.symbol(),
                }))
            }
            Expression::Range(range) => {
                let start = range
                    .start
                    .as_ref()
                    .map(|start| self.insert_tree(start))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .as_ref()
                    .map(|end| self.insert_tree(end))
                    .unwrap_or_else(ExpressionHandle::invalid);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            Expression::StructLiteral(struct_literal) => {
                let fields = self.insert_struct_field_span_from_tree(&struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    type_symbol: SymbolHandle::invalid(),
                    case_name: struct_literal.case_name.clone(),
                    case_symbol: None,
                    fields,
                }))
            }
            Expression::String(value) => self.insert(ExpressionNode::String(value.clone())),
            Expression::Unary(unary) => {
                let operand = self.insert_tree(&unary.operand);
                self.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }
            Expression::ZeroValue(type_reference) => {
                self.insert(ExpressionNode::ZeroValue(*type_reference))
            }
        }
    }
}
