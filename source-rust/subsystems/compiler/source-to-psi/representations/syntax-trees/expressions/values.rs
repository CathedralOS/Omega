use crate::FloatSpelling;
use crate::identifier::Identifier;
use crate::paged_roster::paged_roster;
use arena::{Handle, HandleSpan, PagedArenaFrontier};
use numeric_literals::IntegerLiteral;
use source_text::SourceSpan;

pub type ExpressionHandle = Handle<ExpressionNode>;
pub type ExpressionFrontier = PagedArenaFrontier<ExpressionNode>;

paged_roster! {
    pub struct ExpressionTable => pub(crate) ExpressionTableAppendCheckpoint {
        arena expressions: ExpressionNode,
        arena source_spans: SourceSpan,
        arena expression_handles: ExpressionHandle,
        arena identifier_path_members: Identifier,
        arena struct_fields: TableStructLiteralField,
        arena match_arms: TableMatchArm,
        arena match_fields: TableMatchField,
        arena match_subjects: MatchSubjectCapture,
        arena static_members: StaticMachineArgument,
    }
}

impl ExpressionTable {
    pub fn insert_static_member(
        &mut self,
        member: StaticMachineArgument,
    ) -> Handle<StaticMachineArgument> {
        self.static_members.insert(member)
    }

    pub fn static_member(
        &self,
        member: Handle<StaticMachineArgument>,
    ) -> Option<&StaticMachineArgument> {
        self.static_members
            .is_valid(member)
            .then(|| self.static_members.get(member))
    }

    pub fn static_member_mut(
        &mut self,
        member: Handle<StaticMachineArgument>,
    ) -> Option<&mut StaticMachineArgument> {
        if self.static_members.is_valid(member) {
            Some(self.static_members.get_mut(member))
        } else {
            None
        }
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

    pub fn insert_match_fields(
        &mut self,
        fields: impl IntoIterator<Item = TableMatchField>,
    ) -> HandleSpan<TableMatchField> {
        self.match_fields.insert_many(fields)
    }

    pub fn match_fields(&self, fields: HandleSpan<TableMatchField>) -> &[TableMatchField] {
        self.match_fields.span_or_empty(fields)
    }

    pub fn match_pattern_dependencies(
        &self,
        pattern: MatchPattern,
    ) -> impl Iterator<Item = ExpressionHandle> + '_ {
        let (head, fields, guard) = match pattern {
            MatchPattern::Value(value) => (value, HandleSpan::empty(), ExpressionHandle::invalid()),
            MatchPattern::Guard(condition) => {
                (ExpressionHandle::invalid(), HandleSpan::empty(), condition)
            }
            MatchPattern::Destructure(pattern) => (pattern.head, pattern.fields, pattern.guard),
            MatchPattern::Wildcard => (
                ExpressionHandle::invalid(),
                HandleSpan::empty(),
                ExpressionHandle::invalid(),
            ),
        };
        head.is_valid()
            .then_some(head)
            .into_iter()
            .chain(
                self.match_fields(fields)
                    .iter()
                    .filter_map(|field| match &field.action {
                        MatchFieldAction::Equal(value) => Some(*value),
                        _ => None,
                    }),
            )
            .chain(guard.is_valid().then_some(guard))
    }

    pub fn insert_match_subject(
        &mut self,
        subject: ExpressionHandle,
    ) -> Handle<MatchSubjectCapture> {
        self.match_subjects.insert(MatchSubjectCapture {
            purpose: SubjectCapturePurpose::Match,
            subject,
        })
    }

    pub fn insert_transition_subject(
        &mut self,
        subject: ExpressionHandle,
    ) -> Handle<MatchSubjectCapture> {
        self.match_subjects.insert(MatchSubjectCapture {
            purpose: SubjectCapturePurpose::Transition,
            subject,
        })
    }

    pub fn set_transition_subject(
        &mut self,
        capture: Handle<MatchSubjectCapture>,
        subject: ExpressionHandle,
    ) {
        let captured = self.match_subjects.get_mut(capture);
        debug_assert_eq!(captured.purpose, SubjectCapturePurpose::Transition);
        captured.subject = subject;
    }

