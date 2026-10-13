//! Closed projection of structural crash expressions.
//!
//! Preserve semantic operations recursively, independently of their source
//! arrangement. A collection observation retains its exact entry root and
//! selectors, and conversions retain their policy; neither is interchangeable
//! with an ordinary scalar parameter. Private storage roots have no public
//! entry coordinate and reject rather than exporting compiler-local handles.

use crate::record::{
    PackageReviewArithmeticDomain, PackageReviewBooleanExpression,
    PackageReviewIeeeFloatComparisonKind, PackageReviewIntegerBinaryKind,
    PackageReviewIntegerComparisonKind, PackageReviewIntegerLiteral,
    PackageReviewIntegerLiteralLanding, PackageReviewIntegerRange, PackageReviewPrimitiveType,
    PackageReviewRuntimeIndex, PackageReviewScalarExpression,
    PackageReviewStructuralParameterField, PackageReviewStructuralPredicatePathSegment,
};

pub(in crate::capture::behavior) fn project_boolean_expression(
    expression: &checked_trees::CheckedBooleanExpression,
) -> Option<PackageReviewBooleanExpression> {
    use checked_trees::CheckedBooleanExpression;

    Some(match expression {
        // Review evidence has no mathematical integer vocabulary. Refuse the
        // entire projection rather than erasing a requirement or reinterpreting
        // its unbounded operations as finite scalar arithmetic.
        CheckedBooleanExpression::IntegerMathComparison { .. } => return None,
        CheckedBooleanExpression::ErasedParameter { .. } => return None,
        CheckedBooleanExpression::StorageRead { .. } => return None,
        CheckedBooleanExpression::ByteSequencePredicate { .. } => return None,
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
            path: project_structural_path(path)?,
        },
        CheckedBooleanExpression::Not(operand) => {
            PackageReviewBooleanExpression::Not(Box::new(project_boolean_expression(operand)?))
        }
        CheckedBooleanExpression::Equal { left, right } => PackageReviewBooleanExpression::Equal {
            left: Box::new(project_boolean_expression(left)?),
            right: Box::new(project_boolean_expression(right)?),
        },
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            PackageReviewBooleanExpression::IntegerComparison {
                kind: project_integer_comparison_kind(*kind),
                left: Box::new(project_scalar_expression(left)?),
                right: Box::new(project_scalar_expression(right)?),
            }
        }
        CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } => PackageReviewBooleanExpression::IeeeFloatComparison {
            kind: match kind {
                checked_trees::CheckedIeeeFloatComparisonKind::Equal => {
                    PackageReviewIeeeFloatComparisonKind::Equal
                }
                checked_trees::CheckedIeeeFloatComparisonKind::NotEqual => {
                    PackageReviewIeeeFloatComparisonKind::NotEqual
                }
                checked_trees::CheckedIeeeFloatComparisonKind::LessThan => {
                    PackageReviewIeeeFloatComparisonKind::LessThan
                }
                checked_trees::CheckedIeeeFloatComparisonKind::LessOrEqual => {
                    PackageReviewIeeeFloatComparisonKind::LessOrEqual
                }
                checked_trees::CheckedIeeeFloatComparisonKind::NotLessThan => {
                    PackageReviewIeeeFloatComparisonKind::NotLessThan
                }
                checked_trees::CheckedIeeeFloatComparisonKind::NotLessOrEqual => {
                    PackageReviewIeeeFloatComparisonKind::NotLessOrEqual
                }
            },
            primitive_type: project_primitive_type(*primitive_type),
            left: project_structural_field(left)?,
            right: project_structural_field(right)?,
        },
        CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } => {
            PackageReviewBooleanExpression::ScalarIeeeFloatComparison {
                kind: match kind {
                    checked_trees::CheckedIeeeFloatComparisonKind::Equal => {
                        PackageReviewIeeeFloatComparisonKind::Equal
                    }
                    checked_trees::CheckedIeeeFloatComparisonKind::NotEqual => {
                        PackageReviewIeeeFloatComparisonKind::NotEqual
                    }
                    checked_trees::CheckedIeeeFloatComparisonKind::LessThan => {
                        PackageReviewIeeeFloatComparisonKind::LessThan
                    }
                    checked_trees::CheckedIeeeFloatComparisonKind::LessOrEqual => {
                        PackageReviewIeeeFloatComparisonKind::LessOrEqual
                    }
                    checked_trees::CheckedIeeeFloatComparisonKind::NotLessThan => {
                        PackageReviewIeeeFloatComparisonKind::NotLessThan
                    }
                    checked_trees::CheckedIeeeFloatComparisonKind::NotLessOrEqual => {
                        PackageReviewIeeeFloatComparisonKind::NotLessOrEqual
                    }
                },
                left: Box::new(project_scalar_expression(left)?),
                right: Box::new(project_scalar_expression(right)?),
            }
        }
        CheckedBooleanExpression::ByteSequenceEqual { left, right } => {
            PackageReviewBooleanExpression::ByteSequenceEqual {
                left: project_structural_field(&left.parameter_field()?)?,
                right: project_structural_field(&right.parameter_field()?)?,
            }
        }
        CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } => {
            PackageReviewBooleanExpression::PayloadlessSumEqual {
                left: project_structural_field(left)?,
                right: project_structural_field(right)?,
                cases: cases.iter().map(|case| case.spelling()).collect(),
            }
        }
        CheckedBooleanExpression::StructuralCaseMembership { subject, case } => {
            PackageReviewBooleanExpression::StructuralCaseMembership {
                subject: project_structural_field(&subject.parameter_field()?)?,
                case: case.spelling(),
            }
        }
        CheckedBooleanExpression::And { left, right } => PackageReviewBooleanExpression::And {
            left: Box::new(project_boolean_expression(left)?),
            right: Box::new(project_boolean_expression(right)?),
        },
        CheckedBooleanExpression::Or { left, right } => PackageReviewBooleanExpression::Or {
            left: Box::new(project_boolean_expression(left)?),
            right: Box::new(project_boolean_expression(right)?),
        },
    })
}

