//! Reconstruct owned byte storage and readable operands from exact declarations.
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeShape,
};

pub(super) fn owned_type(
    function: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    let signature = function.structural.as_ref()?;
    let mut matching = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|row| {
            let result = match &row.kind {
                LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                    result,
                    ..
                } => result,
                LegalizedScalarInstructionKind::Call(call) => {
                    super::aggregate_result_input::call_result(function, call)?.0
                }
                _ => return None,
            };
            (result.place == place
                && result.multiplicity == StructuralMultiplicity::Unrestricted
                && result.projected_qualifications.is_empty()
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty()
                && signature.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(
                                ByteSequenceCarrier::BoundedOwned { .. }
                            )
                        )
                }))
            .then_some(result.structural_type)
        });
    let structural_type = matching.next()?;
    matching.next().is_none().then_some(structural_type)
}

/// The structural type of a place whose bytes use the owner layout (length
/// word at offset 0, bytes from offset 8): an owned concatenation result in
/// this activation or a bounded-owner arrival. Owned, shared, mutable and
/// write-only arrivals differ in permission, not the owner's field offsets.
/// Legalization checks the operation's access; this query only chooses storage.
pub(super) fn owner_layout_type(
    function: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    owned_type(function, place).or_else(|| {
        let signature = function.structural.as_ref()?;
        let mut parameters = signature
            .parameters
            .iter()
            .map(|parameter| &parameter.semantic)
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .filter(|parameter| parameter.place == place);
        let parameter = parameters.next()?;
        (parameters.next().is_none()
            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
            && parameter.projected_qualifications.is_empty()
            && signature.structural_types.iter().any(|declaration| {
                declaration.id == parameter.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
                    )
            }))
        .then_some(parameter.structural_type)
    })
}

pub(super) fn capacity(
    function: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<u32> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
        result,
        left,
        right,
        ..
    } = &row.kind
    else {
        return None;
    };
    let signature = function.structural.as_ref()?;
    if row.result.is_some()
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.qualification_establishments.is_empty()
        || !result.projected_qualifications.is_empty()
        || !signature.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == (StructuralPlaceKind::OperationResult {
                        producer: row.operation,
                        structural_type: result.structural_type,
                    })
        })
    {
        return None;
    }
    let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
        signature
            .structural_types
            .iter()
            .find(|declaration| declaration.id == result.structural_type)?
            .shape
    else {
        return None;
    };
    for operand in [left, right] {
        if operand.source() == result.place {
            return None;
        }
        match operand {
            ByteSequenceOperand::Whole { source, .. } => {
                super::established_view_input::readable_byte_type(function, *source)?;
            }
            ByteSequenceOperand::Field { .. } => {
                field_offset(function, operand)?;
            }
        }
    }
    u32::try_from(capacity).ok()?.checked_add(8)?;
    u32::try_from(capacity).ok()
}

pub(super) fn field_offset(
    function: &LegalizedScalarFunction,
    operand: &ByteSequenceOperand,
) -> Option<u32> {
    let ByteSequenceOperand::Field { source, field, .. } = operand else {
        return None;
    };
    if source.access != StructuralAccess::SharedBorrow {
        return None;
    }
    super::field_bytes::FieldBytes::resolve(function, source, *field, &[])
        .map(super::field_bytes::FieldBytes::offset)
}
