//! Copying expressions, name paths and struct literal fields between expression
//! tables.
//!
//! Every subtree copy runs through [`ExpressionTable::copy_subtree`]; the
//! public entry points differ only in the [`ExpressionSource`] they read. The walk
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
use arena::{Handle, HandleSpan};

#[derive(Default)]
struct CopyGraph {
    expressions: Vec<(ExpressionHandle, ExpressionHandle)>,
    subjects: Vec<(
        Handle<super::MatchSubjectCapture>,
        Handle<super::MatchSubjectCapture>,
    )>,
}
use symbols::SymbolHandle;

/// Where a subtree copy reads the rows it appends.
#[derive(Clone, Copy)]
pub(crate) enum ExpressionSource<'source> {
    /// The destination table itself: the copy lands beside its original.
    Own,
    /// A separate table, which the copy leaves untouched.
    Table(&'source ExpressionTable),
}

impl<'source> ExpressionSource<'source> {
    /// The table to read. The result borrows `destination` even for a
    /// separate table, so every read ends before the walk appends again.
    pub(crate) fn table<'table>(
        self,
        destination: &'table ExpressionTable,
    ) -> &'table ExpressionTable
    where
        'source: 'table,
    {
        match self {
            Self::Own => destination,
            Self::Table(source) => source,
        }
    }

    pub(crate) fn copy_into(
        self,
        destination: &mut ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        destination.copy_subtree(self, expression)
    }
}

impl ExpressionTable {
    pub fn copy_from(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        self.copy_subtree(ExpressionSource::Table(source), expression)
    }

    /// Copy one expression graph within this table. The copy's rows follow
    /// every existing row and share no span with the original.
    pub fn copy_from_self(&mut self, expression: ExpressionHandle) -> ExpressionHandle {
        self.copy_subtree(ExpressionSource::Own, expression)
    }

    /// The one subtree copy behind `copy_from` and `copy_from_self`. Each node
    /// and span is cloned out of `source` before anything is appended, which
    /// lets a copy read the table it writes. The copy keeps the node's source
    /// span, authored exposure, compiler selection partition and
    /// authored-selection occurrences.
    fn copy_subtree(
        &mut self,
        source: ExpressionSource<'_>,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        self.copy_graph(source, expression, &mut CopyGraph::default())
    }