fn project_scalar_expression(
    expression: &checked_trees::CheckedScalarExpression,
) -> Option<PackageReviewScalarExpression> {
    use checked_trees::CheckedScalarExpression;

    Some(match expression {
        CheckedScalarExpression::ErasedParameter { .. } => return None,
        CheckedScalarExpression::StorageRead { .. } => return None,
        // A selector has no public crash-predicate vocabulary.
        CheckedScalarExpression::ImplementationSelection { .. } => return None,
        CheckedScalarExpression::StructuralSequenceLength { root, path } => {
            PackageReviewScalarExpression::StructuralParameterByteLength {
                parameter_position: root.parameter()?,
                path: project_structural_path(
                    &checked_trees::CheckedScalarReadProjection::static_path(path)?,
                )?,
            }
        }
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            path,
            index,
            element_path,
            primitive_type,
        } => {
            // Public records retain the existing canonical vocabulary. Initial
            // consecutive selectors are scalar expressions; selectors following
            // members need a public coordinate. Never export source occurrences.
            let prefix_count = element_path
                .iter()
                .take_while(|step| {
                    matches!(
                        step,
                        checked_trees::CheckedScalarReadProjection::RuntimeIndex(_)
                    )
                })
                .count();
            let inner_indexes = element_path[..prefix_count]
                .iter()
                .map(|step| {
                    let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) = step
                    else {
                        return None;
                    };
                    project_scalar_expression(&selector.expression)
                })
                .collect::<Option<_>>()?;
            let element_path = element_path[prefix_count..]
                .iter()
                .map(|step| match step {
                    checked_trees::CheckedScalarReadProjection::Static(segment) => {
                        project_structural_path(std::slice::from_ref(segment))?.pop()
                    }
                    checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                        let coordinate = match &selector.expression {
                            CheckedScalarExpression::Parameter { position, .. } => {
                                PackageReviewRuntimeIndex::Parameter {
                                    position: u32::try_from(*position).ok()?,
                                }
                            }
                            CheckedScalarExpression::Local { position, .. } => {
                                PackageReviewRuntimeIndex::Local {
                                    position: u32::try_from(*position).ok()?,
                                }
                            }
                            CheckedScalarExpression::StructuralParameterField {
                                parameter_position,
                                path,
                                ..
                            } => PackageReviewRuntimeIndex::StructuralField(
                                PackageReviewStructuralParameterField {
                                    parameter_position: *parameter_position,
                                    path: project_structural_path(path)?,
                                },
                            ),
                            _ => return None,
                        };
                        Some(PackageReviewStructuralPredicatePathSegment::RuntimeIndex(
                            coordinate,
                        ))
                    }
                })
                .collect::<Option<_>>()?;
            PackageReviewScalarExpression::StructuralParameterIndexedRead {
                parameter_position: root.parameter()?,
                path: project_structural_path(path)?,
                index: Box::new(project_scalar_expression(&index.expression)?),
                inner_indexes,
                element_path,
                primitive_type: project_primitive_type(*primitive_type),
            }
        }
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
            path: project_structural_path(path)?,
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
        CheckedScalarExpression::IeeeFloatLiteral { value } => match value {
            semantic_vocabulary::IeeeFloatValue::Binary32(bits) => {
                PackageReviewScalarExpression::Float32Literal(*bits)
            }
            semantic_vocabulary::IeeeFloatValue::Binary64(bits) => {
                PackageReviewScalarExpression::Float64Literal(*bits)
            }
        },
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
            ..
        } => PackageReviewScalarExpression::IntegerBinary {
            kind: project_integer_binary_kind(*kind),
            primitive_type: project_primitive_type(*primitive_type),
            left: Box::new(project_scalar_expression(left)?),
            right: Box::new(project_scalar_expression(right)?),
        },
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => PackageReviewScalarExpression::IntegerBitwiseNot {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)?),
        },
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => PackageReviewScalarExpression::IntegerWiden {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)?),
        },
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
            ..
        } => PackageReviewScalarExpression::IntegerExactCast {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)?),
            range: PackageReviewIntegerRange {
                minimum: range.minimum.to_string(),
                maximum: range.maximum.to_string(),
            },
        },
        CheckedScalarExpression::RepresentationView {
            primitive_type,
            operand,
        } => PackageReviewScalarExpression::RepresentationView {
            primitive_type: project_primitive_type(*primitive_type),
            operand: Box::new(project_scalar_expression(operand)?),
        },
        CheckedScalarExpression::Boolean(expression) => PackageReviewScalarExpression::Boolean(
            Box::new(project_boolean_expression(expression)?),
        ),
    })
}

