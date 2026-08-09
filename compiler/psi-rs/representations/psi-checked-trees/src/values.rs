use psi_arena::{Arena, Handle};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    Return,
    Guard,
    TransitionArgument { argument_ordinal: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarExpression {
    Parameter {
        position: usize,
        primitive_type: psi_typed_trees::types::PrimitiveType,
    },
    IntegerLiteral {
        literal: psi_numerics::literals::IntegerLiteral,
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
pub enum CheckedIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}
