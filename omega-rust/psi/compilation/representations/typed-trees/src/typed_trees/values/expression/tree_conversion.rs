//! Conversion between owned `Expression` trees and expression table rows.

use crate::name::Identifier;
use crate::typed_trees::values::expression::{
    AtomicExpression, BinaryExpression, BorrowExpression, CallExpression, CastExpression,
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, IndexedExpression, MatchArm,
    MatchExpression, MatchPattern, MemberExpression, NamePath, OwnedMatchPattern, RangeExpression,
    StructLiteral, StructLiteralField, TableAtomicExpression, TableBinaryExpression,
    TableBorrowExpression, TableCallExpression, TableCastExpression, TableIndexedExpression,
    TableMatchArm, TableMatchExpression, TableMemberExpression, TableNamePath,
    TableRangeExpression, TableStructLiteral, TableStructLiteralField, TableUnaryExpression,
    UnaryExpression,
};
use arena::HandleSpan;
use std::sync::Arc;
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

    pub fn to_tree(&self, expression: ExpressionHandle) -> Expression {
        match self.expression(expression) {
            ExpressionNode::Match(dispatch) => Expression::Match(Box::new(MatchExpression {
                subject: self.to_tree(dispatch.subject),
                arms: self
                    .match_arms(dispatch.arms)
                    .iter()
                    .map(|arm| MatchArm {
                        pattern: match arm.pattern {
                            MatchPattern::Value(value) => {
                                OwnedMatchPattern::Value(self.to_tree(value))
                            }
                            MatchPattern::Wildcard => OwnedMatchPattern::Wildcard,
                        },
                        value: self.to_tree(arm.value),
                        source_span: arm.source_span,
                    })
                    .collect(),
            })),
            ExpressionNode::ArrayLiteral(values) => Expression::ArrayLiteral(
                self.expression_handles(*values)
                    .iter()
                    .map(|value| self.to_tree(*value))
                    .collect::<Arc<[_]>>(),
            ),
            ExpressionNode::Atomic(atomic) => Expression::Atomic(Box::new(AtomicExpression {
                value: self.to_tree(atomic.value),
                result: atomic
                    .result
                    .is_valid()
                    .then(|| self.to_tree(atomic.result)),
                ordering: atomic.ordering,
                result_custody: atomic.result_custody,
            })),
            ExpressionNode::Binary(binary) => Expression::Binary(Box::new(BinaryExpression {
                left: self.to_tree(binary.left),
                operator: binary.operator,
                right: self.to_tree(binary.right),
            })),
            ExpressionNode::Boolean(value) => Expression::Boolean(*value),
            ExpressionNode::Cast(cast) => Expression::Cast(Box::new(CastExpression {
                value: self.to_tree(cast.value),
                target_type: cast.target_type,
                target_label: NamePath::unresolved_from_iter(
                    self.name_path_members(cast.target_label).iter().cloned(),
                ),
                domain: cast.domain,
                form: cast.form,
            })),
            ExpressionNode::Call(call) => Expression::Call(Box::new(CallExpression {
                requirement_application: call.requirement_application,
                exact_coercion_target: call.exact_coercion_target,
                receiver: call
                    .receiver
                    .is_valid()
                    .then(|| Box::new(self.to_tree(call.receiver))),
                target_symbol: call.target_symbol,
                target: call.target.clone(),
                arguments: self
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| self.to_tree(*argument))
                    .collect::<Arc<[_]>>(),
                evidence_arguments: Arc::from(call.evidence_arguments.clone()),
                operational_acknowledgement: call.operational_acknowledgement,
            })),
            ExpressionNode::Float(value) => Expression::Float(value.clone()),
            ExpressionNode::UndeterminedNaN(format) => Expression::UndeterminedNaN(*format),
            ExpressionNode::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
                collection: self.to_tree(indexed.collection),
                index: self.to_tree(indexed.index),
            })),
            ExpressionNode::Integer(value) => Expression::Integer(value.clone()),
            ExpressionNode::Member(member) => Expression::Member(Box::new(MemberExpression {
                receiver: self.to_tree(member.receiver),
                member_symbol: member.member_symbol,
                member: member.member.clone(),
                case_variant: member.case_variant.clone(),
            })),
            ExpressionNode::Borrow(inner_expression) => {
                Expression::Borrow(Box::new(BorrowExpression {
                    target: self.to_tree(inner_expression.target),
                    access: inner_expression.access,
                }))
            }
            ExpressionNode::Name(path) => Expression::Name(NamePath::resolved_with_member_symbols(
                self.name_path_members(path.members).to_vec(),
                self.name_path_member_symbols(path.member_symbols).to_vec(),
                path.head_symbol,
                path.symbol,
            )),
            ExpressionNode::Range(range) => Expression::Range(Box::new(RangeExpression {
                start: range
                    .start
                    .is_valid()
                    .then(|| Box::new(self.to_tree(range.start))),
                end: range
                    .end
                    .is_valid()
                    .then(|| Box::new(self.to_tree(range.end))),
                end_inclusive: range.end_inclusive,
            })),
            ExpressionNode::StructLiteral(struct_literal) => {
                Expression::StructLiteral(StructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    case_name: struct_literal.case_name.clone(),
                    fields: self
                        .struct_fields(struct_literal.fields)
                        .iter()
                        .map(|field| StructLiteralField {
                            name: field.name.clone(),
                            value: self.to_tree(field.value),
                        })
                        .collect::<Arc<[_]>>(),
                })
            }
            ExpressionNode::String(value) => Expression::String(value.clone()),
            ExpressionNode::Unary(unary) => Expression::Unary(Box::new(UnaryExpression {
                operator: unary.operator,
                operand: self.to_tree(unary.operand),
            })),
            ExpressionNode::ZeroValue(type_reference) => Expression::ZeroValue(*type_reference),
        }
    }

    pub fn to_tree_with_place_suffix(
        &self,
        expression: ExpressionHandle,
        suffix: &[Identifier],
    ) -> Expression {
        if suffix.is_empty() {
            return self.to_tree(expression);
        }

        match self.expression(expression) {
            ExpressionNode::Name(path) => {
                let mut resolved_path = self.name_path_to_tree(path);
                resolved_path.extend_from_slice(suffix);
                Expression::Name(resolved_path)
            }
            ExpressionNode::Indexed(indexed) => {
                if let Some(mut indexed_path) = self.indexed_expression_path(indexed) {
                    indexed_path.extend_from_slice(suffix);
                    Expression::Name(indexed_path)
                } else {
                    suffix
                        .iter()
                        .cloned()
                        .fold(self.to_tree(expression), |receiver, member| {
                            Expression::Member(Box::new(MemberExpression {
                                receiver,
                                member_symbol: SymbolHandle::invalid(),
                                member,
                                case_variant: None,
                            }))
                        })
                }
            }
            ExpressionNode::Borrow(target) => Expression::Borrow(Box::new(BorrowExpression {
                target: self.to_tree_with_place_suffix(target.target, suffix),
                access: target.access,
            })),
            // Same rule as the Indexed non-path arm above: NEVER drop the
            // suffix (the old catch-all returned the bare tree, so the caller
            // read the RECEIVER's place instead of the member's).
            _ => suffix
                .iter()
                .cloned()
                .fold(self.to_tree(expression), |receiver, member| {
                    Expression::Member(Box::new(MemberExpression {
                        receiver,
                        member_symbol: SymbolHandle::invalid(),
                        member,
                        case_variant: None,
                    }))
                }),
        }
    }

    fn name_path_to_tree(&self, path: &TableNamePath) -> NamePath {
        NamePath::resolved_with_member_symbols(
            self.name_path_members(path.members).to_vec(),
            self.name_path_member_symbols(path.member_symbols).to_vec(),
            path.head_symbol,
            path.symbol,
        )
    }

    fn indexed_expression_path(&self, indexed: &TableIndexedExpression) -> Option<NamePath> {
        let index = self.integer_node(indexed.index)?;
        let mut path = match self.expression(indexed.collection) {
            ExpressionNode::Name(path) => self.name_path_to_tree(path),
            ExpressionNode::Indexed(inner_indexed) => {
                self.indexed_expression_path(inner_indexed)?
            }
            _ => return None,
        };
        let last_segment = path.last()?.clone();
        path.replace_last_preserving_symbol(Identifier::generated(format!(
            "{last_segment}[{index}]"
        )))?;
        Some(path)
    }
}