fn project_structural_field(
    field: &checked_trees::CheckedStructuralParameterField,
) -> Option<PackageReviewStructuralParameterField> {
    Some(PackageReviewStructuralParameterField {
        parameter_position: field.parameter_position,
        path: project_structural_path(&field.path)?,
    })
}

fn project_structural_path(
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Option<Vec<PackageReviewStructuralPredicatePathSegment>> {
    path.iter()
        .map(|segment| {
            Some(match segment {
                checked_trees::CheckedStructuralPredicatePathSegment::Field(field) => {
                    PackageReviewStructuralPredicatePathSegment::Field(field.spelling())
                }
                checked_trees::CheckedStructuralPredicatePathSegment::Case(case) => {
                    PackageReviewStructuralPredicatePathSegment::Case(case.spelling())
                }
                checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(element_index) => {
                    PackageReviewStructuralPredicatePathSegment::FixedIndex(*element_index)
                }
                checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(index) => {
                    PackageReviewStructuralPredicatePathSegment::RuntimeIndex(
                        project_runtime_index(index)?,
                    )
                }
            })
        })
        .collect()
}

fn project_runtime_index(
    index: &checked_trees::CheckedRuntimeIndex,
) -> Option<PackageReviewRuntimeIndex> {
    Some(match index {
        checked_trees::CheckedRuntimeIndex::StorageRead { .. } => return None,
        checked_trees::CheckedRuntimeIndex::Parameter { position } => {
            PackageReviewRuntimeIndex::Parameter {
                position: *position,
            }
        }
        checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth } => {
            PackageReviewRuntimeIndex::AssignmentIndex { depth: *depth }
        }
        checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
            PackageReviewRuntimeIndex::StructuralField(project_structural_field(field)?)
        }
        checked_trees::CheckedRuntimeIndex::Local { position } => {
            PackageReviewRuntimeIndex::Local {
                position: *position,
            }
        }
    })
}

