//! Reconstruct record field geometry from exact declarations.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use terminal_psi::{
    RecordFieldValue, StructuralFieldType, StructuralMultiplicity, StructuralTypeShape,
};

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
        || crate::structural_inputs::structural_reference_input::shape(
            result.structural_type,
            declarations,
        )? != *shape
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
        let field_shape = field_shape(source, result, field, expected)?;
        let alignment = u32::from(field_shape.alignment);
        offset = offset.checked_add(alignment.checked_sub(1)?)? / alignment * alignment;
        placed.push((offset, field_shape, expected.field_type.scalar_type()));
        offset = offset.checked_add(field_shape.byte_size)?;
    }
    (offset <= shape.byte_size).then_some(placed)
}

/// Rejoin one field's scalar contract or complete owned child independently of
/// whether its enclosing carrier is a record or a selected case.
pub(super) fn field_shape(
    source: &LegalizedScalarFunction,
    parent: &terminal_psi::StructuralOperationResult,
    field: &terminal_psi::RecordFieldInitializer,
    expected: &terminal_psi::StructuralFieldDeclaration,
) -> Option<ValueShape> {
    if field.field != expected.id || expected.relevance.is_erased() {
        return None;
    }
    let declarations = &source.structural.as_ref()?.structural_types;
    let shape = match (&field.value, &expected.field_type) {
        (RecordFieldValue::Structural(argument), field_type)
            if matches!(
                field_type.byte_sequence_carrier(),
                Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
            ) =>
        {
            if argument.access != terminal_psi::StructuralAccess::Owned
                || !argument.path.is_empty()
                || argument.place == parent.place
            {
                return None;
            }
            let signature = source.structural.as_ref()?;
            let mut parameters = signature
                .parameters
                .iter()
                .map(|parameter| &parameter.semantic)
                .chain(
                    source
                        .blocks
                        .iter()
                        .filter(|block| block.id != source.entry_block)
                        .flat_map(|block| &block.structural_parameters),
                )
                .filter(|parameter| parameter.place == argument.place);
            let child_type = if let Some(parameter) = parameters.next() {
                if parameters.next().is_some()
                    || parameter.access != terminal_psi::StructuralAccess::Owned
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.projected_qualifications.is_empty()
                {
                    return None;
                }
                parameter.structural_type
            } else {
                let (_, result) = super::aggregate_result_input::home(source, argument.place)?;
                if result.multiplicity != StructuralMultiplicity::Unrestricted
                    || !result.projected_qualifications.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                {
                    return None;
                }
                result.structural_type
            };
            // Primitive domain implication was checked before target lowering;
            // selection preserves the exact qualified source home and copies
            // its length-prefixed value, not its capacity as a live extent.
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.id == child_type)?;
            if declaration.shape
                != StructuralTypeShape::ByteSequence(field_type.byte_sequence_carrier()?)
            {
                return None;
            }
            crate::structural_inputs::structural_reference_input::shape(child_type, declarations)?
        }
        (
            RecordFieldValue::Scalar {
                range_obligation, ..
            },
            kind,
        ) if kind.scalar_type().is_some() => {
            if matches!(kind, StructuralFieldType::BoundedInteger(_)) != range_obligation.is_some()
            {
                return None;
            }
            super::scalar_call_abi::scalar_shape(kind.scalar_type()?)?
        }
        (RecordFieldValue::Structural(argument), StructuralFieldType::Structural(nested)) => {
            // Owned aggregates copy their bytes; a reference child is a
            // zero-byte custody leaf with no payload of its own.
            if crate::structural_inputs::structural_reference_input::owned_aggregate_shape(
                *nested,
                declarations,
            )
            .is_none()
                && !declarations.iter().any(|declaration| {
                    declaration.id == *nested
                        && matches!(declaration.shape, StructuralTypeShape::Reference { .. })
                })
            {
                return None;
            }
            if argument.access != terminal_psi::StructuralAccess::Owned
                || !argument.path.is_empty()
                || argument.place == parent.place
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
                            && (parent.multiplicity != StructuralMultiplicity::Unrestricted
                                || parameter.multiplicity == StructuralMultiplicity::Unrestricted)
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
                                    result, ..
                                }
                                | LegalizedScalarInstructionKind::EstablishReference {
                                    result,
                                    ..
                                }
                                | LegalizedScalarInstructionKind::EstablishScalarArray {
                                    result,
                                    ..
                                }
                                | LegalizedScalarInstructionKind::EstablishCase {
                                    result, ..
                                }
                                | LegalizedScalarInstructionKind::EstablishZeroedStructural {
                                    result,
                                    ..
                                }
                                | LegalizedScalarInstructionKind::StructuralLeafCopy {
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
                                    && (parent.multiplicity
                                        != StructuralMultiplicity::Unrestricted
                                        || result.multiplicity
                                            == StructuralMultiplicity::Unrestricted)
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
            crate::structural_inputs::structural_reference_input::shape(*nested, declarations)?
        }
        _ => return None,
    };
    Some(shape)
}
