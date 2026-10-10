//! Closed scalar contracts use only the supplied entry/result namespace.
//! A nested body local cannot borrow an equally numbered parameter slot.

use super::super::{
    CheckedBooleanExpression, CheckedScalarExpression, LoweringError, PrimitiveType,
};

pub(super) fn validate(predicate: &CheckedBooleanExpression) -> Result<(), LoweringError> {
    match predicate {
        CheckedBooleanExpression::Local { .. } | CheckedBooleanExpression::StorageRead { .. } => {
            Err(LoweringError::Unsupported(
                "scalar contract predicate contains body-local or mutable storage",
            ))
        }
        CheckedBooleanExpression::Not(operand) => validate(operand),
        facts::checked_boolean_triple_pattern!(left, right, ..) => {
            validate(left)?;
            validate(right)
        }
        CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
            crate::proofs::contract_predicates::validate_math_namespace(left, &scalar)?;
            crate::proofs::contract_predicates::validate_math_namespace(right, &scalar)
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. }
        | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
            scalar(left)?;
            scalar(right)
        }
        CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. }
        | CheckedBooleanExpression::Constant(_) => Ok(()),
        CheckedBooleanExpression::ByteSequencePredicate { root, .. } => root
            .parameter()
            .map(|_| ())
            .ok_or(LoweringError::Unsupported(
                "closed byte predicate requires a structural formal",
            )),
        CheckedBooleanExpression::StructuralParameterField { path, .. } => {
            if path.is_empty() {
                return Err(LoweringError::Unsupported(
                    "closed scalar contract observation requires a field path",
                ));
            }
            Ok(())
        }
        CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. } => Err(LoweringError::Unsupported(
            "closed scalar contract predicate requires a scalar namespace",
        )),
    }
}

fn scalar(expression: &CheckedScalarExpression) -> Result<(), LoweringError> {
    match expression {
        CheckedScalarExpression::RepresentationView { .. } => Err(LoweringError::Unsupported(
            "a representation re-view is not scalar contract arithmetic",
        )),
        CheckedScalarExpression::StructuralParameterIndexedRead { .. } => {
            Err(LoweringError::Unsupported(
                "indexed structural scalar reads are not supported contract operands",
            ))
        }
        CheckedScalarExpression::Local { .. } | CheckedScalarExpression::StorageRead { .. } => {
            Err(LoweringError::Unsupported(
                "scalar contract operand contains body-local or mutable storage",
            ))
        }
        CheckedScalarExpression::ErasedParameter { .. } => Ok(()),
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            scalar(left)?;
            scalar(right)
        }
        checked_trees::integer_widen_triple_pattern!(operand, ..) => scalar(operand),
        CheckedScalarExpression::Boolean(predicate) => validate(predicate),
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::ImplementationSelection { .. } => Ok(()),
        // The term owner rejoins the complete descriptor path against the
        // supplied structural signature. A local storage root cannot borrow
        // an equally numbered contract parameter.
        CheckedScalarExpression::StructuralParameterByteLength { root, .. } => root
            .parameter()
            .map(|_| ())
            .ok_or(LoweringError::Unsupported(
                "closed sequence extent requires a structural formal",
            )),
        // A member leaf roots at a declared structural parameter; the
        // structural signature lowering names its retained place and field
        // path. An empty path would name the whole parameter — never a
        // scalar leaf — and an address member has no fixed-integer carrier.
        // An IEEE leaf keeps its exact format for an IEEE comparison.
        CheckedScalarExpression::StructuralParameterField {
            path,
            primitive_type,
            ..
        } => {
            if path.is_empty() || *primitive_type == PrimitiveType::Addr {
                return Err(LoweringError::Unsupported(
                    "closed scalar contract operand requires a scalar namespace",
                ));
            }
            crate::emission::scalar_types::terminal_scalar_type(*primitive_type).map(|_| ())
        }
    }
}
