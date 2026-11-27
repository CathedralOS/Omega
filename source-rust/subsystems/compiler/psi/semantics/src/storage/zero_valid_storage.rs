use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

pub(crate) fn scalar_declaration_accepts_zero(declaration: &StructuralTypeDeclaration) -> bool {
    matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
        && declaration
            .scalar_restriction
            .as_ref()
            .is_none_or(crate::scalar_field_accepts_zero)
}
