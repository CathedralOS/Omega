//! Checked boolean and scalar terms.

use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::{
    LoweredIntegerBinaryKind, LoweredIntegerComparisonKind,
};
use crate::proofs::{
    CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression, LoweringError,
    ScalarTerm, ScalarType, ValueDeclaration, lower_checked_scalar_expression, unsupported,
};

pub(crate) fn checked_boolean_scalar_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    Ok(match expression {
        CheckedBooleanExpression::ErasedParameter { position } => {
            let value = erased.get(*position).ok_or(LoweringError::Unsupported(
                "erased formal position is outside the contract's erased roster",
            ))?;
            if value.scalar_type != ScalarType::Boolean {
                return unsupported("erased formal predicate value has a non-Boolean type");
            }
            ScalarTerm::value(value.id, value.scalar_type)
        }
        CheckedBooleanExpression::Constant(value) => ScalarTerm::boolean(*value),
        CheckedBooleanExpression::StorageRead { .. } => {
            return unsupported(
                "crash predicate requires an established scalar value, not mutable storage",
            );
        }
        CheckedBooleanExpression::Parameter { position }
        | CheckedBooleanExpression::Local { position } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            if value.scalar_type != ScalarType::Boolean {
                return unsupported("crash predicate Boolean value has a non-Boolean type");
            }
            ScalarTerm::value(value.id, value.scalar_type)
        }
        CheckedBooleanExpression::StructuralParameterField { .. } => {
            return unsupported(
                "structural crash predicates require structural signature lowering",
            );
        }
        CheckedBooleanExpression::Not(operand) => ScalarTerm::boolean_not(
            checked_boolean_scalar_term(identities, operand, values, erased)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate)?,
        CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
            checked_boolean_scalar_term(identities, left, values, erased)?,
            checked_boolean_scalar_term(identities, right, values, erased)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate)?,
        CheckedBooleanExpression::IntegerMathComparison { .. } => {
            return unsupported(
                "mathematical comparison is a proof proposition, not a scalar term",
            );
        }
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            let left = checked_scalar_term(identities, left, values, erased)?;
            let right = checked_scalar_term(identities, right, values, erased)?;
            let Some(integer_type) = left.scalar_type().as_integer() else {
                return unsupported("crash comparison operand is not an integer");
            };
            checked_integer_comparison(*kind, integer_type, left, right)
                .map_err(LoweringError::InvalidCrashPredicate)?
        }
        CheckedBooleanExpression::ScalarIeeeFloatComparison { .. } => {
            return unsupported("scalar IEEE equality is an atomic proposition, not a scalar term");
        }
        CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::StructuralIdentity { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::ByteSequencePredicate { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => {
            return unsupported("structural equality requires structural signature lowering");
        }
        CheckedBooleanExpression::And { .. } | CheckedBooleanExpression::Or { .. } => {
            return unsupported(
                "short-circuit Boolean crash predicate is not one scalar terminal term",
            );
        }
    })
}

/// The integer-comparison ScalarTerm for a checked comparison kind.
pub(super) fn checked_integer_comparison(
    kind: CheckedIntegerComparisonKind,
    integer_type: semantic_vocabulary::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
) -> Result<ScalarTerm, semantic_vocabulary::PropositionError> {
    match kind {
        CheckedIntegerComparisonKind::Equal => ScalarTerm::integer_equal(integer_type, left, right),
        CheckedIntegerComparisonKind::LessThan => {
            ScalarTerm::integer_less_than(integer_type, left, right)
        }
        CheckedIntegerComparisonKind::LessOrEqual => {
            ScalarTerm::integer_less_or_equal(integer_type, left, right)
        }
    }
}

pub(crate) fn checked_scalar_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &CheckedScalarExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let expression = lower_checked_scalar_expression(identities, expression, &[])?;
    lowered_direct_scalar_term(&expression, values, erased)
}

/// `checked_scalar_term` over the contract's evaluated value namespace: the
/// retained expression lowers against the evaluated scalar bindings those
/// values establish.
pub(crate) fn checked_scalar_term_with_views(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &CheckedScalarExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let scalar_bindings = crate::expression_preparation::bindings::ScalarBindings::new(
        values.iter().map(|value| value.scalar_type),
    );
    let expression = crate::expression_preparation::prepare_expression::PreparedExpressionContext {
        structural_fields: &Default::default(),
        structural_cases: &[],
        primitive_storage: &[],
        view_locals: &[],
        structural_locals: &[],
        scalar_bindings: Some(&scalar_bindings),
        proven_indexes: &[],
    }
    .lower_scalar(identities, expression)?;
    lowered_direct_scalar_term(&expression, values, erased)
}