    fn copy_graph(
        &mut self,
        source: ExpressionSource<'_>,
        expression: ExpressionHandle,
        copies: &mut CopyGraph,
    ) -> ExpressionHandle {
        if !expression.is_valid() {
            return expression;
        }
        let position = expression.arena_index() as usize;
        if let Some((original, copied)) = copies.expressions.get(position)
            && *original == expression
        {
            return *copied;
        }
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
                let subject = self.copy_graph(source, dispatch.subject, copies);
                let capture = self.copy_match_subject(source, dispatch.capture, copies);
                let source_arms = source.table(self).match_arms(dispatch.arms).to_vec();
                let mut arms = Vec::with_capacity(source_arms.len());
                for arm in source_arms {
                    let pattern = match arm.pattern {
                        MatchPattern::Value(value) => {
                            MatchPattern::Value(self.copy_graph(source, value, copies))
                        }
                        MatchPattern::Wildcard => MatchPattern::Wildcard,
                        MatchPattern::Destructure(pattern) => MatchPattern::Destructure(
                            self.copy_destructure_pattern(source, pattern, copies),
                        ),
                    };
                    let value = self.copy_graph(source, arm.value, copies);
                    arms.push(TableMatchArm {
                        pattern,
                        value,
                        source_span: arm.source_span,
                    });
                }
                let arms = self.insert_match_arms(arms);
                self.insert(ExpressionNode::Match(TableMatchExpression {
                    subject,
                    capture,
                    arms,
                }))
            }
            ExpressionNode::SavedSubject(saved) => {
                let copied_capture = copies
                    .subjects
                    .iter()
                    .find_map(|(original, copied)| (*original == saved.capture).then_some(*copied));
                if copied_capture.is_none() && matches!(source, ExpressionSource::Own) {
                    self.insert(ExpressionNode::SavedSubject(saved))
                } else {
                    let capture = self.copy_match_subject(source, saved.capture, copies);
                    let pattern_head = self.copy_graph(source, saved.pattern_head, copies);
                    self.insert(ExpressionNode::SavedSubject(super::TableSavedSubject {
                        capture,
                        pattern_head,
                    }))
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                let values = self.copy_expression_span(source, values, copies);
                self.insert(ExpressionNode::ArrayLiteral(values))
            }
            ExpressionNode::Atomic(atomic) => {
                let value = self.copy_graph(source, atomic.value, copies);
                let result = self.copy_optional_subtree(source, atomic.result, copies);
                self.insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: atomic.ordering,
                    result_custody: atomic.result_custody,
                }))
            }
            ExpressionNode::Binary(binary) => {
                let left = self.copy_graph(source, binary.left, copies);
                let right = self.copy_graph(source, binary.right, copies);
                self.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                }))
            }
            ExpressionNode::Boolean(value) => self.insert(ExpressionNode::Boolean(value)),
            ExpressionNode::Cast(cast) => {
                let value = self.copy_graph(source, cast.value, copies);
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
                let receiver = self.copy_optional_subtree(source, call.receiver, copies);
                let arguments = self.copy_expression_span(source, call.arguments, copies);
                self.insert(ExpressionNode::Call(TableCallExpression {
                    authored_call_binding: call.authored_call_binding,
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
                let collection = self.copy_graph(source, indexed.collection, copies);
                let index = self.copy_graph(source, indexed.index, copies);
                self.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection,
                    index,
                }))
            }
            ExpressionNode::Integer(value) => self.insert(ExpressionNode::Integer(value)),
            ExpressionNode::Membership(membership) => {
                let value = self.copy_graph(source, membership.value, copies);
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
                let receiver = self.copy_graph(source, member.receiver, copies);
                self.insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                }))
            }
            ExpressionNode::Borrow(borrow) => {
                let target = self.copy_graph(source, borrow.target, copies);
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
                let start = self.copy_optional_subtree(source, range.start, copies);
                let end = self.copy_optional_subtree(source, range.end, copies);
                self.insert(ExpressionNode::Range(TableRangeExpression {
                    start,
                    end,
                    end_inclusive: range.end_inclusive,
                }))
            }
            ExpressionNode::StructLiteral(literal) => {
                let fields = self.copy_struct_literal_fields(source, literal.fields, copies);
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
                let operand = self.copy_graph(source, unary.operand, copies);
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
        if copies.expressions.len() <= position {
            copies
                .expressions
                .resize(position + 1, (Handle::invalid(), Handle::invalid()));
        }
        copies.expressions[position] = (expression, copied);
        copied
    }

    /// Copies an optional child; the invalid handle marks an absent one and
    /// stays invalid.
    fn copy_optional_subtree(
        &mut self,
        source: ExpressionSource<'_>,
        expression: ExpressionHandle,
        copies: &mut CopyGraph,
    ) -> ExpressionHandle {
        if expression.is_valid() {
            self.copy_graph(source, expression, copies)
        } else {
            ExpressionHandle::invalid()
        }
    }

    fn copy_expression_span(
        &mut self,
        source: ExpressionSource<'_>,
        expressions: HandleSpan<ExpressionHandle>,
        copies: &mut CopyGraph,
    ) -> HandleSpan<ExpressionHandle> {
        let expressions = source.table(self).expression_handles(expressions).to_vec();
        self.copy_expression_list(source, &expressions, copies)
    }

    /// Reserves the copied span before copying its elements, so the span's
    /// rows precede every row its elements append.
    fn copy_expression_list(
        &mut self,
        source: ExpressionSource<'_>,
        expressions: &[ExpressionHandle],
        copies: &mut CopyGraph,
    ) -> HandleSpan<ExpressionHandle> {
        let span = self.reserve_expression_handles(
            expressions
                .len()
                .try_into()
                .expect("expression handle span count overflow"),
        );

        for (offset, expression) in expressions.iter().enumerate() {
            let expression = self.copy_graph(source, *expression, copies);
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

    fn copy_match_subject(
        &mut self,
        source: ExpressionSource<'_>,
        capture: Handle<super::MatchSubjectCapture>,
        copies: &mut CopyGraph,
    ) -> Handle<super::MatchSubjectCapture> {
        if !capture.is_valid() {
            return capture;
        }
        let position = capture.arena_index() as usize;
        if let Some((original, copied)) = copies.subjects.get(position)
            && *original == capture
        {
            return *copied;
        }
        let subject = source.table(self).match_subject(capture).subject;
        let subject = self.copy_graph(source, subject, copies);
        let copied = self.insert_match_subject(subject);
        if copies.subjects.len() <= position {
            copies
                .subjects
                .resize(position + 1, (Handle::invalid(), Handle::invalid()));
        }
        copies.subjects[position] = (capture, copied);
        copied
    }

    fn copy_destructure_pattern(
        &mut self,
        source: ExpressionSource<'_>,
        pattern: super::TableDestructurePattern,
        copies: &mut CopyGraph,
    ) -> super::TableDestructurePattern {
        let head = self.copy_graph(source, pattern.head, copies);
        let mut fields = source.table(self).match_fields(pattern.fields).to_vec();
        for field in &mut fields {
            if let super::MatchFieldAction::Equal(value) = &mut field.action {
                *value = self.copy_graph(source, *value, copies);
            }
        }
        let fields = self.insert_match_fields(fields);
        let guard = self.copy_graph(source, pattern.guard, copies);
        let condition = self.copy_graph(source, pattern.condition, copies);
        let selectors = source
            .table(self)
            .match_proof_selectors(pattern.proof_selectors)
            .to_vec();
        let proof_selectors = self.insert_match_proof_selectors(selectors);
        super::TableDestructurePattern {
            head,
            fields,
            has_rest: pattern.has_rest,
            guard,
            condition,
            proof_selectors,
        }
    }

    fn copy_name_path_members(
        &mut self,
        source: ExpressionSource<'_>,
        members: HandleSpan<DiagnosticName>,
    ) -> HandleSpan<DiagnosticName> {
        let members = source.table(self).name_path_members(members).to_vec();
        self.spans.name_path_members.insert_many(members)
    }

    fn copy_name_path_member_symbols(
        &mut self,
        source: ExpressionSource<'_>,
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
        source: ExpressionSource<'_>,
        fields: HandleSpan<TableStructLiteralField>,
        copies: &mut CopyGraph,
    ) -> HandleSpan<TableStructLiteralField> {
        let fields = source.table(self).struct_fields(fields).to_vec();
        let span = self.reserve_struct_fields(
            fields
                .len()
                .try_into()
                .expect("struct literal field span count overflow"),
        );

        for (offset, field) in fields.into_iter().enumerate() {
            let value = self.copy_graph(source, field.value, copies);
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
}
