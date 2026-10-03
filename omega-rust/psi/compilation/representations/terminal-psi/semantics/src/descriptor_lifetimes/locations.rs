//! Canonical carrier addresses authenticated against the actual type catalog.
use super::{DescriptorLocation, DescriptorPathSegment};
use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub fn descriptor_location(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    final_field: Option<StructuralFieldId>,
) -> Option<DescriptorLocation> {
    let mut result = DescriptorLocation::whole(root);
    let mut tip = Some(root_type);
    let mut case_fields = None;
    for segment in path {
        let declaration = declaration(types, tip?)?;
        let canonical = match segment {
            StructuralPathSegment::Referent if case_fields.is_none() => {
                let StructuralTypeShape::Reference { referent, .. } = declaration.shape else {
                    return None;
                };
                tip = Some(referent);
                DescriptorPathSegment::Referent
            }
            StructuralPathSegment::FixedIndex(index) if case_fields.is_none() => {
                tip = fixed_element(&declaration.shape, *index)?;
                DescriptorPathSegment::FixedIndex(*index)
            }
            StructuralPathSegment::Case(id) if case_fields.is_none() => {
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &declaration.shape
                else {
                    return None;
                };
                let mut matching = cases.iter().filter(|case| case.id == *id);
                case_fields = Some(matching.next()?.fields.as_slice());
                if matching.next().is_some() {
                    return None;
                }
                DescriptorPathSegment::Case(*id)
            }
            StructuralPathSegment::Field(name) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(declaration)?,
                };
                let mut matching = fields
                    .iter()
                    .filter(|field| field.identity == *name && !field.relevance.is_erased());
                let field = matching.next()?;
                if matching.next().is_some()
                    || fields.iter().filter(|other| other.id == field.id).count() != 1
                {
                    return None;
                }
                tip = match field.field_type {
                    StructuralFieldType::Structural(id) => Some(id),
                    _ => None,
                };
                DescriptorPathSegment::Field(field.id)
            }
            _ => return None,
        };
        result.path.push(canonical);
    }
    if let Some(id) = final_field {
        let declaration = declaration(types, tip?)?;
        let fields = match case_fields.take() {
            Some(fields) => fields,
            None => fields(declaration)?,
        };
        let mut matching = fields
            .iter()
            .filter(|field| field.id == id && !field.relevance.is_erased());
        let field = matching.next()?;
        if matching.next().is_some()
            || fields
                .iter()
                .filter(|other| other.identity == field.identity)
                .count()
                != 1
        {
            return None;
        }
        result.path.push(DescriptorPathSegment::Field(id));
    }
    case_fields.is_none().then_some(result)
}

