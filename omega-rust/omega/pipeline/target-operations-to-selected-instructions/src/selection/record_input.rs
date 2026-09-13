//! Reconstruct record geometry and input storage needs from exact declarations.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::PlaceId;
use terminal_psi::{
    RecordFieldValue, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape,
};
/// Owned record inputs only need addressable storage when an operation observes a
/// field or lends the original value. Pure whole-value transport keeps its ABI
/// fragments; a materialized home becomes the value's storage for later uses.
pub(super) fn parameter_home_required(source: &LegalizedScalarFunction, place: PlaceId) -> bool {
    let Some(signature) = &source.structural else {
        return false;
    };
    if !signature.parameters.iter().any(|parameter| {
        parameter.semantic.place == place
            && parameter.semantic.access == StructuralAccess::Owned
            && signature.structural_types.iter().any(|declaration| {
                declaration.id == parameter.semantic.structural_type
                    && matches!(declaration.shape, StructuralTypeShape::Record { .. })
            })
    }) {
        return false;
    }
    source.blocks.iter().flat_map(|block| &block.instructions).any(|row| {
        match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, .. } => source.place == place,
            LegalizedScalarInstructionKind::Call(call) => call.arguments.iter().any(|argument| {
                matches!(argument, legalized_operations::LegalizedScalarArgument::Structural { semantic, .. }
                    if semantic.place == place && semantic.access != StructuralAccess::Owned)
            }),
            _ => false,
        }
    })
}

pub(super) fn fields(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<Vec<(u32, ValueShape, Option<semantic_vocabulary::ScalarType>)>> {
    let LegalizedScalarInstructionKind::EstablishRecord {
        result,
        fields,
        shape,
    } = &row.kind
    else {
        return None;
    };
    let declarations = &source.structural.as_ref()?.structural_types;
    if row.result.is_some()
        || result.multiplicity == StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || crate::structural_reference_input::shape(result.structural_type, declarations)? != *shape
    {
        return None;
    }
    let mut selected_declarations = declarations
        .iter()
        .filter(|entry| entry.id == result.structural_type);
    let declaration = selected_declarations.next()?;
    if selected_declarations.next().is_some() {
        return None;
    }
    let StructuralTypeShape::Record { fields: expected } = &declaration.shape else {
        return None;
    };
    if fields.len() != expected.len() {
        return None;
    }
    let mut offset = 0u32;
    let mut placed = Vec::with_capacity(fields.len());
    for (field, expected) in fields.iter().zip(expected) {
        if field.field != expected.id || expected.relevance.is_erased() {
            return None;
        }
        let field_shape = match (&field.value, &expected.field_type) {
            (
                RecordFieldValue::Scalar {
                    range_obligation, ..
                },
                kind,
            ) if kind.scalar_type().is_some() => {
                if matches!(kind, StructuralFieldType::BoundedInteger(_))
                    != range_obligation.is_some()
                {
                    return None;
                }
                super::scalar_call_abi::scalar_shape(kind.scalar_type()?)?
            }
            (RecordFieldValue::Structural(argument), StructuralFieldType::Structural(nested)) => {
                if !declarations.iter().any(|declaration| {
                    declaration.id == *nested
                        && matches!(declaration.shape, StructuralTypeShape::Record { .. })
                }) {
                    return None;
                }
                if argument.access != terminal_psi::StructuralAccess::Owned
                    || !argument.path.is_empty()
                {
                    return None;
                }
                let signature = source.structural.as_ref()?;
                let parameters = signature
                    .parameters
                    .iter()
                    .map(|parameter| &parameter.semantic)
                    .chain(
                        source
                            .blocks
                            .iter()
                            .filter(|block| block.id != source.entry_block)
                            .flat_map(|block| &block.structural_parameters),
                    );
                let mut matches = parameters
                    .filter(|parameter| parameter.place == argument.place)
                    .map(|parameter| {
                        (
                            parameter.structural_type,
                            parameter.access == terminal_psi::StructuralAccess::Owned
                                && parameter.multiplicity != StructuralMultiplicity::Linear
                                && parameter.qualifications.is_empty()
                                && parameter.projected_qualifications.is_empty(),
                        )
                    })
                    .chain(
                        source
                            .blocks
                            .iter()
                            .flat_map(|block| &block.instructions)
                            .filter_map(|row| {
                                let result = match &row.kind {
                                    LegalizedScalarInstructionKind::EstablishRecord {
                                        result,
                                        ..
                                    } => result,
                                    LegalizedScalarInstructionKind::Call(call) => {
                                        call.structural_result.as_ref()?
                                    }
                                    _ => return None,
                                };
                                (result.place == argument.place).then_some((
                                    result.structural_type,
                                    result.multiplicity != StructuralMultiplicity::Linear
                                        && result.claims.is_empty()
                                        && result.qualifications.is_empty()
                                        && result.projected_qualifications.is_empty(),
                                ))
                            }),
                    );
                let (child_type, plain_owned) = matches.next()?;
                if matches.next().is_some() || child_type != *nested || !plain_owned {
                    return None;
                }
                crate::structural_reference_input::shape(*nested, declarations)?
            }
            _ => return None,
        };
        let alignment = u32::from(field_shape.alignment);
        offset = offset.checked_add(alignment.checked_sub(1)?)? / alignment * alignment;
        placed.push((offset, field_shape, expected.field_type.scalar_type()));
        offset = offset.checked_add(u32::from(field_shape.byte_size))?;
    }
    (offset <= u32::from(shape.byte_size)).then_some(placed)
}

pub(super) fn home(
    source: &LegalizedScalarFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<(
    semantic_vocabulary::OperationId,
    &terminal_psi::StructuralOperationResult,
)> {
    let mut selected = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|row| {
            let result = match &row.kind {
                LegalizedScalarInstructionKind::EstablishRecord { result, .. } => result,
                LegalizedScalarInstructionKind::Call(call) => call.structural_result.as_ref()?,
                _ => return None,
            };
            (result.place == place).then_some((row.operation, result))
        });
    let (operation, result) = selected.next()?;
    if selected.next().is_some()
        || result.multiplicity == StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !source
            .structural
            .as_ref()?
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.id == result.structural_type
                    && matches!(declaration.shape, StructuralTypeShape::Record { .. })
            })
    {
        return None;
    }
    Some((operation, result))
}

/// A selected record lives in the join's own home. Reconstruct its declaration
/// and exact place coordinate; source replay separately checks availability.
pub(super) fn block_home(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<(
    semantic_vocabulary::BlockId,
    &terminal_psi::StructuralParameterDeclaration,
)> {
    let mut matching = source.blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
            .map(move |parameter| (block.id, parameter))
    });
    let (block, parameter) = matching.next()?;
    let signature = source.structural.as_ref()?;
    if matching.next().is_some()
        || parameter.access != StructuralAccess::Owned
        || parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || !signature.structural_places.iter().any(|declaration| {
            declaration.id == place
                && declaration.kind
                    == semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                        block,
                        position: parameter.position,
                    }
        })
    {
        return None;
    }
    crate::structural_reference_input::plain_record_shape(
        parameter.structural_type,
        &signature.structural_types,
    )?;
    Some((block, parameter))
}
