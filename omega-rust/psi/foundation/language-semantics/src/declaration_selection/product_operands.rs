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
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductStaticOperandGraph {
    pub root: Handle<ProductStaticOperand>,
    pub names: Arena<ProductOperandName>,
    pub operands: Arena<ProductStaticOperand>,
    pub applications: Arena<ProductStaticApplication>,
    pub types: Arena<ProductOperandType>,
    pub expressions: Arena<ProductOperandExpression>,
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
    pub kind: ProductExpressionKind,
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
