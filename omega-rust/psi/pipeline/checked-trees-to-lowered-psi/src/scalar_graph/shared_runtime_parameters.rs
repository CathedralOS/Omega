//! Runtime-input custody for checked shared-convergence plans.
//!
//! Arithmetic operation trees are emitted compositionally. Runtime-root
//! collection grants no proof authority; completed operation graphs still need
//! independently checked canonical certificates for every partial operation.
use super::{
    BTreeSet, IntegerType, LoweringError, PlaceId, ScalarType, StructuralFieldId,
    StructuralParameterDeclaration, StructuralTypeDeclaration,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;

mod boolean;

pub(crate) fn shared_boolean_runtime_parameters(
    expression: &LoweredBooleanReturnExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => Some(BTreeSet::new()),
        LoweredBooleanReturnExpression::Parameter { position } => {
            Some(BTreeSet::from([SharedBooleanRuntimeInput::BooleanScalar(
                *position,
            )]))
        }
        LoweredBooleanReturnExpression::StructuralField {
            source,
            path,
            field,
        } => Some(BTreeSet::from([
            SharedBooleanRuntimeInput::StructuralField {
                source: *source,
                path: path.clone(),
                field: *field,
            },
        ])),
        LoweredBooleanReturnExpression::Not { operand } => {
            shared_boolean_runtime_parameters(operand)
        }
        LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            let mut parameters = shared_boolean_runtime_parameters(left)?;
            parameters.extend(shared_boolean_runtime_parameters(right)?);
            Some(parameters)
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            let mut parameters = shared_integer_runtime_parameters(left)?;
            parameters.extend(shared_integer_runtime_parameters(right)?);
            Some(parameters)
        }
        LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::Equal { .. } => None,
    }
}

fn shared_integer_runtime_parameters(
    expression: &LoweredDirectExpression,
) -> Option<BTreeSet<SharedBooleanRuntimeInput>> {
    let ScalarType::Integer(integer_type) = expression.scalar_type() else {
        return None;
    };
    if !native_fixed_integer_type(integer_type) {
        return None;
    }
    match expression {
        LoweredDirectExpression::IntegerLiteral { .. } => Some(BTreeSet::new()),
        LoweredDirectExpression::Parameter { position, .. } => {
            Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                *position,
            )]))
        }
        LoweredDirectExpression::IntegerBinary {
            kind:
                kind @ (LoweredIntegerBinaryKind::BitwiseAnd
                | LoweredIntegerBinaryKind::BitwiseOr
                | LoweredIntegerBinaryKind::BitwiseXor
                | LoweredIntegerBinaryKind::WrappingShiftLeft
                | LoweredIntegerBinaryKind::WrappingShiftRight
                | LoweredIntegerBinaryKind::WrappingAdd
                | LoweredIntegerBinaryKind::SaturatingAdd
                | LoweredIntegerBinaryKind::WrappingSubtract
                | LoweredIntegerBinaryKind::SaturatingSubtract
                | LoweredIntegerBinaryKind::WrappingMultiply
                | LoweredIntegerBinaryKind::SaturatingMultiply
                | LoweredIntegerBinaryKind::ExactAdd
                | LoweredIntegerBinaryKind::ExactSubtract
                | LoweredIntegerBinaryKind::ExactMultiply
                | LoweredIntegerBinaryKind::ExactDivide
                | LoweredIntegerBinaryKind::ExactRemainder
                | LoweredIntegerBinaryKind::ExactShiftLeft
                | LoweredIntegerBinaryKind::ExactShiftRight),
            left,
            right,
            scalar_type,
        } => {
            let is_shift = matches!(
                kind,
                LoweredIntegerBinaryKind::ExactShiftLeft
                    | LoweredIntegerBinaryKind::ExactShiftRight
                    | LoweredIntegerBinaryKind::WrappingShiftLeft
                    | LoweredIntegerBinaryKind::WrappingShiftRight
            );
            if left.scalar_type() != *scalar_type
                || (!is_shift && right.scalar_type() != *scalar_type)
            {
                return None;
            }
            let mut parameters = shared_integer_runtime_parameters(left)?;
            parameters.extend(shared_integer_runtime_parameters(right)?);
            Some(parameters)
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. }
            if operand.scalar_type() == expression.scalar_type() =>
        {
            shared_integer_runtime_parameters(operand)
        }
        LoweredDirectExpression::IntegerExactCast { operand, .. }
            if matches!(operand.scalar_type(), ScalarType::Integer(source_type)
                if source_type.can_exact_cast_to(integer_type)) =>
        {
            shared_integer_runtime_parameters(operand)
        }
        LoweredDirectExpression::IntegerWiden { operand, .. }
            if matches!(operand.scalar_type(), ScalarType::Integer(source_type)
                if source_type.can_widen_to(integer_type)) =>
        {
            shared_integer_runtime_parameters(operand)
        }
        LoweredDirectExpression::IeeeFloatLiteral { .. }
        | LoweredDirectExpression::PrimitiveRead { .. }
        | LoweredDirectExpression::StructuralField { .. }
        | LoweredDirectExpression::ByteSequenceRead { .. }
        | LoweredDirectExpression::ByteSequenceLength { .. }
        | LoweredDirectExpression::ByteSequenceFieldLength { .. }
        | LoweredDirectExpression::Local { .. }
        | LoweredDirectExpression::IntegerBinary { .. }
        | LoweredDirectExpression::IntegerBitwiseNot { .. }
        | LoweredDirectExpression::IntegerExactCast { .. }
        | LoweredDirectExpression::IntegerWiden { .. }
        | LoweredDirectExpression::ErasedParameter { .. }
        | LoweredDirectExpression::Boolean { .. } => None,
    }
}

