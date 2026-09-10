//! Reconstruct exact row-major primitive leaves independently of legalized layout.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{ScalarType, StructuralTypeId};

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
    // Empty arrays retain semantic identity, but have no addressable payload.
    // The current selected local-home path cannot charge that construction
    // without inventing storage; reject until no-storage settlement is modeled.
    if shape.byte_size == 0 {
        return None;
    }
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
