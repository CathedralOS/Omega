//! The declared atomic resident at one exact storage endpoint. Whole cells
//! and actual record projections share this query; no synthetic field or
//! carrier-shaped declaration can supply atomic access authority.

use semantic_vocabulary::{AtomicCellType, StructuralTypeId};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

pub fn atomic_cell_type<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<AtomicCellType> {
    let cell =
        if let Some(field) = crate::runtime_structural_field(declarations.clone(), root, path) {
            field.field_type.atomic_cell_type()?
        } else {
            let declaration = crate::runtime_structural_path_tip(declarations, root, path)?;
            let StructuralTypeShape::AtomicCell(cell) = declaration.shape else {
                return None;
            };
            cell
        };
    // Decoded integer types still need the admitted fixed-width check. This
    // query describes the endpoint only; the operation separately proves
    // live storage, projection bounds, case membership and access.
    cell.is_supported().then_some(cell)
}