const fn project_primitive_type(
    primitive_type: typed_trees::types::PrimitiveType,
) -> PackageReviewPrimitiveType {
    use typed_trees::types::PrimitiveType;
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
        PrimitiveType::Selector => PackageReviewPrimitiveType::Selector,
    }
}

const fn project_landed_integer_type(
    landed_type: numerics::literals::LandedIntegerType,
) -> PackageReviewPrimitiveType {
    use numerics::literals::LandedIntegerType;
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
    domain: numerics::arithmetic::ArithmeticDomain,
) -> PackageReviewArithmeticDomain {
    use numerics::arithmetic::ArithmeticDomain;
    match domain {
        ArithmeticDomain::Exact => PackageReviewArithmeticDomain::Exact,
        ArithmeticDomain::Wrapping => PackageReviewArithmeticDomain::Wrapping,
        ArithmeticDomain::Saturating => PackageReviewArithmeticDomain::Saturating,
        ArithmeticDomain::Trapping => PackageReviewArithmeticDomain::Trapping,
    }
}

const fn project_integer_comparison_kind(
    kind: checked_trees::CheckedIntegerComparisonKind,
) -> PackageReviewIntegerComparisonKind {
    match kind {
        checked_trees::CheckedIntegerComparisonKind::Equal => {
            PackageReviewIntegerComparisonKind::Equal
        }
        checked_trees::CheckedIntegerComparisonKind::LessThan => {
            PackageReviewIntegerComparisonKind::LessThan
        }
        checked_trees::CheckedIntegerComparisonKind::LessOrEqual => {
            PackageReviewIntegerComparisonKind::LessOrEqual
        }
    }
}

const fn project_integer_binary_kind(
    kind: checked_trees::CheckedIntegerBinaryKind,
) -> PackageReviewIntegerBinaryKind {
    use checked_trees::CheckedIntegerBinaryKind;
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
        CheckedIntegerBinaryKind::SaturatingShiftLeft => {
            PackageReviewIntegerBinaryKind::SaturatingShiftLeft
        }
        CheckedIntegerBinaryKind::SaturatingShiftRight => {
            PackageReviewIntegerBinaryKind::SaturatingShiftRight
        }
        CheckedIntegerBinaryKind::ExactShiftRight => {
            PackageReviewIntegerBinaryKind::ExactShiftRight
        }
        CheckedIntegerBinaryKind::TrappingShiftLeft => {
            PackageReviewIntegerBinaryKind::TrappingShiftLeft
        }
        CheckedIntegerBinaryKind::TrappingShiftRight => {
            PackageReviewIntegerBinaryKind::TrappingShiftRight
        }
        CheckedIntegerBinaryKind::TrappingAdd => PackageReviewIntegerBinaryKind::TrappingAdd,
        CheckedIntegerBinaryKind::TrappingSubtract => {
            PackageReviewIntegerBinaryKind::TrappingSubtract
        }
        CheckedIntegerBinaryKind::TrappingMultiply => {
            PackageReviewIntegerBinaryKind::TrappingMultiply
        }
        CheckedIntegerBinaryKind::TrappingDivide => PackageReviewIntegerBinaryKind::TrappingDivide,
        CheckedIntegerBinaryKind::TrappingRemainder => {
            PackageReviewIntegerBinaryKind::TrappingRemainder
        }
    }
}