/// An erased actual may name a checked current field without executing its
/// read. The exact root/path/type remain a symbolic observation, supplying no
/// value premise. Runtime/crash expressions keep their separate read authority.
pub(crate) fn lowered_erased_scalar_term(
    expression: &LoweredDirectExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    if let LoweredDirectExpression::StructuralField {
        source,
        path,
        field,
        scalar_type,
        ..
    } = expression
    {
        let (root, mut path) = source.field_origin(path);
        path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
            *field,
        ));
        return Ok(match scalar_type {
            ScalarType::Boolean => ScalarTerm::boolean_field_path(root, path),
            ScalarType::Integer(integer) => ScalarTerm::integer_field_path(root, path, *integer),
            ScalarType::IeeeFloat(format) => ScalarTerm::ieee_float_field_path(root, path, *format),
        });
    }
    lowered_direct_scalar_term(expression, values, erased)
}

/// Lower one whole erased-actual roster against a single scalar namespace.
/// Every predecessor terminator maps its erased formals through this owner;
/// a term keeps its `ErasedParameter` position only inside that namespace.
pub(crate) fn lowered_erased_scalar_terms(
    expressions: &[LoweredDirectExpression],
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<Vec<ScalarTerm>, LoweringError> {
    expressions
        .iter()
        .map(|expression| lowered_erased_scalar_term(expression, values, erased))
        .collect()
}

pub(crate) fn lowered_direct_scalar_term(
    expression: &LoweredDirectExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    Ok(match expression {
        LoweredDirectExpression::ErasedParameter {
            position,
            scalar_type,
        } => {
            let value = erased.get(*position).ok_or(LoweringError::Unsupported(
                "erased formal position is outside the contract's erased roster",
            ))?;
            if value.scalar_type != *scalar_type {
                return unsupported("erased formal term type does not match its checked plan");
            }
            ScalarTerm::value(value.id, *scalar_type)
        }
        LoweredDirectExpression::PrimitiveRead { .. }
        | LoweredDirectExpression::IndexedPrimitiveRead { .. } => {
            return unsupported(
                "primitive storage read requires an occurrence-bound crash predicate",
            );
        }
        LoweredDirectExpression::StructuralField { .. }
        | LoweredDirectExpression::AtomicLoad { .. } => {
            return unsupported("runtime field read requires an occurrence-bound crash predicate");
        }
        LoweredDirectExpression::ImplementationSelection { .. } => {
            return unsupported("an implementation selector is not a crash predicate term");
        }
        LoweredDirectExpression::ByteSequenceLength {
            source,
            path,
            scalar_type,
            ..
        } => {
            if !path.is_empty() {
                return unsupported(
                    "projected crash extent requires its canonical descriptor path",
                );
            }
            let Some(extent_type) = scalar_type.as_integer() else {
                return unsupported("view extent observation has a non-integer carrier");
            };
            ScalarTerm::ViewExtent {
                root: *source,
                path: Vec::new(),
                scalar_type: extent_type,
            }
        }
        LoweredDirectExpression::ElementViewLength {
            source,
            path,
            scalar_type,
            ..
        } => {
            if !path.is_empty() {
                return unsupported(
                    "projected crash extent requires its canonical descriptor path",
                );
            }
            let Some(extent_type) = scalar_type.as_integer() else {
                return unsupported("view extent observation has a non-integer carrier");
            };
            ScalarTerm::ViewExtent {
                root: *source,
                path: Vec::new(),
                scalar_type: extent_type,
            }
        }
        LoweredDirectExpression::ByteSequenceFieldLength { .. }
        | LoweredDirectExpression::ByteSequenceFieldRead { .. }
        | LoweredDirectExpression::ElementViewRead { .. } => {
            return unsupported("view observation has no retained crash predicate term");
        }
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
        | LoweredDirectExpression::Local {
            position,
            scalar_type,
        } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            if value.scalar_type != *scalar_type {
                return unsupported("crash predicate value type does not match its checked plan");
            }
            ScalarTerm::value(value.id, *scalar_type)
        }
        LoweredDirectExpression::IntegerLiteral { value, scalar_type } => {
            let Some(integer_type) = scalar_type.as_integer() else {
                return unsupported("crash predicate integer literal has a non-integer type");
            };
            ScalarTerm::integer(integer_type, *value)
                .map_err(LoweringError::InvalidCrashPredicate)?
        }
        LoweredDirectExpression::SaturatingShiftLeft {
            scalar_type,
            left,
            right,
        } => {
            // Mathematical denotation may reuse a term; runtime emission binds
            // each operand once before applying the same scaling recipe.
            let expanded = crate::expression_preparation::saturating_shift::expansion(
                *scalar_type,
                *left.clone(),
                *right.clone(),
            );
            lowered_direct_scalar_term(&expanded, values, erased)?
        }
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
            ..
        } => {
            let Some(integer_type) = scalar_type.as_integer() else {
                return unsupported("crash predicate arithmetic has a non-integer type");
            };
            let left = lowered_direct_scalar_term(left, values, erased)?;
            let right = lowered_direct_scalar_term(right, values, erased)?;
            match kind {
                // Shift rows stage their separate count type, then every
                // remaining row applies through the one construction table.
                LoweredIntegerBinaryKind::WrappingShiftLeft => {
                    let Some(count_type) = right.scalar_type().as_integer() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::wrapping_integer_shift_left(integer_type, count_type, left, right)
                        .map_err(LoweringError::InvalidCrashPredicate)?
                }
                LoweredIntegerBinaryKind::WrappingShiftRight => {
                    let Some(count_type) = right.scalar_type().as_integer() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::wrapping_integer_shift_right(integer_type, count_type, left, right)
                        .map_err(LoweringError::InvalidCrashPredicate)?
                }
                LoweredIntegerBinaryKind::ExactShiftLeft => {
                    let Some(count_type) = right.scalar_type().as_integer() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::exact_integer_shift_left(integer_type, count_type, left, right)
                        .map_err(LoweringError::InvalidCrashPredicate)?
                }
                LoweredIntegerBinaryKind::ExactShiftRight => {
                    let Some(count_type) = right.scalar_type().as_integer() else {
                        return unsupported("crash shift count is not an integer");
                    };
                    ScalarTerm::exact_integer_shift_right(integer_type, count_type, left, right)
                        .map_err(LoweringError::InvalidCrashPredicate)?
                }
                // Direct Trapping arithmetic forms no predicate term: its
                // crash is an executable site, not a proof-side value.
                LoweredIntegerBinaryKind::TrappingAdd
                | LoweredIntegerBinaryKind::TrappingSubtract
                | LoweredIntegerBinaryKind::TrappingMultiply
                | LoweredIntegerBinaryKind::TrappingDivide
                | LoweredIntegerBinaryKind::TrappingRemainder
                | LoweredIntegerBinaryKind::TrappingShiftLeft
                | LoweredIntegerBinaryKind::TrappingShiftRight => {
                    return unsupported("Trapping arithmetic forms no crash predicate term");
                }
                kind => {
                    // One construction row per remaining kind: each arm names
                    // the ScalarTerm constructor that owns its operation
                    // identity, and the operands apply once below.
                    let constructor: fn(
                        semantic_vocabulary::IntegerType,
                        ScalarTerm,
                        ScalarTerm,
                    ) -> Result<
                        ScalarTerm,
                        semantic_vocabulary::PropositionError,
                    > = match kind {
                        LoweredIntegerBinaryKind::BitwiseAnd => ScalarTerm::integer_bitwise_and,
                        LoweredIntegerBinaryKind::BitwiseOr => ScalarTerm::integer_bitwise_or,
                        LoweredIntegerBinaryKind::BitwiseXor => ScalarTerm::integer_bitwise_xor,
                        LoweredIntegerBinaryKind::ExactAdd => ScalarTerm::exact_integer_add,
                        LoweredIntegerBinaryKind::ExactSubtract => {
                            ScalarTerm::exact_integer_subtract
                        }
                        LoweredIntegerBinaryKind::ExactMultiply => {
                            ScalarTerm::exact_integer_multiply
                        }
                        LoweredIntegerBinaryKind::ExactDivide => ScalarTerm::exact_integer_divide,
                        LoweredIntegerBinaryKind::ExactRemainder => {
                            ScalarTerm::exact_integer_remainder
                        }
                        LoweredIntegerBinaryKind::WrappingDivide => {
                            ScalarTerm::wrapping_integer_divide
                        }
                        LoweredIntegerBinaryKind::WrappingRemainder => {
                            ScalarTerm::wrapping_integer_remainder
                        }
                        LoweredIntegerBinaryKind::SaturatingDivide => {
                            ScalarTerm::saturating_integer_divide
                        }
                        LoweredIntegerBinaryKind::SaturatingRemainder => {
                            ScalarTerm::saturating_integer_remainder
                        }
                        LoweredIntegerBinaryKind::WrappingAdd => ScalarTerm::wrapping_integer_add,
                        LoweredIntegerBinaryKind::SaturatingAdd => {
                            ScalarTerm::saturating_integer_add
                        }
                        LoweredIntegerBinaryKind::WrappingSubtract => {
                            ScalarTerm::wrapping_integer_subtract
                        }
                        LoweredIntegerBinaryKind::SaturatingSubtract => {
                            ScalarTerm::saturating_integer_subtract
                        }
                        LoweredIntegerBinaryKind::WrappingMultiply => {
                            ScalarTerm::wrapping_integer_multiply
                        }
                        LoweredIntegerBinaryKind::SaturatingMultiply => {
                            ScalarTerm::saturating_integer_multiply
                        }
                        _ => unreachable!("shift and Trapping kinds stage their own terms"),
                    };
                    constructor(integer_type, left, right)
                        .map_err(LoweringError::InvalidCrashPredicate)?
                }
            }
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let Some(integer_type) = scalar_type.as_integer() else {
                return unsupported("crash predicate bitwise-not has a non-integer type");
            };
            ScalarTerm::IntegerBitwiseNot {
                scalar_type: integer_type,
                operand: Box::new(lowered_direct_scalar_term(operand, values, erased)?),
            }
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let Some(target_type) = scalar_type.as_integer() else {
                return unsupported("crash predicate widen has a non-integer target");
            };
            let operand = lowered_direct_scalar_term(operand, values, erased)?;
            let Some(source_type) = operand.scalar_type().as_integer() else {
                return unsupported("crash predicate widen has a non-integer operand");
            };
            ScalarTerm::IntegerWiden {
                source_type,
                target_type: target_type,
                operand: Box::new(operand),
            }
        }
        LoweredDirectExpression::RepresentationView { .. } => {
            return unsupported("a representation re-view forms no crash predicate term");
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
            ..
        } => {
            let Some(target_type) = scalar_type.as_integer() else {
                return unsupported("crash predicate cast has a non-integer target");
            };
            let operand = lowered_direct_scalar_term(operand, values, erased)?;
            let Some(source_type) = operand.scalar_type().as_integer() else {
                return unsupported("crash predicate cast has a non-integer operand");
            };
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: target_type,
                operand: Box::new(operand),
            }
        }
        LoweredDirectExpression::IeeeFloatLiteral { value } => ScalarTerm::IeeeFloatLiteral(*value),
        LoweredDirectExpression::Boolean { expression } => {
            return checked_boolean_scalar_term_from_lowered(expression, values, erased);
        }
    })
}

