//! Exact whole immutable byte-view observation without added proof equations.

use super::*;

pub(super) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidByteSequenceLengthSource {
        operation: operation.id,
        source,
    };
    let expected =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return Err(ModuleError::ByteSequenceLengthRequiresU64Result(
            operation.id,
        ));
    }
    let place = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)
        .ok_or_else(invalid)?;
    let structural_type = match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            let parameter = machine
                .structural_parameters
                .iter()
                .find(|parameter| {
                    parameter.place == source
                        && parameter.position == position
                        && parameter.is_self == is_self
                })
                .ok_or_else(invalid)?;
            if parameter.access != StructuralAccess::SharedBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
            parameter.structural_type
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } => {
            // Literal foundation validation currently requires one block and one
            // establishment per declared literal. Require that establishment before this read.
            let [block] = machine.blocks.as_slice() else {
                return Err(invalid());
            };
            let position = block
                .operations
                .iter()
                .position(|candidate| candidate.id == operation.id)
                .ok_or_else(invalid)?;
            let established = block.operations[..position].iter().any(|candidate| {
                matches!(candidate.kind, OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == source)
            });
            if !established {
                return Err(invalid());
            }
            structural_type
        }
        _ => return Err(invalid()),
    };
    if !module.structural_types.iter().any(|declaration| {
        declaration.id == structural_type
            && matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)
            )
    }) {
        return Err(invalid());
    }
    Ok(())
}
