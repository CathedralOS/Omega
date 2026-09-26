//! Copying expressions, handles, name paths and struct literal fields between
//! expression tables, including member-suffixed copies.

use crate::symbol_resolved_trees::name::DiagnosticName;
use crate::symbol_resolved_trees::symbol_resolved_trees::values::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern, TableAtomicExpression,
    TableBinaryExpression, TableBorrowExpression, TableCallExpression, TableCastExpression,
    TableIndexedExpression, TableMatchArm, TableMatchExpression, TableMemberExpression,
    TableMembershipExpression, TableNamePath, TableRangeExpression, TableStructLiteral,
    TableStructLiteralField, TableUnaryExpression,
};
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;

impl ExpressionTable {
    pub fn copy_from(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        let source_span = source.source_span(expression);
        let copied = match source.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                let subject = self.copy_from(source, dispatch.subject);
                let source_arms = source.match_arms(dispatch.arms).to_vec();
                let mut arms = Vec::with_capacity(source_arms.len());
                for arm in source_arms {
                    let pattern = match arm.pattern {
                        MatchPattern::Value(value) => {
                            MatchPattern::Value(self.copy_from(source, value))
                        }
                        MatchPattern::Wildcard => MatchPattern::Wildcard,
                    };
                    let value = self.copy_from(source, arm.value);
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
            ExpressionNode::ArrayLiteral(values) => {
                let copied_values = self.reserve_expression_handles(values.count());

                for offset in 0..values.count() {
                    let value = source.expression_handle_at_offset(*values, offset);
                    let value = self.copy_from(source, *value);
                    self.set_expression_handle_at_offset(copied_values, offset, value);
                }

                self.insert(ExpressionNode::ArrayLiteral(copied_values))
            }
            ExpressionNode::Atomic(atomic) => {
                let value = self.copy_from(source, atomic.value);
                let result = if atomic.result.is_valid() {
                    self.copy_from(source, atomic.result)
                } else {
                    ExpressionHandle::invalid()
                };
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }))
            }
            ExpressionNode::Binary(binary) => {
                let left = self.copy_from(source, binary.left);
                let right = self.copy_from(source, binary.right);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(*value)),
            ExpressionNode::Cast(cast) => {
                let value = self.copy_from(source, cast.value);
                let target_type = cast.target_type;
                let target_label = self.copy_name_path_members(source, cast.target_label);
                let semantic_domain = self.copy_name_path_members(source, cast.semantic_domain);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                    target_label,
                    domain: cast.domain,
                    semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    form: cast.form,
                }))
            }
            ExpressionNode::Call(call) => {
                let receiver = if call.receiver.is_valid() {
                    self.copy_from(source, call.receiver)
                } else {
                    ExpressionHandle::invalid()
                };
                let arguments = self.copy_expression_handles_from(source, call.arguments);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    machine_arguments: call.machine_arguments.clone(),
                    arguments,
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
                }))
            }
            ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value.clone())),
            ExpressionNode::Indexed(indexed) => {
                let collection = self.copy_from(source, indexed.collection);
                let index = self.copy_from(source, indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value.clone())),
            ExpressionNode::Membership(membership) => {
                let value = self.copy_from(source, membership.value);
                let domain = self.copy_name_path_members(source, membership.domain);
                self.insert(ExpressionNode::Membership(TableMembershipExpression {
                    value,
                    domain,
                    domain_symbol: membership.domain_symbol,
                    case_type_symbol: membership.case_type_symbol,
                    case_symbol: membership.case_symbol,
                }))
            }
            ExpressionNode::Member(member) => {
                let receiver = self.copy_from(source, member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                }))
            }
            ExpressionNode::Borrow(inner_expression) => {
                let target = self.copy_from(source, inner_expression.target);
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: inner_expression.access,
                }))
            }
            ExpressionNode::Name(path) => {
                let members = self.copy_name_path_members(source, path.members);
                let member_symbols =
                    self.copy_name_path_member_symbols(source, path.member_symbols);
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    is_self_value: path.is_self_value,
                    head_symbol: path.head_symbol,
                    symbol: path.symbol,
                }))
            }
            ExpressionNode::Range(range) => {
                let start = if range.start.is_valid() {
                    self.copy_from(source, range.start)
                } else {
                    ExpressionHandle::invalid()
                };
                let end = if range.end.is_valid() {
                    self.copy_from(source, range.end)
                } else {
                    ExpressionHandle::invalid()
                };
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields = self.copy_struct_literal_fields(source, struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name.clone(),
                    type_symbol: struct_literal.type_symbol,
                    case_name: struct_literal.case_name.clone(),
                    case_symbol: struct_literal.case_symbol,
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value.clone())),
            ExpressionNode::Unary(unary) => {
                let operand = self.copy_from(source, unary.operand);
                self.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }
            ExpressionNode::ZeroValue(type_reference) => {
                self.insert(ExpressionNode::ZeroValue(*type_reference))
            }
        };
        self.set_source_span(copied, source_span);
        if let Some(exposure) = source.authored_expression_exposure(expression) {
            self.set_authored_expression_exposure(copied, exposure);
        }
        if let Some(partition) = source.compiler_selection_partition(expression) {
            self.set_compiler_selection_partition(copied, partition);
        }
        self.attach_authored_selection_occurrences(
            copied,
            source.authored_selection_occurrences(expression),
        );
        copied
    }

    pub fn copy_expression_handles_from(
        &mut self,
        source: &ExpressionTable,
        expressions: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.copy_expression_handles_from_slice(source, source.expression_handles(expressions))
    }

    pub fn copy_expression_handles_from_slice(
        &mut self,
        source: &ExpressionTable,
        expressions: &[ExpressionHandle],
    ) -> HandleSpan<ExpressionHandle> {
        let span = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression = self.copy_from(source, *expression);
            self.set_expression_handle_at_offset(
                span,
                offset
                    .try_into()
                    .expect("expression handle span count overflow"),
                expression,
            );
        }

        span
    }

    fn copy_name_path_members(
        &mut self,
        source: &ExpressionTable,
        members: HandleSpan<DiagnosticName>,
    ) -> HandleSpan<DiagnosticName> {
        self.spans
            .name_path_members
            .insert_many(source.name_path_members(members).iter().cloned())
    }

    fn copy_name_path_member_symbols(
        &mut self,
        source: &ExpressionTable,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        self.spans.name_path_member_symbols.insert_many(
            source
                .name_path_member_symbols(member_symbols)
                .iter()
                .copied(),
        )
    }

    fn copy_struct_literal_fields(
        &mut self,
        source: &ExpressionTable,
        fields: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let span = self.reserve_struct_fields(fields.count());

        for offset in 0..fields.count() {
            let field = source.struct_field_at_offset(fields, offset);
            let value = self.copy_from(source, field.value);
            self.set_struct_field_at_offset(
                span,
                offset,
                TableStructLiteralField {
                    name: field.name.clone(),
                    field_symbol: field.field_symbol,
                    value,
                },
            );
        }

        span
    }

    pub fn copy_name_path_members_with_member_suffix(
        &mut self,
        members: HandleSpan<DiagnosticName>,
        suffix_members: HandleSpan<DiagnosticName>,
        suffix_start_offset: u32,
    ) -> HandleSpan<DiagnosticName> {
        let suffix_count = suffix_members.count().saturating_sub(suffix_start_offset);
        let span = self.reserve_name_path_members(
            members
                .count()
                .checked_add(suffix_count)
                .expect("name path member span count overflow"),
        );

        for offset in 0..members.count() {
            let member = self
                .spans
                .name_path_members
                .get(Handle::from_parts(
                    members
                        .start()
                        .arena_index()
                        .checked_add(offset)
                        .expect("name path member index overflow"),
                    members.start().generation(),
                ))
                .clone();
            self.set_name_path_member_at_offset(span, offset, member);
        }

        for (target_offset, offset) in (suffix_start_offset..suffix_members.count()).enumerate() {
            let member = self
                .name_path_member_at_offset(suffix_members, offset)
                .clone();
            self.set_name_path_member_at_offset(
                span,
                members
                    .count()
                    .checked_add(
                        target_offset
                            .try_into()
                            .expect("name path member span count overflow"),
                    )
                    .expect("name path member span count overflow"),
                member,
            );
        }

        span
    }

    pub fn insert_copy_with_member_suffix(
        &mut self,
        expression: ExpressionHandle,
        suffix_members: HandleSpan<DiagnosticName>,
        suffix_start_offset: u32,
    ) -> ExpressionHandle {
        if suffix_start_offset >= suffix_members.count() {
            return expression;
        }

        let occurrences = self
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        let authored_exposure = self.authored_expression_exposure(expression);
        let compiler_partition = self.compiler_selection_partition(expression);
        let copied = match self.expression(expression).clone() {
            ExpressionNode::Name(path) => {
                let members = self.copy_name_path_members_with_member_suffix(
                    path.members,
                    suffix_members,
                    suffix_start_offset,
                );
                let member_symbols = self.reserve_name_path_member_symbols(members.count());
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    is_self_value: path.is_self_value,
                    head_symbol: path.head_symbol,
                    symbol: SymbolHandle::invalid(),
                }))
            }
            ExpressionNode::Borrow(target) => {
                let access = target.access;
                let target = self.insert_copy_with_member_suffix(
                    target.target,
                    suffix_members,
                    suffix_start_offset,
                );
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access,
                }))
            }
            ExpressionNode::Indexed(indexed) => self
                .insert_indexed_expression_path_with_member_suffix(
                    &indexed,
                    suffix_members,
                    suffix_start_offset,
                )
                .unwrap_or_else(|| self.copy_from_self(expression)),
            _ => self.copy_from_self(expression),
        };
        if let Some(exposure) = authored_exposure {
            self.set_authored_expression_exposure(copied, exposure);
        }
        if let Some(partition) = compiler_partition {
            self.set_compiler_selection_partition(copied, partition);
        }
        self.attach_authored_selection_occurrences(copied, occurrences);
        copied
    }

    pub fn copy_from_self(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
        // A same-table rewrite may retain an authored expression before its
        // declaration-selection occurrences are minted. Keep the exact source
        // coordinate so finalization can reunite every retained copy.
        let source_span = self.source_span(expression);
        let occurrences = self
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        let authored_exposure = self.authored_expression_exposure(expression);
        let compiler_partition = self.compiler_selection_partition(expression);
        let copied = match self.expression(expression).clone() {
            ExpressionNode::Match(dispatch) => {
                let subject = self.copy_from_self(dispatch.subject);
                let source_arms = self.match_arms(dispatch.arms).to_vec();
                let mut arms = Vec::with_capacity(source_arms.len());
                for arm in source_arms {
                    let pattern = match arm.pattern {
                        MatchPattern::Value(value) => {
                            MatchPattern::Value(self.copy_from_self(value))
                        }
                        MatchPattern::Wildcard => MatchPattern::Wildcard,
                    };
                    let value = self.copy_from_self(arm.value);
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
            ExpressionNode::ArrayLiteral(values) => {
                let copied_values = self.reserve_expression_handles(values.count());

                for offset in 0..values.count() {
                    let value = *self.expression_handle_at_offset(values, offset);
                    let value = self.copy_from_self(value);
                    self.set_expression_handle_at_offset(copied_values, offset, value);
                }

                self.insert(ExpressionNode::ArrayLiteral(copied_values))
            }
            ExpressionNode::Atomic(atomic) => {
                let value = self.copy_from_self(atomic.value);
                let result = if atomic.result.is_valid() {
                    self.copy_from_self(atomic.result)
                } else {
                    ExpressionHandle::invalid()
                };
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }))
            }
            ExpressionNode::Binary(binary) => {
                let left = self.copy_from_self(binary.left);
                let right = self.copy_from_self(binary.right);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(value)),
            ExpressionNode::Cast(cast) => {
                let value = self.copy_from_self(cast.value);
                let target_type = cast.target_type;
                let target_label = self.copy_name_path_members_from_self(cast.target_label);
                let semantic_domain = self.copy_name_path_members_from_self(cast.semantic_domain);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type,
                    target_label,
                    domain: cast.domain,
                    semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    form: cast.form,
                }))
            }
            ExpressionNode::Call(call) => {
                let receiver = if call.receiver.is_valid() {
                    self.copy_from_self(call.receiver)
                } else {
                    ExpressionHandle::invalid()
                };
                let arguments = self.copy_expression_handles_from_self(call.arguments);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    machine_arguments: call.machine_arguments,
                    arguments,
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                }))
            }
            ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value)),
            ExpressionNode::Indexed(indexed) => {
                let collection = self.copy_from_self(indexed.collection);
                let index = self.copy_from_self(indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value)),
            ExpressionNode::Membership(membership) => {
                let value = self.copy_from_self(membership.value);
                let domain = self.copy_name_path_members_from_self(membership.domain);
                self.insert(ExpressionNode::Membership(TableMembershipExpression {
                    value,
                    domain,
                    domain_symbol: membership.domain_symbol,
                    case_type_symbol: membership.case_type_symbol,
                    case_symbol: membership.case_symbol,
                }))
            }
            ExpressionNode::Member(member) => {
                let receiver = self.copy_from_self(member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                }))
            }
            ExpressionNode::Borrow(inner_expression) => {
                let target = self.copy_from_self(inner_expression.target);
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: inner_expression.access,
                }))
            }
            ExpressionNode::Name(path) => {
                let members = self.copy_name_path_members_from_self(path.members);
                let member_symbols =
                    self.copy_name_path_member_symbols_from_self(path.member_symbols);
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    ..path
                }))
            }
            ExpressionNode::Range(range) => {
                let start = if range.start.is_valid() {
                    self.copy_from_self(range.start)
                } else {
                    ExpressionHandle::invalid()
                };
                let end = if range.end.is_valid() {
                    self.copy_from_self(range.end)
                } else {
                    ExpressionHandle::invalid()
                };
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                let fields = self.copy_struct_literal_fields_from_self(struct_literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: struct_literal.type_name,
                    type_symbol: struct_literal.type_symbol,
                    case_name: struct_literal.case_name,
                    case_symbol: struct_literal.case_symbol,
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value)),
            ExpressionNode::Unary(unary) => {
                let operand = self.copy_from_self(unary.operand);
                self.insert(ExpressionNode::Unary(TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                }))
            }
            ExpressionNode::ZeroValue(type_reference) => {
                self.insert(ExpressionNode::ZeroValue(type_reference))
            }
        };
        self.set_source_span(copied, source_span);
        if let Some(exposure) = authored_exposure {
            self.set_authored_expression_exposure(copied, exposure);
        }
        if let Some(partition) = compiler_partition {
            self.set_compiler_selection_partition(copied, partition);
        }
        self.attach_authored_selection_occurrences(copied, occurrences);
        copied
    }

    fn copy_expression_handles_from_self(
        &mut self,
        expressions: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        let copied = self.reserve_expression_handles(expressions.count());

        for offset in 0..expressions.count() {
            let expression = *self.expression_handle_at_offset(expressions, offset);
            let expression = self.copy_from_self(expression);
            self.set_expression_handle_at_offset(copied, offset, expression);
        }

        copied
    }

    fn copy_name_path_members_from_self(
        &mut self,
        members: HandleSpan<DiagnosticName>,
    ) -> HandleSpan<DiagnosticName> {
        let copied = self.reserve_name_path_members(members.count());

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset).clone();
            self.set_name_path_member_at_offset(copied, offset, member);
        }

        copied
    }

    fn copy_name_path_member_symbols_from_self(
        &mut self,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        let copied = self.reserve_name_path_member_symbols(member_symbols.count());
        for offset in 0..member_symbols.count() {
            let symbol = self.name_path_member_symbols(member_symbols)[offset as usize];
            self.set_name_path_member_symbol_at_offset(copied, offset, symbol);
        }
        copied
    }

    fn copy_struct_literal_fields_from_self(
        &mut self,
        fields: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let copied = self.reserve_struct_fields(fields.count());

        for offset in 0..fields.count() {
            let field = self.struct_field_at_offset(fields, offset).clone();
            let value = self.copy_from_self(field.value);
            self.set_struct_field_at_offset(
                copied,
                offset,
                TableStructLiteralField {
                    name: field.name,
                    field_symbol: field.field_symbol,
                    value,
                },
            );
        }

        copied
    }

    fn insert_indexed_expression_path_with_member_suffix(
        &mut self,
        indexed: &TableIndexedExpression,
        suffix_members: HandleSpan<DiagnosticName>,
        suffix_start_offset: u32,
    ) -> Option<ExpressionHandle> {
        let path_len = self.storage_path_len(indexed.collection)?;
        let suffix_count = suffix_members.count().saturating_sub(suffix_start_offset);
        let members = self.reserve_name_path_members(
            path_len
                .checked_add(suffix_count)
                .expect("name path member span count overflow"),
        );
        self.fill_storage_path_members(indexed.collection, members, 0)?;

        for (target_offset, offset) in (suffix_start_offset..suffix_members.count()).enumerate() {
            let member = self
                .name_path_member_at_offset(suffix_members, offset)
                .clone();
            self.set_name_path_member_at_offset(
                members,
                path_len
                    .checked_add(
                        target_offset
                            .try_into()
                            .expect("name path member span count overflow"),
                    )
                    .expect("name path member span count overflow"),
                member,
            );
        }

        let member_symbols = self.reserve_name_path_member_symbols(members.count());
        Some(self.insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            is_self_value: false,
            head_symbol: self.storage_path_head_symbol(indexed.collection),
            symbol: SymbolHandle::invalid(),
        })))
    }
}