fn checked_boolean_scalar_term_from_lowered(
    expression: &LoweredBooleanReturnExpression,
    values: &[ValueDeclaration],
    erased: &[ValueDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::IndexedCaseMembership { .. }
        | LoweredBooleanReturnExpression::ByteSequencePredicate { .. }
        | LoweredBooleanReturnExpression::ByteSequenceEqual { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. } => {
            unsupported("storage observation requires an occurrence-bound crash predicate")
        }
        LoweredBooleanReturnExpression::Constant { value } => Ok(ScalarTerm::boolean(*value)),
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            let value = values.get(*position).ok_or(LoweringError::Unsupported(
                "crash predicate value position is outside the selected scalar namespace",
            ))?;
            (value.scalar_type == ScalarType::Boolean)
                .then(|| ScalarTerm::value(value.id, value.scalar_type))
                .ok_or(LoweringError::Unsupported(
                    "crash predicate Boolean value has a non-Boolean type",
                ))
        }
        LoweredBooleanReturnExpression::StructuralField {
            source,
            path,
            field,
            ..
        } => {
            let (source, mut path) = source.field_origin(path);
            path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                *field,
            ));
            Ok(ScalarTerm::boolean_field_path(source, path))
        }
        LoweredBooleanReturnExpression::ScalarObservation { operand } => {
            lowered_direct_scalar_term(operand, values, erased)
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed crash-predicate lowering")
        }
        LoweredBooleanReturnExpression::Not { operand } => ScalarTerm::boolean_not(
            checked_boolean_scalar_term_from_lowered(operand, values, erased)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate),
        LoweredBooleanReturnExpression::Equal { left, right } => ScalarTerm::boolean_equal(
            checked_boolean_scalar_term_from_lowered(left, values, erased)?,
            checked_boolean_scalar_term_from_lowered(right, values, erased)?,
        )
        .map_err(LoweringError::InvalidCrashPredicate),
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let left = lowered_direct_scalar_term(left, values, erased)?;
            let right = lowered_direct_scalar_term(right, values, erased)?;
            let Some(integer_type) = left.scalar_type().as_integer() else {
                return unsupported("crash comparison operand is not an integer");
            };
            match kind {
                LoweredIntegerComparisonKind::Equal => {
                    ScalarTerm::integer_equal(integer_type, left, right)
                }
                LoweredIntegerComparisonKind::LessThan => {
                    ScalarTerm::integer_less_than(integer_type, left, right)
                }
                LoweredIntegerComparisonKind::LessOrEqual => {
                    ScalarTerm::integer_less_or_equal(integer_type, left, right)
                }
            }
            .map_err(LoweringError::InvalidCrashPredicate)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            unsupported("short-circuit Boolean crash predicate is not one scalar terminal term")
        }
    }
}
