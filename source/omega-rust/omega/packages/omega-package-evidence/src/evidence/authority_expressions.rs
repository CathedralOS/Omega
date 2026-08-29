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
    Boolean(Box<PackageReviewBooleanExpression>),
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
