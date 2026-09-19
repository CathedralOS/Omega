//! Structural crash route buckets and their root substitution.

use crate::proofs::crash_routes::crash_predicates::{
    boolean_input_budget, charge_boolean_expansion, contains_boolean_connective,
    flatten_checked_boolean_connective,
};
use crate::proofs::crash_routes::scalar_terms::{checked_boolean_scalar_term, checked_scalar_term};
use crate::proofs::crash_routes::structural_arithmetic::{
    safe_exact_structural_divisor, safe_exact_structural_shift, safe_policy_structural_divisor,
};
use crate::proofs::crash_routes::structural_members::{
    lower_byte_sequence_field, lower_ieee_float_field, lower_structural_member_term,
    lower_structural_sum_subject,
};
use crate::proofs::{
    BTreeMap, CanonicalStructuralPathSegment, CheckedBooleanExpression, CheckedIntegerBinaryKind,
    CheckedIntegerComparisonKind, CheckedScalarExpression, IeeeFloatFormat, LoweringError, PlaceId,
    PrimitiveType, Proposition, ScalarTerm, ScalarType, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalCrashCause, ValueDeclaration,
    integer_landing_scalar_type, integer_scalar_type, integer_value, unsupported,
};

pub(crate) fn lower_structural_crash_route_buckets(
    buckets: &[checked_trees::CrashRouteBucket],
    scalar_parameters: &[ValueDeclaration],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    runtime_requirements: &[Proposition],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, LoweringError> {
    fn checked_member_path(
        expression: &checked_trees::CrashPredicateExpression,
        path: &mut Vec<String>,
    ) -> Option<u32> {
        match expression {
            checked_trees::CrashPredicateExpression::Parameter(position) => Some(*position),
            checked_trees::CrashPredicateExpression::Member { receiver, member } => {
                let parameter = checked_member_path(receiver, path)?;
                path.push(member.clone());
                Some(parameter)
            }
            _ => None,
        }
    }

    fn lower_term(
        expression: &CheckedBooleanExpression,
        scalar_parameters: &[ValueDeclaration],
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        runtime_requirements: &[Proposition],
    ) -> Result<ScalarTerm, LoweringError> {
        fn lower_integer_term(
            expression: &CheckedScalarExpression,
            scalar_parameters: &[ValueDeclaration],
            parameters: &[StructuralParameterDeclaration],
            structural_types: &[StructuralTypeDeclaration],
            runtime_requirements: &[Proposition],
        ) -> Result<ScalarTerm, LoweringError> {
            match expression {
                CheckedScalarExpression::Parameter { .. } => {
                    checked_scalar_term(expression, scalar_parameters, &[])
                }
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position,
                    path,
                    primitive_type,
                } => {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported(
                            "structural crash integer member has a non-integer type",
                        );
                    };
                    lower_structural_member_term(
                        *parameter_position,
                        path,
                        ScalarType::Integer(integer_type),
                        parameters,
                        structural_types,
                    )
                }
                CheckedScalarExpression::IntegerLiteral { literal } => {
                    let scalar_type = integer_landing_scalar_type(literal)?;
                    let ScalarType::Integer(integer_type) = scalar_type else {
                        return unsupported(
                            "structural crash integer literal is not fixed-integer",
                        );
                    };
                    ScalarTerm::integer(integer_type, integer_value(literal, scalar_type)?)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBitwiseNot {
                    primitive_type,
                    operand,
                } => {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported("structural crash bitwise-not has a non-integer type");
                    };
                    let operand = lower_integer_term(
                        operand,
                        scalar_parameters,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    ScalarTerm::integer_bitwise_not(integer_type, operand)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                } if matches!(
                    kind,
                    CheckedIntegerBinaryKind::BitwiseAnd
                        | CheckedIntegerBinaryKind::BitwiseOr
                        | CheckedIntegerBinaryKind::BitwiseXor
                ) =>
                {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported(
                            "structural crash bitwise expression has a non-integer type",
                        );
                    };
                    let left = lower_integer_term(
                        left,
                        scalar_parameters,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    let right = lower_integer_term(
                        right,
                        scalar_parameters,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    match kind {
                        CheckedIntegerBinaryKind::BitwiseAnd => {
                            ScalarTerm::integer_bitwise_and(integer_type, left, right)
                        }
                        CheckedIntegerBinaryKind::BitwiseOr => {
                            ScalarTerm::integer_bitwise_or(integer_type, left, right)
                        }
                        CheckedIntegerBinaryKind::BitwiseXor => {
                            ScalarTerm::integer_bitwise_xor(integer_type, left, right)
                        }
                        _ => unreachable!("guarded bitwise kind"),
                    }
                    .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                } if matches!(
                    kind,
                    CheckedIntegerBinaryKind::WrappingShiftLeft
                        | CheckedIntegerBinaryKind::WrappingShiftRight
                        | CheckedIntegerBinaryKind::ExactShiftLeft
                        | CheckedIntegerBinaryKind::ExactShiftRight
                ) =>
                {
                    let ScalarType::Integer(value_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported("structural crash shift has a non-integer value type");
                    };
                    let value = lower_integer_term(
                        left,
                        scalar_parameters,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    let count = lower_integer_term(
                        right,
                        scalar_parameters,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?;
                    if value.scalar_type() != ScalarType::Integer(value_type) {
                        return unsupported(
                            "structural crash shift value does not match its integer type",
                        );
                    }
                    let ScalarType::Integer(count_type) = count.scalar_type() else {
                        return unsupported("structural crash shift count is not an integer");
                    };
                    if matches!(
                        kind,
                        CheckedIntegerBinaryKind::ExactShiftLeft
                            | CheckedIntegerBinaryKind::ExactShiftRight
                    ) && !safe_exact_structural_shift(
                        matches!(kind, CheckedIntegerBinaryKind::ExactShiftLeft),
                        value_type,
                        count_type,
                        &value,
                        &count,
                        runtime_requirements,
                    ) {
                        return unsupported(
                            "structural crash Exact shift requires explicit terminal count and overflow safety evidence",
                        );
                    }
                    match kind {
                        CheckedIntegerBinaryKind::WrappingShiftLeft => {
                            ScalarTerm::wrapping_integer_shift_left(
                                value_type, count_type, value, count,
                            )
                        }
                        CheckedIntegerBinaryKind::WrappingShiftRight => {
                            ScalarTerm::wrapping_integer_shift_right(
                                value_type, count_type, value, count,
                            )
                        }
                        CheckedIntegerBinaryKind::ExactShiftLeft => {
                            ScalarTerm::exact_integer_shift_left(
                                value_type, count_type, value, count,
                            )
                        }
                        CheckedIntegerBinaryKind::ExactShiftRight => {
                            ScalarTerm::exact_integer_shift_right(
                                value_type, count_type, value, count,
                            )
                        }
                        _ => unreachable!("guarded structural shift kind"),
                    }
                    .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedScalarExpression::IntegerBinary {
                    kind,
                    primitive_type,
                    left,
                    right,
                } if matches!(
                    kind,
                    CheckedIntegerBinaryKind::ExactAdd
                        | CheckedIntegerBinaryKind::ExactSubtract
                        | CheckedIntegerBinaryKind::ExactMultiply
                        | CheckedIntegerBinaryKind::ExactDivide
                        | CheckedIntegerBinaryKind::ExactRemainder
                        | CheckedIntegerBinaryKind::WrappingAdd
                        | CheckedIntegerBinaryKind::SaturatingAdd
                        | CheckedIntegerBinaryKind::WrappingSubtract
                        | CheckedIntegerBinaryKind::SaturatingSubtract
                        | CheckedIntegerBinaryKind::WrappingMultiply
                        | CheckedIntegerBinaryKind::SaturatingMultiply
                        | CheckedIntegerBinaryKind::WrappingDivide
                        | CheckedIntegerBinaryKind::WrappingRemainder
                        | CheckedIntegerBinaryKind::SaturatingDivide
                        | CheckedIntegerBinaryKind::SaturatingRemainder
                ) =>
                {
                    let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)?
                    else {
                        return unsupported("structural crash arithmetic has a non-integer type");
                    };
                    let left = Box::new(lower_integer_term(
                        left,
                        scalar_parameters,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?);
                    let right = Box::new(lower_integer_term(
                        right,
                        scalar_parameters,
                        parameters,
                        structural_types,
                        runtime_requirements,
                    )?);
                    if left.scalar_type() != ScalarType::Integer(integer_type)
                        || right.scalar_type() != ScalarType::Integer(integer_type)
                    {
                        return unsupported(
                            "structural crash arithmetic operands do not match its integer type",
                        );
                    }
                    if matches!(
                        kind,
                        CheckedIntegerBinaryKind::ExactDivide
                            | CheckedIntegerBinaryKind::ExactRemainder
                    ) && !safe_exact_structural_divisor(
                        integer_type,
                        &left,
                        &right,
                        runtime_requirements,
                    ) {
                        return unsupported(
                            "structural crash exact division requires explicit terminal divisor safety evidence",
                        );
                    }
                    if matches!(
                        kind,
                        CheckedIntegerBinaryKind::WrappingDivide
                            | CheckedIntegerBinaryKind::WrappingRemainder
                            | CheckedIntegerBinaryKind::SaturatingDivide
                            | CheckedIntegerBinaryKind::SaturatingRemainder
                    ) && !safe_policy_structural_divisor(
                        integer_type,
                        &right,
                        runtime_requirements,
                    ) {
                        return unsupported(
                            "structural crash policy division requires explicit terminal nonzero-divisor evidence",
                        );
                    }
                    Ok(match kind {
                        CheckedIntegerBinaryKind::ExactAdd => ScalarTerm::ExactIntegerAdd {
                            scalar_type: integer_type,
                            left,
                            right,
                        },
                        CheckedIntegerBinaryKind::ExactSubtract => {
                            ScalarTerm::ExactIntegerSubtract {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::ExactMultiply => {
                            ScalarTerm::ExactIntegerMultiply {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::ExactDivide => ScalarTerm::ExactIntegerDivide {
                            scalar_type: integer_type,
                            left,
                            right,
                        },
                        CheckedIntegerBinaryKind::ExactRemainder => {
                            ScalarTerm::ExactIntegerRemainder {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingAdd => ScalarTerm::WrappingIntegerAdd {
                            scalar_type: integer_type,
                            left,
                            right,
                        },
                        CheckedIntegerBinaryKind::SaturatingAdd => {
                            ScalarTerm::SaturatingIntegerAdd {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingSubtract => {
                            ScalarTerm::WrappingIntegerSubtract {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingSubtract => {
                            ScalarTerm::SaturatingIntegerSubtract {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingMultiply => {
                            ScalarTerm::WrappingIntegerMultiply {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingMultiply => {
                            ScalarTerm::SaturatingIntegerMultiply {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingDivide => {
                            ScalarTerm::WrappingIntegerDivide {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::WrappingRemainder => {
                            ScalarTerm::WrappingIntegerRemainder {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingDivide => {
                            ScalarTerm::SaturatingIntegerDivide {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        CheckedIntegerBinaryKind::SaturatingRemainder => {
                            ScalarTerm::SaturatingIntegerRemainder {
                                scalar_type: integer_type,
                                left,
                                right,
                            }
                        }
                        _ => unreachable!("guarded structural arithmetic kind"),
                    })
                }
                _ => unsupported(
                    "structural crash integer predicate contains an unsupported operand",
                ),
            }
        }

        match expression {
            CheckedBooleanExpression::Constant(value) => Ok(ScalarTerm::boolean(*value)),
            CheckedBooleanExpression::Parameter { .. } => {
                checked_boolean_scalar_term(expression, scalar_parameters, &[])
            }
            CheckedBooleanExpression::StorageRead { .. } => {
                unsupported("crash predicate cannot reconstruct mutable storage")
            }
            CheckedBooleanExpression::StructuralParameterField {
                parameter_position,
                path,
            } => lower_structural_member_term(
                *parameter_position,
                path,
                ScalarType::Boolean,
                parameters,
                structural_types,
            ),
            CheckedBooleanExpression::Not(operand) => ScalarTerm::boolean_not(lower_term(
                operand,
                scalar_parameters,
                parameters,
                structural_types,
                runtime_requirements,
            )?)
            .map_err(LoweringError::InvalidCrashPredicate),
            CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                lower_term(
                    left,
                    scalar_parameters,
                    parameters,
                    structural_types,
                    runtime_requirements,
                )?,
                lower_term(
                    right,
                    scalar_parameters,
                    parameters,
                    structural_types,
                    runtime_requirements,
                )?,
            )
            .map_err(LoweringError::InvalidCrashPredicate),
            CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
                let left = lower_integer_term(
                    left,
                    scalar_parameters,
                    parameters,
                    structural_types,
                    runtime_requirements,
                )?;
                let right = lower_integer_term(
                    right,
                    scalar_parameters,
                    parameters,
                    structural_types,
                    runtime_requirements,
                )?;
                let ScalarType::Integer(integer_type) = left.scalar_type() else {
                    return unsupported("structural crash comparison operand is not an integer");
                };
                match kind {
                    CheckedIntegerComparisonKind::Equal => {
                        ScalarTerm::integer_equal(integer_type, left, right)
                    }
                    CheckedIntegerComparisonKind::LessThan => {
                        ScalarTerm::integer_less_than(integer_type, left, right)
                    }
                    CheckedIntegerComparisonKind::LessOrEqual => {
                        ScalarTerm::integer_less_or_equal(integer_type, left, right)
                    }
                }
                .map_err(LoweringError::InvalidCrashPredicate)
            }
            CheckedBooleanExpression::IeeeFloatComparison { .. } => {
                unsupported("IEEE equality lowers as an atomic proposition")
            }
            CheckedBooleanExpression::ByteSequenceEqual { .. } => {
                unsupported("byte-sequence equality lowers as an atomic proposition")
            }
            CheckedBooleanExpression::PayloadlessSumEqual { .. } => {
                unsupported("payload-less sum equality lowers through case-membership propositions")
            }
            CheckedBooleanExpression::StructuralCaseMembership { .. } => {
                unsupported("sum membership lowers as an atomic proposition")
            }
            CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::ErasedParameter { .. }
            | CheckedBooleanExpression::And { .. }
            | CheckedBooleanExpression::Or { .. } => {
                unsupported("structural crash route contains an unsupported Boolean term")
            }
        }
    }

    fn contains_structural_atomic_proposition(expression: &CheckedBooleanExpression) -> bool {
        match expression {
            CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::PayloadlessSumEqual { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => true,
            CheckedBooleanExpression::Not(operand) => {
                contains_structural_atomic_proposition(operand)
            }
            CheckedBooleanExpression::Equal { left, right }
            | CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right } => {
                contains_structural_atomic_proposition(left)
                    || contains_structural_atomic_proposition(right)
            }
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::StorageRead { .. }
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::Local { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::ErasedParameter { .. }
            | CheckedBooleanExpression::IntegerComparison { .. } => false,
        }
    }

    fn lower_polarity(
        expression: &CheckedBooleanExpression,
        positive: bool,
        scalar_parameters: &[ValueDeclaration],
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        runtime_requirements: &[Proposition],
        remaining: &mut usize,
        depth: usize,
    ) -> Result<Proposition, LoweringError> {
        charge_boolean_expansion(remaining, depth)?;
        let lower = |operand: &CheckedBooleanExpression, polarity, budget: &mut usize| {
            lower_polarity(
                operand,
                polarity,
                scalar_parameters,
                parameters,
                structural_types,
                runtime_requirements,
                budget,
                depth + 1,
            )
        };
        match expression {
            CheckedBooleanExpression::Not(operand) => lower(operand, !positive, remaining),
            CheckedBooleanExpression::Equal { left, right } => {
                match (left.as_ref(), right.as_ref()) {
                    (CheckedBooleanExpression::Constant(value), operand)
                    | (operand, CheckedBooleanExpression::Constant(value)) => {
                        lower(operand, *value == positive, remaining)
                    }
                    _ if contains_boolean_connective(expression)
                        || contains_structural_atomic_proposition(expression) =>
                    {
                        crate::proofs::contract_predicates::equality_from_polarities(
                            left, right, positive, remaining, lower,
                        )
                    }
                    _ => lower_atom_polarity(
                        expression,
                        positive,
                        scalar_parameters,
                        parameters,
                        structural_types,
                        runtime_requirements,
                        remaining,
                        depth + 1,
                    ),
                }
            }
            CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right } => {
                crate::proofs::contract_predicates::connective(
                    lower(left, positive, remaining)?,
                    lower(right, positive, remaining)?,
                    matches!(expression, CheckedBooleanExpression::And { .. }) == positive,
                )
            }
            _ => lower_atom_polarity(
                expression,
                positive,
                scalar_parameters,
                parameters,
                structural_types,
                runtime_requirements,
                remaining,
                depth + 1,
            ),
        }
    }

    fn lower_atom_polarity(
        expression: &CheckedBooleanExpression,
        positive: bool,
        scalar_parameters: &[ValueDeclaration],
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        runtime_requirements: &[Proposition],
        remaining: &mut usize,
        depth: usize,
    ) -> Result<Proposition, LoweringError> {
        charge_boolean_expansion(remaining, depth)?;
        if positive || contains_structural_atomic_proposition(expression) {
            let proposition = lower_proposition(
                expression,
                scalar_parameters,
                parameters,
                structural_types,
                runtime_requirements,
                remaining,
                depth + 1,
            )?;
            return Ok(if positive {
                proposition
            } else {
                Proposition::Implication {
                    premise: Box::new(proposition),
                    conclusion: Box::new(Proposition::Falsehood),
                }
            });
        }
        let term = lower_term(
            expression,
            scalar_parameters,
            parameters,
            structural_types,
            runtime_requirements,
        )?;
        crate::proofs::contract_predicates::canonical_equality(
            ScalarTerm::boolean(true),
            ScalarTerm::boolean_not(term).map_err(LoweringError::InvalidCrashPredicate)?,
        )
    }

    fn lower_proposition(
        expression: &CheckedBooleanExpression,
        scalar_parameters: &[ValueDeclaration],
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
        runtime_requirements: &[Proposition],
        remaining: &mut usize,
        depth: usize,
    ) -> Result<Proposition, LoweringError> {
        charge_boolean_expansion(remaining, depth)?;
        // Existing atomic denotations, including proposition-only negation,
        // remain unchanged. Only composition that cannot be a scalar term
        // expands into logical branches, with one budget for the whole guard.
        if matches!(expression, CheckedBooleanExpression::Equal { .. })
            && (contains_boolean_connective(expression)
                || contains_structural_atomic_proposition(expression))
            || matches!(expression, CheckedBooleanExpression::Not(operand)
                if contains_boolean_connective(operand)
                    && !contains_structural_atomic_proposition(operand))
        {
            return lower_polarity(
                expression,
                true,
                scalar_parameters,
                parameters,
                structural_types,
                runtime_requirements,
                remaining,
                depth + 1,
            );
        }
        if let CheckedBooleanExpression::Not(operand) = expression
            && contains_structural_atomic_proposition(operand)
        {
            return Ok(Proposition::Implication {
                premise: Box::new(lower_proposition(
                    operand,
                    scalar_parameters,
                    parameters,
                    structural_types,
                    runtime_requirements,
                    remaining,
                    depth + 1,
                )?),
                conclusion: Box::new(Proposition::Falsehood),
            });
        }
        if let CheckedBooleanExpression::IeeeFloatComparison {
            kind,
            primitive_type,
            left,
            right,
        } = expression
        {
            let format = match primitive_type {
                PrimitiveType::F32 => IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => IeeeFloatFormat::Binary64,
                _ => return unsupported("structural IEEE equality has a non-float format"),
            };
            let mut left = lower_ieee_float_field(left, format, parameters, structural_types)?;
            let mut right = lower_ieee_float_field(right, format, parameters, structural_types)?;
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            return Ok(Proposition::IeeeFloatComparison {
                kind: match kind {
                    checked_trees::CheckedIeeeFloatComparisonKind::Equal => {
                        semantic_vocabulary::IeeeFloatComparisonKind::Equal
                    }
                    checked_trees::CheckedIeeeFloatComparisonKind::NotEqual => {
                        semantic_vocabulary::IeeeFloatComparisonKind::NotEqual
                    }
                },
                format,
                left,
                right,
            });
        }
        if let CheckedBooleanExpression::ByteSequenceEqual { left, right } = expression {
            let mut left = lower_byte_sequence_field(left, parameters, structural_types)?;
            let mut right = lower_byte_sequence_field(right, parameters, structural_types)?;
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            return Ok(Proposition::ByteSequenceEqual { left, right });
        }
        if let CheckedBooleanExpression::StructuralCaseMembership { subject, case } = expression {
            let (subject, structural_type) =
                lower_structural_sum_subject(subject, parameters, structural_types)?;
            let cases = match &structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .expect("sum subject type was resolved")
                .shape
            {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => unreachable!("sum subject resolver returned a sum"),
            };
            let case = cases
                .iter()
                .find(|candidate| candidate.identity == *case)
                .ok_or(LoweringError::Unsupported(
                    "structural sum membership case was redirected",
                ))?;
            return Ok(Proposition::StructuralCaseMembership {
                subject,
                case: case.id,
            });
        }
        if let CheckedBooleanExpression::PayloadlessSumEqual { left, right, cases } = expression {
            let (left, left_type) =
                lower_structural_sum_subject(left, parameters, structural_types)?;
            let (right, right_type) =
                lower_structural_sum_subject(right, parameters, structural_types)?;
            if left_type != right_type {
                return unsupported("payload-less sum equality operands have different types");
            }
            if left == right {
                return Ok(Proposition::Truth);
            }
            let StructuralTypeShape::Sum {
                cases: declared_cases,
            } = &structural_types
                .iter()
                .find(|declaration| declaration.id == left_type)
                .expect("sum subject type was resolved")
                .shape
            else {
                unreachable!("sum subject resolver returned a sum")
            };
            if cases.len() != declared_cases.len()
                || cases
                    .iter()
                    .zip(declared_cases)
                    .any(|(checked, declared)| checked != &declared.identity)
            {
                return unsupported("payload-less sum equality case roster was redirected");
            }
            let mut propositions = Vec::with_capacity(cases.len().saturating_mul(2));
            for declared in declared_cases {
                let left_membership = Proposition::StructuralCaseMembership {
                    subject: left.clone(),
                    case: declared.id,
                };
                let right_membership = Proposition::StructuralCaseMembership {
                    subject: right.clone(),
                    case: declared.id,
                };
                propositions.push(Proposition::Implication {
                    premise: Box::new(left_membership.clone()),
                    conclusion: Box::new(right_membership.clone()),
                });
                propositions.push(Proposition::Implication {
                    premise: Box::new(right_membership),
                    conclusion: Box::new(left_membership),
                });
            }
            let mut keyed = propositions
                .into_iter()
                .map(|proposition| {
                    terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "payload-less sum equality is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            let mut propositions = keyed
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect::<Vec<_>>();
            return Ok(match propositions.len() {
                0 => Proposition::Truth,
                1 => propositions.pop().expect("one proposition"),
                _ => Proposition::Conjunction(propositions),
            });
        }
        if let CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } = expression
        {
            let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
            let mut leaves = Vec::new();
            flatten_checked_boolean_connective(left, conjunction, &mut leaves);
            flatten_checked_boolean_connective(right, conjunction, &mut leaves);
            let propositions = leaves
                .into_iter()
                .map(|leaf| {
                    lower_proposition(
                        leaf,
                        scalar_parameters,
                        parameters,
                        structural_types,
                        runtime_requirements,
                        remaining,
                        depth + 1,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut flattened = Vec::new();
            for proposition in propositions {
                match proposition {
                    Proposition::Conjunction(nested) if conjunction => flattened.extend(nested),
                    Proposition::Disjunction(nested) if !conjunction => flattened.extend(nested),
                    proposition => flattened.push(proposition),
                }
            }
            let mut keyed = flattened
                .into_iter()
                .map(|proposition| {
                    terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "structural crash connective is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            if keyed.len() == 1 {
                return Ok(keyed.pop().expect("one distinct crash predicate").1);
            }
            let propositions = keyed
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect();
            return Ok(if conjunction {
                Proposition::Conjunction(propositions)
            } else {
                Proposition::Disjunction(propositions)
            });
        }
        let mut left = ScalarTerm::boolean(true);
        let mut right = lower_term(
            expression,
            scalar_parameters,
            parameters,
            structural_types,
            runtime_requirements,
        )?;
        let order_key = |term: &ScalarTerm| {
            terminal_codec::canonical_scalar_term_order_key(term).map_err(|_| {
                LoweringError::Unsupported("mixed crash Boolean term is not canonically encodable")
            })
        };
        if order_key(&left)? > order_key(&right)? {
            std::mem::swap(&mut left, &mut right);
        }
        Ok(Proposition::Equal(left, right))
    }

    buckets
        .iter()
        .map(|bucket| {
            let mut alternatives = bucket
                .alternative_guards()
                .iter()
                .map(|guard| match guard {
                    checked_trees::CrashRouteGuard::Truth => {
                        Ok(terminal_psi::CrashRouteGuard::Truth)
                    }
                    checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        let proposition = if let Some(expression) = predicate.scalar_expression() {
                            let mut remaining = boolean_input_budget(expression)?;
                            lower_proposition(
                                expression,
                                scalar_parameters,
                                parameters,
                                structural_types,
                                runtime_requirements,
                                &mut remaining,
                                0,
                            )?
                        } else {
                            let mut path = Vec::new();
                            let parameter_position = predicate
                                .expression()
                                .and_then(|expression| checked_member_path(expression, &mut path))
                                .ok_or(LoweringError::Unsupported(
                                    "structural crash route is outside checked Boolean member lowering",
                                ))?;
                            Proposition::Equal(
                                ScalarTerm::boolean(true),
                                lower_structural_member_term(
                                    parameter_position,
                                    &path
                                        .into_iter()
                                        .map(
                                            checked_trees::CheckedStructuralPredicatePathSegment::Field,
                                        )
                                        .collect::<Vec<_>>(),
                                    ScalarType::Boolean,
                                    parameters,
                                    structural_types,
                                )?,
                            )
                        };
                        Ok(terminal_psi::CrashRouteGuard::Predicate(
                            terminal_psi::CrashPredicateTerm::new(proposition),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            alternatives.sort();
            alternatives.dedup();
            Ok(terminal_psi::CrashRouteBucket {
                cause: match bucket.cause() {
                    checked_trees::CrashCause::Trap => TerminalCrashCause::Trap,
                    checked_trees::CrashCause::Abort => TerminalCrashCause::Abort,
                },
                alternatives,
            })
        })
        .collect()
}

pub(crate) fn substitute_structural_crash_route_roots(
    buckets: &mut [terminal_psi::CrashRouteBucket],
    substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
) -> Result<(), LoweringError> {
    fn substitute_term(
        term: &mut ScalarTerm,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) -> Result<(), LoweringError> {
        match term {
            ScalarTerm::BooleanField { root, path }
            | ScalarTerm::IntegerField { root, path, .. } => {
                let Some((replacement, prefix)) = substitutions.get(root) else {
                    return Ok(());
                };
                *root = *replacement;
                if !prefix.is_empty() {
                    let mut rebased = Vec::with_capacity(prefix.len() + path.len());
                    rebased.extend(prefix);
                    rebased.append(path);
                    *path = rebased;
                }
            }
            ScalarTerm::BooleanNot { operand } => substitute_term(operand, substitutions)?,
            ScalarTerm::IntegerBitwiseNot { operand, .. } => {
                substitute_term(operand, substitutions)?
            }
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. } => {
                substitute_term(left, substitutions)?;
                substitute_term(right, substitutions)?;
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                substitute_term(value, substitutions)?;
                substitute_term(count, substitutions)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn substitute_proposition(
        proposition: &mut Proposition,
        substitutions: &BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>,
    ) -> Result<(), LoweringError> {
        match proposition {
            Proposition::Equal(left, right) => {
                substitute_term(left, substitutions)?;
                substitute_term(right, substitutions)?;
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions.iter_mut() {
                    substitute_proposition(proposition, substitutions)?;
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                substitute_proposition(premise, substitutions)?;
                substitute_proposition(conclusion, substitutions)?;
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                for field in [left, right] {
                    if let Some((root, prefix)) = substitutions.get(&field.root()) {
                        *field = field.rebase(*root, prefix);
                    }
                }
            }
            Proposition::ByteSequenceEqual { left, right } => {
                for field in [left, right] {
                    if let Some((root, prefix)) = substitutions.get(&field.root()) {
                        *field = field.rebase(*root, prefix);
                    }
                }
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                if let Some((root, prefix)) = substitutions.get(&subject.root()) {
                    *subject = subject.rebase(*root, prefix);
                }
            }
            _ => {}
        }
        Ok(())
    }

    for bucket in buckets {
        for alternative in &mut bucket.alternatives {
            let terminal_psi::CrashRouteGuard::Predicate(predicate) = alternative else {
                continue;
            };
            let mut proposition = predicate.proposition().clone();
            substitute_proposition(&mut proposition, substitutions)?;
            *predicate = terminal_psi::CrashPredicateTerm::new(proposition);
        }
    }
    Ok(())
}
