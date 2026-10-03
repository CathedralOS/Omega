use abstract_operations::AbstractOperation;
use terminal_psi::{
    Block, Operation, OperationKind, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalMachine,
};

use super::super::{LoweredAffineLocal, StructuralLiteral};
use crate::lowering::LoweringError;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &Operation,
    block: &Block,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    byte_sequence_literals: &[StructuralLiteral<'_>],
    unit_affine_locals: &[StructuralLiteral<'_>],
    lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::EstablishByteSequenceConcatenation {
            left,
            right,
            capacity_obligation,
        } => {
            let result = operation.result.structural().cloned().ok_or(
                LoweringError::InvalidByteSequenceConcatenation(operation.id),
            )?;
            // Preserve the validated live extents and source projections. An
            // owned result is fresh storage, never a view into either operand.
            AbstractOperation::EstablishByteSequenceConcatenation {
                psi_operation: operation.id,
                result,
                left,
                right,
                capacity_obligation,
            }
        }
        OperationKind::EstablishStructuralArray { elements } => {
            let result = operation
                .result
                .structural()
                .cloned()
                .ok_or(LoweringError::UnsupportedScalarArray(operation.id))?;
            AbstractOperation::EstablishStructuralArray {
                psi_operation: operation.id,
                result,
                elements,
            }
        }
        OperationKind::EstablishScalarArray { elements } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedScalarArray(operation.id));
            };
            // Canonical admission checked exact shape, leaf types and dominance.
            // Preserve those operands and their result identity for independent
            // current-IR validation; native layout is not decided by projection.
            AbstractOperation::EstablishScalarArray {
                psi_operation: operation.id,
                result,
                elements,
            }
        }
        OperationKind::EstablishScalarCase {
            result_case,
            fields,
        } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedPayloadlessCase(operation.id));
            };
            AbstractOperation::EstablishCase {
                psi_operation: operation.id,
                result,
                result_case,
                fields: fields
                    .into_iter()
                    .map(|field| terminal_psi::RecordFieldInitializer {
                        field: field.field,
                        value: terminal_psi::RecordFieldValue::Scalar {
                            value: field.value,
                            range_obligation: field.range_obligation,
                        },
                    })
                    .collect(),
            }
        }
        OperationKind::EstablishStructuralCase {
            result_case,
            fields,
        } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedStructuralCase(operation.id));
            };
            AbstractOperation::EstablishCase {
                psi_operation: operation.id,
                result,
                result_case,
                fields,
            }
        }
        OperationKind::EstablishByteSequenceLiteral {
            destination,
            bytes,
            qualifications,
            ..
        } => {
            let (place, _, structural_type) = byte_sequence_literals
                .iter()
                .find(|(place, _, _)| place.id == destination)
                .copied()
                .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
            let declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .cloned()
                .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
            if !matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
            ) {
                return Err(LoweringError::UnsupportedByteSequenceLiteral(operation.id));
            }
            // The ordinal identifies a declaration, not an evaluation position.
            // Terminal verification already checks dense declaration ordinals,
            // exactly one establishment per place, and dominance of each use.
            // Keep the operation at its authored position and retain that exact
            // place rather than imposing a second traversal-order restriction.
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation: operation.id,
                place: *place,
                structural_type: declaration,
                bytes,
                qualifications,
            }
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            let (place, ordinal, structural_type) = unit_affine_locals
                .iter()
                .find(|(place, _, _)| place.id == destination)
                .copied()
                .ok_or_else(|| LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block
                        .terminator
                        .edges()
                        .next()
                        .expect("every terminator carries an edge"),
                })?;
            let declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .cloned()
                .ok_or_else(|| LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block
                        .terminator
                        .edges()
                        .next()
                        .expect("every terminator carries an edge"),
                })?;
            if usize::try_from(ordinal) != Ok(lowered_unit_affine_locals.len())
                || !matches!(declaration.shape, StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block
                        .terminator
                        .edges()
                        .next()
                        .expect("every terminator carries an edge"),
                });
            }
            lowered_unit_affine_locals.push((operation.id, *place, declaration.clone()));
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: operation.id,
                place: *place,
                structural_type: declaration,
            }
        }
        OperationKind::EstablishZeroedStructural { .. } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block
                        .terminator
                        .edges()
                        .next()
                        .expect("every terminator carries an edge"),
                });
            };
            AbstractOperation::EstablishZeroedStructural {
                psi_operation: operation.id,
                result,
            }
        }
        OperationKind::EstablishStructuralLocal { .. } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block
                        .terminator
                        .edges()
                        .next()
                        .expect("every terminator carries an edge"),
                });
            };
            AbstractOperation::EstablishStructuralLocal {
                psi_operation: operation.id,
                result,
            }
        }
        OperationKind::EstablishRecord { fields } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block
                        .terminator
                        .edges()
                        .next()
                        .expect("every terminator carries an edge"),
                });
            };
            AbstractOperation::EstablishRecord {
                psi_operation: operation.id,
                result,
                fields,
            }
        }
        _ => unreachable!("structural-establishment router is exhaustive"),
    })
}