/// Recover a spelling only after resolving each declaration-local identity.
pub fn descriptor_location_type(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Option<StructuralTypeId> {
    resolved_path(types, root_type, path)?.1
}
pub fn descriptor_path(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Option<Vec<StructuralPathSegment>> {
    Some(resolved_path(types, root_type, path)?.0)
}
fn resolved_path(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
    let mut result = Vec::new();
    let mut tip = Some(root_type);
    let mut case_fields = None;
    for segment in path {
        let declaration = declaration(types, tip?)?;
        result.push(match segment {
            DescriptorPathSegment::Referent if case_fields.is_none() => {
                let StructuralTypeShape::Reference { referent, .. } = declaration.shape else { return None; };
                tip = Some(referent); StructuralPathSegment::Referent
            }
            DescriptorPathSegment::FixedIndex(index) if case_fields.is_none() => {
                tip = fixed_element(&declaration.shape, *index)?;
                StructuralPathSegment::FixedIndex(*index)
            }
            DescriptorPathSegment::Case(id) if case_fields.is_none() => {
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape else { return None; };
                let mut matching = cases.iter().filter(|case| case.id == *id);
                case_fields = Some(matching.next()?.fields.as_slice());
                if matching.next().is_some() { return None; }
                StructuralPathSegment::Case(*id)
            }
            DescriptorPathSegment::Field(id) => {
                let fields = match case_fields.take() { Some(fields) => fields, None => fields(declaration)? };
                let mut matching = fields.iter().filter(|field| field.id == *id && !field.relevance.is_erased());
                let field = matching.next()?;
                if matching.next().is_some() { return None; }
                tip = match field.field_type { StructuralFieldType::Structural(id) => Some(id), _ => None };
                StructuralPathSegment::Field(field.identity.clone())
            }
            _ => return None,
        });
    }
    case_fields.is_none().then_some((result, tip))
}

fn declaration(
    types: &[StructuralTypeDeclaration],
    id: StructuralTypeId,
) -> Option<&StructuralTypeDeclaration> {
    let mut matching = types.iter().filter(|declaration| declaration.id == id);
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}
fn fields(
    declaration: &StructuralTypeDeclaration,
) -> Option<&[terminal_psi::StructuralFieldDeclaration]> {
    match &declaration.shape {
        StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
            Some(fields)
        }
        _ => None,
    }
}

/// A statically addressed byte is a primitive endpoint, not a new structural
/// declaration or a descriptor slot. Capacity bounds its storage coordinate;
/// the operation independently proves readable/writable extent and custody.
fn fixed_element(shape: &StructuralTypeShape, index: u64) -> Option<Option<StructuralTypeId>> {
    match shape {
        StructuralTypeShape::FixedArray { element, length } if index < *length => {
            Some(Some(*element))
        }
        StructuralTypeShape::ByteSequence(
            ByteSequenceCarrier::BoundedView { capacity, .. }
            | ByteSequenceCarrier::BoundedOwned { capacity },
        ) if index < *capacity => Some(None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_byte_index_is_an_exact_primitive_endpoint() {
        let root = PlaceId::new(1).unwrap();
        let ty = StructuralTypeId::new(1).unwrap();
        for carrier in [
            ByteSequenceCarrier::BoundedView {
                access: None,
                capacity: 4,
            },
            ByteSequenceCarrier::BoundedOwned { capacity: 4 },
        ] {
            let types = [StructuralTypeDeclaration {
                id: ty,
                identity: "bounded-bytes".into(),
                shape: StructuralTypeShape::ByteSequence(carrier),
            }];
            let path = [StructuralPathSegment::FixedIndex(3)];
            let location = descriptor_location(&types, root, ty, &path, None).unwrap();
            assert_eq!(location.path, vec![DescriptorPathSegment::FixedIndex(3)]);
            assert_eq!(
                descriptor_path(&types, ty, &location.path),
                Some(path.to_vec())
            );
            assert_eq!(
                descriptor_location_type(&types, ty, &location.path),
                None,
                "the byte is a primitive endpoint, not an invented descriptor declaration"
            );
            assert!(
                descriptor_location(
                    &types,
                    root,
                    ty,
                    &[StructuralPathSegment::FixedIndex(4)],
                    None
                )
                .is_none()
            );
            assert!(descriptor_path(&types, ty, &[DescriptorPathSegment::FixedIndex(4)]).is_none());
            assert!(
                descriptor_location(
                    &types,
                    root,
                    ty,
                    &[path[0].clone(), StructuralPathSegment::FixedIndex(0)],
                    None
                )
                .is_none()
            );
            assert!(
                descriptor_location(
                    &types,
                    root,
                    ty,
                    &path,
                    Some(StructuralFieldId::new(1).unwrap())
                )
                .is_none()
            );
        }
        let types = [StructuralTypeDeclaration {
            id: ty,
            identity: "dynamic-bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: None,
            }),
        }];
        assert!(
            descriptor_location(
                &types,
                root,
                ty,
                &[StructuralPathSegment::FixedIndex(0)],
                None
            )
            .is_none(),
            "a dynamic descriptor's shape does not supply a static storage bound"
        );
    }
}
