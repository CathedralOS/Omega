//! Integer widening and exact casts, and the bit re-view between a scalar
//! carrier and an integer of the same width.

use std::collections::BTreeMap;

use abstract_operations::AbstractOperation;
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::IntegerWiden { operand } => {
            let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied() else {
                return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
            };
            let ScalarType::Integer(target_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
            };
            AbstractOperation::IntegerWiden {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                source_type,
                target_type,
                operand,
            }
        }
        OperationKind::IntegerExactCast {
            operand,
            obligation,
        } => {
            let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied() else {
                return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                    operation.id,
                ));
            };
            let ScalarType::Integer(target_type) = operation.result.expect_scalar().scalar_type
            else {
                return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                    operation.id,
                ));
            };
            AbstractOperation::IntegerExactCast {
                psi_operation: operation.id,
                obligation,
                result: operation.result.expect_scalar().id,
                source_type,
                target_type,
                operand,
            }
        }
        // The verified pair is re-derived from the two declarations, not
        // trusted: a carrier pair outside the closed table is malformed.
        OperationKind::ScalarRepresentationView { operand, .. } => {
            let result = operation.result.expect_scalar();
            let source_type = value_types.get(&operand).copied().ok_or(
                LoweringError::VerifiedScalarRepresentationViewMalformed(operation.id),
            )?;
            terminal_semantics::ScalarRepresentationViewPair::classify(
                source_type,
                result.scalar_type,
            )
            .ok_or(LoweringError::VerifiedScalarRepresentationViewMalformed(
                operation.id,
            ))?;
            AbstractOperation::ScalarRepresentationView {
                psi_operation: operation.id,
                result: result.id,
                source_type,
                target_type: result.scalar_type,
                operand,
            }
        }
        _ => unreachable!("integer-conversion router is exhaustive"),
    })
}
