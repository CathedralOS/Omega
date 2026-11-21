use semantic_vocabulary::StructuralFieldId;
use terminal_psi::{
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineCarrierError {
    pub field: StructuralFieldId,
    pub reason: &'static str,
}

pub fn inline_carrier_declaration<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration>,
    field: &StructuralFieldDeclaration,
) -> Option<&'a StructuralTypeDeclaration> {
    if field.relevance.is_erased() {
        return None;
    }
    let identity = field.inline_carrier?;
    let carrier = crate::exactly_one::exactly_one(types, |candidate| candidate.id == identity)?;
    match (&field.field_type, &carrier.shape) {
        (
            StructuralFieldType::Scalar(_)
            | StructuralFieldType::IeeeFloat(_)
            | StructuralFieldType::BoundedInteger(_)
            | StructuralFieldType::RefinedScalar(_),
            StructuralTypeShape::PrimitiveScalar(scalar),
        ) if field.field_type.scalar_type() == Some(*scalar) => Some(carrier),
        (_, StructuralTypeShape::AtomicCell(cell))
            if field.field_type.atomic_cell_type() == Some(*cell)
                && carrier.scalar_restriction.is_none() =>
        {
            Some(carrier)
        }
        (_, StructuralTypeShape::ByteSequence(declared))
            if field
                .field_type
                .byte_sequence_carrier()
                .is_some_and(|field| {
                    crate::byte_carrier_geometry_matches(field, *declared)
                }) =>
        {
            Some(carrier)
        }
        _ => None,
    }
}

pub fn validate_inline_carriers(
    types: &[StructuralTypeDeclaration],
    declaration: &StructuralTypeDeclaration,
) -> Result<(), InlineCarrierError> {
    let (fields, cases): (&[StructuralFieldDeclaration], &[StructuralCaseDeclaration]) =
        match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => (fields, &[]),
            StructuralTypeShape::Sum { cases } => (&[], cases),
            StructuralTypeShape::Mixed { fields, cases, .. } => (fields, cases),
            _ => (&[], &[]),
        };
    for field in fields
        .iter()
        .chain(cases.iter().flat_map(|case| &case.fields))
    {
        let Some(identity) = field.inline_carrier else {
            continue;
        };
        let fail = |reason| InlineCarrierError {
            field: field.id,
            reason,
        };
        let carrier =
            crate::exactly_one::exactly_one(types.iter(), |candidate| candidate.id == identity)
                .ok_or_else(|| fail("inline field carrier has no exact declaration"))?;
        if field.relevance.is_erased() || field.field_type.as_structural().is_some() {
            return Err(fail(
                "inline carrier cannot belong to an erased or structural field",
            ));
        }
        let compatible = match (&field.field_type, &carrier.shape) {
            (
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::BoundedInteger(_)
                | StructuralFieldType::RefinedScalar(_),
                StructuralTypeShape::PrimitiveScalar(scalar),
            ) => {
                let plain = StructuralFieldType::Scalar(*scalar);
                crate::scalar_field_entails(
                    &field.field_type,
                    carrier.scalar_restriction.as_ref().unwrap_or(&plain),
                )
            }
            (_, StructuralTypeShape::AtomicCell(cell)) => {
                field.field_type.atomic_cell_type() == Some(*cell)
                    && carrier.scalar_restriction.is_none()
            }
            (_, StructuralTypeShape::ByteSequence(declared)) => field
                .field_type
                .byte_sequence_carrier()
                .is_some_and(|field| crate::byte_carrier_geometry_matches(field, *declared)),
            _ => false,
        };
        if !compatible {
            return Err(fail(
                "inline field does not establish its declared carrier contract",
            ));
        }
    }
    Ok(())
}

pub fn inline_field_presentation<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    field: &StructuralFieldDeclaration,
    expected: Option<semantic_vocabulary::StructuralTypeId>,
    contract: Option<StructuralFieldType>,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let declared = inline_carrier_declaration(types.clone(), field)?;
    let Some(expected) = expected else {
        return Some(declared.id);
    };
    let expected = crate::exactly_one::exactly_one(types.clone(), |row| row.id == expected)?;
    let compatible = match contract {
        Some(contract) => contract.declares_leaf(expected),
        None if field.field_type.byte_sequence_carrier().is_some() => {
            crate::borrowed_referent_presentation(types, declared.id, expected.id).is_some()
        }
        None => false,
    };
    compatible.then_some(expected.id)
}

#[cfg(test)]
mod tests;
