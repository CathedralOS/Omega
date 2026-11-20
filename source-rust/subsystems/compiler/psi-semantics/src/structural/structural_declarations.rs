use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

pub(crate) fn structural_declaration<'types>(
    mut types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    id: StructuralTypeId,
) -> Option<&'types StructuralTypeDeclaration> {
    types.find(|declaration| declaration.id == id)
}

pub(crate) fn structural_shape<'types>(
    mut types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    id: StructuralTypeId,
) -> Option<&'types StructuralTypeShape> {
    Some(&types.find(|declaration| declaration.id == id)?.shape)
}
