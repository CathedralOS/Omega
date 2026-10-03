//! Exact element identities and layout for complete fixed-array construction.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

pub(super) fn elements(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<(ValueShape, Vec<u32>)> {
    let LegalizedScalarInstructionKind::EstablishStructuralArray {
        result,
        elements,
        shape,
    } = &row.kind
    else {
        return None;
    };
    let signature = source.structural.as_ref()?;
    let types = &signature.structural_types;
    let declaration = types
        .iter()
        .find(|entry| entry.id == result.structural_type)?;
    let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
        return None;
    };
    let geometry = crate::structural_inputs::structural_reference_input::shape;
    if row.result.is_some()
        || result.multiplicity == StructuralMultiplicity::Linear
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || u64::try_from(elements.len()).ok()? != length
        || geometry(result.structural_type, types)? != *shape
    {
        return None;
    }
    let element_shape = geometry(element, types)?;
    let alignment = u32::from(element_shape.alignment);
    let stride = element_shape
        .byte_size
        .checked_add(alignment.checked_sub(1)?)?
        / alignment
        * alignment;
    let mut offsets = Vec::with_capacity(elements.len());
    for (ordinal, argument) in elements.iter().enumerate() {
        if argument.access != StructuralAccess::Owned || argument.place == result.place {
            return None;
        }
        if super::record_input::structural_operand_shape(
            source,
            result,
            argument,
            &terminal_psi::StructuralFieldType::Structural(element),
        )? != element_shape
        {
            return None;
        }
        offsets.push(u32::try_from(ordinal).ok()?.checked_mul(stride)?);
    }
    Some((element_shape, offsets))
}