fn native_fixed_integer_type(integer_type: IntegerType) -> bool {
    !integer_type.is_address() && matches!(integer_type.bits(), 8 | 16 | 32 | 64)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SharedBooleanRuntimeInput {
    BooleanScalar(usize),
    IntegerScalar(usize),
    StructuralField {
        source: PlaceId,
        path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        field: StructuralFieldId,
    },
}

pub(crate) fn valid_shared_boolean_runtime_inputs(
    inputs: &BTreeSet<SharedBooleanRuntimeInput>,
) -> bool {
    boolean::valid_shared_boolean_runtime_inputs(inputs)
}

pub(crate) fn resolve_shared_boolean_member_fields(
    expression: LoweredBooleanReturnExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    boolean::resolve_shared_boolean_member_fields(expression, parameters, structural_types)
}

pub(crate) fn normalize_shared_boolean_comparison_leaves(
    expression: &LoweredBooleanReturnExpression,
) -> Option<LoweredBooleanReturnExpression> {
    boolean::normalize_shared_boolean_comparison_leaves(expression)
}

#[cfg(test)]
mod tests {
    use super::{
        BTreeSet, IntegerType, LoweredDirectExpression, LoweredIntegerBinaryKind, ScalarType,
        SharedBooleanRuntimeInput, shared_integer_runtime_parameters,
    };
    use semantic_vocabulary::IntegerSign;

    fn integer_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap())
    }

    fn parameter(position: usize) -> LoweredDirectExpression {
        LoweredDirectExpression::Parameter {
            position,
            scalar_type: integer_type(),
        }
    }

    #[test]
    fn runtime_roots_follow_computed_operands_without_shell_limits() {
        let mut expression = parameter(0);
        for kind in [
            LoweredIntegerBinaryKind::BitwiseXor,
            LoweredIntegerBinaryKind::ExactAdd,
            LoweredIntegerBinaryKind::ExactMultiply,
            LoweredIntegerBinaryKind::ExactDivide,
            LoweredIntegerBinaryKind::ExactRemainder,
            LoweredIntegerBinaryKind::ExactShiftLeft,
            LoweredIntegerBinaryKind::ExactShiftRight,
            LoweredIntegerBinaryKind::ExactSubtract,
        ] {
            expression = LoweredDirectExpression::IntegerBinary {
                kind,
                scalar_type: integer_type(),
                left: Box::new(parameter(1)),
                right: Box::new(expression),
            };
            assert_eq!(
                shared_integer_runtime_parameters(&expression),
                Some(BTreeSet::from([
                    SharedBooleanRuntimeInput::IntegerScalar(0),
                    SharedBooleanRuntimeInput::IntegerScalar(1),
                ]))
            );
        }
    }

    #[test]
    fn runtime_root_collection_rejects_local_and_invalid_widen_custody() {
        let independently_typed_count = LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactShiftRight,
            scalar_type: integer_type(),
            left: Box::new(parameter(0)),
            right: Box::new(LoweredDirectExpression::Parameter {
                position: 1,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                ),
            }),
        };
        assert_eq!(
            shared_integer_runtime_parameters(&independently_typed_count),
            Some(BTreeSet::from([
                SharedBooleanRuntimeInput::IntegerScalar(0),
                SharedBooleanRuntimeInput::IntegerScalar(1),
            ]))
        );
        let local = LoweredDirectExpression::Local {
            position: 0,
            scalar_type: integer_type(),
        };
        let invalid_widen = LoweredDirectExpression::IntegerWiden {
            scalar_type: integer_type(),
            operand: Box::new(parameter(0)),
        };
        let wrong_carrier = LoweredDirectExpression::Parameter {
            position: 0,
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
        };
        for rejected in [local, invalid_widen, wrong_carrier] {
            let expression = LoweredDirectExpression::IntegerBinary {
                kind: LoweredIntegerBinaryKind::ExactAdd,
                scalar_type: integer_type(),
                left: Box::new(parameter(1)),
                right: Box::new(rejected),
            };
            assert!(shared_integer_runtime_parameters(&expression).is_none());
        }
    }

    #[test]
    fn exact_cast_inputs_allow_identity_but_not_noninteger_carriers() {
        let identity = LoweredDirectExpression::IntegerExactCast {
            scalar_type: integer_type(),
            operand: Box::new(parameter(0)),
        };
        assert_eq!(
            shared_integer_runtime_parameters(&identity),
            Some(BTreeSet::from([SharedBooleanRuntimeInput::IntegerScalar(
                0
            ),]))
        );
        let noninteger = LoweredDirectExpression::IntegerExactCast {
            scalar_type: integer_type(),
            operand: Box::new(LoweredDirectExpression::Parameter {
                position: 0,
                scalar_type: ScalarType::Boolean,
            }),
        };
        assert!(shared_integer_runtime_parameters(&noninteger).is_none());
    }
}
