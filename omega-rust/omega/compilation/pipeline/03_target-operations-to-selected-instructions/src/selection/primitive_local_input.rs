//! Primitive referent geometry and original local producer identity.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind as Instruction,
};
use semantic_vocabulary::{OperationId, PlaceId, ScalarType, StructuralPlaceKind};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralOperationResult, StructuralTypeShape,
};

pub(crate) fn local(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<(
    OperationId,
    &StructuralOperationResult,
    ScalarType,
    ValueShape,
)> {
    let signature = source.structural.as_ref()?;
    let mut candidates = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|row| match &row.kind {
            Instruction::EstablishPrimitiveLocal {
                result,
                value,
                shape,
            } if result.place == place => Some((row, result, value, shape)),
            _ => None,
        });
    let (row, result, value, shape) = candidates.next()?;
    if candidates.next().is_some()
        || row.result.is_some()
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !signature.structural_places.iter().any(|declaration| {
            declaration.id == place
                && declaration.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: row.operation,
                        structural_type: result.structural_type,
                    }
        })
        || !signature.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && declaration.shape == StructuralTypeShape::PrimitiveScalar(value.scalar_type)
        })
        || super::scalar_call_abi::scalar_shape(value.scalar_type) != Some(*shape)
    {
        return None;
    }
    Some((row.operation, result, value.scalar_type, *shape))
}
pub(super) fn readable(
    source: &LegalizedScalarFunction,
    place: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    scalar: ScalarType,
) -> bool {
    if super::scalar_call_abi::scalar_shape(scalar).is_none() {
        return false;
    }
    if let Some((_, _, actual, _)) = local(source, place) {
        return path.is_empty() && actual == scalar;
    }
    if let Some((_, result)) = super::aggregate_result_input::home(source, place) {
        return !path.is_empty()
            && result.multiplicity == StructuralMultiplicity::Unrestricted
            && source.structural.as_ref().is_some_and(|signature| {
                crate::structural_inputs::structural_reference_input::primitive_read_geometry(
                    result.structural_type,
                    path,
                    scalar,
                    &signature.structural_types,
                )
                .is_some()
            });
    }
    source.structural.as_ref().is_some_and(|signature| {
        signature.entry_claims.is_empty()
            && borrowed_parameter(source, place).is_some_and(|parameter| {
                (matches!(
                    parameter.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                ) || (parameter.access == StructuralAccess::Owned && !path.is_empty()))
                    && parameter.multiplicity != StructuralMultiplicity::Linear
                    && (!path.is_empty()
                        || parameter.multiplicity == StructuralMultiplicity::Unrestricted)
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
                    && crate::structural_inputs::structural_reference_input::primitive_read_geometry(
                        parameter.structural_type,
                        path,
                        scalar,
                        &signature.structural_types,
                    )
                    .is_some()
            })
    })
}

/// Entry referents and state address joins retain the same declared storage
/// type. The join lookup also rejoins its exact block/place declaration.
fn borrowed_parameter(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<&terminal_psi::StructuralParameterDeclaration> {
    source
        .structural
        .as_ref()?
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == place)
        .map(|parameter| &parameter.semantic)
        .or_else(|| super::address_join_input::join(source, place).map(|(_, parameter)| parameter))
}

