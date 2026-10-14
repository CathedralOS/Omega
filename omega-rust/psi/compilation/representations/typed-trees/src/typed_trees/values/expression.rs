//! The typed expression table and its nodes.
//!
//! This file owns the expression table, its nodes and the owned
//! `Expression` tree forms. `table_copies.rs` copies expression subtrees
//! into a table and remaps a copy's symbols; `tree_conversion.rs` converts
//! between owned trees and table rows.

use crate::AuthoredDeclarationSelectionOccurrenceId;
use crate::name::Identifier;
use crate::static_applications::{
    StaticMachineApplication, StaticMachineApplicationHandle, StaticMachineBinding,
    StaticMachineLifetimeBinding,
};
use arena::{Arena, Handle, HandleSpan};
use numerics::literals::IntegerLiteral;
use source::SourceSpan;
use std::ops::Deref;
use std::sync::Arc;
use symbols::JoinedPath;
use symbols::SymbolHandle;

mod display;
mod index_authority;
mod shape;
mod table_copies;
mod tree_conversion;

pub use display::{display_call_label, display_name_path};
pub use index_authority::{
    IndexExpressionAuthority, OpenIndexNormalization, OpenIndexOperationSelection,
};

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
pub enum Expression {
    Match(Box<MatchExpression>),
    ArrayLiteral(Arc<[Expression]>),
    Atomic(Box<AtomicExpression>),
    Binary(Box<BinaryExpression>),
    Boolean(bool),
    Cast(Box<CastExpression>),
    Call(Box<CallExpression>),
    Float(FloatLiteral),
    /// Internal semantic value; no runtime representation or source spelling is implied.
    UndeterminedNaN(numerics::literals::FloatFormat),
    Indexed(Box<IndexedExpression>),
    Integer(IntegerLiteral),
    Member(Box<MemberExpression>),
    Borrow(Box<BorrowExpression>),
    Name(NamePath),
    Range(Box<RangeExpression>),
    StructLiteral(StructLiteral),
    String(Arc<[u8]>),
    Unary(Box<UnaryExpression>),
    ZeroValue(crate::types::TypeReferenceHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowExpression {
    pub target: Expression,
    pub access: language_core::ReferenceAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicExpression {
    pub value: Expression,
    pub result: Option<Expression>,
    pub ordering: language_core::atomic::AtomicOrderingPlan,
    pub result_custody: language_core::atomic::AtomicExpressionResultCustody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionTable {
    expressions: Arena<ExpressionNode>,
    /// Iteration-visible expressions of this program. Backing rows remain at
    /// their stable handles when a compilation projects a closed body roster.
    expression_roots: Arena<ExpressionHandle>,
    source_spans: Vec<SourceSpan>,
    /// Complete constructed value types derived from this occurrence's
    /// selected declaration and completed children after call binding. Zero
    /// means unresolved; a projected element cannot supply an aggregate type.
    constructed_value_types: Arena<crate::types::TypeReferenceHandle>,
    constructed_value_type_handles: Vec<Handle<crate::types::TypeReferenceHandle>>,
    index_expression_authorities: Arena<IndexExpressionAuthority>,
    index_expression_authority_handles: Vec<Handle<IndexExpressionAuthority>>,
    authored_selection_occurrences: Vec<HandleSpan<StoredAuthoredSelectionOccurrenceId>>,
    authored_selection_occurrence_ids: Arena<StoredAuthoredSelectionOccurrenceId>,
    expression_handles: Arena<ExpressionHandle>,
    name_path_members: Arena<Identifier>,
    name_path_member_symbols: Arena<SymbolHandle>,
    struct_fields: Arena<TableStructLiteralField>,
    match_arms: Arena<TableMatchArm>,
    static_machine_applications: Arena<StaticMachineApplication>,
    static_machine_arguments: Arena<StaticMachineArgument>,
    static_machine_bindings: Arena<StaticMachineBinding>,
    static_machine_lifetime_bindings: Arena<StaticMachineLifetimeBinding>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExpressionTableCapacity {
    pub expressions: usize,
    pub authored_selection_occurrences: usize,
    pub expression_handles: usize,
    pub name_path_members: usize,
    pub name_path_member_symbols: usize,
    pub struct_fields: usize,
    pub match_arms: usize,
}

impl ExpressionTableCapacity {}

impl ExpressionTable {
    pub(crate) fn retains_exact_prefix(&self, base: &Self) -> bool {
        let Self {
            expressions,
            expression_roots,
            authored_selection_occurrence_ids,
            expression_handles,
            name_path_members,
            name_path_member_symbols,
            struct_fields,
            match_arms,
            source_spans,
            constructed_value_types,
            constructed_value_type_handles,
            index_expression_authorities,
            index_expression_authority_handles,
            authored_selection_occurrences,
            static_machine_applications,
            static_machine_arguments,
            static_machine_bindings,
            static_machine_lifetime_bindings,
        } = base;
        crate::typed_trees::retained_prefix::arena_is_exact_prefix(expressions, &self.expressions)
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                index_expression_authorities,
                &self.index_expression_authorities,
            )
            && self
                .index_expression_authority_handles
                .starts_with(index_expression_authority_handles)
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                static_machine_applications,
                &self.static_machine_applications,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                static_machine_arguments,
                &self.static_machine_arguments,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                static_machine_bindings,
                &self.static_machine_bindings,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                static_machine_lifetime_bindings,
                &self.static_machine_lifetime_bindings,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                expression_roots,
                &self.expression_roots,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                authored_selection_occurrence_ids,
                &self.authored_selection_occurrence_ids,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                expression_handles,
                &self.expression_handles,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                name_path_members,
                &self.name_path_members,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                name_path_member_symbols,
                &self.name_path_member_symbols,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                struct_fields,
                &self.struct_fields,
            )
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                match_arms,
                &self.match_arms,
            )
            && self.source_spans.starts_with(source_spans)
            && crate::typed_trees::retained_prefix::arena_is_exact_prefix(
                constructed_value_types,
                &self.constructed_value_types,
            )
            && self
                .constructed_value_type_handles
                .starts_with(constructed_value_type_handles)
            && self
                .authored_selection_occurrences
                .starts_with(authored_selection_occurrences)
    }

    pub fn insert_match_arms(
        &mut self,
        arms: impl IntoIterator<Item = TableMatchArm>,
    ) -> HandleSpan<TableMatchArm> {
        self.match_arms.insert_many(arms)
    }

    pub fn match_arms(&self, arms: HandleSpan<TableMatchArm>) -> &[TableMatchArm] {
        self.match_arms.span_or_empty(arms)
    }

    /// Every arm's value passes `visit`, with wildcard patterns visiting no
    /// pattern handle.
    pub fn match_arms_all(
        &self,
        arms: HandleSpan<TableMatchArm>,
        mut visit: impl FnMut(MatchPattern, ExpressionHandle) -> bool,
    ) -> bool {
        self.match_arms(arms)
            .iter()
            .all(|arm| visit(arm.pattern, arm.value))
    }

    /// `visit` holds for the subject, every arm's value, and every `Value`
    /// pattern — the deep predicate over a dispatch's full handle set.
    pub fn match_deep_all(
        &self,
        dispatch: &TableMatchExpression,
        mut visit: impl FnMut(ExpressionHandle) -> bool,
    ) -> bool {
        visit(dispatch.subject)
            && self.match_arms_all(dispatch.arms, |pattern, value| {
                !matches!(pattern, MatchPattern::Value(pattern) if !visit(pattern)) && visit(value)
            })
    }

    /// `visit` holds for the subject, some arm's value, or some `Value`
    /// pattern — the deep disjunctive predicate over a dispatch's handles.
    pub fn match_deep_any(
        &self,
        dispatch: &TableMatchExpression,
        mut visit: impl FnMut(ExpressionHandle) -> bool,
    ) -> bool {
        visit(dispatch.subject)
            || self.match_arms(dispatch.arms).iter().any(|arm| {
                matches!(arm.pattern, MatchPattern::Value(pattern) if visit(pattern))
                    || visit(arm.value)
            })
    }

    /// The worklist order for a depth-first walk over `dispatch`: arm values
    /// in reverse so the first arm pops first off a stack, each `Value`
    /// pattern after its arm's value, and the subject last.
    pub fn match_pending(&self, dispatch: &TableMatchExpression) -> Vec<ExpressionHandle> {
        let mut pending = Vec::new();
        for arm in self.match_arms(dispatch.arms).iter().rev() {
            pending.push(arm.value);
            if let MatchPattern::Value(pattern) = arm.pattern {
                pending.push(pattern);
            }
        }
        pending.push(dispatch.subject);
        pending
    }

    pub fn new() -> Self {
        Self::with_expression_capacity(0)
    }

    /// The owned freshness counter over every arena and side Vec the table
    /// owns — the freshness key for indexes and memos derived from
    /// expression rows. Each arena's revision only moves forward, so their
    /// sum moves on any mutation, including same-length rewrites; the two
    /// `Vec` columns have no revision of their own and join by length.
    pub fn revision(&self) -> u64 {
        self.expressions
            .revision()
            .wrapping_add(self.expression_roots.revision())
            .wrapping_add(self.index_expression_authorities.revision())
            .wrapping_add(self.authored_selection_occurrence_ids.revision())
            .wrapping_add(self.expression_handles.revision())
            .wrapping_add(self.name_path_members.revision())
            .wrapping_add(self.name_path_member_symbols.revision())
            .wrapping_add(self.struct_fields.revision())
            .wrapping_add(self.match_arms.revision())
            .wrapping_add(self.static_machine_applications.revision())
            .wrapping_add(self.static_machine_arguments.revision())
            .wrapping_add(self.constructed_value_types.revision())
            .wrapping_add(self.static_machine_bindings.revision())
            .wrapping_add(self.static_machine_lifetime_bindings.revision())
            .wrapping_add(self.source_spans.len() as u64)
            .wrapping_add(self.authored_selection_occurrences.len() as u64)
    }

    pub fn with_expression_capacity(expression_capacity: usize) -> Self {
        Self::with_expression_and_handle_capacity(expression_capacity, 0)
    }

    pub fn with_expression_and_handle_capacity(
        expression_capacity: usize,
        expression_handle_capacity: usize,
    ) -> Self {
        Self::with_capacities(ExpressionTableCapacity {
            expressions: expression_capacity,
            expression_handles: expression_handle_capacity,
            ..ExpressionTableCapacity::default()
        })
    }

    pub fn with_capacities(capacity: ExpressionTableCapacity) -> Self {
        Self {
            expressions: Arena::with_capacity(capacity.expressions),
            expression_roots: Arena::with_capacity(capacity.expressions),
            source_spans: Vec::with_capacity(capacity.expressions),
            constructed_value_types: Arena::default(),
            constructed_value_type_handles: Vec::new(),
            index_expression_authorities: Arena::default(),
            index_expression_authority_handles: Vec::new(),
            authored_selection_occurrences: Vec::with_capacity(capacity.expressions),
            authored_selection_occurrence_ids: Arena::with_capacity(
                capacity.authored_selection_occurrences,
            ),
            expression_handles: Arena::with_capacity(capacity.expression_handles),
            name_path_members: Arena::with_capacity(capacity.name_path_members),
            name_path_member_symbols: Arena::with_capacity(capacity.name_path_member_symbols),
            struct_fields: Arena::with_capacity(capacity.struct_fields),
            match_arms: Arena::with_capacity(capacity.match_arms),
            static_machine_applications: Arena::default(),
            static_machine_arguments: Arena::default(),
            static_machine_bindings: Arena::default(),
            static_machine_lifetime_bindings: Arena::default(),
        }
    }

    pub fn clear(&mut self) {
        self.expressions.reset_retain_capacity();
        self.expression_roots.reset_retain_capacity();
        self.source_spans.clear();
        self.constructed_value_types.reset_retain_capacity();
        self.constructed_value_type_handles.clear();
        self.index_expression_authorities.reset_retain_capacity();
        self.index_expression_authority_handles.clear();
        self.authored_selection_occurrences.clear();
        self.authored_selection_occurrence_ids
            .reset_retain_capacity();
        self.expression_handles.reset_retain_capacity();
        self.name_path_members.reset_retain_capacity();
        self.name_path_member_symbols.reset_retain_capacity();
        self.struct_fields.reset_retain_capacity();
        self.match_arms.reset_retain_capacity();
        self.static_machine_applications.reset_retain_capacity();
        self.static_machine_arguments.reset_retain_capacity();
        self.static_machine_bindings.reset_retain_capacity();
        self.static_machine_lifetime_bindings
            .reset_retain_capacity();
    }

    /// Preserve the source application before folding or specializing its call.
    /// This records operands only; immutable checking still owes admission.
    pub fn retain_static_machine_application(
        &mut self,
        registration_operation: SymbolHandle,
        target: Identifier,
        arguments: &[StaticMachineArgument],
    ) -> StaticMachineApplicationHandle {
        // An inferred application may have no authored operands. Its retained
        // row still owns the exact tuple chosen before executable retargeting.
        let arguments = self
            .static_machine_arguments
            .insert_many(arguments.iter().cloned());
        self.static_machine_applications
            .insert(StaticMachineApplication {
                registration_operation,
                lexical_owner: SymbolHandle::invalid(),
                target,
                arguments,
                selected_machine_arguments: HandleSpan::empty(),
                bindings: HandleSpan::empty(),
                lifetime_bindings: HandleSpan::empty(),
                binding_declaration: SymbolHandle::invalid(),
                conformance_selection: None,
            })
    }

    pub fn static_machine_applications(
        &self,
    ) -> impl Iterator<Item = (StaticMachineApplicationHandle, &StaticMachineApplication)> {
        self.static_machine_applications.iter()
    }

    pub fn static_machine_application(
        &self,
        application: StaticMachineApplicationHandle,
    ) -> &StaticMachineApplication {
        self.static_machine_applications.get(application)
    }

    pub fn static_machine_application_mut(
        &mut self,
        application: StaticMachineApplicationHandle,
    ) -> &mut StaticMachineApplication {
        self.static_machine_applications.get_mut(application)
    }

    pub fn static_machine_application_bindings(
        &self,
        application: StaticMachineApplicationHandle,
    ) -> &[StaticMachineBinding] {
        self.static_machine_bindings
            .span_or_empty(self.static_machine_application(application).bindings)
    }

    pub fn static_machine_application_bindings_mut(
        &mut self,
        application: StaticMachineApplicationHandle,
    ) -> &mut [StaticMachineBinding] {
        let bindings = self.static_machine_applications.get(application).bindings;
        self.static_machine_bindings.span_mut_or_empty(bindings)
    }

    /// Refinement updates the same application's rows, rather than appending
    /// another tuple on every fixed-point round. Copies own independent spans.
    pub fn set_static_machine_application_bindings(
        &mut self,
        application: StaticMachineApplicationHandle,
        bindings: &[StaticMachineBinding],
    ) {
        let previous = self.static_machine_applications.get(application).bindings;
        if self.static_machine_bindings.span_or_empty(previous) == bindings {
            return;
        }
        if previous.len() == bindings.len() {
            self.static_machine_bindings
                .span_mut_or_empty(previous)
                .clone_from_slice(bindings);
        } else {
            let bindings = self
                .static_machine_bindings
                .insert_many(bindings.iter().cloned());
            self.static_machine_applications
                .get_mut(application)
                .bindings = bindings;
        }
    }

    pub fn static_machine_application_arguments(
        &self,
        application: StaticMachineApplicationHandle,
    ) -> &[StaticMachineArgument] {
        self.static_machine_arguments
            .span_or_empty(self.static_machine_application(application).arguments)
    }

    pub fn static_machine_application_selected_machines(
        &self,
        application: StaticMachineApplicationHandle,
    ) -> &[StaticMachineArgument] {
        self.static_machine_arguments.span_or_empty(
            self.static_machine_application(application)
                .selected_machine_arguments,
        )
    }

    pub fn static_machine_application_selected_machines_mut(
        &mut self,
        application: StaticMachineApplicationHandle,
    ) -> &mut [StaticMachineArgument] {
        let selected = self
            .static_machine_applications
            .get(application)
            .selected_machine_arguments;
        self.static_machine_arguments.span_mut_or_empty(selected)
    }

    pub fn set_static_machine_application_selected_machines(
        &mut self,
        application: StaticMachineApplicationHandle,
        selected: &[StaticMachineArgument],
    ) {
        if self.static_machine_application_selected_machines(application) == selected {
            return;
        }
        let selected = self
            .static_machine_arguments
            .insert_many(selected.iter().cloned());
        self.static_machine_applications
            .get_mut(application)
            .selected_machine_arguments = selected;
    }

    pub fn static_machine_application_lifetimes(
        &self,
        application: StaticMachineApplicationHandle,
    ) -> &[StaticMachineLifetimeBinding] {
        self.static_machine_lifetime_bindings.span_or_empty(
            self.static_machine_application(application)
                .lifetime_bindings,
        )
    }

    pub fn set_static_machine_application_lifetimes(
        &mut self,
        application: StaticMachineApplicationHandle,
        bindings: &[StaticMachineLifetimeBinding],
    ) {
        let previous = self
            .static_machine_applications
            .get(application)
            .lifetime_bindings;
        if self
            .static_machine_lifetime_bindings
            .span_or_empty(previous)
            == bindings
        {
            return;
        }
        if previous.len() == bindings.len() {
            self.static_machine_lifetime_bindings
                .span_mut_or_empty(previous)
                .clone_from_slice(bindings);
        } else {
            let bindings = self
                .static_machine_lifetime_bindings
                .insert_many(bindings.iter().cloned());
            self.static_machine_applications
                .get_mut(application)
                .lifetime_bindings = bindings;
        }
    }

    /// Preparation substitutes a fresh copied application's operands alongside
    /// its executable call. The source declaration identity remains unchanged.
    pub fn static_machine_application_arguments_mut(
        &mut self,
        application: StaticMachineApplicationHandle,
    ) -> &mut [StaticMachineArgument] {
        let arguments = self.static_machine_applications.get(application).arguments;
        self.static_machine_arguments.span_mut_or_empty(arguments)
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        let handle = self.expressions.insert(expression);
        self.expression_roots.insert(handle);
        self.source_spans.push(SourceSpan::default());
        self.authored_selection_occurrences
            .push(HandleSpan::empty());
        debug_assert_eq!(source_span_index(handle), self.source_spans.len() - 1);
        debug_assert_eq!(
            source_span_index(handle),
            self.authored_selection_occurrences.len() - 1
        );
        handle
    }

    pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
        self.source_spans[source_span_index(handle)]
    }

    pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
        self.source_spans[source_span_index(handle)] = source_span;
    }

