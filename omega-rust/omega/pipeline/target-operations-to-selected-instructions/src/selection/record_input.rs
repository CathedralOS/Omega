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
