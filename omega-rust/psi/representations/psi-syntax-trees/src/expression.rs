use crate::identifier::Identifier;
use psi_arena::{Arena, Handle, HandleSpan};
use psi_numerics::literals::IntegerLiteral;
use psi_source::{SourceSpan, SourceText};
use std::sync::Arc;

mod display;
#[cfg(test)]
mod tests;

pub type ExpressionHandle = Handle<ExpressionNode>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionTable {
    expressions: Arena<ExpressionNode>,
    source_spans: Vec<SourceSpan>,
    expression_handles: Arena<ExpressionHandle>,
    identifier_path_members: Arena<Identifier>,
    struct_fields: Arena<TableStructLiteralField>,
}

impl ExpressionTable {
    pub fn new() -> Self {
        Self {
            expressions: Arena::new(),
            source_spans: Vec::new(),
            expression_handles: Arena::new(),
            identifier_path_members: Arena::new(),
            struct_fields: Arena::new(),
        }
    }

    pub fn insert(&mut self, expression: ExpressionNode) -> ExpressionHandle {
        let handle = self.expressions.insert(expression);
        self.source_spans.push(SourceSpan::default());
        debug_assert_eq!(source_span_index(handle), self.source_spans.len() - 1);
        handle
    }

    pub fn source_span(&self, handle: ExpressionHandle) -> SourceSpan {
        self.source_spans[source_span_index(handle)]
    }

    pub fn set_source_span(&mut self, handle: ExpressionHandle, source_span: SourceSpan) {
        self.source_spans[source_span_index(handle)] = source_span;
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

    pub fn append_struct_field(
        &mut self,
        field: TableStructLiteralField,
    ) -> Handle<TableStructLiteralField> {
        self.struct_fields.append(field)
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

    pub fn expression_count(&self) -> usize {
        self.expressions.len()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionNode {
    ArrayLiteral(HandleSpan<ExpressionHandle>),
    Atomic(TableAtomicExpression),
    Binary(TableBinaryExpression),
    Boolean(bool),
    Cast(TableCastExpression),
    Call(TableCallExpression),
    Float(SourceText),
    Indexed(TableIndexedExpression),
    Integer(IntegerLiteral),
    Membership(TableMembershipExpression),
    Member(TableMemberExpression),
    Borrow(TableBorrowExpression),
    Name(HandleSpan<Identifier>),
    Range(TableRangeExpression),
    SelfValue,
    StructLiteral(TableStructLiteral),
    /// Exact decoded literal octets. A string literal is not required to be
    /// UTF-8 after `\xNN` escape decoding.
    String(Arc<[u8]>),
    Unary(TableUnaryExpression),
    /// Proof-only observation of a type's normalized all-zero home value.
    ZeroValue(crate::types::TypeReferenceHandle),
}

/// One explicit exclusive borrow expression. The access mode is retained on
/// the expression rather than reconstructed from the callee's expected type:
/// `&mut place` and its explicit `&write place` attenuation are distinct
/// source operations even though both lower to an exclusive pointer ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableBorrowExpression {
    pub target: ExpressionHandle,
    pub access: psi_language_core::ReferenceAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableAtomicExpression {
    pub value: ExpressionHandle,
    /// Compiler-authored destination for operations that return the value
    /// observed by the atomic instruction. Invalid for load/store.
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
    pub target_type: crate::types::TypeReferenceHandle,
    /// Diagnostic spelling only. Semantic identity and checking use
    /// `target_type`; this cached label keeps expression-only diagnostics
    /// independent of the separate type-reference arena.
    pub target_label: HandleSpan<Identifier>,
    /// Arithmetic domain cast (`x as u8 in Saturating`), decision 17 S2. `Exact`
    /// when the cast has no `in <Domain>` suffix.
    pub domain: psi_numerics::arithmetic::ArithmeticDomain,
    /// A NON-policy `in <Name>` suffix (`x as i64 in Km`) -- a semantic-
    /// domain qualification spelling (decision 19). Carried for the checked
    /// layers to judge; EMPTY for policy/no-suffix casts.
    pub semantic_domain: HandleSpan<Identifier>,
    /// PDI2 proof-static argument pack for a declared semantic-domain family.
    /// EMPTY for policy casts and monomorphic declared domains.
    pub semantic_domain_arguments: HandleSpan<crate::types::TypeReferenceHandle>,
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
    pub domain: HandleSpan<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCallExpression {
    pub receiver: ExpressionHandle,
    pub target: Identifier,
    /// Compile-time machine-symbol selections (`map<Card::power>(items)`).
    /// These are declaration identities, never runtime expression values.
    pub machine_arguments: Box<[StaticMachineArgument]>,
    pub arguments: HandleSpan<ExpressionHandle>,
    /// Explicit erased evidence-term arguments after the `;` call lane.
    pub evidence_arguments: Box<[Identifier]>,
    pub operational_acknowledgement: psi_language_core::CallOperationalAcknowledgement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticMachineArgument {
    /// Historical storage name: proposition calls also use this record for
    /// type and const arguments, classified by the target's static telescope.
    pub path: Box<[Identifier]>,
    /// Nested application owned by the selected static declaration. This is
    /// what distinguishes `Family<A>` from an outer call argument `A` and
    /// keeps a conformance telescope delimited from the callee telescope.
    pub application: Option<Box<StaticSymbolApplication>>,
    /// Integer const argument. `Some` makes `path` empty; the target's binder
    /// kind determines whether the static argument is legal.
    pub const_literal: Option<psi_numerics::literals::IntegerLiteral>,
    /// Proof-static projection from one named evidence term. This remains
    /// structurally distinct from a declaration path: `term.member` is not
    /// interchangeable with `term::member`.
    pub evidence_projection: Option<EvidenceProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticSymbolApplication {
    /// Explicit erased lifetime arguments. An empty lane may be filled only
    /// by ordinary lifetime elision after the declaration is resolved.
    pub lifetime_arguments: Box<[Identifier]>,
    /// Complete explicit non-lifetime telescope of the selected declaration.
    pub arguments: Box<[StaticMachineArgument]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceProjection {
    pub term: Identifier,
    pub member: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStructLiteral {
    pub type_name: Identifier,
    /// `Some` when the literal constructs a CASE of `type_name`
    /// (`Command::Say { text: ... }`); `None` for a plain record literal.
    pub case_name: Option<Identifier>,
    pub fields: HandleSpan<TableStructLiteralField>,
}

impl Default for TableStructLiteral {
    fn default() -> Self {
        Self {
            type_name: Identifier::generated(""),
            case_name: None,
            fields: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
