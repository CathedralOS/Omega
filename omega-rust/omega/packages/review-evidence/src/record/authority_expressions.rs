//! Closed structural runtime-requirement evidence used by crash review.

use super::contracts::PackageReviewArithmeticDomain;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewPrimitiveType {
    Bool,
    F32,
    F64,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Addr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewStructuralPredicatePathSegment {
    Field(String),
    Case(String),
    FixedIndex(u64),
    RuntimeIndex(PackageReviewRuntimeIndex),
}

/// The runtime coordinate one element-path selector evaluates to — the same
/// vocabulary `CheckedRuntimeIndex` spells on carrier paths, projected
/// verbatim: a scalar parameter, an assignment's indexed step, a
/// program-point field read, or an immutable integer local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewRuntimeIndex {
    Parameter { position: u32 },
    AssignmentIndex { depth: u32 },
    StructuralField(PackageReviewStructuralParameterField),
    Local { position: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewStructuralParameterField {
    pub(crate) parameter_position: u32,
    pub(crate) path: Vec<PackageReviewStructuralPredicatePathSegment>,
}

impl PackageReviewStructuralParameterField {
    pub const fn parameter_position(&self) -> u32 {
        self.parameter_position
    }

    pub fn path(&self) -> &[PackageReviewStructuralPredicatePathSegment] {
        &self.path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewIeeeFloatComparisonKind {
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    NotLessThan,
    NotLessOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewIntegerBinaryKind {
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
    TrappingShiftLeft,
    TrappingShiftRight,
    TrappingAdd,
    TrappingSubtract,
    TrappingMultiply,
    TrappingDivide,
    TrappingRemainder,
    SaturatingShiftLeft,
    SaturatingShiftRight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewIntegerLiteralLanding {
    pub(crate) landed_type: PackageReviewPrimitiveType,
    pub(crate) arithmetic_domain: PackageReviewArithmeticDomain,
}

impl PackageReviewIntegerLiteralLanding {
    pub const fn landed_type(&self) -> PackageReviewPrimitiveType {
        self.landed_type
    }

    pub const fn arithmetic_domain(&self) -> PackageReviewArithmeticDomain {
        self.arithmetic_domain
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewIntegerLiteral {
    pub(crate) canonical_text: String,
    pub(crate) landing: Option<PackageReviewIntegerLiteralLanding>,
}

impl PackageReviewIntegerLiteral {
    pub fn canonical_text(&self) -> &str {
        &self.canonical_text
    }

    pub const fn landing(&self) -> Option<&PackageReviewIntegerLiteralLanding> {
        self.landing.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewIntegerRange {
    pub(crate) minimum: String,
    pub(crate) maximum: String,
}

impl PackageReviewIntegerRange {
    pub fn minimum(&self) -> &str {
        &self.minimum
    }

    pub fn maximum(&self) -> &str {
        &self.maximum
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewScalarExpression {
    Parameter {
        position: usize,
        primitive_type: PackageReviewPrimitiveType,
    },
    Local {
        position: usize,
        primitive_type: PackageReviewPrimitiveType,
    },
    StructuralParameterField {
        parameter_position: u32,
        path: Vec<PackageReviewStructuralPredicatePathSegment>,
        primitive_type: PackageReviewPrimitiveType,
    },
    /// Live extent of the selected entry carrier, not its capacity. Element
    /// views count elements; byte views and bounded byte fields count bytes.
    StructuralParameterByteLength {
        parameter_position: u32,
        path: Vec<PackageReviewStructuralPredicatePathSegment>,
    },
    /// Ordered element selections and the final scalar projection. Retaining
    /// each index separately preserves its own extent obligation; encoding
    /// this evidence neither proves the bounds nor authorizes the read.
    StructuralParameterIndexedRead {
        parameter_position: u32,
        path: Vec<PackageReviewStructuralPredicatePathSegment>,
        index: Box<PackageReviewScalarExpression>,
        inner_indexes: Vec<PackageReviewScalarExpression>,
        element_path: Vec<PackageReviewStructuralPredicatePathSegment>,
        primitive_type: PackageReviewPrimitiveType,
    },
    IntegerLiteral(PackageReviewIntegerLiteral),
    IntegerBinary {
        kind: PackageReviewIntegerBinaryKind,
        primitive_type: PackageReviewPrimitiveType,
        left: Box<PackageReviewScalarExpression>,
        right: Box<PackageReviewScalarExpression>,
    },
    IntegerBitwiseNot {
        primitive_type: PackageReviewPrimitiveType,
        operand: Box<PackageReviewScalarExpression>,
    },
    IntegerWiden {
        primitive_type: PackageReviewPrimitiveType,
        operand: Box<PackageReviewScalarExpression>,
    },
    IntegerExactCast {
        primitive_type: PackageReviewPrimitiveType,
        operand: Box<PackageReviewScalarExpression>,
        range: PackageReviewIntegerRange,
    },
    IntegerWrappingCast {
        primitive_type: PackageReviewPrimitiveType,
        operand: Box<PackageReviewScalarExpression>,
    },
    IntegerSaturatingCast {
        primitive_type: PackageReviewPrimitiveType,
        operand: Box<PackageReviewScalarExpression>,
    },
    /// Trapping policy is not an exactness or normal-return proof.
    IntegerTrappingCast {
        primitive_type: PackageReviewPrimitiveType,
        operand: Box<PackageReviewScalarExpression>,
    },
    /// Observe the operand's storage bits under the target carrier. This is
    /// distinct from numeric conversion; the operand retains its own type.
    RepresentationView {
        primitive_type: PackageReviewPrimitiveType,
        operand: Box<PackageReviewScalarExpression>,
    },
    Boolean(Box<PackageReviewBooleanExpression>),
    /// IEEE interchange bits, retaining format, signed zero and NaN payload.
    Float32Literal(u32),
    Float64Literal(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewBooleanExpression {
    Constant(bool),
    Parameter {
        position: usize,
    },
    Local {
        position: usize,
    },
    StructuralParameterField {
        parameter_position: u32,
        path: Vec<PackageReviewStructuralPredicatePathSegment>,
    },
    Not(Box<PackageReviewBooleanExpression>),
    Equal {
        left: Box<PackageReviewBooleanExpression>,
        right: Box<PackageReviewBooleanExpression>,
    },
    IntegerComparison {
        kind: PackageReviewIntegerComparisonKind,
        left: Box<PackageReviewScalarExpression>,
        right: Box<PackageReviewScalarExpression>,
    },
    IeeeFloatComparison {
        kind: PackageReviewIeeeFloatComparisonKind,
        primitive_type: PackageReviewPrimitiveType,
        left: PackageReviewStructuralParameterField,
        right: PackageReviewStructuralParameterField,
    },
    ScalarIeeeFloatComparison {
        kind: PackageReviewIeeeFloatComparisonKind,
        left: Box<PackageReviewScalarExpression>,
        right: Box<PackageReviewScalarExpression>,
    },
    ByteSequenceEqual {
        left: PackageReviewStructuralParameterField,
        right: PackageReviewStructuralParameterField,
    },
    PayloadlessSumEqual {
        left: PackageReviewStructuralParameterField,
        right: PackageReviewStructuralParameterField,
        cases: Vec<String>,
    },
    StructuralCaseMembership {
        subject: PackageReviewStructuralParameterField,
        case: String,
    },
    And {
        left: Box<PackageReviewBooleanExpression>,
        right: Box<PackageReviewBooleanExpression>,
    },
    Or {
        left: Box<PackageReviewBooleanExpression>,
        right: Box<PackageReviewBooleanExpression>,
    },
}
