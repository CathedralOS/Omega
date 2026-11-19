//! The kinds of node a syntax tree can hold.

/// What a node in the tree is. Each kind corresponds to a rule in GRAMMAR.md.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NodeKind {
    // Roots.
    File,
    Fragment, // Declarations and statements mixed, as the spec's examples are written.

    // Declarations.
    Use,
    Const,
    Data,
    Field,
    Case,
    CaseField,
    StableNumber,
    Attributes,
    Attribute,
    DroppedBy,
    Domain,
    Access,
    Trait,
    Restriction,
    Conformance,
    SelectionRow,
    Hypothesis,
    Machine,
    AsmMachine,
    OperatorToken,
    MathLet,
    Path,

    // Generic parameters and signatures.
    GenericParams,
    TypeParam,
    KindedParam,
    ConstParam,
    MachineParam,
    SomeParam,
    LifetimeParam,
    Params,
    Param,
    Result,
    Body,

    // Header clauses.
    Requires,
    Ensures,
    Reaches,
    Crashes,
    Blocks,
    Suspends,
    Terminates,
    Invokes,
    Satisfies,
    Registers,
    Clobbers,

    // Types.
    ReferenceType,
    SliceType,
    ArrayType,
    MachineType,
    FunctionType,
    PathType,
    TypeArgs,
    QualifiedType,
    Lifetime,

    // Statements.
    Let,
    Assign,
    Discard,
    ExpressionStatement,
    Tail,
    AsmBlock,
    Instruction,
    AsmTransition,

    // match and transition.
    Match,
    Transition,
    Arm,
    WildcardPattern,
    BindingPattern,
    RecordPattern,
    FieldPattern,
    RestPattern,
    TuplePattern,
    ParenPattern,
    ValuePattern,

    // Expressions.
    Literal,
    PathExpression,
    ExpressionArgs,
    Paren,
    Tuple,
    RecordLiteral,
    FieldInit,
    Omission,
    ArrayLiteral,
    Selection,
    Call,
    Args,
    FieldAccess,
    Index,
    Range,
    Borrow,
    Unary,
    Move,
    CallMarker,
    Cast,
    CastEntry,
    Binary,

    /// A node that was opened and never given its kind. Finding one in a finished tree is a bug
    /// in the parser, and the tests check that none is left.
    Unfinished,
}
