//! Retained optimization-fact reconstruction and exact index validation.
use crate::OptimizationUnitValidationError;
use optimization_unit::{OptimizationFact, PsiOptimizationFunction};

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
    use abstract_operations::{AbstractOperation as O, IntegerShiftPolicy};

    let mut expected = Vec::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        match operation {
            O::StructuralScalarFieldStore {
                psi_operation,
                range_obligation: Some(obligation),
                ..
            }
            | O::StructuralByteViewFieldStore {
                psi_operation,
                predicate_obligation: Some(obligation),
                ..
            } => expected.push(OptimizationFact::OperationObligationReference {
                obligation: *obligation,
                support: *psi_operation,
            }),
            O::EstablishCase {
                psi_operation,
                fields,
                ..
            }
            | O::EstablishRecord {
                psi_operation,
                fields,
                ..
            } => {
                expected.extend(fields.iter().filter_map(|initializer| {
                    let terminal_psi::RecordFieldValue::Scalar {
                        range_obligation, ..
                    } = &initializer.value
                    else {
                        return None;
                    };
                    range_obligation.map(|obligation| {
                        OptimizationFact::OperationObligationReference {
                            obligation,
                            support: *psi_operation,
                        }
                    })
                }));
            }

            O::EstablishByteSequenceConcatenation {
                psi_operation,
                capacity_obligation: obligation,
                ..
            }
            | O::ByteSequenceSubslice {
                psi_operation,
                obligation,
                ..
            }
            | O::ByteSequenceWrite {
                psi_operation,
                obligation,
                ..
            }
            | O::ByteSequenceStore {
                psi_operation,
                obligation,
                ..
            }
            | O::StructuralByteSequenceFieldStore {
                psi_operation,
                obligation,
                ..
            }
            | O::StructuralByteSequenceFieldByteStore {
                psi_operation,
                obligation,
                ..
            }
            | O::ByteSequenceRead {
                psi_operation,
                obligation,
                ..
            }
            | O::StructuralByteSequenceFieldRead {
                psi_operation,
                obligation,
                ..
            }
            | O::ElementViewSubslice {
                psi_operation,
                obligation,
                ..
            }
            | O::ElementViewRead {
                psi_operation,
                obligation,
                ..
            }
            | O::ElementViewWrite {
                psi_operation,
                obligation,
                ..
            }
            | O::IntegerExactCast {
                psi_operation,
                obligation,
                ..
            } => expected.push(OptimizationFact::OperationObligationReference {
                obligation: *obligation,
                support: *psi_operation,
            }),
            O::IntegerShift {
                psi_operation,
                policy: IntegerShiftPolicy::Exact { obligation },
                ..
            } => expected.push(OptimizationFact::OperationObligationReference {
                obligation: *obligation,
                support: *psi_operation,
            }),
            O::IntegerArithmetic {
                psi_operation,
                arithmetic,
                ..
            } => {
                if let Some(obligation) = arithmetic.obligation() {
                    expected.push(OptimizationFact::OperationObligationReference {
                        obligation,
                        support: *psi_operation,
                    });
                }
            }
            _ => {}
        }
        // Each runtime-selected path element's bound, owned by its operation.
        if let Some(support) = operation.psi_operation() {
            expected.extend(
                operation
                    .runtime_indices()
                    .into_iter()
                    .map(
                        |(_, obligation)| OptimizationFact::OperationObligationReference {
                            obligation,
                            support,
                        },
                    ),
            );
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
