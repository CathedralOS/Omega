//! Closed projection of structural crash expressions.

use crate::record::{
    PackageReviewArithmeticDomain, PackageReviewBooleanExpression,
    PackageReviewIeeeFloatComparisonKind, PackageReviewIntegerBinaryKind,
    PackageReviewIntegerComparisonKind, PackageReviewIntegerLiteral,
    PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange, PackageReviewPrimitiveType,
    PackageReviewScalarExpression, PackageReviewStructuralParameterField,
    PackageReviewStructuralPredicatePathSegment,
};

pub(super) fn project_boolean_expression(
    expression: &psi_checked_trees::CheckedBooleanExpression,
) -> PackageReviewBooleanExpression {
    use psi_checked_trees::CheckedBooleanExpression;

    match expression {
        CheckedBooleanExpression::Constant(value) => {
            PackageReviewBooleanExpression::Constant(*value)
        }
        CheckedBooleanExpression::Parameter { position } => {
            PackageReviewBooleanExpression::Parameter {
                position: *position,
            }
        }
        CheckedBooleanExpression::Local { position } => PackageReviewBooleanExpression::Local {
            position: *position,
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => PackageReviewBooleanExpression::StructuralParameterField {
            parameter_position: *parameter_position,
            path: project_structural_path(path),
        },
        CheckedBooleanExpression::Not(operand) => {
            PackageReviewBooleanExpression::Not(Box::new(project_boolean_expression(operand)))
        }
        CheckedBooleanExpression::Equal { left, right } => PackageReviewBooleanExpression::Equal {
            left: Box::new(project_boolean_expression(left)),
            right: Box::new(project_boolean_expression(right)),
        },
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            PackageReviewBooleanExpression::IntegerComparison {
                kind: project_integer_comparison_kind(*kind),
                left: Box::new(project_scalar_expression(left)),
                right: Box::new(project_scalar_expression(right)),
            }
        }
        CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => PackageReviewBooleanExpression::IeeeFloatComparison {
            kind: match kind {
                psi_checked_trees::CheckedIeeeFloatComparisonKind::Equal => {
                    PackageReviewIeeeFloatComparisonKind::Equal
                }
                psi_checked_trees::CheckedIeeeFloatComparisonKind::NotEqual => {
                    PackageReviewIeeeFloatComparisonKind::NotEqual
                }
            },
            primitive_type: project_primitive_type(*primitive_type),
            left: project_structural_field(left),
            right: project_structural_field(right),
        },
        CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
            PackageReviewBooleanExpression::ByteSequenceEqual {
                left: project_structural_field(left),
                right: project_structural_field(right),
            }
        }
        CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } => {
            PackageReviewBooleanExpression::PayloadlessSumEqual {
                left: project_structural_field(left),
                right: project_structural_field(right),
                cases: cases.clone(),
            }
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            PackageReviewBooleanExpression::StructuralCaseMembership {
                subject: project_structural_field(subject),
                case: case.clone(),
            }
        }
        CheckedBooleanExpression::And { left, right } => PackageReviewBooleanExpression::And {
            left: Box::new(project_boolean_expression(left)),
            right: Box::new(project_boolean_expression(right)),
        },
        CheckedBooleanExpression::Or { left, right } => PackageReviewBooleanExpression::Or {
            left: Box::new(project_boolean_expression(left)),
            right: Box::new(project_boolean_expression(right)),
        },
    }
}

