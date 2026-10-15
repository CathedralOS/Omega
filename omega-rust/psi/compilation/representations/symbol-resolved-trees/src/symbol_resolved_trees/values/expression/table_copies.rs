//! Copying expressions, handles, name paths and struct literal fields between
//! expression tables, including member-suffixed copies.
//!
//! Every subtree copy runs through [`ExpressionTable::copy_subtree`]; the
//! public entry points differ only in the [`CopySource`] they read. The walk
//! appends children before their parent and reserves or inserts each span in
//! walk order, so a copy appends the same rows whichever table it reads.

use crate::name::DiagnosticName;
use crate::symbol_resolved_trees::values::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, MatchPattern, TableAtomicExpression,
    TableBinaryExpression, TableBorrowExpression, TableCallExpression, TableCastExpression,
    TableIndexedExpression, TableMatchArm, TableMatchExpression, TableMemberExpression,
    TableMembershipExpression, TableNamePath, TableRangeExpression, TableStructLiteral,
    TableStructLiteralField, TableUnaryExpression,
};
use arena::HandleSpan;
use symbols::SymbolHandle;

/// Where a subtree copy reads the rows it appends.
#[derive(Clone, Copy)]
enum CopySource<'source> {
    /// The destination table itself: the copy lands beside its original.
    Own,
    /// A separate table, which the copy leaves untouched.
    Table(&'source ExpressionTable),
}

impl<'source> CopySource<'source> {
    /// The table to read. The result borrows `destination` even for a
    /// separate table, so every read ends before the walk appends again.
    fn table<'table>(self, destination: &'table ExpressionTable) -> &'table ExpressionTable
    where
        'source: 'table,
    {
        match self {
            Self::Own => destination,
            Self::Table(source) => source,
        }
    }
}

impl ExpressionTable {
    pub fn copy_from(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        self.copy_subtree(CopySource::Table(source), expression)
    }

    /// Copy one expression graph within this table. The copy's rows follow
    /// every existing row and share no span with the original.
    pub fn copy_from_self(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
        self.copy_subtree(CopySource::Own, expression)
    }

