use semantic_vocabulary::{
    CanonicalStructuralPathSegment, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, live_field_by_id,
};

pub(crate) struct StorageRegion {
    pub path: Vec<StructuralPathSegment>,
    pub indirect: bool,
}

pub(crate) fn crosses_unaliased_reference(path: &[StructuralPathSegment]) -> bool {
    path.iter()
        .skip(1)
        .any(|segment| *segment == StructuralPathSegment::Referent)
}

pub(crate) fn projected_storage_region<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> StorageRegion {
    let static_length = path
        .iter()
        .position(|segment| {
            matches!(
                segment,
                StructuralPathSegment::RuntimeIndex { .. }
                    | StructuralPathSegment::RepresentationView { .. }
                    | StructuralPathSegment::Referent
            )
        })
        .unwrap_or(path.len());
    let mut region = path[..static_length].to_vec();
    if static_length == path.len()
        && let Some(field) = field
        && let Some(declaration) =
            crate::runtime_structural_path_tip(declarations, root, &region)
        && let StructuralTypeShape::Record { fields, .. } = &declaration.shape
        && let Some(field) = live_field_by_id(fields, field)
    {
        region.push(StructuralPathSegment::Field(field.identity));
    }
    StorageRegion {
        path: region,
        indirect: crosses_unaliased_reference(path),
    }
}

pub(crate) fn canonical_field_storage_region<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Vec<StructuralPathSegment> {
    let mut region = Vec::with_capacity(path.len() + 1);
    for (position, segment) in path
        .iter()
        .chain(std::iter::once(&CanonicalStructuralPathSegment::Field(
            field,
        )))
        .enumerate()
    {
        match segment {
            CanonicalStructuralPathSegment::Case(_)
            | CanonicalStructuralPathSegment::Referent => {
                return Vec::new();
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                region.push(StructuralPathSegment::FixedIndex(*index))
            }
            CanonicalStructuralPathSegment::Field(field) => {
                let Some(declaration) = crate::canonical_structural_path_tip(
                    declarations.clone(),
                    root,
                    &path[..position],
                ) else {
                    return Vec::new();
                };
                let Some(fields) = declaration.shape.record_fields() else {
                    return Vec::new();
                };
                let Some(field) = live_field_by_id(fields, *field) else {
                    return Vec::new();
                };
                region.push(StructuralPathSegment::Field(field.identity));
            }
        }
    }
    region
}
