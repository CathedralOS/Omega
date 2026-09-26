//! The symbol-resolved expression table and its nodes.
//!
//! This file owns the expression table and its nodes. `table_copies.rs` copies
//! expressions between tables.

mod display;
mod table_copies;
#[cfg(test)]
mod tests;

pub use display::display_name_path;
/// The shared TEXT-based float carrier (F2): the source spelling plus an
/// optional format landing ride every tree layer, exactly like
/// IntegerLiteral -- per-format reads are each correctly rounded from the
/// spelling, so f32 never routes through f64.
pub use numerics::literals::FloatLiteral;

use crate::AuthoredDeclarationSelectionOccurrenceId;
use crate::name::DiagnosticName;
use arena::{Arena, Handle, HandleSpan};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, CompilerDerivedSelectionPartition,
};
use numerics::literals::IntegerLiteral;
use source::SourceSpan;
use std::sync::Arc;
use symbols::SymbolHandle;

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
    compiler_selection_partitions: Vec<Option<CompilerDerivedSelectionPartition>>,
    authored_selection_occurrences: Vec<HandleSpan<StoredAuthoredSelectionOccurrenceId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpressionSpanStorage {
    expression_handles: Arena<ExpressionHandle>,
    name_path_members: Arena<DiagnosticName>,
    name_path_member_symbols: Arena<SymbolHandle>,
    struct_fields: Arena<TableStructLiteralField>,
    match_arms: Arena<TableMatchArm>,
    authored_selection_occurrence_ids: Arena<StoredAuthoredSelectionOccurrenceId>,
}

impl ExpressionTable {
    pub fn insert_match_arms(
        &mut self,
        arms: impl IntoIterator<Item = TableMatchArm>,
    ) -> HandleSpan<TableMatchArm> {
        self.spans.match_arms.insert_many(arms)
    }

    pub fn match_arms(&self, arms: HandleSpan<TableMatchArm>) -> &[TableMatchArm] {
        self.spans.match_arms.span_or_empty(arms)
    }

    pub fn new() -> Self {
        Self {
            nodes: ExpressionNodeStorage {
                expressions: Arena::new(),
                source_spans: Vec::new(),
                authored_expression_exposures: Vec::new(),
                compiler_selection_partitions: Vec::new(),
                authored_selection_occurrences: Vec::new(),
            },
            spans: ExpressionSpanStorage {
                expression_handles: Arena::new(),
                name_path_members: Arena::new(),
                name_path_member_symbols: Arena::new(),
                struct_fields: Arena::new(),
                match_arms: Arena::new(),
                authored_selection_occurrence_ids: Arena::new(),
            },
        }
    }

    pub fn clear(&mut self) {
        self.nodes.expressions.reset_retain_capacity();
        self.nodes.source_spans.clear();
        self.nodes.authored_expression_exposures.clear();
        self.nodes.compiler_selection_partitions.clear();
        self.nodes.authored_selection_occurrences.clear();
        self.spans.expression_handles.reset_retain_capacity();
        self.spans.name_path_members.reset_retain_capacity();
        self.spans.name_path_member_symbols.reset_retain_capacity();
        self.spans.struct_fields.reset_retain_capacity();
        self.spans.match_arms.reset_retain_capacity();
        self.spans
            .authored_selection_occurrence_ids
            .reset_retain_capacity();
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        let handle = self.nodes.expressions.insert(expression);
        self.nodes.source_spans.push(SourceSpan::default());
        self.nodes.authored_expression_exposures.push(None);
        self.nodes.compiler_selection_partitions.push(None);
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
            self.nodes.compiler_selection_partitions.len() - 1
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

    pub fn compiler_selection_partition(
        &self,
        handle: ExpressionHandle,
    ) -> Option<CompilerDerivedSelectionPartition> {
        self.nodes.compiler_selection_partitions[source_span_index(handle)]
    }

    pub fn set_compiler_selection_partition(
        &mut self,
        handle: ExpressionHandle,
        partition: CompilerDerivedSelectionPartition,
    ) {
        let slot = &mut self.nodes.compiler_selection_partitions[source_span_index(handle)];
        if let Some(existing) = *slot {
            assert_eq!(
                existing, partition,
                "one expression handle cannot belong to two compiler selection partitions"
            );
        }
        *slot = Some(partition);
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
        rebase: language_semantics::declaration_selection::AuthoredDeclarationSelectionSuffixRebase,
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

    pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.nodes.expressions.get(handle)
    }

    /// Check arena membership and generation before indexing expression side tables.
    pub fn expression_is_valid(&self, handle: ExpressionHandle) -> bool {
        self.nodes.expressions.is_valid(handle)
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
    Match(TableMatchExpression),
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
    ZeroValue(arena::Handle<crate::types::TypeReference>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableBorrowExpression {
    pub target: ExpressionHandle,
    pub access: language_core::ReferenceAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableAtomicExpression {
    pub value: ExpressionHandle,
    pub result: ExpressionHandle,
    pub ordering: language_core::atomic::AtomicOrderingPlan,
    pub result_custody: language_core::atomic::AtomicExpressionResultCustody,
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
    pub target_type: arena::Handle<crate::types::TypeReference>,
    /// Diagnostic spelling only; semantic identity uses `target_type`.
    pub target_label: HandleSpan<DiagnosticName>,
    /// Arithmetic domain cast (`x as u8 in Saturating`), decision 17 S2.
    pub domain: numerics::arithmetic::ArithmeticDomain,
    /// A NON-policy `in <Name>` suffix -- the semantic-domain qualification
    /// spelling (decision 19), judged at validation. EMPTY = no suffix.
    pub semantic_domain: HandleSpan<DiagnosticName>,
    /// PDI2 proof-static family arguments in the declarations child-type arena.
    pub semantic_domain_arguments: HandleSpan<crate::types::TypeReference>,
    /// Normalized declaration identity for `semantic_domain`. Populated in
    /// typed normalization after carrier-aware domain lookup.
    pub semantic_domain_symbol: SymbolHandle,
    /// Value conversion vs §5b borrow recast (`&x as &T`).
    pub form: language_core::cast_form::CastForm,
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
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticMachineArgument {
    /// An explicit structural type argument; zero denotes another static argument kind.
    pub type_reference: arena::Handle<crate::types::TypeReference>,
    /// Historical storage name shared by type/const/machine proposition
    /// arguments; the typed target telescope validates the category.
    pub path: Box<[DiagnosticName]>,
    pub application: Option<Box<StaticSymbolApplication>>,
    pub const_literal: Option<numerics::literals::IntegerLiteral>,
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

/// Ordered value dispatch. The subject is evaluated once before testing arms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableMatchExpression {
    pub subject: ExpressionHandle,
    pub arms: HandleSpan<TableMatchArm>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableMatchArm {
    pub pattern: MatchPattern,
    pub value: ExpressionHandle,
    pub source_span: SourceSpan,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MatchPattern {
    Value(ExpressionHandle),
    #[default]
    Wildcard,
}