    /// The complete value type retained before immutable checking. Copies
    /// begin unresolved and derive their own type after binding/substitution;
    /// a source instance's static arguments are not authority for its copy.
    pub fn constructed_value_type_reference(
        &self,
        handle: ExpressionHandle,
    ) -> crate::types::TypeReferenceHandle {
        if !self.expression_is_valid(handle) {
            return crate::types::TypeReferenceHandle::invalid();
        }
        let Some(reference) = self
            .constructed_value_type_handles
            .get(source_span_index(handle))
        else {
            return crate::types::TypeReferenceHandle::invalid();
        };
        if reference.is_valid() {
            *self.constructed_value_types.get(*reference)
        } else {
            crate::types::TypeReferenceHandle::invalid()
        }
    }

    pub fn set_constructed_value_type_reference(
        &mut self,
        handle: ExpressionHandle,
        type_reference: crate::types::TypeReferenceHandle,
    ) {
        let position = source_span_index(handle);
        if self.constructed_value_type_handles.len() <= position {
            self.constructed_value_type_handles
                .resize(position + 1, Handle::invalid());
        }
        let reference = &mut self.constructed_value_type_handles[position];
        if reference.is_valid() {
            *self.constructed_value_types.get_mut(*reference) = type_reference;
        } else {
            *reference = self.constructed_value_types.insert(type_reference);
        }
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

        self.authored_selection_occurrences[index] =
            self.authored_selection_occurrence_ids.insert_many(
                combined
                    .into_iter()
                    .map(StoredAuthoredSelectionOccurrenceId::new),
            );
    }

