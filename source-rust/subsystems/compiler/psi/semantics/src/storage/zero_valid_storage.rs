use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub fn zero_valid_record_storage(
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    visiting: &mut Vec<StructuralTypeId>,
    zero_valid_field: impl Fn(
        &[StructuralTypeDeclaration],
        &StructuralFieldDeclaration,
        &mut Vec<StructuralTypeId>,
    ) -> bool
    + Copy,
) -> bool {
    let Some(declaration) =
        terminal_psi::unique_structural_declaration(declarations, structural_type, visiting)
    else {
        return false;
    };
    visiting.push(structural_type);
    let valid = match &declaration.shape {
        StructuralTypeShape::Record { fields, .. } => fields
            .iter()
            .all(|field| zero_valid_field(declarations, field, visiting)),
        StructuralTypeShape::FixedArray { element, length } => {
            if length.literal() == Some(0)
                && crate::scalar_array_leaf_declaration(declarations.iter(), structural_type)
                    .is_some()
            {
                true
            } else {
                zero_valid_record_storage(declarations, *element, visiting, zero_valid_field)
            }
        }
        StructuralTypeShape::Sum { cases } => cases.first().is_some_and(|case| {
            case.fields
                .iter()
                .all(|field| zero_valid_field(declarations, field, visiting))
        }),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields
                .iter()
                .all(|field| zero_valid_field(declarations, field, visiting))
                && cases.first().is_some_and(|case| {
                    case.fields
                        .iter()
                        .all(|field| zero_valid_field(declarations, field, visiting))
                })
        }
        StructuralTypeShape::PrimitiveScalar(
            ScalarType::Boolean | ScalarType::Integer(_) | ScalarType::IeeeFloat(_),
        ) => scalar_declaration_accepts_zero(declaration),
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => true,
        _ => false,
    };
    visiting.pop();
    valid
}

pub fn scalar_declaration_accepts_zero(declaration: &StructuralTypeDeclaration) -> bool {
    matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
        && declaration
            .scalar_restriction
            .as_ref()
            .is_none_or(crate::scalar_field_accepts_zero)
}