    pub fn match_subject(&self, capture: Handle<MatchSubjectCapture>) -> &MatchSubjectCapture {
        self.match_subjects.get(capture)
    }

    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        let handle = self.expressions.insert(expression);
        let source_span = self.source_spans.append(SourceSpan::default());
        debug_assert_eq!(source_span.arena_index(), handle.arena_index());
        debug_assert_eq!(source_span.generation(), handle.generation());
        handle
    }

    pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
        *self.source_spans.get(source_span_handle(handle))
    }

    pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
        *self.source_spans.get_mut(source_span_handle(handle)) = source_span;
    }

    pub fn append_expression_handle(
        &mut self,
        expression: ExpressionHandle,
    ) -> Handle<ExpressionHandle> {
        self.expression_handles.append(expression)
    }

    pub fn insert_expression_handles(
        &mut self,
        expressions: impl IntoIterator<Item = ExpressionHandle>,
    ) -> HandleSpan<ExpressionHandle> {
        self.expression_handles.insert_many(expressions)
    }

    pub fn insert_struct_fields(
        &mut self,
        fields: impl IntoIterator<Item = TableStructLiteralField>,
    ) -> HandleSpan<TableStructLiteralField> {
        self.struct_fields.insert_many(fields)
    }

    pub fn append_identifier_path_member(&mut self, member: Identifier) -> Handle<Identifier> {
        self.identifier_path_members.append(member)
    }

    pub fn append_identifier_path_member_to_span(
        &mut self,
        span: &mut HandleSpan<Identifier>,
        member: Identifier,
    ) {
        self.identifier_path_members.append_to_span(span, member);
    }

    pub fn copy_identifier_path_prefix(
        &mut self,
        span: HandleSpan<Identifier>,
        count: usize,
    ) -> HandleSpan<Identifier> {
        let count = count.min(span.len());
        if count == 0 {
            return HandleSpan::empty();
        }

        self.identifier_path_members.copy_span_pair(
            HandleSpan::from_parts(span.start(), count as u32),
            HandleSpan::empty(),
        )
    }

    pub fn contains_expression(&self, handle: ExpressionHandle) -> bool {
        self.expressions.is_valid(handle)
    }

    #[inline]
    pub fn call(&self, handle: ExpressionHandle) -> &TableCallExpression {
        let ExpressionNode::Call(call) = self.expression(handle) else {
            unreachable!("a call handle must point to a call expression");
        };
        call
    }

    pub fn expression(&self, handle: ExpressionHandle) -> &ExpressionNode {
        self.expressions.get(handle)
    }

    pub fn expression_handles(&self, span: HandleSpan<ExpressionHandle>) -> &[ExpressionHandle] {
        self.expression_handles.span_or_empty(span)
    }

    pub fn struct_fields(
        &self,
        span: HandleSpan<TableStructLiteralField>,
    ) -> &[TableStructLiteralField] {
        self.struct_fields.span_or_empty(span)
    }

    pub fn identifier_path_members(&self, span: HandleSpan<Identifier>) -> &[Identifier] {
        self.identifier_path_members.span_or_empty(span)
    }

    pub fn iter_expressions(&self) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
        self.expressions.iter()
    }

    pub fn replace_expression(&mut self, handle: ExpressionHandle, node: ExpressionNode) {
        *self.expressions.get_mut(handle) = node;
    }

    pub fn set_call_acknowledgement(
        &mut self,
        expression: ExpressionHandle,
        acknowledgement: language_core::CallOperationalAcknowledgement,
    ) -> bool {
        if !matches!(self.expressions.get(expression), ExpressionNode::Call(_)) {
            return false;
        }
        let ExpressionNode::Call(call) = self.expressions.get_mut(expression) else {
            unreachable!("call kind was checked before acquiring the mutable page")
        };
        call.operational_acknowledgement = acknowledgement;
        true
    }

    pub fn expression_count(&self) -> usize {
        self.expressions.len()
    }

    pub fn frontier(&self) -> ExpressionFrontier {
        self.expressions.frontier()
    }
}

fn source_span_handle(handle: ExpressionHandle) -> Handle<SourceSpan> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

