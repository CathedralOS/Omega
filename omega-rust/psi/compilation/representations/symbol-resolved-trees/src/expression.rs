//! Resolved expression nodes and their owning table. Copies across tables and symbols live beside these records.
pub(crate) mod display;
pub(crate) mod static_members;
pub(crate) mod symbol_copies;
pub(crate) mod table_copies;
pub(crate) use table_copies::ExpressionSource;
#[cfg(test)]
pub(crate) mod tests;

pub use display::display_name_path;
pub use numerics::literals::FloatLiteral;

use crate::AuthoredDeclarationSelectionOccurrenceId;
use crate::name::DiagnosticName;
use arena::{Arena, Handle, HandleSpan};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, CompilerDerivedSelectionPartition,
};
use numerics::literals::IntegerLiteral;
use source::SourceSpan;
use symbols::SymbolHandle;

pub type ExpressionHandle = Handle<ExpressionNode>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
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

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ExpressionTable {
    nodes: ExpressionNodeStorage,
    spans: ExpressionSpanStorage,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
struct ExpressionNodeStorage {
    expressions: Arena<ExpressionNode>,
    source_spans: Vec<SourceSpan>,
    unresolved_names: Vec<symbols::UnresolvedName>,
    materialization_sources: Vec<ExpressionHandle>,

    authored_expression_exposures: Vec<Option<AuthoredDeclarationSelectionExposure>>,
    compiler_selection_partitions: Vec<Option<CompilerDerivedSelectionPartition>>,
    authored_selection_occurrences: Vec<HandleSpan<StoredAuthoredSelectionOccurrenceId>>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
struct ExpressionSpanStorage {
    expression_handles: Arena<ExpressionHandle>,
    name_path_members: Arena<DiagnosticName>,
    name_path_member_symbols: Arena<SymbolHandle>,
    struct_fields: Arena<TableStructLiteralField>,
    match_arms: Arena<TableMatchArm>,
    match_fields: Arena<TableMatchField>,
    match_subjects: Arena<MatchSubjectCapture>,
    static_members: Arena<StaticMachineArgument>,
    match_proof_selectors: Arena<crate::statement::OutcomeProofSelector>,
    authored_selection_occurrence_ids: Arena<StoredAuthoredSelectionOccurrenceId>,
}

pub(crate) struct PreparedStaticArgumentRebase {
    arguments: Box<[StaticMachineArgument]>,
    members: Vec<(Handle<StaticMachineArgument>, StaticMachineArgument)>,
}

pub(crate) struct PreparedExpressionSelectionRebase {
    remapped: Vec<Option<Vec<AuthoredDeclarationSelectionOccurrenceId>>>,
    arguments: Vec<(
        ExpressionHandle,
        Option<Handle<StaticMachineArgument>>,
        PreparedStaticArgumentRebase,
    )>,
}

macro_rules! expression_node_accessors {
($($(#[$doc:meta])* $name:ident($variant:ident) -> $node_type:ty;)*) => {
    $(
        $(#[$doc])*
        pub fn $name(&self, handle: ExpressionHandle) -> Option<&$node_type> {
            match self.expression(handle) {
                ExpressionNode::$variant(node) => Some(node),
                _ => None,
            }
        }
    )*
};
}

impl ExpressionTable {
    pub fn insert_static_member(
        &mut self,
        member: StaticMachineArgument,
    ) -> Handle<StaticMachineArgument> {
        self.spans.static_members.insert(member)
    }

    pub fn static_member(
        &self,
        member: Handle<StaticMachineArgument>,
    ) -> Option<&StaticMachineArgument> {
        self.spans
            .static_members
            .is_valid(member)
            .then(|| self.spans.static_members.get(member))
    }

    pub fn static_member_mut(
        &mut self,
        member: Handle<StaticMachineArgument>,
    ) -> Option<&mut StaticMachineArgument> {
        if self.spans.static_members.is_valid(member) {
            Some(self.spans.static_members.get_mut(member))
        } else {
            None
        }
    }

    pub fn insert_match_arms(
        &mut self,
        arms: impl IntoIterator<Item = TableMatchArm>,
    ) -> HandleSpan<TableMatchArm> {
        self.spans.match_arms.insert_many(arms)
    }

    pub fn match_arms(&self, arms: HandleSpan<TableMatchArm>) -> &[TableMatchArm] {
        self.spans.match_arms.span_or_empty(arms)
    }

    pub fn match_arm_mut(&mut self, arm: Handle<TableMatchArm>) -> &mut TableMatchArm {
        self.spans.match_arms.get_mut(arm)
    }

    pub fn insert_match_fields(
        &mut self,
        fields: impl IntoIterator<Item = TableMatchField>,
    ) -> HandleSpan<TableMatchField> {
        self.spans.match_fields.insert_many(fields)
    }

    pub fn match_fields(&self, fields: HandleSpan<TableMatchField>) -> &[TableMatchField] {
        self.spans.match_fields.span_or_empty(fields)
    }

    pub fn match_pattern_dependencies(
        &self,
        pattern: MatchPattern,
    ) -> impl Iterator<Item = ExpressionHandle> + '_ {
        // Resolution combines field equalities and the authored guard into
        // condition. Traverse that graph once, not its retained source fields too.
        let roots = match pattern {
            MatchPattern::Value(value) => [value, ExpressionHandle::invalid()],
            MatchPattern::Destructure(pattern) => [pattern.head, pattern.condition],
            MatchPattern::Wildcard => [ExpressionHandle::invalid(); 2],
        };
        roots.into_iter().filter(|root| root.is_valid())
    }

    pub fn match_field_mut(&mut self, field: Handle<TableMatchField>) -> &mut TableMatchField {
        self.spans.match_fields.get_mut(field)
    }

    pub fn insert_match_subject(
        &mut self,
        subject: ExpressionHandle,
    ) -> Handle<MatchSubjectCapture> {
        self.spans.match_subjects.insert(MatchSubjectCapture {
            purpose: SubjectCapturePurpose::Match,
            subject,
        })
    }

    pub fn insert_transition_subject(
        &mut self,
        subject: ExpressionHandle,
    ) -> Handle<MatchSubjectCapture> {
        self.spans.match_subjects.insert(MatchSubjectCapture {
            purpose: SubjectCapturePurpose::Transition,
            subject,
        })
    }

    pub fn match_subject(&self, capture: Handle<MatchSubjectCapture>) -> &MatchSubjectCapture {
        self.spans.match_subjects.get(capture)
    }

    pub fn insert_match_proof_selectors(
        &mut self,
        selectors: impl IntoIterator<Item = crate::statement::OutcomeProofSelector>,
    ) -> HandleSpan<crate::statement::OutcomeProofSelector> {
        self.spans.match_proof_selectors.insert_many(selectors)
    }

    pub fn match_proof_selectors(
        &self,
        selectors: HandleSpan<crate::statement::OutcomeProofSelector>,
    ) -> &[crate::statement::OutcomeProofSelector] {
        self.spans.match_proof_selectors.span_or_empty(selectors)
    }

    pub fn new() -> Self {
        Self {
            nodes: ExpressionNodeStorage {
                expressions: Arena::new(),
                source_spans: Vec::new(),
                unresolved_names: Vec::new(),
                materialization_sources: Vec::new(),
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
                match_fields: Arena::new(),
                match_subjects: Arena::new(),
                static_members: Arena::new(),
                match_proof_selectors: Arena::new(),
                authored_selection_occurrence_ids: Arena::new(),
            },
        }
    }

    pub fn clear(&mut self) {
        self.nodes.expressions.retire_all_retain_capacity();
        self.nodes.source_spans.clear();
        self.nodes.unresolved_names.clear();
        self.nodes.materialization_sources.clear();
        self.nodes.authored_expression_exposures.clear();
        self.nodes.compiler_selection_partitions.clear();
        self.nodes.authored_selection_occurrences.clear();
        self.spans.expression_handles.retire_all_retain_capacity();
        self.spans.name_path_members.retire_all_retain_capacity();
        self.spans
            .name_path_member_symbols
            .retire_all_retain_capacity();
        self.spans.struct_fields.retire_all_retain_capacity();
        self.spans.match_arms.retire_all_retain_capacity();
        self.spans.match_fields.retire_all_retain_capacity();
        self.spans.match_subjects.retire_all_retain_capacity();
        self.spans.static_members.retire_all_retain_capacity();
        self.spans
            .match_proof_selectors
            .retire_all_retain_capacity();
        self.spans
            .authored_selection_occurrence_ids
            .retire_all_retain_capacity();
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        let handle = self.nodes.expressions.insert(expression);
        self.nodes.source_spans.push(SourceSpan::default());
        self.nodes.unresolved_names.push(Default::default());
        self.nodes
            .materialization_sources
            .push(ExpressionHandle::invalid());
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

    pub fn materialization_source(&self, handle: ExpressionHandle) -> ExpressionHandle {
        self.nodes
            .materialization_sources
            .get(source_span_index(handle))
            .copied()
            .unwrap_or_default()
    }

    pub fn retain_materialization_source(
        &mut self,
        value: ExpressionHandle,
        authored: ExpressionHandle,
    ) {
        assert!(value.is_valid() && authored.is_valid() && value != authored);
        assert!(!self.materialization_source(value).is_valid());
        self.nodes.materialization_sources[source_span_index(value)] = authored;
    }

    pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
        self.nodes.source_spans[source_span_index(handle)]
    }

    /// Resolution's outcome for this exact occurrence, not another source's name.
    pub fn unresolved_name(&self, handle: ExpressionHandle) -> symbols::UnresolvedName {
        if !self.nodes.expressions.is_valid(handle) {
            return Default::default();
        }
        self.nodes.unresolved_names[source_span_index(handle)]
    }

    pub fn set_unresolved_name(
        &mut self,
        handle: ExpressionHandle,
        outcome: symbols::UnresolvedName,
    ) {
        self.nodes.unresolved_names[source_span_index(handle)] = outcome;
    }

    pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
        self.nodes.source_spans[source_span_index(handle)] = source_span;
    }

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

    pub fn attach_authored_selection_occurrences(
        &mut self,
        handle: ExpressionHandle,
        occurrences: impl IntoIterator<Item = AuthoredDeclarationSelectionOccurrenceId>,
    ) {
        let mut combined = self
            .authored_selection_occurrences(handle)
            .collect::<Vec<_>>();

        for occurrence in occurrences {
            if !combined.contains(&occurrence) {
                combined.push(occurrence);
            }
        }

        self.replace_authored_selection_occurrences(handle, combined);
    }

    pub fn replace_authored_selection_occurrences(
        &mut self,
        handle: ExpressionHandle,
        occurrences: impl IntoIterator<Item = AuthoredDeclarationSelectionOccurrenceId>,
    ) {
        let mut unique = Vec::new();
        for occurrence in occurrences {
            if !unique.contains(&occurrence) {
                unique.push(occurrence);
            }
        }
        self.nodes.authored_selection_occurrences[source_span_index(handle)] =
            self.spans.authored_selection_occurrence_ids.insert_many(
                unique
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

    pub(crate) fn prepare_static_argument_rebase(
        &self,
        arguments: &[StaticMachineArgument],
        remap: impl Fn(
            AuthoredDeclarationSelectionOccurrenceId,
        ) -> Option<AuthoredDeclarationSelectionOccurrenceId>,
    ) -> Result<PreparedStaticArgumentRebase, ()> {
        fn argument(
            table: &ExpressionTable,
            value: &mut StaticMachineArgument,
            remap: &impl Fn(
                AuthoredDeclarationSelectionOccurrenceId,
            ) -> Option<AuthoredDeclarationSelectionOccurrenceId>,
            members: &mut Vec<(Handle<StaticMachineArgument>, StaticMachineArgument)>,
            active: &mut Vec<Handle<StaticMachineArgument>>,
        ) -> Result<(), ()> {
            if let Some(occurrence) = value.authored_selection {
                value.authored_selection = Some(remap(occurrence).ok_or(())?);
            }
            if let Some(application) = &mut value.application {
                for child in &mut application.arguments {
                    argument(table, child, remap, members, active)?;
                }
                let handle = application.member;
                if handle.is_valid() {
                    let key = |handle: Handle<StaticMachineArgument>| {
                        (handle.arena_index(), handle.generation())
                    };
                    if members
                        .binary_search_by_key(&key(handle), |(handle, _)| key(*handle))
                        .is_err()
                    {
                        if active.contains(&handle) {
                            return Err(());
                        }
                        active.push(handle);
                        let mut child = table.static_member(handle).ok_or(())?.clone();
                        argument(table, &mut child, remap, members, active)?;
                        active.pop();
                        let at =
                            members.partition_point(|(current, _)| key(*current) < key(handle));
                        members.insert(at, (handle, child));
                    }
                }
            }
            Ok(())
        }
        let mut values = arguments.to_vec().into_boxed_slice();
        let mut members = Vec::new();
        for value in &mut values {
            argument(self, value, &remap, &mut members, &mut Vec::new())?;
        }
        Ok(PreparedStaticArgumentRebase {
            arguments: values,
            members,
        })
    }

    pub(crate) fn apply_static_argument_rebase(
        &mut self,
        prepared: PreparedStaticArgumentRebase,
    ) -> Box<[StaticMachineArgument]> {
        for (handle, member) in prepared.members {
            *self
                .static_member_mut(handle)
                .expect("prepared static member remains current") = member;
        }
        prepared.arguments
    }

    pub(crate) fn prepare_authored_selection_rebase(
        &self,
        expression_frontier: usize,
        rebase: language_semantics::declaration_selection::AuthoredDeclarationSelectionSuffixRebase,
    ) -> Result<PreparedExpressionSelectionRebase, ()> {
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

        let mut arguments = Vec::new();
        for (index, (handle, node)) in self.iter_expressions().enumerate() {
            let (members, values) = match node {
                ExpressionNode::Call(call) => (None, call.machine_arguments.as_ref()),
                ExpressionNode::Name(name) if name.static_application.is_valid() => (
                    Some(name.static_application),
                    std::slice::from_ref(self.static_member(name.static_application).ok_or(())?),
                ),
                _ => continue,
            };
            let prepared = self.prepare_static_argument_rebase(values, |occurrence| {
                if index < expression_frontier {
                    rebase.retain_base(occurrence)
                } else {
                    rebase.rebase_appended(occurrence)
                }
            })?;
            if index >= expression_frontier {
                arguments.push((handle, members, prepared));
            }
        }
        Ok(PreparedExpressionSelectionRebase {
            remapped,
            arguments,
        })
    }

    pub(crate) fn apply_authored_selection_rebase(
        &mut self,
        prepared: PreparedExpressionSelectionRebase,
    ) {
        let PreparedExpressionSelectionRebase {
            remapped,
            arguments,
        } = prepared;
        for (handle, member, prepared) in arguments {
            let mut arguments = self.apply_static_argument_rebase(prepared);
            if let Some(member) = member {
                *self
                    .static_member_mut(member)
                    .expect("prepared applied name remains current") = arguments[0].clone();
            } else {
                let ExpressionNode::Call(call) = self.expression_mut(handle) else {
                    unreachable!("prepared call remains current")
                };
                call.machine_arguments = std::mem::take(&mut arguments);
            }
        }
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
        *self
            .spans
            .expression_handles
            .get_mut(expressions.handle_at(offset)) = expression;
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
        *self.spans.struct_fields.get_mut(fields.handle_at(offset)) = field;
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
        *self
            .spans
            .name_path_members
            .get_mut(members.handle_at(offset)) = member;
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
            .get_mut(member_symbols.handle_at(offset)) = member_symbol;
    }

    pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.nodes.expressions.get(handle)
    }

    expression_node_accessors! {
        name_path_node(Name) -> TableNamePath;

        member_node(Member) -> TableMemberExpression;

        indexed_node(Indexed) -> TableIndexedExpression;

        cast_node(Cast) -> TableCastExpression;

        binary_node(Binary) -> TableBinaryExpression;

        call_node(Call) -> TableCallExpression;

        unary_node(Unary) -> TableUnaryExpression;

        integer_node(Integer) -> IntegerLiteral;

    }

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
        self.spans
            .expression_handles
            .get(expressions.handle_at(offset))
    }

    pub fn struct_fields(
        &self,
        span: HandleSpan<TableStructLiteralField>,
    ) -> &[TableStructLiteralField] {
        self.spans.struct_fields.span_or_empty(span)
    }

    pub fn name_path_members(&self, span: HandleSpan<DiagnosticName>) -> &[DiagnosticName] {
        self.spans.name_path_members.span_or_empty(span)
    }

    pub fn name_path_member_symbols(&self, span: HandleSpan<SymbolHandle>) -> &[SymbolHandle] {
        self.spans.name_path_member_symbols.span_or_empty(span)
    }

    pub fn expression_count(&self) -> usize {
        self.nodes.expressions.len()
    }

    pub fn display_name(&self, handle: ExpressionHandle) -> String {
        self.expression(handle).display_name(self)
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

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum ExpressionNode {
    Match(TableMatchExpression),
    SavedSubject(TableSavedSubject),
    ArrayLiteral(HandleSpan<ExpressionHandle>),
    Atomic(TableAtomicExpression),
    Binary(TableBinaryExpression),
    Boolean(bool),
    Cast(TableCastExpression),
    Call(TableCallExpression),
    Float(FloatLiteral),
    UndeterminedNaN(numerics::literals::FloatFormat),
    Indexed(TableIndexedExpression),
    Integer(IntegerLiteral),
    Membership(TableMembershipExpression),
    Member(TableMemberExpression),
    Borrow(TableBorrowExpression),
    Name(TableNamePath),
    Range(TableRangeExpression),
    StructLiteral(TableStructLiteral),
    String(source::SourceBytes),
    Unary(TableUnaryExpression),
    TypeExpression(arena::Handle<crate::types::TypeReference>),
    ZeroValue(arena::Handle<crate::types::TypeReference>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableBorrowExpression {
    pub target: ExpressionHandle,
    pub access: language_core::ReferenceAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableBinaryExpression {
    pub left: ExpressionHandle,
    pub operator: BinaryOperator,
    pub right: ExpressionHandle,
    /// The policy an operator suffix chose for this one operation
    /// (`+%`, `+|`, `+!`); `Exact` for the bare operator.
    pub policy: numerics::arithmetic::ArithmeticDomain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableUnaryExpression {
    pub operator: UnaryOperator,
    pub operand: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableCastExpression {
    pub value: ExpressionHandle,
    pub target_type: arena::Handle<crate::types::TypeReference>,
    pub target_label: HandleSpan<DiagnosticName>,
    pub domain: numerics::arithmetic::ArithmeticDomain,
    pub semantic_domain: HandleSpan<DiagnosticName>,
    pub semantic_domain_arguments: HandleSpan<crate::types::TypeReference>,
    pub semantic_domain_symbol: SymbolHandle,
    pub form: language_core::cast_form::CastForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableIndexedExpression {
    pub collection: ExpressionHandle,
    pub index: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableRangeExpression {
    pub start: ExpressionHandle,
    pub end: ExpressionHandle,
    pub end_inclusive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct TableMembershipExpression {
    pub is_pattern: bool,
    pub value: ExpressionHandle,
    pub domain: HandleSpan<DiagnosticName>,
    pub domain_symbol: SymbolHandle,
    pub case_type_symbol: SymbolHandle,
    pub case_symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub member_symbol: SymbolHandle,
    pub member: DiagnosticName,
    pub case_variant: Option<DiagnosticName>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TableCallExpression {
    pub callee: ExpressionHandle,
    /// Keep static qualification until lexical binding can distinguish a
    /// namespace from a parameter or local with the same name.
    pub target_is_static: bool,
    pub authored_call_binding:
        Option<language_semantics::declaration_selection::ResolvedAuthoredCallBinding>,
    pub receiver: ExpressionHandle,
    pub target_symbol: SymbolHandle,
    pub target: DiagnosticName,
    pub requirement_application: arena::Handle<crate::types::TypeReference>,
    pub machine_arguments: Box<[StaticMachineArgument]>,
    pub arguments: HandleSpan<ExpressionHandle>,
    pub evidence_arguments: Box<[DiagnosticName]>,
    pub operational_acknowledgement: language_semantics::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum SignatureFreeRequirementResolutionError {
    InvalidPath,
    TraitNotUnique,
    RequirementNotUnique,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct ResolvedSignatureFreeRequirement {
    pub trait_definition: SymbolHandle,
    pub declaring_trait: SymbolHandle,
    pub requirement: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticMachineArgument {
    /// Exact ledger occurrence attached when resolution mints this input.
    pub authored_selection:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
    pub authored_selection_target: Option<SymbolHandle>,
    pub type_reference: arena::Handle<crate::types::TypeReference>,
    pub path: Box<[DiagnosticName]>,
    pub application: Option<Box<StaticSymbolApplication>>,
    pub const_literal: Option<numerics::literals::IntegerLiteral>,
    pub evidence_projection: Option<EvidenceProjection>,
    pub symbol: SymbolHandle,
    /// Resolution records the whole qualified operand's requirement lookup.
    /// A refusal is retained too: typing must not retry with a call signature.
    pub signature_free_requirement:
        Option<Result<ResolvedSignatureFreeRequirement, SignatureFreeRequirementResolutionError>>,
}

impl compiler_storage::Record for StaticMachineArgument {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        b"ARGSEL2".write(output)?;
        self.authored_selection.write(output)?;
        self.authored_selection_target.write(output)?;
        self.type_reference.write(output)?;
        self.path.write(output)?;
        self.application.write(output)?;
        self.const_literal.write(output)?;
        self.evidence_projection.write(output)?;
        self.symbol.write(output)?;
        self.signature_free_requirement.write(output)
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        if <[u8; 7]>::read(input)? != *b"ARGSEL2" {
            return Err(input.error("static input selection storage has another version"));
        }
        Ok(Self {
            authored_selection: Option::read(input)?,
            authored_selection_target: Option::read(input)?,
            type_reference: Handle::read(input)?,
            path: Box::read(input)?,
            application: Option::read(input)?,
            const_literal: Option::read(input)?,
            evidence_projection: Option::read(input)?,
            symbol: SymbolHandle::read(input)?,
            signature_free_requirement: Option::read(input)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct StaticSymbolApplication {
    pub member: Handle<StaticMachineArgument>,
    pub lifetime_arguments: Box<[DiagnosticName]>,
    pub arguments: Box<[StaticMachineArgument]>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct EvidenceProjection {
    pub term: DiagnosticName,
    pub member: DiagnosticName,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TableNamePath {
    pub members: HandleSpan<DiagnosticName>,
    pub member_symbols: HandleSpan<SymbolHandle>,
    pub head_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
    pub static_application: arena::Handle<StaticMachineArgument>,
    /// The callable whose reserved `result` this name denotes, or invalid
    /// when it denotes no result. Stage 02 sets it once, where it decides a
    /// contract name is the reserved result occurrence
    /// (`wiki/spec/terminal-psi/calls_and_outcomes.md`: "The reserved
    /// result occurrence belongs to its contract owner, not any local or
    /// parameter spelled `result`"), so a later reader takes the result
    /// by this identity instead of comparing the spelling. `symbol` stays
    /// invalid, as before, so readers of the spelling route are unchanged.
    pub reserved_result_of: SymbolHandle,
}

impl TableNamePath {
    pub const fn new(
        members: HandleSpan<DiagnosticName>,
        member_symbols: HandleSpan<SymbolHandle>,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
    ) -> Self {
        Self {
            members,
            member_symbols,
            head_symbol,
            symbol,
            static_application: arena::Handle::invalid(),
            reserved_result_of: SymbolHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
pub struct TableStructLiteral {
    /// A final source `..` requests ordinary zero values for missing runtime fields.
    pub zero_omitted_fields: bool,
    pub application_type: Handle<crate::types::TypeReference>,
    pub type_name: DiagnosticName,
    pub type_symbol: SymbolHandle,
    pub case_name: Option<DiagnosticName>,
    pub case_symbol: Option<SymbolHandle>,
    pub fields: HandleSpan<TableStructLiteralField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, compiler_storage::Record)]
pub struct TableStructLiteralField {
    pub name: DiagnosticName,
    pub field_symbol: SymbolHandle,
    pub value: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
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

impl BinaryOperator {
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

    pub fn result_is_boolean(self) -> bool {
        self.is_comparison() || matches!(self, BinaryOperator::And | BinaryOperator::Or)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum UnaryOperator {
    BitwiseNot,
    LogicalNot,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TableMatchExpression {
    pub subject: ExpressionHandle,
    pub capture: Handle<MatchSubjectCapture>,
    pub arms: HandleSpan<TableMatchArm>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum SubjectCapturePurpose {
    #[default]
    Match,
    Transition,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct MatchSubjectCapture {
    pub purpose: SubjectCapturePurpose,
    pub subject: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TableSavedSubject {
    pub component: language_core::PatternComponent,
    pub capture: Handle<MatchSubjectCapture>,
    pub pattern_head: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TableMatchArm {
    pub pattern: MatchPattern,
    pub value: ExpressionHandle,
    pub source_span: SourceSpan,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum MatchPattern {
    Value(ExpressionHandle),
    Destructure(TableDestructurePattern),
    #[default]
    Wildcard,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TableDestructurePattern {
    pub application_type: Handle<crate::types::TypeReference>,
    pub head: ExpressionHandle,
    pub fields: HandleSpan<TableMatchField>,
    pub has_rest: bool,
    pub guard: ExpressionHandle,
    pub condition: ExpressionHandle,
    pub proof_selectors: HandleSpan<crate::statement::OutcomeProofSelector>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TableMatchField {
    pub component: language_core::PatternComponent,
    pub member: DiagnosticName,
    pub field_symbol: SymbolHandle,
    pub action: MatchFieldAction,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum MatchFieldAction {
    Bind(DiagnosticName),
    Equal(ExpressionHandle),
    #[default]
    Waive,
}

#[cfg(test)]
mod static_input_record_tests;
