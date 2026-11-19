use terminal_psi::{
    StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// A vacant home holds nothing, so the memberships a record home
/// declares grant nothing: they are the type of the value a later store
/// must deliver, which the structural qualification judgment holds that
/// store to. A reference home declares such memberships beneath its
/// referent. A byte view keeps its whole-value byte domains.
pub fn validate_vacant_structural_local<'a>(
    declarations: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
    result: &StructuralOperationResult,
) -> Result<(), &'static str> {
    let mut matching = declarations
        .into_iter()
        .filter(|item| item.id == result.structural_type);
    let declaration = matching
        .next()
        .ok_or("structural local has no declared storage type")?;
    if matching.next().is_some() {
        return Err("structural local repeats its storage type declaration");
    }
    if !result.claims.is_empty() || !result.qualification_establishments.is_empty() {
        return Err("vacant structural local introduces value custody");
    }
    let byte_view = declaration.shape.is_borrowed_byte_view();
    let record = matches!(
        declaration.shape,
        StructuralTypeShape::Record { .. }
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. }
    );
    let projected = &result.projected_qualifications;
    let declares_contents = record
        || match declaration.shape {
            StructuralTypeShape::Reference { .. } => projected
                .iter()
                .all(|row| row.path.first() == Some(&StructuralPathSegment::Referent)),
            StructuralTypeShape::FixedArray { .. }
            | StructuralTypeShape::ElementView { .. } => {
                projected.iter().all(|row| row.is_element_membership())
            }
            _ => projected.is_empty(),
        };
    if !declares_contents || (!result.qualifications.is_empty() && !byte_view && !record) {
        return Err("vacant structural local has invalid declared qualifications");
    }
    Ok(())
}
