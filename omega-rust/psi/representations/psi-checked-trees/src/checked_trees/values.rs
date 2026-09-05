use psi_arena::{Arena, Handle, HandleSpan};
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::types::TypeReferenceHandle;

pub type CheckedValueHandle = Handle<CheckedValueFact>;

/// A checker-established inclusive integer interval for one value at its exact
/// use site. Big integers preserve the full `u64` proof domain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedIntegerRange {
    pub minimum: psi_numerics::bignum::BigInt,
    pub maximum: psi_numerics::bignum::BigInt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckedValueOrigin {
    MachineDecrease {
        machine_symbol: SymbolHandle,
        ordinal: usize,
    },
    MachineOwnedDataInitializer {
        machine_symbol: SymbolHandle,
        data_symbol: SymbolHandle,
    },
    StateStatement {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        role: CheckedValueStatementRole,
    },
    NestedExpression {
        parent: ExpressionHandle,
    },
}

impl Default for CheckedValueOrigin {
    fn default() -> Self {
        Self::NestedExpression {
            parent: ExpressionHandle::invalid(),
        }
    }
}

impl CheckedValueOrigin {
    pub const fn machine_symbol(self) -> Option<SymbolHandle> {
        match self {
            Self::MachineDecrease { machine_symbol, .. }
            | Self::MachineOwnedDataInitializer { machine_symbol, .. }
            | Self::StateStatement { machine_symbol, .. } => Some(machine_symbol),
            Self::NestedExpression { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CheckedValueStatementRole {
    #[default]
    Expression,
    AssignmentTargetSubexpression,
    AssignmentValue,
    CallArgument,
    LocalInitializer,
    TransitionGuard,
    TransitionTargetArgument,
    TransitionTargetValue,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedValueFact {
    pub expression: ExpressionHandle,
    pub origin: CheckedValueOrigin,
    /// The checker-resolved declared type of this value at its exact use site.
    /// Invalid means the expression has no standalone declared type (for
    /// example, an anonymous literal) or resolution conservatively failed.
    /// Later lowering may consume validated declaration facts through this
    /// handle; it must never reconstruct a stronger type from storage shape.
    pub type_reference: TypeReferenceHandle,
    /// Exact scalar carrier selected at this use site. This remains available
    /// when a context-typed literal has no standalone type-reference handle
    /// (notably validated builtin operands).
    pub primitive_type: Option<psi_typed_trees::types::PrimitiveType>,
    /// The range discharged by Psi for this value in its origin context,
    /// including stable flow guards and retained boundary witnesses.
    pub integer_range: Option<CheckedIntegerRange>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedValueFacts {
    pub values: Arena<CheckedValueFact>,
    pub scalar_expressions: CheckedScalarExpressionPlans,
}

impl CheckedValueFacts {
    pub fn with_roots(values: Arena<CheckedValueFact>) -> Self {
        Self {
            values,
            scalar_expressions: CheckedScalarExpressionPlans::default(),
        }
    }

    pub fn expression_values(
        &self,
        expression: ExpressionHandle,
    ) -> impl Iterator<Item = (CheckedValueHandle, &CheckedValueFact)> + '_ {
        self.values
            .iter()
            .filter(move |(_, value)| value.expression == expression)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedScalarExpressionPlans {
    pub expressions: Vec<CheckedLocatedScalarExpression>,
    /// Source custody for plans consumed by proof. Plans without a retained
    /// binding row cannot recover values by reconstructing a positional scope.
    pub source_bindings: Arena<CheckedScalarExpressionBindings>,
    pub binding_symbols: Arena<SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarExpressionBindings {
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub role: CheckedScalarExpressionRole,
    pub expression: ExpressionHandle,
    /// The producer's dense scalar namespace: parameters followed by locals.
    /// This records declarations only, never initializer expressions to replay.
    pub symbols: HandleSpan<SymbolHandle>,
}

impl Default for CheckedScalarExpressionBindings {
    fn default() -> Self {
        Self {
            state: SymbolHandle::invalid(),
            statement_ordinal: 0,
            role: CheckedScalarExpressionRole::Return,
            expression: ExpressionHandle::invalid(),
            symbols: HandleSpan::empty(),
        }
    }
}

impl CheckedScalarExpressionPlans {
    pub fn expression_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
    ) -> Option<&CheckedScalarExpression> {
        self.expressions
            .iter()
            .find(|expression| {
                expression.state == state
                    && expression.statement_ordinal == statement_ordinal
                    && expression.role == role
            })
            .map(|expression| &expression.expression)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedLocatedScalarExpression {
    pub state: SymbolHandle,
    pub statement_ordinal: u32,
    pub role: CheckedScalarExpressionRole,
    pub expression: CheckedScalarExpression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedScalarExpressionRole {
    LocalInitializer {
        binding_ordinal: u32,
    },
    CallArgument {
        binding_ordinal: u32,
        argument_ordinal: u32,
    },
    /// Primitive argument to a bodyless boundary call, keyed by the exact
    /// call coordinate within its statement and dense scalar-parameter order.
    BoundaryCallArgument {
        call_ordinal: u32,
        argument_ordinal: u32,
    },
    /// Primitive argument to an in-module Unit call, keyed by the exact call
    /// coordinate and dense scalar-parameter order. Structural arguments keep
    /// their separate checked custody rows.
    UnitCallArgument {
        call_ordinal: u32,
        argument_ordinal: u32,
    },
    /// Right-hand side of one direct typed assignment. The coordinate remains
    /// statement-local and does not imply that every assignment is admitted
    /// by a later executable plan.
    AssignmentValue,
    Return,
    Guard,
    TransitionArgument {
        argument_ordinal: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarExpression {
    /// Dense position in the consuming plan's scalar parameter namespace. A
    /// mixed structural/scalar producer must separately retain the authored
    /// source-position partition.
    Parameter {
        position: usize,
        primitive_type: psi_typed_trees::types::PrimitiveType,
    },
    /// Dense position after scalar parameters and earlier primitive locals.
    Local {
        position: usize,
        primitive_type: psi_typed_trees::types::PrimitiveType,
    },
    /// Nonempty path to a relevant primitive field below one structural entry
    /// parameter. This form is retained only for structural crash predicates;
    /// ordinary scalar execution plans reject it.
    StructuralParameterField {
        parameter_position: u32,
        path: Vec<CheckedStructuralPredicatePathSegment>,
        primitive_type: psi_typed_trees::types::PrimitiveType,
    },
    IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral,
    },
    IeeeFloatLiteral {
        value: psi_core::IeeeFloatValue,
    },
    IntegerBinary {
        kind: CheckedIntegerBinaryKind,
        primitive_type: psi_typed_trees::types::PrimitiveType,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
    },
    IntegerBitwiseNot {
        primitive_type: psi_typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
    IntegerWiden {
        primitive_type: psi_typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
    },
    IntegerExactCast {
        primitive_type: psi_typed_trees::types::PrimitiveType,
        operand: Box<CheckedScalarExpression>,
        range: CheckedIntegerRange,
    },
    Boolean(Box<CheckedBooleanExpression>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedIntegerBinaryKind {
    ExactAdd,
    ExactSubtract,
    ExactMultiply,
    ExactDivide,
    ExactRemainder,
    WrappingDivide,
    WrappingRemainder,
    SaturatingDivide,
    SaturatingRemainder,
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    WrappingShiftLeft,
    WrappingShiftRight,
    ExactShiftLeft,
    ExactShiftRight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedBooleanExpression {
    Constant(bool),
    Parameter {
        position: usize,
    },
    Local {
        position: usize,
    },
    /// Nonempty path to a relevant Boolean field below one structural entry
    /// parameter. Terminal production resolves every authored identity to the
    /// canonical structural field ID at that exact type level.
    StructuralParameterField {
        parameter_position: u32,
        path: Vec<CheckedStructuralPredicatePathSegment>,
    },
    Not(Box<CheckedBooleanExpression>),
    Equal {
        left: Box<CheckedBooleanExpression>,
        right: Box<CheckedBooleanExpression>,
    },
    IntegerComparison {
        kind: CheckedIntegerComparisonKind,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
    },
    /// Atomic IEEE comparison between exact relevant structural leaves. It is
    /// deliberately not represented as generic/reflexive scalar equality.
    IeeeFloatComparison {
        kind: CheckedIeeeFloatComparisonKind,
        primitive_type: psi_typed_trees::types::PrimitiveType,
        left: CheckedStructuralParameterField,
        right: CheckedStructuralParameterField,
    },
    /// Content equality between exact byte-sequence structural leaves.
    ByteSequenceEqual {
        left: CheckedStructuralParameterField,
        right: CheckedStructuralParameterField,
    },
    /// Equality of two structural sums. Terminal lowering expands
    /// this closed case roster into canonical per-case membership equivalence.
    PayloadlessSumEqual {
        left: CheckedStructuralParameterField,
        right: CheckedStructuralParameterField,
        cases: Vec<String>,
    },
    /// Exact active-case test over a structural sum subject. The path reaches
    /// the sum itself; payload paths use an explicit following Case segment.
    StructuralCaseMembership {
        subject: CheckedStructuralParameterField,
        case: String,
    },
    And {
        left: Box<CheckedBooleanExpression>,
        right: Box<CheckedBooleanExpression>,
    },
    Or {
        left: Box<CheckedBooleanExpression>,
        right: Box<CheckedBooleanExpression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedIeeeFloatComparisonKind {
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedStructuralParameterField {
    pub parameter_position: u32,
    pub path: Vec<CheckedStructuralPredicatePathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedStructuralPredicatePathSegment {
    Field(String),
    Case(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}
