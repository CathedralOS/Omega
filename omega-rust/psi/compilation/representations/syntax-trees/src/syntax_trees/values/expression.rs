use crate::FloatSpelling;
use crate::identifier::Identifier;
use arena::{Handle, HandleSpan, PagedArenaFrontier, PagedArenaSet as Arena};
use numerics::literals::IntegerLiteral;
use source::SourceSpan;

mod display;
#[cfg(test)]
mod tests;

pub type ExpressionHandle = Handle<ExpressionNode>;
pub type ExpressionFrontier = PagedArenaFrontier<ExpressionNode>;

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct ExpressionTable {
    expressions: Arena<ExpressionNode>,
    source_spans: Arena<SourceSpan>,
    expression_handles: Arena<ExpressionHandle>,
    identifier_path_members: Arena<Identifier>,
    struct_fields: Arena<TableStructLiteralField>,
    match_arms: Arena<TableMatchArm>,
}

pub(crate) struct ExpressionTableAppendCheckpoint {
    expressions: arena::PagedArenaFrontier<ExpressionNode>,
    source_spans: arena::PagedArenaFrontier<SourceSpan>,
    expression_handles: arena::PagedArenaFrontier<ExpressionHandle>,
    identifier_path_members: arena::PagedArenaFrontier<Identifier>,
    struct_fields: arena::PagedArenaFrontier<TableStructLiteralField>,
    match_arms: arena::PagedArenaFrontier<TableMatchArm>,
}

impl ExpressionTable {
    pub(crate) fn append_checkpoint(&self) -> ExpressionTableAppendCheckpoint {
        ExpressionTableAppendCheckpoint {
            expressions: self.expressions.frontier(),
            source_spans: self.source_spans.frontier(),
            expression_handles: self.expression_handles.frontier(),
            identifier_path_members: self.identifier_path_members.frontier(),
            struct_fields: self.struct_fields.frontier(),
            match_arms: self.match_arms.frontier(),
        }
    }

    pub(crate) fn rewind_appended(&mut self, checkpoint: ExpressionTableAppendCheckpoint) {
        self.expressions.rewind_appended(checkpoint.expressions);
        self.source_spans.rewind_appended(checkpoint.source_spans);
        self.expression_handles
            .rewind_appended(checkpoint.expression_handles);
        self.identifier_path_members
            .rewind_appended(checkpoint.identifier_path_members);
        self.struct_fields.rewind_appended(checkpoint.struct_fields);
        self.match_arms.rewind_appended(checkpoint.match_arms);
    }

