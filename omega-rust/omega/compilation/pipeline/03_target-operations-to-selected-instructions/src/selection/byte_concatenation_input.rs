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

/// A whole owned home presents a shared descriptor without changing storage
/// identity. Selection still resolves the live home at the call; this only
/// classifies its exact producer and the descriptor's two-word geometry.
pub(super) fn shared_view_argument(
    function: &LegalizedScalarFunction,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
) -> Option<u64> {
    let signature = function.structural.as_ref()?;
    let mut producers = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|row| match &row.kind {
            LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
                result, ..
            } if result.place == semantic.place => Some((row, result)),
            _ => None,
        });
    let (producer_row, result) = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    if !semantic.path.is_empty()
        || semantic.access != StructuralAccess::SharedBorrow
        || target.place != semantic.place
        || target.access != semantic.access
        || !target.path.is_empty()
        || target.root_structural_type != result.structural_type
        || target.source
            != (target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer_row.operation,
            })
        || target.source_byte_offset != 0
        || target.fixed_array_length.is_some()
        || target.element_stride.is_some()
        || !target.indices.is_empty()
        || target.shape != calling_conventions::ValueShape::borrowed_reference(16, 8)
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !signature.structural_types.iter().any(|declaration| {
            declaration.id == target.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: None | Some(StructuralAccess::SharedBorrow),
                    })
                )
        })
    {
        return None;
    }
    capacity(function, producer_row).map(u64::from)
}

/// The structural type of a place whose bytes use the owner layout (length
/// word at offset 0, bytes from offset 8): an owned concatenation result in
/// this activation, or a shared bounded owner loan whose signature or block
/// parameter pointer is the owner's address.
pub(super) fn owner_layout_type(
    function: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    owned_type(function, place).or_else(|| {
        let signature = function.structural.as_ref()?;
        signature
            .parameters
            .iter()
            .map(|parameter| &parameter.semantic)
            .chain(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|parameter| parameter.place == place)
            .filter(|parameter| {
                abstract_operations::control_flow::owner_loans::is_shared_bounded_owner_loan_in(
                    parameter,
                    &signature.structural_types,
                )
                    || (signature.parameters.iter().any(|declared| declared.semantic.place == place)
                        && abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter_in(
                            parameter, &signature.structural_types,
                        ))
            })
            .map(|parameter| parameter.structural_type)
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
    let signature = function.structural.as_ref()?;
    let parameter = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == source.place);
    let structural_type = if let Some(parameter) = parameter {
        if !matches!(
            parameter.access,
            StructuralAccess::Owned
                | StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
        ) || parameter.multiplicity == StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return None;
        }
        parameter.structural_type
    } else {
        super::aggregate_result_input::home(function, source.place)?
            .1
            .structural_type
    };
    let (offset, _) = crate::structural_inputs::structural_reference_input::byte_field_storage(
        structural_type,
        &source.path,
        *field,
        &signature.structural_types,
    )?;
    offset.checked_add(8)
}
