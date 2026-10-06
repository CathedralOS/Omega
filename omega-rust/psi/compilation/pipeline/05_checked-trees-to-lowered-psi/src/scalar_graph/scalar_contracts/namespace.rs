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
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
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
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => {
            Err(LoweringError::Unsupported(
                "closed scalar contract predicate requires a scalar namespace",
            ))
        }
    }
}

fn scalar(expression: &CheckedScalarExpression) -> Result<(), LoweringError> {
    match expression {
        CheckedScalarExpression::IntegerWrappingCast { .. } => Err(LoweringError::Unsupported(
            "wrapping conversion is not supported scalar contract arithmetic",
        )),
        CheckedScalarExpression::RepresentationView { .. } => Err(LoweringError::Unsupported(
            "a representation re-view is not scalar contract arithmetic",
        )),
        CheckedScalarExpression::StructuralParameterIndexedRead { .. } => {
            Err(LoweringError::Unsupported(
                "indexed structural scalar reads are not supported contract operands",
            ))
        }
        CheckedScalarExpression::IntegerTrappingCast { .. } => Err(LoweringError::Unsupported(
            "trapping conversion is not total scalar contract arithmetic",
        )),
        CheckedScalarExpression::IntegerSaturatingCast { .. } => Err(LoweringError::Unsupported(
            "saturating conversion is not supported scalar contract arithmetic",
        )),
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
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. } => scalar(operand),
        CheckedScalarExpression::Boolean(predicate) => validate(predicate),
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. } => Ok(()),
        // A whole-view extent reads the parameter's own view descriptor — an
        // entry observation the closed namespace admits once the lowering
        // names the retained place. A nested byte-field extent still fails
        // closed at the term: its carrier path needs canonical field ids the
        // contract namespace does not mint.
        CheckedScalarExpression::StructuralParameterByteLength { path, .. } if path.is_empty() => {
            Ok(())
        }
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
        CheckedScalarExpression::StructuralParameterByteLength { .. } => {
            Err(LoweringError::Unsupported(
                "closed scalar contract operand requires a scalar namespace",
            ))
        }
    }
}