impl Default for ExpressionTable {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub enum ExpressionNode {
    Match(TableMatchExpression),
    Block(TableBlockExpression),
    SavedSubject(TableSavedSubject),
    ArrayLiteral(HandleSpan<ExpressionHandle>),
    Atomic(TableAtomicExpression),
    Binary(TableBinaryExpression),
    Boolean(bool),
    Cast(TableCastExpression),
    Call(TableCallExpression),
    Float(FloatSpelling),
    UndeterminedNaN(numeric_literals::FloatFormat),
    Indexed(TableIndexedExpression),
    Integer(IntegerLiteral),
    Membership(TableMembershipExpression),
    Member(TableMemberExpression),
    Borrow(TableBorrowExpression),
    // A declaration query needs each owner's and member's chosen type,
    // constant and lifetime arguments without invoking the declaration.
    Name {
        members: HandleSpan<Identifier>,
        application: Option<Box<StaticSymbolApplication>>,
    },
    Range(TableRangeExpression),
    StructLiteral(TableStructLiteral),
    String(source_text::SourceBytes),
    Unary(TableUnaryExpression),
    TypeExpression(crate::types::TypeReferenceHandle),
    ZeroValue(crate::types::TypeReferenceHandle),
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableBorrowExpression {
    pub target: ExpressionHandle,
    pub access: language_core::ReferenceAccess,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
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

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableBinaryExpression {
    pub left: ExpressionHandle,
    pub operator: BinaryOperator,
    pub right: ExpressionHandle,
    /// The policy an operator suffix chose for this one operation
    /// (`+%`, `+|`, `+!`); `Exact` for the bare operator.
    pub policy: numerics::arithmetic::ArithmeticDomain,
    /// `true` when the parser formed this subtraction from prefix negation
    /// `-right`; `left` is then the implied zero. Operator selection reads
    /// it to select a declared one-operand `-` (expressions.md, "Operator
    /// families"); with no such declaration the meaning stays `0 - right`.
    pub negation: bool,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableUnaryExpression {
    pub operator: UnaryOperator,
    pub operand: ExpressionHandle,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableCastExpression {
    pub value: ExpressionHandle,
    pub target_type: crate::types::TypeReferenceHandle,
    pub target_label: HandleSpan<Identifier>,
    pub domain: numerics::arithmetic::ArithmeticDomain,
    pub semantic_domain: HandleSpan<Identifier>,
    pub semantic_domain_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    pub form: language_core::cast_form::CastForm,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableIndexedExpression {
    pub collection: ExpressionHandle,
    pub index: ExpressionHandle,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableRangeExpression {
    pub start: ExpressionHandle,
    pub end: ExpressionHandle,
    pub end_inclusive: bool,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableMembershipExpression {
    pub value: ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub access_source_span: SourceSpan,
    pub member: Identifier,
    /// Set on the read a positional pattern generates for one binding: the
    /// field at this place in the receiver's declaration order. `member` is
    /// then empty, because the pattern wrote a binding name and no field
    /// name, and a lookup by name must find nothing.
    pub position: Option<u32>,
    pub case_variant: Option<Identifier>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableCallExpression {
    pub callee: ExpressionHandle,
    pub receiver: ExpressionHandle,
    pub target: Identifier,
    pub target_is_static: bool,
    pub machine_arguments: Box<[StaticMachineArgument]>,
    pub arguments: HandleSpan<ExpressionHandle>,
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: language_core::CallOperationalAcknowledgement,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct StaticMachineArgument {
    pub type_reference: crate::types::TypeReferenceHandle,
    pub path: Box<[Identifier]>,
    pub application: Option<Box<StaticSymbolApplication>>,
    pub const_literal: Option<numeric_literals::IntegerLiteral>,
    pub evidence_projection: Option<EvidenceProjection>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct StaticSymbolApplication {
    pub member: Handle<StaticMachineArgument>,
    pub lifetime_arguments: Box<[Identifier]>,
    pub arguments: Box<[StaticMachineArgument]>,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct EvidenceProjection {
    pub term: Identifier,
    pub member: Identifier,
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableStructLiteral {
    /// A final source `..` requests ordinary zero values for missing runtime fields.
    pub zero_omitted_fields: bool,
    pub application_type: crate::types::TypeReferenceHandle,
    pub constructor_name: Identifier,
    pub fields: HandleSpan<TableStructLiteralField>,
}

impl Default for TableStructLiteral {
    fn default() -> Self {
        Self {
            zero_omitted_fields: false,
            application_type: Default::default(),
            constructor_name: Identifier::generated(""),
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableStructLiteralField {
    pub name: Identifier,
    pub value: ExpressionHandle,
}

impl Default for TableStructLiteralField {
    fn default() -> Self {
        Self {
            name: Identifier::generated(""),
            value: ExpressionHandle::invalid(),
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, record_storage::Record, syntax_trees_derive::SyntaxRelocate,
)]
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

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, record_storage::Record, syntax_trees_derive::SyntaxRelocate,
)]
pub enum UnaryOperator {
    BitwiseNot,
    LogicalNot,
    Dereference,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableMatchExpression {
    pub subject: ExpressionHandle,
    pub capture: Handle<MatchSubjectCapture>,
    pub arms: HandleSpan<TableMatchArm>,
}

/// A block-valued dispatch arm, `0 -> { let port: u32 = 1; port }`
/// (patterns.md: "a comma separates each arm from the next, including after
/// a block-valued arm"). Its statements run in order in the block's own
/// scope; `tail` then supplies the arm's value. A block without a tail
/// completes with Unit. The statements are ordinary body statements, held in
/// `StatementTable::block_statements` because the enclosing body's statement
/// span is still being built when the block is parsed.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableBlockExpression {
    pub statements: HandleSpan<crate::statement::StatementHandle>,
    pub tail: ExpressionHandle,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub enum SubjectCapturePurpose {
    #[default]
    Match,
    Transition,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct MatchSubjectCapture {
    pub purpose: SubjectCapturePurpose,
    pub subject: ExpressionHandle,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableSavedSubject {
    pub component: language_core::PatternComponent,
    pub capture: Handle<MatchSubjectCapture>,
    pub pattern_head: ExpressionHandle,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    syntax_trees_derive::PlainSyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableMatchArm {
    pub pattern: MatchPattern,
    pub value: ExpressionHandle,
    pub source_span: SourceSpan,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    record_storage::Record,
    syntax_trees_derive::SyntaxRelocate,
)]
/// What an arm tests. `Guard` is the arm of a `match` without a subject: its
/// condition reads existing values, and the arm is taken when it is true
/// (`wiki/spec/language/patterns.md`, "In a dispatch without a subject, an
/// arm condition such as `ready -> 1` reads the existing value of `ready`").
pub enum MatchPattern {
    Value(ExpressionHandle),
    Destructure(TableDestructurePattern),
    Guard(ExpressionHandle),
    #[default]
    Wildcard,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    record_storage::Record,
    syntax_trees_derive::SyntaxRelocate,
)]
pub struct TableDestructurePattern {
    /// The authored identifier binding the whole subject of an irrefutable arm.
    pub subject_binding: ExpressionHandle,
    pub application_type: crate::types::TypeReferenceHandle,
    pub head: ExpressionHandle,
    pub fields: HandleSpan<TableMatchField>,
    pub has_rest: bool,
    pub guard: ExpressionHandle,
    pub proof_selectors: HandleSpan<crate::statement::TableOutcomeProofSelector>,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub struct TableMatchField {
    pub component: language_core::PatternComponent,
    /// A named row keeps its field name in `member`. A positional row and a
    /// final `..` keep only the source span of what was written there:
    /// `member` is empty for a position and spells `..` for the rest, so
    /// neither can be mistaken for a field name.
    pub selector: language_core::PatternSelector,
    pub member: Identifier,
    pub action: MatchFieldAction,
}

#[derive(
    Debug,
    Clone,
    Default,
    syntax_trees_derive::SyntaxEquivalent,
    syntax_trees_derive::SyntaxRelocate,
    record_storage::Record,
)]
pub enum MatchFieldAction {
    Bind(Identifier),
    Equal(ExpressionHandle),
    #[default]
    Waive,
}

#[path = "display.rs"]
pub(crate) mod display;

#[cfg(test)]
#[path = "../tests/unit/expression_tests.rs"]
mod tests;