    /// The one subtree copy behind `copy_from`, `copy_from_self` and the
    /// expression-handle copies. Each node and span is cloned out of `source`
    /// before anything is appended, which lets a copy read the table it
    /// writes. The copy keeps the node's source span, authored exposure,
    /// compiler selection partition and authored-selection occurrences.
    fn copy_subtree(
        &mut self,
        source: CopySource<'_>,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        let origin = source.table(self);
        // A same-table rewrite may retain an authored expression before its
        // declaration-selection occurrences are minted. Keep the exact source
        // coordinate so finalization can reunite every retained copy.
        let source_span = origin.source_span(expression);
        let occurrences = origin
            .authored_selection_occurrences(expression)
            .collect::<Vec<_>>();
        let authored_exposure = origin.authored_expression_exposure(expression);
        let compiler_partition = origin.compiler_selection_partition(expression);
        let copied = match origin.expression(expression).clone() {
            ExpressionNode::Match(dispatch) => {
                let subject = self.copy_subtree(source, dispatch.subject);
                let source_arms = source.table(self).match_arms(dispatch.arms).to_vec();
                let mut arms = Vec::with_capacity(source_arms.len());
                for arm in source_arms {
                    let pattern = match arm.pattern {
                        MatchPattern::Value(value) => {
                            MatchPattern::Value(self.copy_subtree(source, value))
                        }
                        MatchPattern::Wildcard => MatchPattern::Wildcard,
                    };
                    let value = self.copy_subtree(source, arm.value);
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
                let values = self.copy_expression_span(source, values);
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            ExpressionNode::Atomic(atomic) => {
                let value = self.copy_subtree(source, atomic.value);
                let result = self.copy_optional_subtree(source, atomic.result);
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }))
            }
            ExpressionNode::Binary(binary) => {
                let left = self.copy_subtree(source, binary.left);
                let right = self.copy_subtree(source, binary.right);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(value)),
            ExpressionNode::Cast(cast) => {
                let value = self.copy_subtree(source, cast.value);
                let target_label = self.copy_name_path_members(source, cast.target_label);
                let semantic_domain = self.copy_name_path_members(source, cast.semantic_domain);
                self.insert(ExpressionNode::Cast(TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label,
                    domain: cast.domain,
                    semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    form: cast.form,
                }))
            }
            ExpressionNode::Call(call) => {
                let receiver = self.copy_optional_subtree(source, call.receiver);
                let arguments = self.copy_expression_span(source, call.arguments);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target,
                    requirement_application: call.requirement_application,
                    machine_arguments: call.machine_arguments,
                    arguments,
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                }))
            }
            ExpressionNode::Float(value) => self.insert(ExpressionNode::Float(value)),
            ExpressionNode::UndeterminedNaN(format) => {
                self.insert(ExpressionNode::UndeterminedNaN(format))
            }
            ExpressionNode::Indexed(indexed) => {
                let collection = self.copy_subtree(source, indexed.collection);
                let index = self.copy_subtree(source, indexed.index);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value)),
            ExpressionNode::Membership(membership) => {
                let value = self.copy_subtree(source, membership.value);
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
                let receiver = self.copy_subtree(source, member.receiver);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                }))
            }
            ExpressionNode::Borrow(borrow) => {
                let target = self.copy_subtree(source, borrow.target);
                self.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target,
                    access: borrow.access,
                }))
            }
            ExpressionNode::Name(path) => {
                let members = self.copy_name_path_members(source, path.members);
                let member_symbols =
                    self.copy_name_path_member_symbols(source, path.member_symbols);
                self.insert(ExpressionNode::Name(TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: path.head_symbol,
                    symbol: path.symbol,
                }))
            }
            ExpressionNode::Range(range) => {
                let start = self.copy_optional_subtree(source, range.start);
                let end = self.copy_optional_subtree(source, range.end);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = self.copy_struct_literal_fields(source, literal.fields);
                self.insert(ExpressionNode::StructLiteral(TableStructLiteral {
                    type_name: literal.type_name,
                    type_symbol: literal.type_symbol,
                    case_name: literal.case_name,
                    case_symbol: literal.case_symbol,
                    fields,
                }))
            }
            ExpressionNode::String(value) => self.insert(ExpressionNode::String(value)),
            ExpressionNode::Unary(unary) => {
                let operand = self.copy_subtree(source, unary.operand);
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

    /// Copies an optional child; the invalid handle marks an absent one and
    /// stays invalid.
    fn copy_optional_subtree(
        &mut self,
        source: CopySource<'_>,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        if expression.is_valid() {
            self.copy_subtree(source, expression)
        } else {
            ExpressionHandle::invalid()
        }
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
        self.copy_expression_list(CopySource::Table(source), expressions)
    }

    fn copy_expression_span(
        &mut self,
        source: CopySource<'_>,
        expressions: HandleSpan<ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        let expressions = source.table(self).expression_handles(expressions).to_vec();
        self.copy_expression_list(source, &expressions)
    }

    /// Reserves the copied span before copying its elements, so the span's
    /// rows precede every row its elements append.
    fn copy_expression_list(
        &mut self,
        source: CopySource<'_>,
        expressions: &[ExpressionHandle],
    ) -> HandleSpan<ExpressionHandle> {
        let span = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression = self.copy_subtree(source, *expression);
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
        source: CopySource<'_>,
        members: HandleSpan<DiagnosticName>,
    ) -> HandleSpan<DiagnosticName> {
        let members = source.table(self).name_path_members(members).to_vec();
        self.spans.name_path_members.insert_many(members)
    }

    fn copy_name_path_member_symbols(
        &mut self,
        source: CopySource<'_>,
        member_symbols: HandleSpan<SymbolHandle>,
    ) -> HandleSpan<SymbolHandle> {
        let member_symbols = source
            .table(self)
            .name_path_member_symbols(member_symbols)
            .to_vec();
        self.spans
            .name_path_member_symbols
            .insert_many(member_symbols)
    }

    /// Reserves the fields' span ahead of their values' rows.
    fn copy_struct_literal_fields(
        &mut self,
        source: CopySource<'_>,
        fields: HandleSpan<TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        let fields = source.table(self).struct_fields(fields).to_vec();
        let span = self.reserve_struct_fields(
            fields
                .len()
                .try_into()
                .expect("struct literal field span count overflow"),
        );

        for (offset, field) in fields.into_iter().enumerate() {
            let value = self.copy_subtree(source, field.value);
            self.set_struct_field_at_offset(
                span,
                offset
                    .try_into()
                    .expect("struct literal field span count overflow"),
                TableStructLiteralField {
                    name: field.name,
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
                .get(members.handle_at(offset))
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
            head_symbol: self.storage_path_head_symbol(indexed.collection),
            symbol: SymbolHandle::invalid(),
        })))
    }
}
