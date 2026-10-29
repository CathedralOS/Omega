//! An owned designated operand graph. Its handles index only these owned
//! arenas; no parser, SRT, Build AST, or prepared-program arena survives here.
//! Original lexical coordinates and certified builtin atoms preserve naming
//! custody. The graph describes source, never declaration/execution authority.
use arena::{Arena, Handle, HandleSpan};
use source::SourceSpan;

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductOperandName {
    pub text: String,
    pub source_span: SourceSpan,
    pub declaration: Option<SourceSpan>,
    pub builtin_type: Option<symbols::BuiltinTypeAtom>,
    pub builtin_function: Option<symbols::BuiltinFunction>,
    /// Issued by the ordinary type-reference producer on this original token.
    /// Generated environment names and declaration descriptors retain absence.
    pub authored_selection: Handle<ProductOperandSelection>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductStaticOperandGraph {
    pub root: Handle<ProductStaticOperand>,
    pub names: Arena<ProductOperandName>,
    pub operands: Arena<ProductStaticOperand>,
    pub applications: Arena<ProductStaticApplication>,
    pub types: Arena<ProductOperandType>,
    pub expressions: Arena<ProductOperandExpression>,
    pub selections: Arena<ProductOperandSelection>,
    pub constraints: Arena<ProductOperandConstraint>,
    pub fields: Arena<ProductOperandField>,
    pub match_subjects: Arena<ProductMatchSubject>,
    pub match_arms: Arena<ProductMatchArm>,
    pub match_fields: Arena<ProductMatchField>,
    pub proof_selectors: Arena<ProductProofSelector>,
    pub type_arguments: Arena<Handle<ProductOperandType>>,
    pub static_arguments: Arena<Handle<ProductStaticOperand>>,
    pub expression_arguments: Arena<Handle<ProductOperandExpression>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductStaticOperand {
    /// Captured from this real original-call argument attachment. The root
    /// Product descriptor has no such ordinary input obligation.
    pub authored_selection: Handle<ProductOperandSelection>,
    pub path: HandleSpan<ProductOperandName>,
    pub application: Handle<ProductStaticApplication>,
    pub structural_type: Handle<ProductOperandType>,
    pub integer: Option<numerics::literals::IntegerLiteral>,
    pub projection: Option<(Handle<ProductOperandName>, Handle<ProductOperandName>)>,
    pub declaration: Option<SourceSpan>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductStaticApplication {
    pub lifetimes: HandleSpan<ProductOperandName>,
    pub arguments: HandleSpan<Handle<ProductStaticOperand>>,
    pub member: Handle<ProductStaticOperand>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductOperandType {
    /// Already evaluated, typed const actual from a checked specialization.
    /// Ordinary const replay validates its independent receiving carrier.
    CanonicalConst(crate::const_value::CanonicalConstValue),
    #[default]
    Invalid,
    Unit,
    Named(Handle<ProductOperandName>),
    Generic {
        name: Handle<ProductOperandName>,
        lifetimes: HandleSpan<ProductOperandName>,
        arguments: HandleSpan<Handle<ProductOperandType>>,
    },
    Reference {
        referee: Handle<ProductOperandType>,
        access: language_core::ReferenceAccess,
        lifetime: Option<Handle<ProductOperandName>>,
    },
    Array {
        element: Handle<ProductOperandType>,
        length: ProductArrayLength,
    },
    Slice(Handle<ProductOperandType>),
    Constrained {
        base: Handle<ProductOperandType>,
        constraints: HandleSpan<ProductOperandConstraint>,
    },
    Dynamic {
        name: Handle<ProductOperandName>,
        arguments: HandleSpan<Handle<ProductOperandType>>,
    },
    ConstExpression(Handle<ProductOperandExpression>),
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductArrayLength {
    Literal(usize),
    Expression(Handle<ProductOperandType>),
    Parameter(Handle<ProductOperandName>),
    Call(Handle<ProductOperandName>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductOperandConstraint {
    #[default]
    Invalid,
    Named(Handle<ProductOperandName>),
    Range {
        minimum: Handle<ProductOperandExpression>,
        maximum: Handle<ProductOperandExpression>,
        inclusive: bool,
    },
    Arithmetic(numerics::arithmetic::ArithmeticDomain),
    Domain {
        name: Handle<ProductOperandName>,
        arguments: HandleSpan<Handle<ProductOperandType>>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductOperandExpression {
    pub source_span: SourceSpan,
    pub authored_selections: HandleSpan<ProductOperandSelection>,
    pub kind: ProductExpressionKind,
}

/// Coordinates issued from actual expression-to-ledger attachments. Local
/// occurrence IDs and compiler-copy partition ordinals never escape their
/// owning program; receiving preparation rejoins these immutable source facts.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductOperandSelection {
    pub source_span: SourceSpan,
    pub exposure: super::AuthoredDeclarationSelectionExposure,
    /// None is only the arena absence row, never an authored selection kind.
    pub kind: Option<super::AuthoredDeclarationSelectionKind>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductOperandField {
    pub name: Handle<ProductOperandName>,
    pub value: Handle<ProductOperandExpression>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductExpressionKind {
    #[default]
    Invalid,
    Match {
        subject: Handle<ProductOperandExpression>,
        capture: Handle<ProductMatchSubject>,
        arms: HandleSpan<ProductMatchArm>,
    },
    SavedSubject {
        capture: Handle<ProductMatchSubject>,
        pattern_head: Handle<ProductOperandExpression>,
    },
    Borrow {
        target: Handle<ProductOperandExpression>,
        access: language_core::ReferenceAccess,
    },
    Atomic {
        value: Handle<ProductOperandExpression>,
        result: Handle<ProductOperandExpression>,
        ordering: language_core::atomic::AtomicOrderingPlan,
        result_custody: language_core::atomic::AtomicExpressionResultCustody,
    },
    Range {
        start: Handle<ProductOperandExpression>,
        end: Handle<ProductOperandExpression>,
        inclusive: bool,
    },
    Membership {
        value: Handle<ProductOperandExpression>,
        domain: HandleSpan<ProductOperandName>,
        declaration: Option<SourceSpan>,
        case_type: Option<SourceSpan>,
        case: Option<SourceSpan>,
    },
    ZeroValue(Handle<ProductOperandType>),
    UndeterminedNaN(numerics::literals::FloatFormat),
    Integer(numerics::literals::IntegerLiteral),
    Boolean(bool),
    Float(numerics::literals::FloatLiteral),
    Text(Vec<u8>),
    Name(HandleSpan<ProductOperandName>),
    Unary {
        operator: ProductUnaryOperator,
        operand: Handle<ProductOperandExpression>,
    },
    Binary {
        left: Handle<ProductOperandExpression>,
        operator: ProductBinaryOperator,
        right: Handle<ProductOperandExpression>,
    },
    Call {
        receiver: Handle<ProductOperandExpression>,
        target: Handle<ProductOperandName>,
        owner: Handle<ProductOperandType>,
        static_arguments: HandleSpan<Handle<ProductStaticOperand>>,
        arguments: HandleSpan<Handle<ProductOperandExpression>>,
        evidence: HandleSpan<ProductOperandName>,
        acknowledgement: crate::CallOperationalAcknowledgement,
    },
    Member {
        receiver: Handle<ProductOperandExpression>,
        member: Handle<ProductOperandName>,
    },
    Indexed {
        receiver: Handle<ProductOperandExpression>,
        index: Handle<ProductOperandExpression>,
    },
    Array(HandleSpan<Handle<ProductOperandExpression>>),
    Record {
        name: Handle<ProductOperandName>,
        case: Option<Handle<ProductOperandName>>,
        fields: HandleSpan<ProductOperandField>,
    },
    Cast {
        value: Handle<ProductOperandExpression>,
        target: Handle<ProductOperandType>,
        policy: numerics::arithmetic::ArithmeticDomain,
        domain: HandleSpan<ProductOperandName>,
        domain_arguments: HandleSpan<Handle<ProductOperandType>>,
        form: language_core::cast_form::CastForm,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductUnaryOperator {
    BitwiseNot,
    LogicalNot,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductBinaryOperator {
    /// Typed tag observation; its exact right case remains in the owned graph.
    CaseMembership,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductMatchSubject {
    pub subject: Handle<ProductOperandExpression>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductMatchArm {
    pub pattern: ProductMatchPattern,
    pub value: Handle<ProductOperandExpression>,
    pub source_span: SourceSpan,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductMatchPattern {
    #[default]
    Wildcard,
    Value(Handle<ProductOperandExpression>),
    Destructure {
        head: Handle<ProductOperandExpression>,
        fields: HandleSpan<ProductMatchField>,
        has_rest: bool,
        guard: Handle<ProductOperandExpression>,
        condition: Handle<ProductOperandExpression>,
        proof_selectors: HandleSpan<ProductProofSelector>,
    },
}
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductMatchField {
    pub member: Handle<ProductOperandName>,
    pub action: ProductMatchFieldAction,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum ProductMatchFieldAction {
    Bind(Handle<ProductOperandName>),
    Equal(Handle<ProductOperandExpression>),
    #[default]
    Waive,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductProofSelector {
    pub output: Handle<ProductOperandName>,
    pub binding: Handle<ProductOperandName>,
}

/// Actual static substitutions of the checked helper activation that executed
/// an inert Product request. Arena handles belong only to this owned graph;
/// source declaration coordinates preserve exact binder and supplier scopes.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductStaticEnvironment {
    /// Canonical comparison input from the exact specialization receipt, not
    /// an alternate type/value decoder. Semantic actuals remain in `graph`.
    pub identity: Vec<u8>,
    pub template: Option<SourceSpan>,
    pub lexical_owner: Option<SourceSpan>,
    pub graph: ProductStaticOperandGraph,
    pub bindings: Arena<ProductStaticEnvironmentBinding>,
    pub lifetimes: Arena<ProductStaticEnvironmentLifetime>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductStaticEnvironmentBinding {
    pub parameter: SourceSpan,
    pub argument: Handle<ProductStaticOperand>,
    pub lexical_owner: SourceSpan,
    pub declared_type: Handle<ProductOperandType>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductStaticEnvironmentLifetime {
    pub parameter_owner: SourceSpan,
    pub parameter_ordinal: u32,
    pub argument: Handle<ProductOperandName>,
    pub lexical_owner: SourceSpan,
}

impl ProductStaticOperandGraph {
    /// Rebase retained source coordinates through the owning SourceMap's exact
    /// correspondence. Graph handles and semantic operands stay unchanged.
    pub fn remap_source_spans(&mut self, mut remap: impl FnMut(SourceSpan) -> SourceSpan) {
        self.names.for_each_mut(|_, name| {
            name.source_span = remap(name.source_span);
            name.declaration = name.declaration.map(&mut remap);
        });
        self.operands.for_each_mut(|_, operand| {
            operand.declaration = operand.declaration.map(&mut remap);
        });
        self.expressions.for_each_mut(|_, expression| {
            expression.source_span = remap(expression.source_span);
            if let ProductExpressionKind::Membership {
                declaration,
                case_type,
                case,
                ..
            } = &mut expression.kind
            {
                *declaration = declaration.map(&mut remap);
                *case_type = case_type.map(&mut remap);
                *case = case.map(&mut remap);
            }
        });
        self.selections
            .for_each_mut(|_, selection| selection.source_span = remap(selection.source_span));
        self.match_arms
            .for_each_mut(|_, arm| arm.source_span = remap(arm.source_span));
    }

    /// The complete authored operand, rather than only its leading name.
    /// Declaration anchors and foreign supplying expressions do not enlarge
    /// this source range; their separate coordinates retain their own custody.
    pub fn authored_operand_span(&self, leading: SourceSpan) -> SourceSpan {
        let mut end = leading.span.end;
        let mut include = |span: SourceSpan| {
            if span.source_id == leading.source_id && span.span.start >= leading.span.start {
                end = end.max(span.span.end);
            }
        };
        for (_, name) in self.names.iter() {
            include(name.source_span);
        }
        for (_, expression) in self.expressions.iter() {
            include(expression.source_span);
        }
        for (_, arm) in self.match_arms.iter() {
            include(arm.source_span);
        }
        SourceSpan::new(
            leading.source_id,
            source::Span::new(leading.span.start, end),
        )
    }
}
