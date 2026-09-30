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

#[cfg(test)]
mod tests {
    use super::fused_service_field_matches;
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    #[test]
    fn fused_service_field_matches_only_its_exact_retained_or_erased_carrier() {
        let declaration = StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: "binding-console".into(),
            shape: StructuralTypeShape::FusedServiceBinding,
        };
        let mut field = StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: "console".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(declaration.id),
        };
        let mut declarations = vec![declaration.clone()];
        assert!(fused_service_field_matches(
            &declarations,
            &field,
            "binding-console"
        ));
        assert!(!fused_service_field_matches(
            &declarations,
            &field,
            "binding-clock"
        ));
        assert!(!fused_service_field_matches(&[], &field, "binding-console"));
        declarations.push(declaration.clone());
        assert!(!fused_service_field_matches(
            &declarations,
            &field,
            "binding-console"
        ));
        declarations.pop();
        declarations[0].shape = StructuralTypeShape::Record { fields: Vec::new() };
        assert!(!fused_service_field_matches(
            &declarations,
            &field,
            "binding-console"
        ));
        declarations[0] = declaration;
        field.relevance = BindingRelevance::Erased;
        assert!(!fused_service_field_matches(
            &declarations,
            &field,
            "binding-console"
        ));
        field.field_type = StructuralFieldType::Erased {
            type_identity: "binding-console".into(),
        };
        assert!(fused_service_field_matches(&[], &field, "binding-console"));
        assert!(!fused_service_field_matches(&[], &field, "binding-clock"));
        assert!(!fused_service_field_matches(&[], &field, ""));
    }
}