    pub(crate) fn share_pages(&mut self) {
        self.expressions.share_pages();
        self.source_spans.share_pages();
        self.expression_handles.share_pages();
        self.identifier_path_members.share_pages();
        self.struct_fields.share_pages();
        self.match_arms.share_pages();
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

    pub fn new() -> Self {
        Self::new_with_generation(1)
    }

    pub fn new_with_generation(generation: u32) -> Self {
        Self {
            expressions: Arena::with_generation(generation),
            source_spans: Arena::with_generation(generation),
            expression_handles: Arena::with_generation(generation),
            identifier_path_members: Arena::with_generation(generation),
            struct_fields: Arena::with_generation(generation),
            match_arms: Arena::with_generation(generation),
        }
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

    pub(crate) fn adopt(&mut self, other: Self) {
        self.expressions.adopt(other.expressions);
        self.source_spans.adopt(other.source_spans);
        self.expression_handles.adopt(other.expression_handles);
        self.identifier_path_members
            .adopt(other.identifier_path_members);
        self.struct_fields.adopt(other.struct_fields);
        self.match_arms.adopt(other.match_arms);
    }

    pub(crate) fn begin_page(&mut self, generation: u32) {
        self.expressions.begin_page(generation);
        self.source_spans.begin_page(generation);
        self.expression_handles.begin_page(generation);
        self.identifier_path_members.begin_page(generation);
        self.struct_fields.begin_page(generation);
        self.match_arms.begin_page(generation);
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

    /// Check the live generational identity before reading retained source custody.
    pub fn contains_expression(&self, handle: ExpressionHandle) -> bool {
        self.expressions.is_valid(handle)
    }

    #[inline]
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

    /// Iterate every expression with its handle (the desugar passes' walk).
    pub fn iter_expressions(&self) -> impl Iterator<Item = (ExpressionHandle, &ExpressionNode)> {
        self.expressions.iter()
    }

    /// Replace a node in place. Reserved for the pre-resolution DESUGAR
    /// passes (const substitution) -- downstream stages treat the
    /// table as immutable.
    pub fn replace_expression(&mut self, handle: ExpressionHandle, node: ExpressionNode) {
        *self.expressions.get_mut(handle) = node;
    }

    /// Update only call acknowledgement flags through the page's normal COW
    /// path. A rejected non-call leaves its payload and revision untouched.
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

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub enum ExpressionNode {
    Match(TableMatchExpression),
    ArrayLiteral(HandleSpan<ExpressionHandle>),
    Atomic(TableAtomicExpression),
    Binary(TableBinaryExpression),
    Boolean(bool),
    Cast(TableCastExpression),
    Call(TableCallExpression),
    Float(FloatSpelling),
    /// Internal semantic value; no runtime representation or source spelling is implied.
    UndeterminedNaN(numerics::literals::FloatFormat),
    Indexed(TableIndexedExpression),
    Integer(IntegerLiteral),
    Membership(TableMembershipExpression),
    Member(TableMemberExpression),
    Borrow(TableBorrowExpression),
    Name(HandleSpan<Identifier>),
    Range(TableRangeExpression),
    StructLiteral(TableStructLiteral),
    /// Exact decoded literal octets. A string literal is not required to be
    /// UTF-8 after `\xNN` escape decoding.
    String(source::SourceBytes),
    Unary(TableUnaryExpression),
    /// A static type operand retained for type-equation checking. This is not
    /// a runtime value and must be consumed or rejected before value lowering.
    TypeExpression(crate::types::TypeReferenceHandle),
    /// Proof-only observation of a type's normalized all-zero home value.
    ZeroValue(crate::types::TypeReferenceHandle),
}

/// One explicit exclusive borrow expression. The access mode is retained on
/// the expression rather than reconstructed from the callee's expected type:
/// `&mut place` and its explicit `&write place` attenuation are distinct
/// source operations even though both lower to an exclusive pointer ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
pub struct TableBorrowExpression {
    pub target: ExpressionHandle,
    pub access: language_core::ReferenceAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
pub struct TableAtomicExpression {
    pub value: ExpressionHandle,
    /// Scalar observed-prior destination. Invalid for loads, stores and whole
    /// structural outcomes, whose receiving binding supplies their type.
    pub result: ExpressionHandle,
    pub ordering: language_core::atomic::AtomicOrderingPlan,
    pub result_custody: language_core::atomic::AtomicExpressionResultCustody,
}

impl Default for ExpressionNode {
    fn default() -> Self {
        Self::Integer(IntegerLiteral::zero())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
pub struct TableBinaryExpression {
    pub left: ExpressionHandle,
    pub operator: BinaryOperator,
    pub right: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
pub struct TableUnaryExpression {
    pub operator: UnaryOperator,
    pub operand: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
pub struct TableCastExpression {
    pub value: ExpressionHandle,
    pub target_type: crate::types::TypeReferenceHandle,
    /// Diagnostic spelling only. Semantic identity and checking use
    /// `target_type`; this cached label keeps expression-only diagnostics
    /// independent of the separate type-reference arena.
    pub target_label: HandleSpan<Identifier>,
    /// Arithmetic domain cast (`x as u8 in Saturating`), decision 17 S2. `Exact`
    /// when the cast has no `in <Domain>` suffix.
    pub domain: numerics::arithmetic::ArithmeticDomain,
    /// A NON-policy `in <Name>` suffix (`x as i64 in Km`) -- a semantic-
    /// domain qualification spelling (decision 19). Carried for the checked
    /// layers to judge; EMPTY for policy/no-suffix casts.
    pub semantic_domain: HandleSpan<Identifier>,
    /// PDI2 proof-static argument pack for a declared semantic-domain family.
    /// EMPTY for policy casts and monomorphic declared domains.
    pub semantic_domain_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
    /// Value conversion vs §5b borrow recast (`&x as &T`).
    pub form: language_core::cast_form::CastForm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
pub struct TableIndexedExpression {
    pub collection: ExpressionHandle,
    pub index: ExpressionHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
pub struct TableRangeExpression {
    pub start: ExpressionHandle,
    pub end: ExpressionHandle,
    pub end_inclusive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent)]
pub struct TableMembershipExpression {
    pub value: ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct TableMemberExpression {
    pub receiver: ExpressionHandle,
    pub member: Identifier,
    /// `Some(variant)` when this access reads a CASE-PAYLOAD field bound by a
    /// destructure pattern (`Tx::Transfer { amount }` rewrites the arm's `amount`
    /// to `subject.amount`), naming the variant so the field resolves to THAT
    /// variant's field even when another variant has a same-named field at a
    /// different offset. `None` for ordinary field access.
    pub case_variant: Option<Identifier>,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct TableCallExpression {
    pub receiver: ExpressionHandle,
    pub target: Identifier,
    /// The target was an authored static name path, not a value member selected with `.`.
    pub target_is_static: bool,
    /// Compile-time machine-symbol selections (`map<Card::power>(items)`).
    /// These are declaration identities, never runtime expression values.
    pub machine_arguments: Box<[StaticMachineArgument]>,
    pub arguments: HandleSpan<ExpressionHandle>,
    /// Explicit erased evidence-term arguments after the `;` call lane.
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: language_core::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct StaticMachineArgument {
    /// An explicit structural type argument; zero denotes another static argument kind.
    pub type_reference: crate::types::TypeReferenceHandle,
    /// Historical storage name: proposition calls also use this record for
    /// type and const arguments, classified by the target's static telescope.
    pub path: Box<[Identifier]>,
    /// Nested application owned by the selected static declaration. This is
    /// what distinguishes `Family<A>` from an outer call argument `A` and
    /// keeps a conformance telescope delimited from the callee telescope.
    pub application: Option<Box<StaticSymbolApplication>>,
    /// Integer const argument. `Some` makes `path` empty; the target's binder
    /// kind determines whether the static argument is legal.
    pub const_literal: Option<numerics::literals::IntegerLiteral>,
    /// Proof-static projection from one named evidence term. This remains
    /// structurally distinct from a declaration path: `term.member` is not
    /// interchangeable with `term::member`.
    pub evidence_projection: Option<EvidenceProjection>,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct StaticSymbolApplication {
    /// Explicit erased lifetime arguments. An empty lane may be filled only
    /// by ordinary lifetime elision after the declaration is resolved.
    pub lifetime_arguments: Box<[Identifier]>,
    /// Complete explicit non-lifetime telescope of the selected declaration.
    pub arguments: Box<[StaticMachineArgument]>,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct EvidenceProjection {
    pub term: Identifier,
    pub member: Identifier,
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
pub struct TableStructLiteral {
    /// Complete authored static name; resolution decides whether it selects
    /// a record or a case. Path length does not determine that distinction.
    pub constructor_name: Identifier,
    pub fields: HandleSpan<TableStructLiteralField>,
}

impl Default for TableStructLiteral {
    fn default() -> Self {
        Self {
            constructor_name: Identifier::generated(""),
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, syntax_trees_derive::SyntaxEquivalent)]
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
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent,
)]
pub struct TableMatchExpression {
    pub subject: ExpressionHandle,
    pub arms: HandleSpan<TableMatchArm>,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, syntax_trees_derive::PlainSyntaxEquivalent,
)]
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
