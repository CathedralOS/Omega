//! Independent recognition of the exact structural extent admitted by selection.

use crate::selection::shared::*;

pub(super) fn is_extent_structural_type(source: &SourceStructuralUnitFunction) -> bool {
    let structural_type = source.parameters[0].semantic.structural_type;
    let Some(declaration) = source
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
    else {
        return false;
    };
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return false;
    };
    if fields.len() != 2
        || fields
            .iter()
            .any(|field| field.relevance != BindingRelevance::Relevant)
    {
        return false;
    }
    matches!(
        fields[0].field_type,
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
            if integer.carrier() == IntegerCarrier::Address
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    ) && matches!(
        fields[1].field_type,
        StructuralFieldType::Scalar(ScalarType::Integer(integer))
            if integer.carrier() == IntegerCarrier::Fixed
                && integer.sign() == IntegerSign::Unsigned
                && integer.bits() == 64
    )
}
