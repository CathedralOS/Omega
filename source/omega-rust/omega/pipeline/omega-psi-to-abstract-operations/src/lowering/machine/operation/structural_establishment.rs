use omega_abstract_operations::AbstractOperation;
use psi_terminal::{
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
    retain_payloadless_for_optimization: bool,
    byte_sequence_literals: &[StructuralLiteral<'_>],
    unit_affine_locals: &[StructuralLiteral<'_>],
    lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
    lowered_byte_sequence_literals: &mut usize,
) -> Result<AbstractOperation, LoweringError> {
    Ok(match operation.kind.clone() {
        OperationKind::EstablishPayloadlessCase { result_case } => {
            if !retain_payloadless_for_optimization {
                return Err(LoweringError::UnsupportedPayloadlessCase(operation.id));
            }
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedPayloadlessCase(operation.id));
            };
            AbstractOperation::EstablishPayloadlessCase {
                psi_operation: operation.id,
                result,
                result_case,
            }
        }
        OperationKind::EstablishByteSequenceLiteral { destination, bytes } => {
            let (place, ordinal, structural_type) = byte_sequence_literals
                .iter()
                .find(|(place, _, _)| place.id == destination)
                .copied()
                .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
            let declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .cloned()
                .ok_or(LoweringError::UnsupportedByteSequenceLiteral(operation.id))?;
            if usize::try_from(ordinal) != Ok(*lowered_byte_sequence_literals)
                || !matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        psi_terminal::ByteSequenceCarrier::BorrowedView
                    )
                )
            {
                return Err(LoweringError::UnsupportedByteSequenceLiteral(operation.id));
            }
            *lowered_byte_sequence_literals += 1;
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation: operation.id,
                place: *place,
                structural_type: declaration,
                bytes,
            }
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            let (place, ordinal, structural_type) = unit_affine_locals
                .iter()
                .find(|(place, _, _)| place.id == destination)
                .copied()
                .ok_or(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block.terminator.edge(),
                })?;
            let declaration = structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)
                .cloned()
                .ok_or(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block.terminator.edge(),
                })?;
            if usize::try_from(ordinal) != Ok(lowered_unit_affine_locals.len())
                || !matches!(declaration.shape, StructuralTypeShape::Record { ref fields } if fields.is_empty())
            {
                return Err(LoweringError::UnsupportedStructuralReturn {
                    machine: machine.id,
                    edge: block.terminator.edge(),
                });
            }
            lowered_unit_affine_locals.push((operation.id, *place, declaration.clone()));
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: operation.id,
                place: *place,
                structural_type: declaration,
            }
        }
        _ => unreachable!("structural-establishment router is exhaustive"),
    })
}
