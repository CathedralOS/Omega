//! Exact fixed-array extent for ordinary mutable byte-view borrowing.

use semantic_vocabulary::{IntegerSign, ScalarType};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeShape, TerminalModule,
};

/// Rejoin a mutable byte-view argument to initialized fixed-u8-array storage.
/// This preserves the actual type and field path; the returned length grants
/// neither new storage nor resize permission. Callers separately check claims,
/// exclusive aliasing, availability, and the ordinary call site.
pub fn mutable_fixed_byte_array_extent(
    module: &TerminalModule,
    actual: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
    expected: &StructuralParameterDeclaration,
) -> Option<u64> {
    if actual.place != argument.place
        || argument.access != StructuralAccess::MutableBorrow
        || [actual, expected].iter().any(|parameter| {
            parameter.access != StructuralAccess::MutableBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
        })
        || !module.structural_types.iter().any(|declaration| {
            declaration.id == expected.structural_type
                && declaration.shape
                    == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
        })
    {
        return None;
    }
    let mut structural_type = actual.structural_type;
    for segment in &argument.path {
        let StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let declaration = module
            .structural_types
            .iter()
            .find(|row| row.id == structural_type)?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return None;
        };
        let field = fields
            .iter()
            .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
        let StructuralFieldType::Structural(next) = field.field_type else {
            return None;
        };
        structural_type = next;
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|row| row.id == structural_type)?;
    let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
        return None;
    };
    // Preserve the current Terminal fixed-array declaration admission floor.
    if length == 0 {
        return None;
    }
    let element = module
        .structural_types
        .iter()
        .find(|row| row.id == element)?;
    match element.shape {
        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
            if integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 8
                && !integer.is_address() =>
        {
            Some(length)
        }
        _ => None,
    }
}
