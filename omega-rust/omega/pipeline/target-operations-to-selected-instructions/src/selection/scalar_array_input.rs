//! Reconstruct exact row-major primitive leaves independently of legalized layout.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{ScalarType, StructuralTypeId};

/// Reconstruct constructor stores from the semantic type and exact operands.
/// Construction and independent instruction replay each consume this input.
pub(super) fn storage<'a>(
    source: &LegalizedScalarFunction,
    row: &'a LegalizedScalarInstruction,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    ValueShape,
    Vec<(semantic_vocabulary::ValueId, ScalarType, u32, u8)>,
)> {
    match &row.kind {
        LegalizedScalarInstructionKind::EstablishScalarArray {
            result,
            elements: values,
            shape,
        } => {
            let (scalar, width) = elements(source, row)?;
            let stores = values
                .iter()
                .enumerate()
                .map(|(ordinal, value)| {
                    Some((
                        *value,
                        scalar,
                        u32::try_from(ordinal).ok()?.checked_mul(u32::from(width))?,
                        width,
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some((result, *shape, stores))
        }
        LegalizedScalarInstructionKind::EstablishScalarRecord {
            result,
            fields,
            shape,
        } => {
            if row.result.is_some()
                || result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
                || !result.claims.is_empty()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
            {
                return None;
            }
            let declarations = &source.structural.as_ref()?.structural_types;
            let mut matches = declarations
                .iter()
                .filter(|declaration| declaration.id == result.structural_type);
            let declaration = matches.next()?;
            let terminal_psi::StructuralTypeShape::Record {
                fields: declarations,
            } = &declaration.shape
            else {
                return None;
            };
            if matches.next().is_some() || declarations.len() != fields.len() {
                return None;
            }
            let mut size = 0_u16;
            let mut alignment = 1_u16;
            let mut stores = Vec::with_capacity(fields.len());
            for (field, declaration) in fields.iter().zip(declarations) {
                let scalar = declaration.field_type.scalar_type()?;
                if declaration.relevance.is_erased()
                    || declaration.id != field.field
                    || matches!(
                        declaration.field_type,
                        terminal_psi::StructuralFieldType::BoundedInteger(_)
                    )
                {
                    return None;
                }
                let field_shape = super::scalar_call_abi::scalar_shape(scalar)?;
                alignment = alignment.max(field_shape.alignment);
                size = size.checked_next_multiple_of(field_shape.alignment)?;
                stores.push((
                    field.value,
                    scalar,
                    u32::from(size),
                    u8::try_from(field_shape.byte_size).ok()?,
                ));
                size = size.checked_add(field_shape.byte_size)?;
            }
            if *shape != ValueShape::integer(size.checked_next_multiple_of(alignment)?, alignment) {
                return None;
            }
            Some((result, *shape, stores))
        }
        _ => None,
    }
}

pub(super) fn shape(
    source: &LegalizedScalarFunction,
    structural_type: StructuralTypeId,
) -> Option<(ScalarType, u64, ValueShape)> {
    let declarations = &source.structural.as_ref()?.structural_types;
    let mut current = structural_type;
    let mut count = Some(1u64);
    let mut empty = false;
    // A single element chain suffices for fixed arrays. The declaration bound
    // detects cycles even below an empty dimension, without recursive stack use.
    for depth in 0..=declarations.len() {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.id == current)?;
        match declaration.shape {
            terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                empty |= length == 0;
                count = count.and_then(|count| count.checked_mul(length));
                current = element;
            }
            terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) if depth != 0 => {
                let leaf = super::scalar_call_abi::scalar_shape(scalar)?;
                let count = if empty { 0 } else { count? };
                let bytes = u16::try_from(count.checked_mul(u64::from(leaf.byte_size))?).ok()?;
                return Some((
                    scalar,
                    count,
                    ValueShape::integer(bytes, if bytes == 0 { 1 } else { leaf.alignment }),
                ));
            }
            _ => return None,
        }
    }
    None
}

pub(super) fn elements(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<(ScalarType, u8)> {
    let LegalizedScalarInstructionKind::EstablishScalarArray {
        result,
        elements,
        shape: established,
    } = &row.kind
    else {
        return None;
    };
    let (scalar, count, shape) = shape(source, result.structural_type)?;
    if row.result.is_some()
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || *established != shape
        || u64::try_from(elements.len()).ok()? != count
    {
        return None;
    }
    Some((
        scalar,
        u8::try_from(super::scalar_call_abi::scalar_shape(scalar)?.byte_size).ok()?,
    ))
}
