//! Retain an immutable literal's exact place, type, and bytes in an ordered Unit body.

use super::super::shared::*;

pub(super) fn lower(
    operation: &AbstractOperation,
    machine: MachineId,
    nonreturning_boundary: bool,
    established: &mut BTreeMap<PlaceId, (OperationId, StructuralTypeDeclaration, Vec<u8>)>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::EstablishByteSequenceLiteral {
        psi_operation,
        place,
        structural_type,
        bytes,
    } = operation
    else {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(machine));
    };
    if nonreturning_boundary
        || !matches!(
            (&place.kind, &structural_type.shape),
            (
                semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type: place_type, ..
                },
                StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)
            ) if *place_type == structural_type.id
        )
        || established
            .insert(
                place.id,
                (*psi_operation, structural_type.clone(), bytes.clone()),
            )
            .is_some()
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(machine));
    }
    operations.push(TargetUnitOperation::EstablishByteSequenceLiteral {
        psi_operation: *psi_operation,
        place: *place,
        structural_type: structural_type.clone(),
        bytes: bytes.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
