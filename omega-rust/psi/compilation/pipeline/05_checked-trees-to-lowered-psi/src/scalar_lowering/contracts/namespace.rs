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
        // The term owner resolves both a whole primitive referent and a
        // projected leaf against the declared formal's exact type and access.
        CheckedBooleanExpression::StructuralParameterField { .. } => Ok(()),
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
            validate_indexed_operands(expression, &scalar)
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
        CheckedScalarExpression::StructuralSequenceLength { root, path } => {
            root.parameter().ok_or(LoweringError::Unsupported(
                "closed sequence extent requires a structural formal",
            ))?;
            for step in path {
                if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) = step {
                    scalar(&selector.expression)?;
                }
            }
            Ok(())
        }
        // A member leaf roots at a declared structural parameter; the
        // structural signature lowering names its retained place and field
        // path. An empty path observes the whole primitive referent; the term
        // owner requires that exact scalar endpoint and readable access.
        // An address member has no fixed-integer carrier.
        // An IEEE leaf keeps its exact format for an IEEE comparison.
        CheckedScalarExpression::StructuralParameterField { primitive_type, .. } => {
            if *primitive_type == PrimitiveType::Addr {
                return Err(LoweringError::Unsupported(
                    "closed scalar contract operand requires a scalar namespace",
                ));
            }
            crate::emission::scalar_types::terminal_scalar_type(*primitive_type).map(|_| ())
        }
    }
}

/// A closed observation and every selector must belong to the supplied formal
/// namespace; accepting its outer structural root cannot admit a body local
/// hidden inside a nested index expression.
pub(crate) fn validate_indexed_operands(
    expression: &CheckedScalarExpression,
    validate: &impl Fn(&CheckedScalarExpression) -> Result<(), LoweringError>,
) -> Result<(), LoweringError> {
    let CheckedScalarExpression::StructuralParameterIndexedRead {
        root,
        index,
        element_path,
        ..
    } = expression
    else {
        return Err(LoweringError::Unsupported(
            "contract has no indexed observation",
        ));
    };
    root.parameter().ok_or(LoweringError::Unsupported(
        "indexed contract observation requires a structural formal",
    ))?;
    validate(&index.expression)?;
    for projection in element_path {
        if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) = projection {
            validate(&selector.expression)?;
        }
    }
    Ok(())
}
