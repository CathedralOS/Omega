//! Retained optimization-fact reconstruction and exact index validation.

use super::*;

pub(crate) fn validate_fact_index(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    let expected = reconstruct_fact_index(function);
    if expected != function.facts {
        return Err(OptimizationUnitValidationError::FactIndexMismatch(
            function.machine,
        ));
    }
    Ok(())
}

/// Every executable structural root is available only after its current
/// producer. Immutable source-frontier rows do not authorize a root at a
/// rewritten site. Compressed return-tuple locals are metadata-only and have
/// no executable producer, so they are deliberately absent from this walk.
pub(crate) fn reconstruct_fact_index(function: &PsiOptimizationFunction) -> Vec<OptimizationFact> {
    use abstract_operations::AbstractOperation as O;

    let mut expected = Vec::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        match operation {
            O::IntegerExactCast {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerAdd {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                ..
            } => expected.push(OptimizationFact::OperationObligationReference {
                obligation: *obligation,
                support: *psi_operation,
            }),
            _ => {}
        }
        match operation {
            O::BooleanConstant {
                psi_operation,
                result,
                value,
            } => expected.push(OptimizationFact::BooleanConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            O::IntegerConstant {
                psi_operation,
                result,
                value,
                ..
            } => expected.push(OptimizationFact::IntegerConstant {
                value: *result,
                constant: *value,
                support: *psi_operation,
            }),
            _ => {}
        }
    }
    expected
}