/// Exact field geometry is independent of whether its home came from an owned
/// value ABI, an established local, or an original borrowed referent.
pub(super) fn read_geometry(
    source: &LegalizedScalarFunction,
    row: &legalized_operations::LegalizedScalarInstruction,
) -> Option<(PlaceId, u32)> {
    let scalar = row.result?.scalar_type;
    match &row.kind {
        // A projection through runtime elements returns its static offset;
        // the row's traversals must be exactly the path's runtime elements,
        // which the read scales into the address before the load.
        Instruction::PrimitiveScalarRead {
            source: place,
            path,
            indices,
        } => {
            if !readable(source, *place, path, scalar) {
                return None;
            }
            if local(source, *place).is_some() {
                return indices.is_empty().then_some((*place, 0));
            }
            let signature = source.structural.as_ref()?;
            let structural_type = super::aggregate_result_input::home(source, *place)
                .map(|(_, result)| result.structural_type)
                .or_else(|| {
                    borrowed_parameter(source, *place).map(|parameter| parameter.structural_type)
                })?;
            let (offset, _, elements) =
                crate::structural_inputs::structural_reference_input::primitive_read_geometry(
                    structural_type,
                    path,
                    scalar,
                    &signature.structural_types,
                )?;
            crate::structural_inputs::structural_reference_input::runtime_indices_match(
                indices, &elements,
            )
            .then_some((*place, offset))
        }
        Instruction::StructuralScalarFieldRead {
            source: argument,
            field,
            ..
        }
        | Instruction::StructuralByteSequenceFieldLength {
            source: argument,
            field,
        } => {
            let observes_byte_length = matches!(
                row.kind,
                Instruction::StructuralByteSequenceFieldLength { .. }
            );
            let signature = source.structural.as_ref()?;
            if !signature.entry_claims.is_empty() {
                return None;
            }
            let structural_type = if let Some(parameter) = signature
                .parameters
                .iter()
                .map(|parameter| &parameter.semantic)
                .chain(
                    source
                        .blocks
                        .iter()
                        .flat_map(|block| &block.structural_parameters),
                )
                .find(|parameter| parameter.place == argument.place)
            {
                if parameter.access != argument.access
                    || (parameter.access == StructuralAccess::WriteOnlyBorrow
                        && !observes_byte_length)
                    || (parameter.access == StructuralAccess::Owned && observes_byte_length)
                    || parameter.multiplicity == StructuralMultiplicity::Linear
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return None;
                }
                parameter.structural_type
            } else {
                if argument.access != StructuralAccess::Owned || observes_byte_length {
                    return None;
                }
                let mut producers = source
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .filter_map(|row| {
                        // Explicit and zeroed construction, copies and call
                        // results all publish owned homes. Reads retain the
                        // exact producer/type custody regardless of how that
                        // home's fields were initialized.
                        let result = match &row.kind {
                            Instruction::EstablishRecord { result, .. }
                            | Instruction::EstablishZeroedStructural { result, .. }
                            | Instruction::StructuralLeafCopy { result, .. } => result,
                            Instruction::Call(call) => call.structural_result.as_ref()?,
                            _ => return None,
                        };
                        (result.place == argument.place).then_some((row.operation, result))
                    });
                let (producer, result) = producers.next()?;
                if producers.next().is_some()
                    || result.multiplicity == StructuralMultiplicity::Linear
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                    || !signature.structural_places.iter().any(|declaration| {
                        declaration.id == argument.place
                            && declaration.kind
                                == StructuralPlaceKind::OperationResult {
                                    producer,
                                    structural_type: result.structural_type,
                                }
                    })
                {
                    return None;
                }
                result.structural_type
            };
            // A case-qualified read loads from its payload overlay; the
            // offset comes from that declared case's layout, never the row.
            let (offset, _) = match &row.kind {
                Instruction::StructuralScalarFieldRead {
                    case: Some(case), ..
                } => crate::structural_inputs::structural_reference_input::case_field_read(
                    structural_type,
                    &argument.path,
                    *case,
                    *field,
                    scalar,
                    &signature.structural_types,
                )?,
                _ if observes_byte_length => {
                    crate::structural_inputs::structural_reference_input::byte_field_length(
                        structural_type,
                        &argument.path,
                        *field,
                        scalar,
                        &signature.structural_types,
                    )?
                }
                _ => crate::structural_inputs::structural_reference_input::field_read(
                    structural_type,
                    &argument.path,
                    *field,
                    scalar,
                    &signature.structural_types,
                )?,
            };
            Some((argument.place, offset))
        }
        _ => None,
    }
}
