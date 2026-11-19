use semantic_vocabulary::{AtomicCellType, StructuralTypeId};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

pub fn atomic_cell_type<'types>(
    declarations: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<AtomicCellType> {
    let cell = if let Some(field) =
        crate::runtime_structural_field(declarations.clone(), root, path)
    {
        field.field_type.atomic_cell_type()?
    } else {
        let declaration = crate::runtime_structural_path_tip(declarations, root, path)?;
        let StructuralTypeShape::AtomicCell(cell) = declaration.shape else {
            return None;
        };
        cell
    };
    cell.is_supported().then_some(cell)
}
