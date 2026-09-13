use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// The original record carrier reached by exact declaration-local field IDs.
/// The runtime path names come from those declarations, never from source spelling.
pub struct RecordFieldCarrier {
    pub structural_type: StructuralTypeId,
    pub path: Vec<StructuralPathSegment>,
}

/// Resolve record-only scalar observation carriers; the final scalar field is
/// deliberately separate. Access, ownership and scalar leaf validation remain
/// the consuming operation's independent obligations.
pub fn record_field_carrier<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<RecordFieldCarrier> {
    let mut structural_type = root;
    let mut runtime_path = Vec::with_capacity(path.len());
    for segment in path {
        let CanonicalStructuralPathSegment::Field(field) = segment else {
            return None;
        };
        let declaration = types
            .clone()
            .find(|declaration| declaration.id == structural_type)?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return None;
        };
        let field = fields
            .iter()
            .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())?;
        let StructuralFieldType::Structural(child) = field.field_type else {
            return None;
        };
        runtime_path.push(StructuralPathSegment::Field(field.identity.clone()));
        structural_type = child;
    }
    matches!(
        types
            .clone()
            .find(|declaration| declaration.id == structural_type)?
            .shape,
        StructuralTypeShape::Record { .. }
    )
    .then_some(RecordFieldCarrier {
        structural_type,
        path: runtime_path,
    })
}
