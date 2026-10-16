//! Closed structural runtime-requirement evidence used by crash review.

use super::contracts::PackageReviewArithmeticDomain;

pub type PackageReviewPrimitiveType = typed_trees::types::PrimitiveType;

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

impl PackageReviewStructuralParameterField {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageReviewIntegerComparisonKind {
    Equal,
    LessThan,
    LessOrEqual,
}

pub type PackageReviewIeeeFloatComparisonKind = semantic_vocabulary::IeeeFloatComparisonKind;

pub type PackageReviewIntegerBinaryKind = facts::CheckedIntegerBinaryKind;

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
    /// Proof identity of whole values, typed by the enclosing entry telescope.
    /// This is not a request to execute a field or element comparison.
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
