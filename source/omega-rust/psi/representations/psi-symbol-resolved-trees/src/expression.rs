use crate::AuthoredDeclarationSelectionOccurrenceId;
use crate::name::DiagnosticName;
use psi_arena::{Arena, Handle, HandleSpan};
use psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
use psi_numerics::literals::IntegerLiteral;
use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

mod display;
#[cfg(test)]
mod tests;

pub use display::display_name_path;

pub type ExpressionHandle = Handle<ExpressionNode>;

/// Arena dummy storage for an occurrence identity. `None` exists only to
/// satisfy the arena's private dummy slot; expression spans contain only
/// `Some` values inserted through `new`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct StoredAuthoredSelectionOccurrenceId(Option<AuthoredDeclarationSelectionOccurrenceId>);

impl StoredAuthoredSelectionOccurrenceId {
    fn new(occurrence: AuthoredDeclarationSelectionOccurrenceId) -> Self {
        Self(Some(occurrence))
    }

    fn occurrence(self) -> AuthoredDeclarationSelectionOccurrenceId {
        self.0
            .expect("expression occurrence spans cannot contain the arena dummy sentinel")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionTable {
    nodes: ExpressionNodeStorage,
    spans: ExpressionSpanStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpressionNodeStorage {
    expressions: Arena<ExpressionNode>,
    source_spans: Vec<SourceSpan>,
    authored_expression_exposures: Vec<Option<AuthoredDeclarationSelectionExposure>>,
    authored_selection_occurrences: Vec<HandleSpan<StoredAuthoredSelectionOccurrenceId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpressionSpanStorage {
    expression_handles: Arena<ExpressionHandle>,
    name_path_members: Arena<DiagnosticName>,
    name_path_member_symbols: Arena<SymbolHandle>,
    struct_fields: Arena<TableStructLiteralField>,
    authored_selection_occurrence_ids: Arena<StoredAuthoredSelectionOccurrenceId>,
}

impl ExpressionTable {
    pub fn new() -> Self {
        Self {
            nodes: ExpressionNodeStorage {
                expressions: Arena::new(),
                source_spans: Vec::new(),
                authored_expression_exposures: Vec::new(),
                authored_selection_occurrences: Vec::new(),
            },
            spans: ExpressionSpanStorage {
                expression_handles: Arena::new(),
                name_path_members: Arena::new(),
                name_path_member_symbols: Arena::new(),
                struct_fields: Arena::new(),
                authored_selection_occurrence_ids: Arena::new(),
            },
        }
    }

    pub fn clear(&mut self) {
        self.nodes.expressions.reset_retain_capacity();
        self.nodes.source_spans.clear();
        self.nodes.authored_expression_exposures.clear();
        self.nodes.authored_selection_occurrences.clear();
        self.spans.expression_handles.reset_retain_capacity();
        self.spans.name_path_members.reset_retain_capacity();
        self.spans.name_path_member_symbols.reset_retain_capacity();
        self.spans.struct_fields.reset_retain_capacity();
        self.spans
            .authored_selection_occurrence_ids
            .reset_retain_capacity();
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        let handle = self.nodes.expressions.insert(expression);
        self.nodes.source_spans.push(SourceSpan::default());
        self.nodes.authored_expression_exposures.push(None);
        self.nodes
            .authored_selection_occurrences
            .push(HandleSpan::empty());
        debug_assert_eq!(source_span_index(handle), self.nodes.source_spans.len() - 1);
        debug_assert_eq!(
            source_span_index(handle),
            self.nodes.authored_expression_exposures.len() - 1
        );
        debug_assert_eq!(
            source_span_index(handle),
            self.nodes.authored_selection_occurrences.len() - 1
        );
        handle
    }

    pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
        self.nodes.source_spans[source_span_index(handle)]
    }

    pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
        self.nodes.source_spans[source_span_index(handle)] = source_span;
    }

    /// Exact public/private source position in which this authored expression
    /// occurred. Compiler-generated expressions retain `None`.
    pub fn authored_expression_exposure(
        &self,
        handle: ExpressionHandle,
    ) -> Option<AuthoredDeclarationSelectionExposure> {
        self.nodes.authored_expression_exposures[source_span_index(handle)]
    }

    pub fn set_authored_expression_exposure(
        &mut self,
        handle: ExpressionHandle,
        exposure: AuthoredDeclarationSelectionExposure,
    ) {
        let slot = &mut self.nodes.authored_expression_exposures[source_span_index(handle)];
        if let Some(existing) = *slot {
            assert_eq!(
                existing, exposure,
                "one expression handle cannot represent two authored visibility positions"
            );
        }
        *slot = Some(exposure);
    }

    /// Attach exact authored-selection occurrence identities to an expression.
    ///
    /// Associations are arena-backed and keyed by the expression handle. The
    /// occurrence identity is semantic custody; source spans remain diagnostic
    /// metadata and are never used to reconstruct this association.
    pub fn attach_authored_selection_occurrences(
        &mut self,
        handle: ExpressionHandle,
        occurrences: impl IntoIterator<Item = AuthoredDeclarationSelectionOccurrenceId>,
    ) {
        let index = source_span_index(handle);
        let mut combined = self
            .authored_selection_occurrences(handle)
            .collect::<Vec<_>>();

        for occurrence in occurrences {
            if !combined.contains(&occurrence) {
                combined.push(occurrence);
            }
        }

        self.nodes.authored_selection_occurrences[index] =
            self.spans.authored_selection_occurrence_ids.insert_many(
                combined
                    .into_iter()
                    .map(StoredAuthoredSelectionOccurrenceId::new),
            );
    }

    pub fn authored_selection_occurrences(
        &self,
        handle: ExpressionHandle,
    ) -> impl ExactSizeIterator<Item = AuthoredDeclarationSelectionOccurrenceId> + '_ {
        self.spans
            .authored_selection_occurrence_ids
            .span_or_empty(self.nodes.authored_selection_occurrences[source_span_index(handle)])
            .iter()
            .copied()
            .map(StoredAuthoredSelectionOccurrenceId::occurrence)
    }

    pub(crate) fn rebase_authored_selection_extension(
        &mut self,
        expression_frontier: usize,
        rebase: psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionSuffixRebase,
    ) -> Result<(), ()> {
        if expression_frontier > self.expression_count() {
            return Err(());
        }
        let remapped = self
            .iter_expressions()
            .enumerate()
            .map(|(index, (handle, _))| {
                let occurrences = self
                    .authored_selection_occurrences(handle)
                    .map(|occurrence| {
                        if index < expression_frontier {
                            rebase.retain_base(occurrence)
                        } else {
                            rebase.rebase_appended(occurrence)
                        }
                        .ok_or(())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((index >= expression_frontier).then_some(occurrences))
            })
            .collect::<Result<Vec<_>, _>>()?;

        for (index, occurrences) in remapped.into_iter().enumerate() {
            let Some(occurrences) = occurrences else {
                continue;
            };
            self.nodes.authored_selection_occurrences[index] =
                self.spans.authored_selection_occurrence_ids.insert_many(
                    occurrences
                        .into_iter()
                        .map(StoredAuthoredSelectionOccurrenceId::new),
                );
        }
        Ok(())
    }

    pub fn insert_expression_handles(
        &mut self,
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.spans.expression_handles.insert_many(expressions)
    }

    pub fn reserve_expression_handles(&mut self, count: u32) -> HandleSpan<ExpressionHandle> {
        self.spans.expression_handles.insert_many(
            std::iter::repeat_with(ExpressionHandle::invalid)
                .take(usize::try_from(count).expect("expression handle span count overflow")),
        )
    }

    pub fn set_expression_handle_at_offset(
        &mut self,
        expressions: HandleSpan<ExpressionHandle>,
        offset: u32,
        expression: ExpressionHandle,
    ) {
        *self.spans.expression_handles.get_mut(Handle::from_parts(
            expressions
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("expression handle index overflow"),
            expressions.start().generation(),
        )) = expression;
    }

    pub fn push_expression_handle(
        &mut self,
        span: &mut HandleSpan<ExpressionHandle>,
        expression: ExpressionHandle,
    ) {
        self.spans
            .expression_handles
            .append_to_span(span, expression);
    }

    pub fn insert_struct_fields(
        &mut self,
        fields: impl IntoIterator<Item = TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        self.spans.struct_fields.insert_many(fields)
    }

    pub fn reserve_struct_fields(&mut self, count: u32) -> HandleSpan<TableStructLiteralField> {
        self.spans.struct_fields.insert_many(
            std::iter::repeat_with(TableStructLiteralField::default)
                .take(usize::try_from(count).expect("struct literal field span count overflow")),
        )
    }

    pub fn set_struct_field_at_offset(
        &mut self,
        fields: HandleSpan<TableStructLiteralField>,
        offset: u32,
        field: TableStructLiteralField,
    ) {
        *self.spans.struct_fields.get_mut(Handle::from_parts(
            fields
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("struct literal field index overflow"),
            fields.start().generation(),
        )) = field;
    }

    pub fn push_struct_field(
        &mut self,
        span: &mut HandleSpan<TableStructLiteralField>,
        field: TableStructLiteralField,
    ) {
        self.spans.struct_fields.append_to_span(span, field);
    }

    pub fn push_name_path_member(
        &mut self,
        span: &mut HandleSpan<DiagnosticName>,
        member: DiagnosticName,
    ) {
        self.spans.name_path_members.append_to_span(span, member);
    }

    pub fn reserve_name_path_members(&mut self, count: u32) -> HandleSpan<DiagnosticName> {
        self.spans.name_path_members.insert_many(
            std::iter::repeat_with(DiagnosticName::default)
                .take(usize::try_from(count).expect("name path member span count overflow")),
        )
    }

    pub fn set_name_path_member_at_offset(
        &mut self,
        members: HandleSpan<DiagnosticName>,
        offset: u32,
        member: DiagnosticName,
    ) {
        *self.spans.name_path_members.get_mut(Handle::from_parts(
            members
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member index overflow"),
            members.start().generation(),
        )) = member;
    }

    pub fn reserve_name_path_member_symbols(&mut self, count: u32) -> HandleSpan<SymbolHandle> {
        self.spans.name_path_member_symbols.insert_many(
            std::iter::repeat_with(SymbolHandle::invalid)
                .take(usize::try_from(count).expect("name path member symbol span count overflow")),
        )
    }

    pub fn set_name_path_member_symbol_at_offset(
        &mut self,
        member_symbols: HandleSpan<SymbolHandle>,
        offset: u32,
        member_symbol: SymbolHandle,
    ) {
        *self
            .spans
            .name_path_member_symbols
            .get_mut(Handle::from_parts(
                member_symbols
                    .start()
                    .arena_index()
                    .checked_add(offset)
                    .expect("name path member symbol index overflow"),
                member_symbols.start().generation(),
            )) = member_symbol;
    }

    pub fn copy_from(
        &mut self,
        source: &ExpressionTable,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        let source_span = source.source_span(expression);
        let copied = match source.expression(expression) {
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
                let result = atomic
                    .result
                    .is_valid()
                    .then(|| self.copy_from(source, atomic.result))
                    .unwrap_or_else(ExpressionHandle::invalid);
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
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| self.copy_from(source, call.receiver))
                    .unwrap_or_else(ExpressionHandle::invalid);
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
                let start = range
                    .start
                    .is_valid()
                    .then(|| self.copy_from(source, range.start))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .is_valid()
                    .then(|| self.copy_from(source, range.end))
                    .unwrap_or_else(ExpressionHandle::invalid);
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

    pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.nodes.expressions.get(handle)
    }

    pub fn expression_mut(&mut self, handle: ExpressionHandle) -> &mut ExpressionNode {
        self.nodes.expressions.get_mut(handle)
    }

    pub fn iter_expressions(&self) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
        self.nodes.expressions.iter()
    }

    pub fn expression_handles(&self, span: HandleSpan<ExpressionHandle>) -> &[ExpressionHandle] {
        self.spans.expression_handles.span_or_empty(span)
    }

    pub fn expression_handle_at_offset(
        &self,
        expressions: HandleSpan<ExpressionHandle>,
        offset: u32,
    ) -> &ExpressionHandle {
        self.spans.expression_handles.get(Handle::from_parts(
            expressions
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("expression handle index overflow"),
            expressions.start().generation(),
        ))
    }

    pub fn struct_fields(
        &self,
        span: HandleSpan<TableStructLiteralField>,
    ) -> &[TableStructLiteralField] {
        self.spans.struct_fields.span_or_empty(span)
    }

    pub fn struct_field_at_offset(
        &self,
        fields: HandleSpan<TableStructLiteralField>,
        offset: u32,
    ) -> &TableStructLiteralField {
        self.spans.struct_fields.get(Handle::from_parts(
            fields
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("struct literal field index overflow"),
            fields.start().generation(),
        ))
    }

    pub fn name_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
        self.spans.name_path_members.span_or_empty(span)
    }

    pub fn name_path_member_symbols(&self, span: HandleSpan<SymbolHandle>) -> &[SymbolHandle] {
        self.spans.name_path_member_symbols.span_or_empty(span)
    }

    pub fn name_path_member_at_offset(
        &self,
        members: HandleSpan<DiagnosticName>,
        offset: u32,
    ) -> &DiagnosticName {
        self.spans.name_path_members.get(Handle::from_parts(
            members
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("name path member index overflow"),
            members.start().generation(),
        ))
    }

    pub fn copy_name_path_members_with_suffix(
        &mut self,
        members: HandleSpan<DiagnosticName>,
        suffix: DiagnosticName,
    ) -> HandleSpan<DiagnosticName> {
        let span = self.reserve_name_path_members(
            members
                .count()
                .checked_add(1)
                .expect("name path member span count overflow"),
        );

        for offset in 0..members.count() {
            let member = self.name_path_member_at_offset(members, offset).clone();
            self.set_name_path_member_at_offset(span, offset, member);
        }

        self.set_name_path_member_at_offset(span, members.count(), suffix);

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
        let copied = match self.expression(expression).clone() {
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
                let result = atomic
                    .result
                    .is_valid()
                    .then(|| self.copy_from_self(atomic.result))
                    .unwrap_or_else(ExpressionHandle::invalid);
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
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| self.copy_from_self(call.receiver))
                    .unwrap_or_else(ExpressionHandle::invalid);
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
                let start = range
                    .start
                    .is_valid()
                    .then(|| self.copy_from_self(range.start))
                    .unwrap_or_else(ExpressionHandle::invalid);
                let end = range
                    .end
                    .is_valid()
                    .then(|| self.copy_from_self(range.end))
                    .unwrap_or_else(ExpressionHandle::invalid);
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

    fn storage_path_len(&self, expression: ExpressionHandle) -> Option<u32> {
        match self.expression(expression) {
            ExpressionNode::Name(path) => Some(path.members.count()),
            ExpressionNode::Indexed(indexed) => self.storage_path_len(indexed.collection),
            ExpressionNode::Borrow(target) => self.storage_path_len(target.target),
            _ => None,
        }
    }

    fn storage_path_head_symbol(&self, expression: ExpressionHandle) -> SymbolHandle {
        match self.expression(expression) {
            ExpressionNode::Name(path) => path.head_symbol,
            ExpressionNode::Indexed(indexed) => self.storage_path_head_symbol(indexed.collection),
            ExpressionNode::Borrow(target) => self.storage_path_head_symbol(target.target),
            _ => SymbolHandle::invalid(),
        }
    }

    fn fill_storage_path_members(
        &mut self,
        expression: ExpressionHandle,
        members: HandleSpan<DiagnosticName>,
        offset: u32,
    ) -> Option<()> {
        match self.expression(expression).clone() {
            ExpressionNode::Name(path) => {
                for source_offset in 0..path.members.count() {
                    let member = self
                        .name_path_member_at_offset(path.members, source_offset)
                        .clone();
                    self.set_name_path_member_at_offset(
                        members,
                        offset
                            .checked_add(source_offset)
                            .expect("name path member span count overflow"),
                        member,
                    );
                }
                Some(())
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) = self.expression(indexed.index) else {
                    return None;
                };
                // An index beyond i64 cannot name a real element; treat it
                // like any non-constant index.
                let index = index.value_i64()?;
                self.fill_storage_path_members(indexed.collection, members, offset)?;
                let path_len = self.storage_path_len(indexed.collection)?;
                let last_offset = offset
                    .checked_add(path_len.checked_sub(1)?)
                    .expect("name path member span count overflow");
                let last_member = self.name_path_member_at_offset(members, last_offset);
                let indexed_member = DiagnosticName::generated(format!("{last_member}[{index}]"));
                self.set_name_path_member_at_offset(members, last_offset, indexed_member);
                Some(())
            }
            ExpressionNode::Borrow(target) => {
                self.fill_storage_path_members(target.target, members, offset)
            }
            _ => None,
        }
    }

    pub fn expression_count(&self) -> usize {
        self.nodes.expressions.len()
    }

    pub fn struct_field_count(&self) -> usize {
        self.spans.struct_fields.len()
    }

    pub fn display_name(&self, handle: ExpressionHandle) -> String {
        self.expression(handle).display_name(self)
    }

    pub fn string_literal(&self, handle: ExpressionHandle) -> Option<&[u8]> {
        match self.expression(handle) {
            ExpressionNode::String(value) => Some(value.as_ref()),
            _ => None,
        }
    }
}

fn source_span_index(handle: ExpressionHandle) -> usize {
    usize::try_from(handle.arena_index())
        .expect("expression index overflow")
        .checked_sub(1)
        .expect("invalid expression handle has no source span")
}

impl Default for ExpressionTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionNode {
    ArrayLiteral(HandleSpan<ExpressionHandle>),
    Atomic(TableAtomicExpression),
    Binary(TableBinaryExpression),
    Boolean(bool),
    Cast(TableCastExpression),
    Call(TableCallExpression),
    Float(FloatLiteral),
    Indexed(TableIndexedExpression),
    Integer(IntegerLiteral),
    Membership(TableMembershipExpression),
    Member(TableMemberExpression),
    Borrow(TableBorrowExpression),
    Name(TableNamePath),
    Range(TableRangeExpression),
    StructLiteral(TableStructLiteral),
    /// Exact decoded literal octets, including non-UTF-8 `\xNN` values.
    String(Arc<[u8]>),
    Unary(TableUnaryExpression),
    /// Proof-only observation of a type's normalized all-zero home value.
    ZeroValue(psi_arena::Handle<crate::types::TypeReference>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableBorrowExpression {
    pub target: ExpressionHandle,
    pub access: psi_language_core::ReferenceAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableAtomicExpression {
    pub value: ExpressionHandle,
    pub result: ExpressionHandle,
    pub ordering: psi_language_core::atomic::AtomicOrderingPlan,
    pub result_custody: psi_language_core::atomic::AtomicExpressionResultCustody,
}

impl Default for ExpressionNode {
    fn default() -> Self {
        Self::Integer(IntegerLiteral::zero())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableBinaryExpression {
    pub left: ExpressionHandle,
    pub operator: BinaryOperator,
    pub right: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableUnaryExpression {
    pub operator: UnaryOperator,
    pub operand: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCastExpression {
    pub value: ExpressionHandle,
    /// Root of the complete cast target in the program's child-type arena.
    pub target_type: psi_arena::Handle<crate::types::TypeReference>,
    /// Diagnostic spelling only; semantic identity uses `target_type`.
    pub target_label: HandleSpan<DiagnosticName>,
    /// Arithmetic domain cast (`x as u8 in Saturating`), decision 17 S2.
    pub domain: psi_numerics::arithmetic::ArithmeticDomain,
    /// A NON-policy `in <Name>` suffix -- the semantic-domain qualification
    /// spelling (decision 19), judged at validation. EMPTY = no suffix.
    pub semantic_domain: HandleSpan<DiagnosticName>,
    /// PDI2 proof-static family arguments in the declarations child-type arena.
    pub semantic_domain_arguments: HandleSpan<crate::types::TypeReference>,
    /// Normalized declaration identity for `semantic_domain`. Populated in
    /// typed normalization after carrier-aware domain lookup.
    pub semantic_domain_symbol: SymbolHandle,
    /// Value conversion vs §5b borrow recast (`&x as &T`).
    pub form: psi_language_core::cast_form::CastForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableIndexedExpression {
    pub collection: ExpressionHandle,
    pub index: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRangeExpression {
    pub start: ExpressionHandle,
    pub end: ExpressionHandle,
    pub end_inclusive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableMembershipExpression {
    pub value: ExpressionHandle,
    pub domain: HandleSpan<DiagnosticName>,
    /// Exact declared-domain identity. Invalid for implicit `Type::Case` domains.
    pub domain_symbol: SymbolHandle,
    /// Exact data identity for an implicit `Type::Case` domain.
    pub case_type_symbol: SymbolHandle,
    /// Exact variant identity for an implicit `Type::Case` domain.
    pub case_symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub member_symbol: SymbolHandle,
    pub member: DiagnosticName,
    /// The case variant a destructure-bound payload field came from, so symbol
    /// resolution binds `member_symbol` to THAT variant's field rather than a
    /// same-named field in another variant. `None` for ordinary field access.
    pub case_variant: Option<DiagnosticName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCallExpression {
    pub receiver: ExpressionHandle,
    pub target_symbol: SymbolHandle,
    pub target: DiagnosticName,
    pub machine_arguments: Box<[StaticMachineArgument]>,
    pub arguments: HandleSpan<ExpressionHandle>,
    /// Erased evidence-term spellings remain outside runtime name resolution.
    pub evidence_arguments: Box<[DiagnosticName]>,
    pub operational_acknowledgement: psi_language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticMachineArgument {
    /// Historical storage name shared by type/const/machine proposition
    /// arguments; the typed target telescope validates the category.
    pub path: Box<[DiagnosticName]>,
    pub application: Option<Box<StaticSymbolApplication>>,
    pub const_literal: Option<psi_numerics::literals::IntegerLiteral>,
    /// Proof-static projection from one named evidence term. It is resolved
    /// against checked contract terms, not the runtime symbol table.
    pub evidence_projection: Option<EvidenceProjection>,
    /// Entry-state symbol of the selected concrete machine.
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticSymbolApplication {
    pub lifetime_arguments: Box<[DiagnosticName]>,
    pub arguments: Box<[StaticMachineArgument]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceProjection {
    pub term: DiagnosticName,
    pub member: DiagnosticName,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableNamePath {
    pub members: HandleSpan<DiagnosticName>,
    /// One exact declaration identity per authored path segment.
    pub member_symbols: HandleSpan<SymbolHandle>,
    pub is_self_value: bool,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteral {
    pub type_name: DiagnosticName,
    pub type_symbol: SymbolHandle,
    /// `Some` when the literal constructs a CASE of `type_name`
    /// (`Command::Say { text: ... }`); `None` for a plain record literal.
    pub case_name: Option<DiagnosticName>,
    pub case_symbol: Option<SymbolHandle>,
    pub fields: HandleSpan<TableStructLiteralField>,
}

impl Default for TableStructLiteral {
    fn default() -> Self {
        Self {
            type_name: DiagnosticName::default(),
            type_symbol: SymbolHandle::invalid(),
            case_name: None,
            case_symbol: None,
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteralField {
    pub name: DiagnosticName,
    pub field_symbol: SymbolHandle,
    pub value: ExpressionHandle,
}

impl Default for TableStructLiteralField {
    fn default() -> Self {
        Self {
            name: DiagnosticName::default(),
            field_symbol: SymbolHandle::invalid(),
            value: ExpressionHandle::invalid(),
        }
    }
}

/// The shared TEXT-based float carrier (F2): the source spelling plus an
/// optional format landing ride every tree layer, exactly like
/// IntegerLiteral -- per-format reads are each correctly rounded from the
/// spelling, so f32 never routes through f64.
pub use psi_numerics::literals::FloatLiteral;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    And,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Divide,
    Equal,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Modulo,
    Multiply,
    NotEqual,
    Or,
    ShiftLeft,
    ShiftRight,
    Subtract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    BitwiseNot,
    LogicalNot,
}
