//! A local home is storage until checked stores install a value.

use terminal_psi::{StructuralOperationResult, StructuralTypeDeclaration};

/// Reserving storage introduces no value, claim or proved membership, even
/// when the eventual value is linear. Its stores supply that custody later.
/// A byte-view home may retain declared byte requirements; this does not prove
/// them or grant access before an establishing store.
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
    if !result.projected_qualifications.is_empty()
        || (!result.qualifications.is_empty() && !declaration.shape.is_borrowed_byte_view())
    {
        return Err("vacant structural local has invalid declared qualifications");
    }
    Ok(())
}
