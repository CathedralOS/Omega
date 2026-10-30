//! Declaration lookup by structural type identity.
//!
//! A roster is scanned for the declaration keyed by one `StructuralTypeId`.
//! The lookup carries no shape meaning, admission or authority: every caller
//! still interprets the declaration it selects, and an absent id resolves to
//! `None` rather than a fabricated row.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

/// The declaration keyed `id` in a declaration roster.
pub(crate) fn structural_declaration<'types>(
    mut types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    id: StructuralTypeId,
) -> Option<&'types StructuralTypeDeclaration> {
    types.find(|declaration| declaration.id == id)
}

/// The shape of the declaration keyed `id` in a declaration roster.
pub(crate) fn structural_shape<'types>(
    mut types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    id: StructuralTypeId,
) -> Option<&'types StructuralTypeShape> {
    Some(&types.find(|declaration| declaration.id == id)?.shape)
}
