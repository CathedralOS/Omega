//! Byte-sequence literal and trivial affine local establishments: each
//! destination is the place its declaration ordinal names.

use crate::validation::{
    ModuleError, OperationKind, StructuralPlaceKind, StructuralTypeShape, TerminalMachine,
    TerminalModule,
};

pub(super) fn validate_establish_byte_sequence_literal(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::EstablishByteSequenceLiteral { destination, .. } = &operation.kind else {
        unreachable!("dispatched validate_establish_byte_sequence_literal")
    };
    let Some(place) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *destination)
    else {
        return Err(ModuleError::UnknownByteSequenceLiteral {
            operation: operation.id,
            place: *destination,
        });
    };
    let StructuralPlaceKind::ByteSequenceLiteral {
        structural_type, ..
    } = place.kind
    else {
        return Err(ModuleError::UnknownByteSequenceLiteral {
            operation: operation.id,
            place: *destination,
        });
    };
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
    else {
        return Err(ModuleError::UnknownStructuralType(structural_type));
    };
    if !matches!(
        declaration.shape,
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)
    ) {
        return Err(ModuleError::ByteSequenceLiteralRequiresBorrowedView {
            operation: operation.id,
            place: *destination,
        });
    }
    Ok(())
}

pub(crate) fn validate_establish_trivial_affine_local(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::EstablishTrivialAffineLocal { destination } = &operation.kind else {
        unreachable!("dispatched validate_establish_trivial_affine_local")
    };
    let Some(place) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *destination)
    else {
        return Err(ModuleError::UnknownTrivialAffineLocal {
            operation: operation.id,
            place: *destination,
        });
    };
    let StructuralPlaceKind::TrivialAffineLocal {
        structural_type, ..
    } = place.kind
    else {
        return Err(ModuleError::UnknownTrivialAffineLocal {
            operation: operation.id,
            place: *destination,
        });
    };
    let Some(declaration) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
    else {
        return Err(ModuleError::UnknownStructuralType(structural_type));
    };
    if !matches!(declaration.shape, StructuralTypeShape::Record { ref fields } if fields.is_empty())
    {
        return Err(ModuleError::TrivialAffineLocalRequiresEmptyRecord {
            operation: operation.id,
            place: *destination,
        });
    }
    Ok(())
}
