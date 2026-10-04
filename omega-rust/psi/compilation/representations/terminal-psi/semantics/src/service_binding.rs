//! Shape correspondence for an independently authenticated Fused binding leaf.

use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};

/// Match the exact carrier named by a service receipt or establishment roster.
/// Legacy products erase the field; retained products keep a nonconstructible
/// service child so ordinary loans can name the original field occurrence.
///
/// This is only shape correspondence. The caller must authenticate the expected
/// carrier, field path, requirement and selected provider independently. Neither
/// this correspondence nor the service marker grants establishment authority.
pub fn fused_service_field_matches(
    declarations: &[StructuralTypeDeclaration],
    field: &StructuralFieldDeclaration,
    expected_identity: &str,
) -> bool {
    !expected_identity.is_empty()
        && fused_service_field_carrier(declarations, field) == Some(expected_identity)
}

/// Recover a field's carrier for an independent receipt join. Erased fields
/// include non-service evidence too: this result never classifies their authority.
/// A retained child must carry the explicit nonconstructible service marker.
pub fn fused_service_field_carrier<'a>(
    declarations: &'a [StructuralTypeDeclaration],
    field: &'a StructuralFieldDeclaration,
) -> Option<&'a str> {
    match &field.field_type {
        StructuralFieldType::Erased { type_identity } if !type_identity.is_empty() => {
            Some(type_identity)
        }
        StructuralFieldType::Structural(child) if !field.relevance.is_erased() => {
            let mut matching = declarations
                .iter()
                .filter(|declaration| declaration.id == *child);
            let declaration = matching.next()?;
            (matching.next().is_none()
                && !declaration.identity.is_empty()
                && matches!(declaration.shape, StructuralTypeShape::FusedServiceBinding))
            .then_some(declaration.identity.as_str())
        }
        _ => None,
    }
}
