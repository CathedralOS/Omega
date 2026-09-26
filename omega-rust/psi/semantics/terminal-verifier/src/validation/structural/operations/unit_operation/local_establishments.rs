//! Byte-sequence literal and trivial affine local establishments: each
//! destination is the place its declaration ordinal names.

use crate::validation::{
    ModuleError, OperationKind, StructuralPlaceKind, StructuralTypeShape, TerminalMachine,
    TerminalModule,
};

pub(in crate::validation) fn validate_establish_byte_sequence_literal(
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
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView { .. })
    ) {
        return Err(ModuleError::ByteSequenceLiteralRequiresBorrowedView {
            operation: operation.id,
            place: *destination,
        });
    }
    Ok(())
}

/// A zeroed structural establishment declares no operands: its structural
/// result must be an unrestricted operation result over a type whose whole
/// storage is zero-constructible (recursively plain-scalar leaves under
/// records and fixed arrays).
pub(in crate::validation) fn validate_establish_zeroed_structural(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::EstablishZeroedStructural {} = &operation.kind else {
        unreachable!("dispatched validate_establish_zeroed_structural")
    };
    let Some(result) = operation.result.structural() else {
        return Err(ModuleError::ZeroedStructuralResultMismatch(operation.id));
    };
    if result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !matches!(
            machine.structural_places.iter().find(|place| place.id == result.place),
            Some(terminal_psi::StructuralPlaceDeclaration {
                kind: StructuralPlaceKind::OperationResult { producer, structural_type },
                ..
            }) if *producer == operation.id && *structural_type == result.structural_type
        )
        || !terminal_semantics::zeroed_structural_shape(
            module.structural_types.iter(),
            result.structural_type,
        )
    {
        return Err(ModuleError::ZeroedStructuralResultMismatch(operation.id));
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
