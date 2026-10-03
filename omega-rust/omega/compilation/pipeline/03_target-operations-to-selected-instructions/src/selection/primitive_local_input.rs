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
            && matches!(
                result.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            )
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
            && read_parameter(source, place).is_some_and(|parameter| {
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

/// Entry and state parameters retain their declared referent type regardless
/// of native carrier. The state lookup rejoins the exact block/place identity;
/// callers independently check access, multiplicity and projection geometry.
pub(super) fn read_parameter(
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
        .or_else(|| block_arrival(source, place))
}

/// A non-entry block's structural parameter at its exact block/place
/// declaration.
fn block_arrival(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<&terminal_psi::StructuralParameterDeclaration> {
    let signature = source.structural.as_ref()?;
    let mut matching = source.blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
            .map(move |parameter| (block.id, parameter))
    });
    let (block, parameter) = matching.next()?;
    (matching.next().is_none()
        && block != source.entry_block
        && signature.structural_places.iter().any(|declaration| {
            declaration.id == place
                && declaration.kind
                    == (StructuralPlaceKind::BlockParameter {
                        block,
                        position: parameter.position,
                    })
        }))
    .then_some(parameter)
}

/// Exact field geometry is independent of whether its home came from an owned
/// value ABI, an established local, or an original borrowed referent.
pub(super) fn read_geometry(
    source: &LegalizedScalarFunction,
    row: &legalized_operations::LegalizedScalarInstruction,
) -> Option<(PlaceId, u32)> {
    let scalar = row.result?.scalar_type;
    match &row.kind {
        Instruction::StructuralByteSequenceFieldLength {
            source: argument,
            field,
            indices,
        } => {
            let signature = source.structural.as_ref()?;
            let root = super::field_bytes::root_type(source, argument)?;
            let geometry =
                crate::structural_inputs::structural_reference_input::projected_byte_field(
                    root,
                    &argument.path,
                    *field,
                    &signature.structural_types,
                )?;
            let count = ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .ok()?,
            );
            (scalar == count
                && crate::structural_inputs::structural_reference_input::runtime_indices_match(
                    indices,
                    &geometry.elements,
                ))
            .then_some((argument.place, geometry.length_offset))
        }

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
                    read_parameter(source, *place).map(|parameter| parameter.structural_type)
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
        } => {
            let signature = source.structural.as_ref()?;
            if !signature.entry_claims.is_empty() {
                return None;
            }
            // Field length and scalar reads use the same exact incoming or
            // state-owned storage declaration. Ownership does not hide the
            // byte field's extent; it remains separate from content authority.
            let structural_type = if let Some(parameter) = read_parameter(source, argument.place) {
                if parameter.access != argument.access
                    || parameter.access == StructuralAccess::WriteOnlyBorrow
                    || parameter.multiplicity == StructuralMultiplicity::Linear
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return None;
                }
                parameter.structural_type
            } else {
                // The producer publishes the exact storage declaration. Reads
                // consume that identity, not a second list of admitted producer
                // operations. Replaying geometry cannot create permissions.
                let (_, result) =
                    super::aggregate_result_input::operation_result(source, argument.place)?;
                if result.multiplicity == StructuralMultiplicity::Linear
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                {
                    return None;
                }
                let expected_access = match signature
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == result.structural_type)?
                    .shape
                {
                    // A read does not attenuate the retained carrier declaration:
                    // mutable references remain mutable, even when observing.
                    StructuralTypeShape::Reference { access, .. } => access,
                    StructuralTypeShape::ElementView { .. } => StructuralAccess::SharedBorrow,
                    _ => StructuralAccess::Owned,
                };
                if argument.access != expected_access {
                    return None;
                }
                result.structural_type
            };
            let (structural_type, path) = if let Some((
                terminal_psi::StructuralPathSegment::Referent,
                suffix,
            )) = argument.path.split_first()
            {
                let StructuralTypeShape::Reference {
                    referent,
                    access:
                        access @ (StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow),
                } = signature
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)?
                    .shape
                else {
                    return None;
                };
                if argument.access != access {
                    return None;
                }
                (referent, suffix)
            } else {
                (structural_type, argument.path.as_slice())
            };
            // A case-qualified read loads from its payload overlay; the
            // offset comes from that declared case's layout, never the row.
            let (offset, _) = match &row.kind {
                Instruction::StructuralScalarFieldRead {
                    case: Some(case), ..
                } => crate::structural_inputs::structural_reference_input::case_field_read(
                    structural_type,
                    path,
                    *case,
                    *field,
                    scalar,
                    &signature.structural_types,
                )?,
                _ => crate::structural_inputs::structural_reference_input::field_read(
                    structural_type,
                    path,
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