fn project_scalar_expression(
    expression: &psi_checked_trees::CheckedScalarExpression,
) -> PackageReviewScalarExpression {
    use psi_checked_trees::CheckedScalarExpression;

    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => PackageReviewScalarExpression::Parameter {
            position: *position,
            primitive_type: project_primitive_type(*primitive_type),
        },
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => PackageReviewScalarExpression::Local {
            position: *position,
            primitive_type: project_primitive_type(*primitive_type),
        },
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            primitive_type,
        } => PackageReviewScalarExpression::StructuralParameterField {
            parameter_position: *parameter_position,
            path: project_structural_path(path),
            primitive_type: project_primitive_type(*primitive_type),
        },
        CheckedScalarExpression::IntegerLiteral { literal } => {
            PackageReviewScalarExpression::IntegerLiteral(PackageReviewIntegerLiteral {
                canonical_text: literal.text().to_owned(),
                landing: literal
                    .landing()
                    .map(|landing| PackageReviewIntegerLiteralLanding {
                        landed_type: project_landed_integer_type(landing.landed_type),
                        arithmetic_domain: project_arithmetic_domain(landing.domain),
                    }),
            })
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => PackageReviewScalarExpression::IntegerBinary {
            kind: project_integer_binary_kind(*kind),
            primitive_type: project_primitive_type(*primitive_type),
            left: Box::new(project_scalar_expression(left)),
            right: Box::new(project_scalar_expression(right)),
        },
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => PackageReviewScalarExpression::IntegerBitwiseNot {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)),
        },
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => PackageReviewScalarExpression::IntegerWiden {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)),
        },
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => PackageReviewScalarExpression::IntegerExactCast {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)),
            range: PackageReviewIntegerRange {
                minimum: range.minimum.to_string(),
                maximum: range.maximum.to_string(),
            },
        },
        CheckedScalarExpression::Boolean(expression) => {
            PackageReviewScalarExpression::Boolean(Box::new(project_boolean_expression(expression)))
        }
    }
}

fn project_structural_field(
    field: &psi_checked_trees::CheckedStructuralParameterField,
) -> PackageReviewStructuralParameterField {
    PackageReviewStructuralParameterField {
        parameter_position: field.parameter_position,
        path: project_structural_path(&field.path),
    }
}

fn project_structural_path(
    path: &[psi_checked_trees::CheckedStructuralPredicatePathSegment],
) -> Vec<PackageReviewStructuralPredicatePathSegment> {
    path.iter()
        .map(|segment| match segment {
            psi_checked_trees::CheckedStructuralPredicatePathSegment::Field(field) => {
                PackageReviewStructuralPredicatePathSegment::Field(field.clone())
            }
            psi_checked_trees::CheckedStructuralPredicatePathSegment::Case(case) => {
                PackageReviewStructuralPredicatePathSegment::Case(case.clone())
            }
        })
        .collect()
}

const fn project_primitive_type(
    primitive_type: psi_typed_trees::types::PrimitiveType,
) -> PackageReviewPrimitiveType {
    use psi_typed_trees::types::PrimitiveType;
    match primitive_type {
        PrimitiveType::Bool => PackageReviewPrimitiveType::Bool,
        PrimitiveType::F32 => PackageReviewPrimitiveType::F32,
        PrimitiveType::F64 => PackageReviewPrimitiveType::F64,
        PrimitiveType::I8 => PackageReviewPrimitiveType::I8,
        PrimitiveType::I16 => PackageReviewPrimitiveType::I16,
        PrimitiveType::I32 => PackageReviewPrimitiveType::I32,
        PrimitiveType::I64 => PackageReviewPrimitiveType::I64,
        PrimitiveType::U8 => PackageReviewPrimitiveType::U8,
        PrimitiveType::U16 => PackageReviewPrimitiveType::U16,
        PrimitiveType::U32 => PackageReviewPrimitiveType::U32,
        PrimitiveType::U64 => PackageReviewPrimitiveType::U64,
        PrimitiveType::Addr => PackageReviewPrimitiveType::Addr,
    }
}

const fn project_landed_integer_type(
    landed_type: psi_numerics::literals::LandedIntegerType,
) -> PackageReviewPrimitiveType {
    use psi_numerics::literals::LandedIntegerType;
    match landed_type {
        LandedIntegerType::I8 => PackageReviewPrimitiveType::I8,
        LandedIntegerType::I16 => PackageReviewPrimitiveType::I16,
        LandedIntegerType::I32 => PackageReviewPrimitiveType::I32,
        LandedIntegerType::I64 => PackageReviewPrimitiveType::I64,
        LandedIntegerType::U8 => PackageReviewPrimitiveType::U8,
        LandedIntegerType::U16 => PackageReviewPrimitiveType::U16,
        LandedIntegerType::U32 => PackageReviewPrimitiveType::U32,
        LandedIntegerType::U64 => PackageReviewPrimitiveType::U64,
        LandedIntegerType::Addr => PackageReviewPrimitiveType::Addr,
    }
}