    pub fn authored_selection_occurrences(
        &self,
        handle: ExpressionHandle,
    ) -> impl ExactSizeIterator<Item = AuthoredDeclarationSelectionOccurrenceId> + '_ {
        self.authored_selection_occurrence_ids
            .span_or_empty(self.authored_selection_occurrences[source_span_index(handle)])
            .iter()
            .copied()
            .map(StoredAuthoredSelectionOccurrenceId::occurrence)
    }

    pub fn insert_expression_handles(
        &mut self,
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.expression_handles.insert_many(expressions)
    }

    pub fn reserve_expression_handles(&mut self, count: u32) -> HandleSpan<ExpressionHandle> {
        self.expression_handles.insert_many(
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
        *self
            .expression_handles
            .get_mut(expressions.handle_at(offset)) = expression;
    }

    pub fn push_expression_handle(
        &mut self,
        span: &mut HandleSpan<ExpressionHandle>,
        expression: ExpressionHandle,
    ) {
        self.expression_handles.append_to_span(span, expression);
    }

    pub fn insert_struct_fields(
        &mut self,
        fields: impl IntoIterator<Item = TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        self.struct_fields.insert_many(fields)
    }

    pub fn reserve_struct_fields(&mut self, count: u32) -> HandleSpan<TableStructLiteralField> {
        self.struct_fields.insert_many(
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
        *self.struct_fields.get_mut(fields.handle_at(offset)) = field;
    }

    pub fn push_struct_field(
        &mut self,
        span: &mut HandleSpan<TableStructLiteralField>,
        field: TableStructLiteralField,
    ) {
        self.struct_fields.append_to_span(span, field);
    }

    pub fn push_name_path_member(&mut self, span: &mut HandleSpan<Identifier>, member: Identifier) {
        self.name_path_members.append_to_span(span, member);
    }

    pub fn reserve_name_path_members(&mut self, count: u32) -> HandleSpan<Identifier> {
        self.name_path_members.insert_many(
            std::iter::repeat_with(Identifier::default)
                .take(usize::try_from(count).expect("name path member span count overflow")),
        )
    }

    pub fn set_name_path_member_at_offset(
        &mut self,
        members: HandleSpan<Identifier>,
        offset: u32,
        member: Identifier,
    ) {
        *self.name_path_members.get_mut(members.handle_at(offset)) = member;
    }

    pub fn push_name_path_member_symbol(
        &mut self,
        span: &mut HandleSpan<SymbolHandle>,
        member_symbol: SymbolHandle,
    ) {
        self.name_path_member_symbols
            .append_to_span(span, member_symbol);
    }

    pub fn reserve_name_path_member_symbols(&mut self, count: u32) -> HandleSpan<SymbolHandle> {
        self.name_path_member_symbols.insert_many(
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
            .name_path_member_symbols
            .get_mut(member_symbols.handle_at(offset)) = member_symbol;
    }

    pub fn expression_handle_at_offset(
        &self,
        expressions: HandleSpan<ExpressionHandle>,
        offset: u32,
    ) -> ExpressionHandle {
        *self.expression_handles.get(expressions.handle_at(offset))
    }

    /// Mutable node access for tree-normalization passes (the F2b float
    /// destination stamp rewrites an unlanded literal in place; handles and
    /// spans are untouched).
    pub fn expression_mut(&mut self, handle: ExpressionHandle) -> &mut ExpressionNode {
        self.expressions.get_mut(handle)
    }

    #[inline]
    pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.expressions.get(handle)
    }

    /// `handle` chased through every `Borrow` node — the expression the
    /// wrapper chain ultimately names.
    pub fn peeled_expression_handle(&self, mut handle: ExpressionHandle) -> ExpressionHandle {
        while let ExpressionNode::Borrow(inner) = self.expression(handle) {
            handle = inner.target;
        }
        handle
    }

    /// The node `handle`'s borrow chain ultimately names.
    pub fn peeled_expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.expression(self.peeled_expression_handle(handle))
    }

    /// The typed `Name` node, or `None` for every other kind.
    pub fn name_path_node(&self, handle: ExpressionHandle) -> Option<&TableNamePath> {
        match self.expression(handle) {
            ExpressionNode::Name(node) => Some(node),
            _ => None,
        }
    }

    /// The typed `Range` node, or `None` for every other kind.
    pub fn range_node(&self, handle: ExpressionHandle) -> Option<&TableRangeExpression> {
        match self.expression(handle) {
            ExpressionNode::Range(node) => Some(node),
            _ => None,
        }
    }

    /// The typed `Call` node, or `None` for every other kind.
    pub fn call_node(&self, handle: ExpressionHandle) -> Option<&TableCallExpression> {
        match self.expression(handle) {
            ExpressionNode::Call(node) => Some(node),
            _ => None,
        }
    }

    /// The typed `Member` node, or `None` for every other kind.
    pub fn member_node(&self, handle: ExpressionHandle) -> Option<&TableMemberExpression> {
        match self.expression(handle) {
            ExpressionNode::Member(node) => Some(node),
            _ => None,
        }
    }

    /// The typed `Indexed` node, or `None` for every other kind.
    pub fn indexed_node(&self, handle: ExpressionHandle) -> Option<&TableIndexedExpression> {
        match self.expression(handle) {
            ExpressionNode::Indexed(node) => Some(node),
            _ => None,
        }
    }

    /// Whether the indexed expression's index operand is a `Range` — a range
    /// denotes a window, not one element, which every index-semantics rule
    /// gates on identically.
    pub fn indexed_on_range(&self, indexed: &TableIndexedExpression) -> bool {
        matches!(self.expression(indexed.index), ExpressionNode::Range(_))
    }

    /// The typed `Cast` node, or `None` for every other kind.
    pub fn cast_node(&self, handle: ExpressionHandle) -> Option<&TableCastExpression> {
        match self.expression(handle) {
            ExpressionNode::Cast(node) => Some(node),
            _ => None,
        }
    }

    /// The typed binary application node, or `None` for every other kind.
    /// Recursive walks open with this check on every expression they visit.
    pub fn binary_node(&self, handle: ExpressionHandle) -> Option<&TableBinaryExpression> {
        match self.expression(handle) {
            ExpressionNode::Binary(binary) => Some(binary),
            _ => None,
        }
    }

    /// The typed `Boolean` node, or `None` for every other kind.
    pub fn boolean_node(&self, handle: ExpressionHandle) -> Option<&bool> {
        match self.expression(handle) {
            ExpressionNode::Boolean(node) => Some(node),
            _ => None,
        }
    }

    /// The typed `Match` node, or `None` for every other kind.
    pub fn match_node(&self, handle: ExpressionHandle) -> Option<&TableMatchExpression> {
        match self.expression(handle) {
            ExpressionNode::Match(node) => Some(node),
            _ => None,
        }
    }

    /// The typed array literal handle span, or `None` for every other kind.
    pub fn array_literal_node(
        &self,
        handle: ExpressionHandle,
    ) -> Option<&HandleSpan<ExpressionHandle>> {
        match self.expression(handle) {
            ExpressionNode::ArrayLiteral(node) => Some(node),
            _ => None,
        }
    }

    /// The typed `StructLiteral` node, or `None` for every other kind.
    pub fn struct_literal_node(&self, handle: ExpressionHandle) -> Option<&TableStructLiteral> {
        match self.expression(handle) {
            ExpressionNode::StructLiteral(node) => Some(node),
            _ => None,
        }
    }

    /// The typed `String` node, or `None` for every other kind.
    pub fn string_node(&self, handle: ExpressionHandle) -> Option<&Arc<[u8]>> {
        match self.expression(handle) {
            ExpressionNode::String(node) => Some(node),
            _ => None,
        }
    }

    /// The typed `Unary` node, or `None` for every other kind.
    pub fn unary_node(&self, handle: ExpressionHandle) -> Option<&TableUnaryExpression> {
        match self.expression(handle) {
            ExpressionNode::Unary(node) => Some(node),
            _ => None,
        }
    }

    /// The typed `Integer` node, or `None` for every other kind.
    pub fn integer_node(&self, handle: ExpressionHandle) -> Option<&IntegerLiteral> {
        match self.expression(handle) {
            ExpressionNode::Integer(node) => Some(node),
            _ => None,
        }
    }

    /// Whether an exact generational handle still names a live expression.
    /// Custody checks must distinguish a real zero literal from dummy storage.
    #[inline]
    pub fn expression_is_valid(&self, handle: ExpressionHandle) -> bool {
        self.expressions.is_valid(handle)
    }

    /// The depth-bounded lookup every recursive expression walk opens with:
    /// `None` at the 128-level recursion ceiling or on a dangling handle, so
    /// the caller's `match` only ever sees a live node.
    /// The `Borrow` node at `handle`, if the expression is a borrow.
    pub fn borrow_expression(&self, handle: ExpressionHandle) -> Option<&TableBorrowExpression> {
        let ExpressionNode::Borrow(borrow) = self.expression(handle) else {
            return None;
        };
        Some(borrow)
    }

    pub fn expression_at_depth(
        &self,
        handle: ExpressionHandle,
        depth: usize,
    ) -> Option<&ExpressionNode> {
        if depth >= 128 || !self.expression_is_valid(handle) {
            return None;
        }
        Some(self.expression(handle))
    }

    /// The receiver of an authored `.length` member access: the member spelling
    /// resolves to `CollectionMeasure::Length` and neither a compiled symbol nor
    /// a case variant is bound yet. `None` for every other expression.
    pub fn authored_length_receiver(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        use language_semantics::declaration_selection::CollectionMeasure;
        let member = self.member_node(expression)?;
        if CollectionMeasure::from_authored_spelling(member.member.as_str())
            != Some(CollectionMeasure::Length)
            || member.member_symbol.is_valid()
            || member.case_variant.is_some()
        {
            return None;
        }
        Some(member.receiver)
    }

    /// The name path's `::`-joined spelling for diagnostics and identity
    /// strings; the table's members stay the identity.
    pub fn name_path_string(&self, span: HandleSpan<Identifier>) -> String {
        self.name_path_members(span).joined_path()
    }

    pub fn iter_expressions(&self) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
        self.expression_roots
            .iter()
            .map(|(_, handle)| (*handle, self.expressions.get(*handle)))
    }

    /// Project iteration roots without rewriting any expression or inbound
    /// handle. The caller owns graph closure; newly inserted expressions join
    /// the resulting roster through the ordinary insertion operation.
    pub fn retain_expression_roots(&mut self, mut retain: impl FnMut(ExpressionHandle) -> bool) {
        let retained = self
            .expression_roots
            .iter()
            .map(|(_, handle)| *handle)
            .filter(|handle| retain(*handle))
            .collect::<Vec<_>>();
        self.expression_roots.reset_retain_capacity();
        self.expression_roots.insert_many(retained);
    }

    pub fn expression_is_direct_place_path(&self, handle: ExpressionHandle) -> bool {
        match self.expression(handle) {
            ExpressionNode::Name(_) => true,
            ExpressionNode::Member(member) => self.expression_is_direct_place_path(member.receiver),
            ExpressionNode::Borrow(inner) => self.expression_is_direct_place_path(inner.target),
            _ => false,
        }
    }

    #[inline]
    pub fn expression_handles(&self, span: HandleSpan<ExpressionHandle>) -> &[ExpressionHandle] {
        self.expression_handles.span_or_empty(span)
    }

    #[inline]
    pub fn struct_fields(
        &self,
        span: HandleSpan<TableStructLiteralField>,
    ) -> &[TableStructLiteralField] {
        self.struct_fields.span_or_empty(span)
    }

    #[inline]
    /// A name expression consisting of a single member, as `(path, member)` —
    /// the only shape that can denote one unqualified identifier.
    pub fn single_name_member(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(&TableNamePath, &Identifier)> {
        let path = self.name_path_node(expression)?;
        let [name] = self.name_path_members(path.members) else {
            return None;
        };
        Some((path, name))
    }

    pub fn name_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
        self.name_path_members.span_or_empty(span)
    }

    #[inline]
    pub fn name_path_member_symbols(&self, span: HandleSpan<SymbolHandle>) -> &[SymbolHandle] {
        self.name_path_member_symbols.span_or_empty(span)
    }

    /// Whether `path` spells a resolved one-member name: the head is the
    /// path's resolved symbol and no further members follow — the plain
    /// `x` spelling, not `x::y` or an unresolved name.
    pub fn name_path_is_single_member(&self, path: &TableNamePath) -> bool {
        path.symbol.is_valid()
            && path.head_symbol == path.symbol
            && self.name_path_members(path.members).len() == 1
    }

    /// Whether `expression` is the reserved `result` occurrence of its
    /// contract owner: a bare `result` that resolution left unbound. An entry
    /// parameter named `result` shadows the binder, and resolution binds the
    /// occurrence to that parameter instead; see `language_core::contract_result`.
    pub fn names_reserved_result(&self, expression: ExpressionHandle) -> bool {
        let Some(path) = self.name_path_node(expression) else {
            return false;
        };
        !path.symbol.is_valid()
            && !path.head_symbol.is_valid()
            && matches!(self.name_path_members(path.members), [name] if name.is_reserved_result())
            && self
                .name_path_member_symbols(path.member_symbols)
                .iter()
                .all(|symbol| !symbol.is_valid())
    }

    /// Whether `expression` is the exact bare occurrence of `parameter`: a
    /// single-member name path bound to that parameter's symbol, or spelled
    /// with its exact name when resolution left the occurrence unbound. A
    /// projection (`parameter.field`) and a qualified path are both longer
    /// than one member, so neither names the parameter itself. The zero symbol
    /// is absence, so an invalid parameter symbol never matches an unbound
    /// occurrence by handle — only the authored spelling can.
    pub fn names_parameter(
        &self,
        expression: ExpressionHandle,
        parameter: &crate::signature::StateParameter,
    ) -> bool {
        let Some(path) = self.name_path_node(expression) else {
            return false;
        };
        let [name] = self.name_path_members(path.members) else {
            return false;
        };
        (parameter.symbol.is_valid()
            && (path.symbol == parameter.symbol || path.head_symbol == parameter.symbol))
            || name.as_str() == parameter.name.as_str()
    }

    /// Whether two expressions in this table are structurally identical: same node
    /// shapes, literals, names/symbols, and operators, recursively. Lowering copies
    /// expression trees per use site, so a SHARED source expression (e.g. the subject
    /// of `transition self.f(x) { true -> a false -> b }`, which the parser inserts
    /// once and references from every arm's guard) arrives as several distinct
    /// handles; structural equality recognizes those copies so consumers can evaluate
    /// the source expression ONCE. Conservative: node kinds not listed compare as not
    /// equal.
    pub fn expressions_structurally_equal(&self, a: ExpressionHandle, b: ExpressionHandle) -> bool {
        if a == b {
            return true;
        }
        if !a.is_valid() || !b.is_valid() {
            return false;
        }
        match (self.expression(a), self.expression(b)) {
            (ExpressionNode::Match(x), ExpressionNode::Match(y)) => {
                let x_arms = self.match_arms(x.arms);
                let y_arms = self.match_arms(y.arms);
                self.expressions_structurally_equal(x.subject, y.subject)
                    && x_arms.len() == y_arms.len()
                    && x_arms.iter().zip(y_arms).all(|(x, y)| {
                        let patterns_equal = match (x.pattern, y.pattern) {
                            (MatchPattern::Value(x), MatchPattern::Value(y)) => {
                                self.expressions_structurally_equal(x, y)
                            }
                            (MatchPattern::Wildcard, MatchPattern::Wildcard) => true,
                            _ => false,
                        };
                        patterns_equal && self.expressions_structurally_equal(x.value, y.value)
                    })
            }
            (ExpressionNode::Integer(x), ExpressionNode::Integer(y)) => x == y,
            (ExpressionNode::Boolean(x), ExpressionNode::Boolean(y)) => x == y,
            (ExpressionNode::String(x), ExpressionNode::String(y)) => x == y,
            (ExpressionNode::Float(x), ExpressionNode::Float(y)) => x == y,
            (ExpressionNode::UndeterminedNaN(left), ExpressionNode::UndeterminedNaN(right)) => {
                left == right
            }
            (ExpressionNode::Name(x), ExpressionNode::Name(y)) => {
                x.head_symbol == y.head_symbol
                    && x.symbol == y.symbol
                    && identifier_texts_equal(
                        self.name_path_members(x.members),
                        self.name_path_members(y.members),
                    )
            }
            (ExpressionNode::Member(x), ExpressionNode::Member(y)) => {
                x.member_symbol == y.member_symbol
                    && x.member.as_str() == y.member.as_str()
                    && self.expressions_structurally_equal(x.receiver, y.receiver)
            }
            (ExpressionNode::Borrow(x), ExpressionNode::Borrow(y)) => {
                x.access == y.access && self.expressions_structurally_equal(x.target, y.target)
            }
            (ExpressionNode::Unary(x), ExpressionNode::Unary(y)) => {
                x.operator == y.operator
                    && self.expressions_structurally_equal(x.operand, y.operand)
            }
            (ExpressionNode::Binary(x), ExpressionNode::Binary(y)) => {
                x.operator == y.operator
                    && self.expressions_structurally_equal(x.left, y.left)
                    && self.expressions_structurally_equal(x.right, y.right)
            }
            (ExpressionNode::Indexed(x), ExpressionNode::Indexed(y)) => {
                self.expressions_structurally_equal(x.collection, y.collection)
                    && self.expressions_structurally_equal(x.index, y.index)
            }
            (ExpressionNode::Range(x), ExpressionNode::Range(y)) => {
                x.end_inclusive == y.end_inclusive
                    && self.optional_expression_structurally_equal(x.start, y.start)
                    && self.optional_expression_structurally_equal(x.end, y.end)
            }
            (ExpressionNode::Call(x), ExpressionNode::Call(y)) => {
                x.target_symbol == y.target_symbol
                    && x.exact_coercion_target == y.exact_coercion_target
                    && x.target.as_str() == y.target.as_str()
                    && self.expressions_structurally_equal(x.receiver, y.receiver)
                    && self.expression_spans_structurally_equal(x.arguments, y.arguments)
            }
            _ => false,
        }
    }

    /// A range bound that is absent on both sides denotes the same implicit
    /// endpoint (element zero for `start`, the collection's length for `end`);
    /// absent on one side it cannot be proved equal to a spelled bound.
    fn optional_expression_structurally_equal(
        &self,
        a: ExpressionHandle,
        b: ExpressionHandle,
    ) -> bool {
        if a.is_valid() != b.is_valid() {
            return false;
        }
        !a.is_valid() || self.expressions_structurally_equal(a, b)
    }

    fn expression_spans_structurally_equal(
        &self,
        a: HandleSpan<ExpressionHandle>,
        b: HandleSpan<ExpressionHandle>,
    ) -> bool {
        let a = self.expression_handles(a);
        let b = self.expression_handles(b);
        a.len() == b.len()
            && a.iter()
                .zip(b.iter())
                .all(|(x, y)| self.expressions_structurally_equal(*x, *y))
    }

    pub fn expression_count(&self) -> usize {
        self.expressions.len()
    }

    pub fn expression_nodes(&self) -> impl Iterator<Item = &ExpressionNode> {
        self.iter_expressions().map(|(_, node)| node)
    }

    /// Semantic iteration follows the current program's expression roster.
    /// Backing nodes can still belong to retained declaration descriptions;
    /// their exact handles remain readable without becoming executable roots.
    pub fn expression_entries(&self) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
        self.iter_expressions()
    }

    pub fn struct_field_count(&self) -> usize {
        self.struct_fields.len()
    }

    pub fn display_name(&self, handle: ExpressionHandle) -> String {
        let mut rendered = String::new();
        self.write_display_name(handle, &mut rendered);
        rendered
    }

    /// Appends `handle`'s display name to `out`; see
    /// `ExpressionNode::write_display_name`.
    pub fn write_display_name(&self, handle: ExpressionHandle, out: &mut String) {
        self.expression(handle).write_display_name(self, out);
    }

    /// Const-evaluate a PURE integer expression: a literal, or checked
    /// `+ - * / %` over constant integer subtrees (peeling `Mutable`). `None`
    /// the moment any leaf is not a constant integer (a place read, a call, a
    /// float) or an operation overflows / divides by zero -- callers treat
    /// `None` strictly as "not a constant", never as a value. The first
    /// consumer is range-constraint bounds (`[0 - 1..=40]` folds to
    /// `[-1..=40]` instead of silently behaving unbounded).
    pub fn constant_integer_value(&self, handle: ExpressionHandle) -> Option<i64> {
        if !handle.is_valid() {
            return None;
        }
        match self.expression(handle) {
            // The i64 window is this helper's CONTRACT (its consumers are
            // parse-time-numeric positions like range bounds); an oversize
            // literal reads as "not a constant here", which its callers
            // already reject loudly.
            ExpressionNode::Integer(value) => value.value_i64(),
            ExpressionNode::Borrow(inner) => self.constant_integer_value(inner.target),
            ExpressionNode::Binary(binary) => {
                let left = self.constant_integer_value(binary.left)?;
                let right = self.constant_integer_value(binary.right)?;
                match binary.operator {
                    BinaryOperator::Add => left.checked_add(right),
                    BinaryOperator::Subtract => left.checked_sub(right),
                    BinaryOperator::Multiply => left.checked_mul(right),
                    BinaryOperator::Divide => left.checked_div(right),
                    BinaryOperator::Modulo => left.checked_rem(right),
                    _ => None,
                }
            }
            _ => None,
        }
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
    /// Internal semantic value; no runtime representation or source spelling is implied.
    UndeterminedNaN(numerics::literals::FloatFormat),
    Indexed(TableIndexedExpression),
    Integer(IntegerLiteral),
    Member(TableMemberExpression),
    Borrow(TableBorrowExpression),
    Name(TableNamePath),
    Range(TableRangeExpression),
    StructLiteral(TableStructLiteral),
    /// Exact decoded literal octets. Text interpretation, when required by a
    /// particular language construct, is an explicit checked operation.
    String(Arc<[u8]>),
    Unary(TableUnaryExpression),
    /// Proof-only observation of a type's normalized all-zero home value.
    ZeroValue(crate::types::TypeReferenceHandle),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryExpression {
    pub operator: UnaryOperator,
    pub operand: Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableUnaryExpression {
    pub operator: UnaryOperator,
    pub operand: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableCastExpression {
    pub value: ExpressionHandle,
    pub target_type: crate::types::TypeReferenceHandle,
    /// Complete result qualification, retained by typed normalization. The
    /// authored target remains separate because membership must still be
    /// proved. Zero means unresolved (or an unnormalized internal builder),
    /// never permission to erase a semantic-domain suffix.
    pub result_type: crate::types::TypeReferenceHandle,
    /// Diagnostic spelling only; semantic identity uses `target_type`.
    pub target_label: HandleSpan<Identifier>,
    /// Arithmetic domain cast (`x as u8 in Saturating`), decision 17 S2.
    pub domain: numerics::arithmetic::ArithmeticDomain,
    /// A NON-policy `in <Name>` suffix -- the semantic-domain qualification
    /// spelling (decision 19), judged at validation (the staged mint fence).
    /// EMPTY = no suffix.
    pub semantic_domain: HandleSpan<Identifier>,
    /// PDI2 proof-static family arguments in the typed type-reference table.
    pub semantic_domain_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// Carrier-aware declared-domain identity, normalized once before
    /// validation. Invalid for an unknown or ambiguous spelling.
    pub semantic_domain_symbol: SymbolHandle,
    /// Exact normalized family-instance identity. Equal to the declaration ID
    /// for a monomorphic domain and distinct per canonical closed index pack.
    pub semantic_domain_id: language_semantics::SemanticDomainId,
    /// Value conversion vs §5b borrow recast (`&x as &T`). Only `Value`
    /// survives past the typed trees today: the resolved->typed lowering is
    /// the recast judgment's choke point (rung A).
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeExpression {
    pub start: Option<Box<Expression>>,
    pub end: Option<Box<Expression>>,
    pub end_inclusive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberExpression {
    pub receiver: Expression,
    pub member_symbol: SymbolHandle,
    pub member: Identifier,
    /// Case variant a destructure-bound payload field came from, so the backend
    /// offset resolver picks THAT variant's field (decision 17 / case payloads).
    pub case_variant: Option<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub member_symbol: SymbolHandle,
    pub member: Identifier,
    /// Case variant a destructure-bound payload field came from, so the backend
    /// offset resolver picks THAT variant's field (decision 17 / case payloads).
    pub case_variant: Option<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallExpression {
    pub exact_coercion_target: crate::types::TypeReferenceHandle,
    pub receiver: Option<Box<Expression>>,
    pub target_symbol: SymbolHandle,
    pub target: Identifier,
    pub arguments: Arc<[Expression]>,
    pub evidence_arguments: Arc<[Identifier]>,
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCallExpression {
    /// Zero until specialization erases this call's authored static operands.
    pub authored_machine_application: StaticMachineApplicationHandle,
    /// An Exact carrier coercion elaborated to this conversion requirement.
    /// The zero handle denotes an ordinary named call. Keep the authored
    /// target until checking discharges unchanged denotation from this
    /// occurrence's live facts; the selected named conversion alone may round.
    pub exact_coercion_target: crate::types::TypeReferenceHandle,
    pub receiver: ExpressionHandle,
    pub target_symbol: SymbolHandle,
    /// Original static binder contract, retained separately from execution.
    pub static_machine_parameter: SymbolHandle,
    pub target: Identifier,
    /// Public requirement identity plus private closed realization retained
    /// when static conformance dispatch rewrites `target_symbol`.
    pub static_requirement_dispatch: Option<crate::typed_trees::StaticRequirementDispatch>,
    pub machine_arguments: Box<[StaticMachineArgument]>,
    /// An explicitly authored sealed quotient operation request. Retention is
    /// not admission: semantic validation must independently check quotient
    /// formation, operation correspondence, and the named conformance before
    /// this request can become executable.
    pub quotient_operation: Option<QuotientOperationRequest>,
    /// Exact compiler-known `Plan::place_private<Slot>` request. The selected
    /// conformance is proof-static identity; ordinary generic dispatch never
    /// sees it and source code never receives the resulting private place.
    pub private_layout_operation: Option<PrivateLayoutOperationRequest>,
    /// Exact compiler-known `size_of<T>`/`align_of<T>` request. The operand's
    /// lowered static argument keeps the queried type's authored selection
    /// custody. The answer needs the selected target's validated layout, so
    /// the request stays a portable query through checking and consumes the
    /// exact selected layout after the Psi boundary; typed source is never
    /// rewritten into a target-dependent literal.
    pub layout_query: Option<DataLayoutQueryRequest>,
    pub arguments: HandleSpan<ExpressionHandle>,
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
}

/// A factory selection, not an address, resource profile or custody grant.
/// Checking must join the applied signature and exact source borrow before
/// this declaration can become a placed-reference establishment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedBorrowOperationRequest {
    pub factory: SymbolHandle,
    pub access: language_core::ReferenceAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateLayoutOperationRequest {
    pub selected_slot: StaticMachineArgument,
}

/// An authored `size_of<T>`/`align_of<T>` query over the selected target's
/// validated layout of `operand`. The operand retains its complete static
/// argument (type reference, path and resolved symbol) so the portable query
/// can rejoin the exact authored selection instead of re-parsing a name.
/// Evaluation answers the query from the selected target's materialized
/// layout — an observation naming its subject from target closure, never a
/// compiler-host fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayoutQueryRequest {
    pub kind: language_core::data_layout_queries::DataLayoutQueryKind,
    pub operand: StaticMachineArgument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotientOperationKind {
    Lift,
    Define,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuotientTheoremRole {
    Congruence,
    ForwardPreconditionTransport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientTheoremSelection {
    pub role: QuotientTheoremRole,
    /// One exact selected resultless theorem-machine application, never a
    /// conformance or structurally discovered proof.
    pub application: StaticMachineArgument,
}

/// Exact source-selected identities for the sealed quotient operation forms.
/// The role-ordered theorem roster is retained before any derived quotient
/// admission and grants no executable lowering authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientOperationRequest {
    pub kind: QuotientOperationKind,
    pub representative_operation: StaticMachineArgument,
    pub theorem_evidence: Box<[QuotientTheoremSelection]>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticMachineArgument {
    /// An explicit structural type argument; zero denotes another static argument kind.
    pub type_reference: crate::types::TypeReferenceHandle,
    /// Historical storage name shared by type/const/machine proposition
    /// arguments; proposition proof facts retain their final category.
    pub path: Box<[Identifier]>,
    pub application: Option<Box<StaticSymbolApplication>>,
    pub const_literal: Option<numerics::literals::IntegerLiteral>,
    /// Proof-static projection from one named evidence term. The checked proof
    /// layer binds it to one stable opaque member of that retained term.
    pub evidence_projection: Option<EvidenceProjection>,
    /// Entry-state symbol of the selected concrete machine.
    pub symbol: SymbolHandle,
}

impl StaticMachineArgument {
    /// Remap every nested lexical operand. The generic declaration governing
    /// an enclosing retained application is not part of this operand map.
    pub fn remap_symbols(&mut self, symbols: &[(SymbolHandle, SymbolHandle)]) {
        if let Some((_, replacement)) = symbols.iter().find(|(source, _)| *source == self.symbol) {
            self.symbol = *replacement;
        }
        if let Some(application) = &mut self.application {
            for argument in application.arguments.iter_mut() {
                argument.remap_symbols(symbols);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticSymbolApplication {
    pub lifetime_arguments: Box<[Identifier]>,
    pub arguments: Box<[StaticMachineArgument]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceProjection {
    pub term: Identifier,
    pub member: Identifier,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableNamePath {
    pub members: HandleSpan<Identifier>,
    pub member_symbols: HandleSpan<SymbolHandle>,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
}

impl TableNamePath {
    /// The name path spelling a resolved name: ordered members with their
    /// per-member resolution symbols, the head symbol, and the symbol the
    /// whole path resolves to.
    pub const fn new(
        members: HandleSpan<Identifier>,
        member_symbols: HandleSpan<SymbolHandle>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        Self {
            members,
            member_symbols,
            head_symbol,
            symbol,
        }
    }

    /// The symbol this path resolves to: the resolution-authoritative head
    /// symbol when the name resolved, else the whole-path symbol.
    pub fn resolved_symbol(&self) -> Option<SymbolHandle> {
        self.head_symbol
            .is_valid()
            .then_some(self.head_symbol)
            .or_else(|| self.symbol.is_valid().then_some(self.symbol))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableStructLiteral {
    pub type_name: Identifier,
    pub type_symbol: SymbolHandle,
    /// `Some` when the literal constructs a CASE of `type_name`
    /// (`Command::Say { text: ... }`); `None` for a plain record literal.
    pub case_name: Option<Identifier>,
    pub case_symbol: Option<SymbolHandle>,
    pub fields: HandleSpan<TableStructLiteralField>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableStructLiteralField {
    pub name: Identifier,
    pub field_symbol: SymbolHandle,
    pub value: ExpressionHandle,
}

impl Default for Expression {
    fn default() -> Self {
        Self::Integer(IntegerLiteral::zero())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamePath {
    members: Arc<[Identifier]>,
    member_symbols: Arc<[SymbolHandle]>,
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
}

impl NamePath {
    pub fn unresolved(members: Vec<Identifier>) -> Self {
        let member_symbols = vec![SymbolHandle::invalid(); members.len()];
        Self {
            members: Arc::from(members.into_boxed_slice()),
            member_symbols: Arc::from(member_symbols.into_boxed_slice()),
            head_symbol: SymbolHandle::invalid(),
            symbol: SymbolHandle::invalid(),
        }
    }

    pub fn unresolved_from_iter(members: impl IntoIterator<Item = Identifier>) -> Self {
        Self::unresolved(members.into_iter().collect())
    }

    pub fn resolved(
        members: Vec<Identifier>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        let member_symbols = vec![SymbolHandle::invalid(); members.len()];
        Self::resolved_with_member_symbols(members, member_symbols, head_symbol, symbol)
    }

    pub fn resolved_with_member_symbols(
        members: Vec<Identifier>,
        mut member_symbols: Vec<SymbolHandle>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        if member_symbols.len() != members.len() {
            member_symbols.resize(members.len(), SymbolHandle::invalid());
        }
        if let Some(first_symbol) = member_symbols.first_mut() {
            *first_symbol = head_symbol;
        }
        if let Some(last_symbol) = member_symbols.last_mut() {
            *last_symbol = symbol;
        }
        Self {
            members: Arc::from(members.into_boxed_slice()),
            member_symbols: Arc::from(member_symbols.into_boxed_slice()),
            head_symbol,
            symbol,
        }
    }

    pub fn resolved_from_iter(
        members: impl IntoIterator<Item = Identifier>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        Self::resolved(members.into_iter().collect(), head_symbol, symbol)
    }

    pub fn members(&self) -> &[Identifier] {
        &self.members
    }

    pub fn member_symbols(&self) -> &[SymbolHandle] {
        &self.member_symbols
    }

    pub fn member_symbol(&self, index: usize) -> SymbolHandle {
        self.member_symbols
            .get(index)
            .copied()
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn first(&self) -> Option<&Identifier> {
        self.members.first()
    }

    pub fn last(&self) -> Option<&Identifier> {
        self.members.last()
    }

    pub fn last_mut(&mut self) -> Option<&mut Identifier> {
        self.symbol = SymbolHandle::invalid();
        if let Some(last_symbol) = Arc::make_mut(&mut self.member_symbols).last_mut() {
            *last_symbol = SymbolHandle::invalid();
        }
        Arc::make_mut(&mut self.members).last_mut()
    }

    pub fn replace_last_preserving_symbol(&mut self, member: Identifier) -> Option<()> {
        let last = Arc::make_mut(&mut self.members).last_mut()?;
        *last = member;
        Some(())
    }

    pub fn push(&mut self, member: Identifier) {
        let mut members = self.members.iter().cloned().collect::<Vec<_>>();
        members.push(member);
        self.members = Arc::from(members.into_boxed_slice());

        let mut member_symbols = self.member_symbols.iter().copied().collect::<Vec<_>>();
        member_symbols.push(SymbolHandle::invalid());
        self.member_symbols = Arc::from(member_symbols.into_boxed_slice());
        self.symbol = SymbolHandle::invalid();
    }

    pub fn push_resolved(&mut self, member: Identifier, symbol: SymbolHandle) {
        let mut members = self.members.iter().cloned().collect::<Vec<_>>();
        members.push(member);
        self.members = Arc::from(members.into_boxed_slice());

        let mut member_symbols = self.member_symbols.iter().copied().collect::<Vec<_>>();
        member_symbols.push(symbol);
        self.member_symbols = Arc::from(member_symbols.into_boxed_slice());
        self.symbol = symbol;
    }

    pub fn extend_from_slice(&mut self, members: &[Identifier]) {
        let mut path = self.members.iter().cloned().collect::<Vec<_>>();
        path.extend_from_slice(members);
        self.members = Arc::from(path.into_boxed_slice());

        let mut member_symbols = self.member_symbols.iter().copied().collect::<Vec<_>>();
        member_symbols.extend(std::iter::repeat_with(SymbolHandle::invalid).take(members.len()));
        self.member_symbols = Arc::from(member_symbols.into_boxed_slice());
        self.symbol = SymbolHandle::invalid();
    }

    pub fn head_symbol(&self) -> SymbolHandle {
        self.head_symbol
    }

    pub fn symbol(&self) -> SymbolHandle {
        self.symbol
    }
}

impl Deref for NamePath {
    type Target = [Identifier];

    fn deref(&self) -> &Self::Target {
        self.members()
    }
}

impl<'path> IntoIterator for &'path NamePath {
    type Item = &'path Identifier;
    type IntoIter = std::slice::Iter<'path, Identifier>;

    fn into_iter(self) -> Self::IntoIter {
        self.members.iter()
    }
}

/// The shared TEXT-based float carrier (F2): the source spelling plus an
/// optional format landing ride every tree layer, exactly like
/// IntegerLiteral -- per-format reads are each correctly rounded from the
/// spelling, so f32 never routes through f64.
pub use numerics::literals::FloatLiteral;

fn identifier_texts_equal(a: &[Identifier], b: &[Identifier]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(left, right)| left.as_str() == right.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpression {
    pub left: Expression,
    pub operator: BinaryOperator,
    pub right: Expression,
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
    /// Observe the subject's case tag, not value equality or construction.
    /// Structural equality synthesis uses this operation because generated tag
    /// tests have no authored membership occurrence. The right operand names
    /// an exact case; consumers must validate its carrier against the subject.
    CaseMembership,
}

impl BinaryOperator {
    /// `left <self> right` is one of the ten arithmetic and bitwise
    /// operators — everything but the two logicals, the six comparisons
    /// and `CaseMembership`.
    pub fn is_arithmetic(self) -> bool {
        matches!(
            self,
            BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
        )
    }

    /// `left <self> right` is one of the six ordering comparisons — the set
    /// `evaluate_comparison`, `mirrored_comparison` and `complement` serve.
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
        )
    }

    /// `left <self> right` evaluated as an ordering comparison; `None` when
    /// the operator is not one of the six comparisons.
    pub fn evaluate_comparison<T: Ord>(self, left: &T, right: &T) -> Option<bool> {
        Some(match self {
            BinaryOperator::Equal => left == right,
            BinaryOperator::NotEqual => left != right,
            BinaryOperator::Less => left < right,
            BinaryOperator::LessOrEqual => left <= right,
            BinaryOperator::Greater => left > right,
            BinaryOperator::GreaterOrEqual => left >= right,
            _ => return None,
        })
    }

    /// [`Self::mirrored`] when the operator is one of the six comparisons;
    /// `None` otherwise — a non-comparison cannot have its operands swapped.
    pub fn mirrored_comparison(self) -> Option<BinaryOperator> {
        self.complement().map(|_| self.mirrored())
    }

    /// This operator with its operands swapped — `a < b` mirrors to `b > a`.
    /// Every other operator mirrors to itself.
    pub fn mirrored(self) -> BinaryOperator {
        match self {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            other => other,
        }
    }

    /// The comparison expressing this operator's negation, when one exists.
    pub fn complement(self) -> Option<BinaryOperator> {
        match self {
            BinaryOperator::Equal => Some(BinaryOperator::NotEqual),
            BinaryOperator::NotEqual => Some(BinaryOperator::Equal),
            BinaryOperator::Less => Some(BinaryOperator::GreaterOrEqual),
            BinaryOperator::LessOrEqual => Some(BinaryOperator::Greater),
            BinaryOperator::Greater => Some(BinaryOperator::LessOrEqual),
            BinaryOperator::GreaterOrEqual => Some(BinaryOperator::Less),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    BitwiseNot,
    LogicalNot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastExpression {
    pub value: Expression,
    pub target_type: crate::types::TypeReferenceHandle,
    /// Diagnostic spelling only; semantic identity uses `target_type`.
    pub target_label: NamePath,
    /// Arithmetic domain cast (`x as u8 in Saturating`), decision 17 S2.
    pub domain: numerics::arithmetic::ArithmeticDomain,
    /// Value conversion vs §5b borrow recast (`&x as &T`).
    pub form: language_core::cast_form::CastForm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedExpression {
    pub collection: Expression,
    pub index: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteral {
    pub type_name: Identifier,
    /// `Some` when the literal constructs a CASE of `type_name`; `None` for a
    /// plain record literal.
    pub case_name: Option<Identifier>,
    pub fields: Arc<[StructLiteralField]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteralField {
    pub name: Identifier,
    pub value: Expression,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchExpression {
    pub subject: Expression,
    pub arms: Arc<[MatchArm]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: OwnedMatchPattern,
    pub value: Expression,
    pub source_span: SourceSpan,
}

/// The leaf-literal `ExpressionNode` variant set that terminates recursive
/// expression walks — every walk leaves literals untouched.
/// Executable leaf expressions — every literal plus `Name`, but not
/// `ZeroValue`, whose presence is a proof-only observation.
#[macro_export]
macro_rules! executable_leaf_expression_pattern {
    () => {
        $crate::expression::ExpressionNode::Boolean(_)
            | $crate::expression::ExpressionNode::Float(_)
            | $crate::expression::ExpressionNode::UndeterminedNaN(_)
            | $crate::expression::ExpressionNode::Integer(_)
            | $crate::expression::ExpressionNode::Name(_)
            | $crate::expression::ExpressionNode::String(_)
    };
}

/// The spelled literal leaves only — no `Name` path; walks folding a `Name`
/// arm separately end on this set.
#[macro_export]
macro_rules! spelled_literal_expression_pattern {
    () => {
        $crate::expression::ExpressionNode::Boolean(_)
            | $crate::expression::ExpressionNode::Float(_)
            | $crate::expression::ExpressionNode::UndeterminedNaN(_)
            | $crate::expression::ExpressionNode::Integer(_)
            | $crate::expression::ExpressionNode::String(_)
            | $crate::expression::ExpressionNode::ZeroValue(_)
    };
}

#[macro_export]
macro_rules! literal_expression_pattern {
    () => {
        $crate::expression::ExpressionNode::Boolean(_)
            | $crate::expression::ExpressionNode::Float(_)
            | $crate::expression::ExpressionNode::UndeterminedNaN(_)
            | $crate::expression::ExpressionNode::Integer(_)
            | $crate::expression::ExpressionNode::Name(_)
            | $crate::expression::ExpressionNode::String(_)
            | $crate::expression::ExpressionNode::ZeroValue(_)
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedMatchPattern {
    Value(Expression),
    Wildcard,
}

/// The runtime IEEE interchange value of a float literal landed at `format`:
/// its bit payload read through that format's encoding, never re-decoded.
pub fn ieee_float_value(
    literal: &numerics::literals::FloatLiteral,
    format: numerics::literals::FloatFormat,
) -> semantic_vocabulary::IeeeFloatValue {
    match format {
        numerics::literals::FloatFormat::F32 => {
            semantic_vocabulary::IeeeFloatValue::Binary32(literal.f32_bits())
        }
        numerics::literals::FloatFormat::F64 => {
            semantic_vocabulary::IeeeFloatValue::Binary64(literal.value_f64().to_bits())
        }
    }
}
