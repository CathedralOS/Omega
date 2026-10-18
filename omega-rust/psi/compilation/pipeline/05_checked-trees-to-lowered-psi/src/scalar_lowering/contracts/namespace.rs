//! Closed scalar contracts use only the supplied entry/result namespace.
//! A nested body local cannot borrow an equally numbered parameter slot.

use super::super::{CheckedBooleanExpression, CheckedScalarExpression, LoweringError};
use super::{ContractStructuralResult, ContractViewNamespace};

pub(super) fn validate(
    predicate: &CheckedBooleanExpression,
    views: &ContractViewNamespace<'_>,
) -> Result<(), LoweringError> {
    let namespace = ObservationNamespace {
        views,
        unavailable_result: std::cell::Cell::new(false),
    };
    inspect(predicate, &namespace)?;
    // Keep visiting after an unavailable result so it cannot hide undeclared
    // structural roots or unsupported grammar elsewhere in this predicate.
    // Scalar positions and complete field geometry stay with their term owners.
    if namespace.unavailable_result.get() {
        return Err(LoweringError::UnavailableContractResult);
    }
    Ok(())
}

struct ObservationNamespace<'scope, 'declarations> {
    views: &'scope ContractViewNamespace<'declarations>,
    // This one-pass reduction is invocation-local. The scalar/math visitors
    // borrow the same immutable namespace; no compiler-owned data is mutated.
    unavailable_result: std::cell::Cell<bool>,
}

impl ObservationNamespace<'_, '_> {
    fn structural(&self, position: u32) -> Result<(), LoweringError> {
        if self
            .views
            .parameters
            .iter()
            .any(|(candidate, _)| *candidate == position)
        {
            return Ok(());
        }
        match self.views.result {
            ContractStructuralResult::Declared {
                position: candidate,
                ..
            } if position == candidate => Ok(()),
            ContractStructuralResult::Unavailable {
                position: candidate,
            } if position == candidate => {
                self.unavailable_result.set(true);
                Ok(())
            }
            _ => Err(LoweringError::Unsupported(
                "closed contract names an undeclared structural observation",
            )),
        }
    }

    fn result(&self) -> Result<(), LoweringError> {
        match self.views.result {
            ContractStructuralResult::Declared { .. } => Ok(()),
            ContractStructuralResult::Unavailable { .. } => {
                self.unavailable_result.set(true);
                Ok(())
            }
            ContractStructuralResult::Absent => Err(LoweringError::Unsupported(
                "closed contract names an absent structural result",
            )),
        }
    }
}

fn inspect(
    predicate: &CheckedBooleanExpression,
    namespace: &ObservationNamespace<'_, '_>,
) -> Result<(), LoweringError> {
    match predicate {
        CheckedBooleanExpression::Local { .. } | CheckedBooleanExpression::StorageRead { .. } => {
            Err(LoweringError::Unsupported(
                "scalar contract predicate contains body-local or mutable storage",
            ))
        }
        CheckedBooleanExpression::Not(operand) => inspect(operand, namespace),
        facts::checked_boolean_triple_pattern!(left, right, ..) => {
            inspect(left, namespace)?;
            inspect(right, namespace)
        }
        CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
            crate::proofs::contract_predicates::validate_math_namespace(left, &|operand| {
                scalar(operand, namespace)
            })?;
            crate::proofs::contract_predicates::validate_math_namespace(right, &|operand| {
                scalar(operand, namespace)
            })
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. }
        | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
            scalar(left, namespace)?;
            scalar(right, namespace)
        }
        CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::Constant(_) => Ok(()),
        CheckedBooleanExpression::StructuralCaseMembership { subject, .. } => match subject.root {
            checked_trees::CheckedStructuralCaseRoot::Parameter { position } => {
                namespace.structural(position)
            }
            checked_trees::CheckedStructuralCaseRoot::Result => namespace.result(),
        },
        CheckedBooleanExpression::ByteSequencePredicate { root, .. } => {
            let position = root.parameter().ok_or(LoweringError::Unsupported(
                "closed byte predicate requires a structural formal",
            ))?;
            namespace.structural(position)
        }
        // The term owner resolves both a whole primitive referent and a
        // projected leaf against the declared formal's exact type and access.
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position, ..
        } => namespace.structural(*parameter_position),
        CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::StructuralIdentity { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. } => Err(LoweringError::Unsupported(
            "closed scalar contract predicate requires a scalar namespace",
        )),
    }
}

fn scalar(
    expression: &CheckedScalarExpression,
    namespace: &ObservationNamespace<'_, '_>,
) -> Result<(), LoweringError> {
    match expression {
        CheckedScalarExpression::RepresentationView { .. } => Err(LoweringError::Unsupported(
            "a representation re-view is not scalar contract arithmetic",
        )),
        CheckedScalarExpression::StructuralParameterIndexedRead { root, .. } => {
            let position = root.parameter().ok_or(LoweringError::Unsupported(
                "indexed contract observation requires a structural formal",
            ))?;
            namespace.structural(position)?;
            validate_indexed_operands(expression, &|operand| scalar(operand, namespace))
        }
        CheckedScalarExpression::Local { .. } | CheckedScalarExpression::StorageRead { .. } => {
            Err(LoweringError::Unsupported(
                "scalar contract operand contains body-local or mutable storage",
            ))
        }
        CheckedScalarExpression::ErasedParameter { .. } => Ok(()),
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            scalar(left, namespace)?;
            scalar(right, namespace)
        }
        checked_trees::integer_widen_triple_pattern!(operand, ..) => scalar(operand, namespace),
        CheckedScalarExpression::Boolean(predicate) => inspect(predicate, namespace),
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::ImplementationSelection { .. } => Ok(()),
        // The term owner rejoins the complete descriptor path against the
        // supplied structural signature. A local storage root cannot borrow
        // an equally numbered contract parameter.
        CheckedScalarExpression::StructuralSequenceLength { root, path } => {
            let position = root.parameter().ok_or(LoweringError::Unsupported(
                "closed sequence extent requires a structural formal",
            ))?;
            namespace.structural(position)?;
            for step in path {
                if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) = step {
                    scalar(&selector.expression, namespace)?;
                }
            }
            Ok(())
        }
        // A member leaf roots at a declared structural parameter; the
        // structural signature lowering names its retained place and field
        // path. An empty path observes the whole primitive referent; the term
        // owner requires that exact scalar endpoint and readable access.
        // Address leaves retain their address role; IEEE leaves retain their
        // exact format. Namespace admission does not convert either carrier.
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            primitive_type,
            ..
        } => {
            crate::emission::scalar_types::terminal_scalar_type(*primitive_type)?;
            namespace.structural(*parameter_position)
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