const fn project_arithmetic_domain(
    domain: psi_numerics::arithmetic::ArithmeticDomain,
) -> PackageReviewArithmeticDomain {
    use psi_numerics::arithmetic::ArithmeticDomain;
    match domain {
        ArithmeticDomain::Exact => PackageReviewArithmeticDomain::Exact,
        ArithmeticDomain::Wrapping => PackageReviewArithmeticDomain::Wrapping,
        ArithmeticDomain::Saturating => PackageReviewArithmeticDomain::Saturating,
        ArithmeticDomain::Trapping => PackageReviewArithmeticDomain::Trapping,
    }
}

const fn project_integer_comparison_kind(
    kind: psi_checked_trees::CheckedIntegerComparisonKind,
) -> PackageReviewIntegerComparisonKind {
    match kind {
        psi_checked_trees::CheckedIntegerComparisonKind::Equal => {
            PackageReviewIntegerComparisonKind::Equal
        }
        psi_checked_trees::CheckedIntegerComparisonKind::LessThan => {
            PackageReviewIntegerComparisonKind::LessThan
        }
        psi_checked_trees::CheckedIntegerComparisonKind::LessOrEqual => {
            PackageReviewIntegerComparisonKind::LessOrEqual
        }
    }
}

const fn project_integer_binary_kind(
    kind: psi_checked_trees::CheckedIntegerBinaryKind,
) -> PackageReviewIntegerBinaryKind {
    use psi_checked_trees::CheckedIntegerBinaryKind;
    match kind {
        CheckedIntegerBinaryKind::ExactAdd => PackageReviewIntegerBinaryKind::ExactAdd,
        CheckedIntegerBinaryKind::ExactSubtract => PackageReviewIntegerBinaryKind::ExactSubtract,
        CheckedIntegerBinaryKind::ExactMultiply => PackageReviewIntegerBinaryKind::ExactMultiply,
        CheckedIntegerBinaryKind::ExactDivide => PackageReviewIntegerBinaryKind::ExactDivide,
        CheckedIntegerBinaryKind::ExactRemainder => PackageReviewIntegerBinaryKind::ExactRemainder,
        CheckedIntegerBinaryKind::WrappingDivide => PackageReviewIntegerBinaryKind::WrappingDivide,
        CheckedIntegerBinaryKind::WrappingRemainder => {
            PackageReviewIntegerBinaryKind::WrappingRemainder
        }
        CheckedIntegerBinaryKind::SaturatingDivide => {
            PackageReviewIntegerBinaryKind::SaturatingDivide
        }
        CheckedIntegerBinaryKind::SaturatingRemainder => {
            PackageReviewIntegerBinaryKind::SaturatingRemainder
        }
        CheckedIntegerBinaryKind::WrappingAdd => PackageReviewIntegerBinaryKind::WrappingAdd,
        CheckedIntegerBinaryKind::SaturatingAdd => PackageReviewIntegerBinaryKind::SaturatingAdd,
        CheckedIntegerBinaryKind::WrappingSubtract => {
            PackageReviewIntegerBinaryKind::WrappingSubtract
        }
        CheckedIntegerBinaryKind::SaturatingSubtract => {
            PackageReviewIntegerBinaryKind::SaturatingSubtract
        }
        CheckedIntegerBinaryKind::WrappingMultiply => {
            PackageReviewIntegerBinaryKind::WrappingMultiply
        }
        CheckedIntegerBinaryKind::SaturatingMultiply => {
            PackageReviewIntegerBinaryKind::SaturatingMultiply
        }
        CheckedIntegerBinaryKind::BitwiseAnd => PackageReviewIntegerBinaryKind::BitwiseAnd,
        CheckedIntegerBinaryKind::BitwiseOr => PackageReviewIntegerBinaryKind::BitwiseOr,
        CheckedIntegerBinaryKind::BitwiseXor => PackageReviewIntegerBinaryKind::BitwiseXor,
        CheckedIntegerBinaryKind::WrappingShiftLeft => {
            PackageReviewIntegerBinaryKind::WrappingShiftLeft
        }
        CheckedIntegerBinaryKind::WrappingShiftRight => {
            PackageReviewIntegerBinaryKind::WrappingShiftRight
        }
        CheckedIntegerBinaryKind::ExactShiftLeft => PackageReviewIntegerBinaryKind::ExactShiftLeft,
        CheckedIntegerBinaryKind::ExactShiftRight => {
            PackageReviewIntegerBinaryKind::ExactShiftRight
        }
    }
}
