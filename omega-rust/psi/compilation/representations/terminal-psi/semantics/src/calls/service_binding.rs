//! Shape correspondence for an independently authenticated Fused binding leaf.

use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, TerminalIdentityIndex};

/// Match the exact carrier named by an establishment roster: the erased field
/// that holds the established service.
///
/// This is only shape correspondence. The caller must authenticate the expected
/// carrier, field path, requirement and selected provider independently. This
/// correspondence grants no establishment authority.
pub fn fused_service_field_matches(
    field: &StructuralFieldDeclaration,
    expected_identity: TerminalIdentityIndex,
) -> bool {
    !expected_identity.is_absent() && fused_service_field_carrier(field) == Some(expected_identity)
}

/// Recover a field's erased carrier for an independent roster join. Erased
/// fields include non-service evidence too: this result never classifies their
/// authority.
pub fn fused_service_field_carrier(
    field: &StructuralFieldDeclaration,
) -> Option<TerminalIdentityIndex> {
    match &field.field_type {
        StructuralFieldType::Erased { type_identity } if !type_identity.is_absent() => {
            Some(*type_identity)
        }
        _ => None,
    }
}
