// Closed structural runtime-requirement evidence used by crash review.
//
// Runtime indices project `CheckedRuntimeIndex` coordinates verbatim (scalar
// parameter, assignment index step, program-point field read, or immutable
// integer local). `StructuralParameterByteLength` measures live carrier extent
// rather than capacity; `StructuralParameterIndexedRead` retains ordered
// element selections and the final scalar projection so each index preserves
// its own extent obligation; `RepresentationView` observes storage bits under
// the target carrier without numeric conversion; float literals retain exact
// IEEE interchange bits (format, signed zero, and NaN payload); and
// `StructuralIdentity` records proof identity of whole values rather than an
// executable field-by-field comparison.

use super::contracts::PackageReviewArithmeticDomain;

pub type PackageReviewPrimitiveType = typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewStructuralPredicatePathSegment {
    Referent,
    Field(String),
    Case(String),
    FixedIndex(u64),
    RuntimeIndex(PackageReviewRuntimeIndex),
}

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

impl PackageReviewStructuralParameterField {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

pub type PackageReviewIeeeFloatComparisonKind = semantic_vocabulary::IeeeFloatComparisonKind;

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
    SaturatingShiftLeft,
    SaturatingShiftRight,
    TrappingShiftLeft,
    TrappingShiftRight,
    TrappingAdd,
    TrappingSubtract,
    TrappingMultiply,
    TrappingDivide,
    TrappingRemainder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewIntegerLiteralLanding {
    pub(crate) landed_type: PackageReviewPrimitiveType,
    pub(crate) arithmetic_domain: PackageReviewArithmeticDomain,
}

impl PackageReviewIntegerLiteralLanding {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewIntegerLiteral {
    pub(crate) canonical_text: String,
    pub(crate) landing: Option<PackageReviewIntegerLiteralLanding>,
}

impl PackageReviewIntegerLiteral {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewIntegerRange {
    pub(crate) minimum: String,
    pub(crate) maximum: String,
}

impl PackageReviewIntegerRange {}

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
    StructuralParameterByteLength {
        parameter_position: u32,
        path: Vec<PackageReviewStructuralPredicatePathSegment>,
    },
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
    RepresentationView {
        primitive_type: PackageReviewPrimitiveType,
        operand: Box<PackageReviewScalarExpression>,
    },
    Boolean(Box<PackageReviewBooleanExpression>),
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
    StructuralIdentity {
        left: PackageReviewStructuralParameterField,
        right: PackageReviewStructuralParameterField,
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
