use super::{DescriptorLocation, DescriptorPathSegment};
use crate::exactly_one::exactly_one;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

pub fn canonical_descriptor_location(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<DescriptorLocation> {
    let path = path
        .iter()
        .map(|segment| match segment {
            CanonicalStructuralPathSegment::Field(field) => {
                DescriptorPathSegment::Field(*field)
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                DescriptorPathSegment::FixedIndex(*index)
            }
            CanonicalStructuralPathSegment::Case(case) => {
                DescriptorPathSegment::Case(*case)
            }
            CanonicalStructuralPathSegment::Referent => DescriptorPathSegment::Referent,
        })
        .collect::<Vec<_>>();
    resolved_path(types, root_type, &path, |_, _, _| None)?;
    Some(DescriptorLocation { root, path })
}

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
            StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } if case_fields.is_none() => {
                tip = Some(
                    crate::representation_view_declaration(
                        types.iter(),
                        *structural_type,
                        *stored_scalar,
                    )?
                    .id,
                );
                DescriptorPathSegment::RepresentationView {
                    structural_type: *structural_type,
                    stored_scalar: *stored_scalar,
                }
            }
            StructuralPathSegment::Referent if case_fields.is_none() => {
                let StructuralTypeShape::Reference { referent, .. } = declaration.shape
                else {
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
                let cases = declaration.shape.sum_cases()?;
                let case = exactly_one(cases.iter(), |case| case.id == *id)?;
                case_fields = Some(case.fields.as_slice());
                DescriptorPathSegment::Case(*id)
            }
            StructuralPathSegment::Field(name) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(declaration)?,
                };
                let mut matching = fields.iter().filter(|field| {
                    field.identity == *name && !field.relevance.is_erased()
                });
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

pub fn fixed_runtime_index_alternatives(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<DescriptorLocation>> {
    let index_at = path.iter().position(|segment| {
        matches!(segment, StructuralPathSegment::RuntimeIndex { .. })
    })?;
    let suffix = &path[index_at + 1..];
    if suffix
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
    {
        return None;
    }
    let prefix = &path[..index_at];
    let prefix_location = descriptor_location(types, root, root_type, prefix, None)?;
    let prefix_type = descriptor_location_type(types, root_type, &prefix_location.path)?;
    let length = indexed_carrier_length(types, prefix_type)?;
    if length == 0 || length > 4096 {
        return None;
    }
    let mut alternatives = Vec::with_capacity(usize::try_from(length).ok()?);
    for index in 0..length {
        let mut structural = prefix.to_vec();
        if !matches!(
            declaration(types, prefix_type)?.shape,
            StructuralTypeShape::FixedArray { .. }
                | StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BoundedView { .. }
                        | ByteSequenceCarrier::BoundedOwned { .. }
                )
        ) {
            structural.push(StructuralPathSegment::Referent);
        }
        structural.push(StructuralPathSegment::FixedIndex(index));
        structural.extend_from_slice(suffix);
        alternatives.push(descriptor_location(
            types,
            root,
            root_type,
            &structural,
            None,
        )?);
    }
    Some(alternatives)
}

fn indexed_carrier_length(
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<u64> {
    let shape = &declaration(types, structural_type)?.shape;
    match shape {
        StructuralTypeShape::FixedArray { length, .. } => length.literal(),
        StructuralTypeShape::ByteSequence(
            ByteSequenceCarrier::BoundedView { capacity, .. }
            | ByteSequenceCarrier::BoundedOwned { capacity },
        ) => Some(*capacity),
        StructuralTypeShape::Reference { referent, .. } => {
            indexed_carrier_length(types, *referent)
        }
        _ => None,
    }
}

/// Indexing a view borrows through that descriptor, even when the
/// selected element is an ordinary scalar. This identifies permission,
/// not the address or extent of the payload.
pub fn view_element_descriptor_prefix(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Result<Option<usize>, &'static str> {
    let mut tip = Some(root_type);
    let mut case_fields = None;
    let mut element = None;
    for (position, segment) in path.iter().enumerate() {
        let owner = declaration(types, tip.ok_or("a loan path crosses a scalar leaf")?)
            .ok_or("a loan path has no exact structural declaration")?;
        if case_fields.is_none()
            && matches!(
                segment,
                DescriptorPathSegment::FixedIndex(_)
                    | DescriptorPathSegment::RuntimeIndex { .. }
            )
            && owner.shape.is_view_descriptor()
            && element.replace(position).is_some()
        {
            return Err("an inner view needs its own captured descriptor");
        }
        tip = match segment {
            DescriptorPathSegment::Field(id) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(owner).ok_or("a loan field has no record owner")?,
                };
                let field = exactly_one(fields.iter(), |field| {
                    field.id == *id && !field.relevance.is_erased()
                })
                .ok_or("a loan field has no exact declaration")?;
                match field.field_type {
                    StructuralFieldType::Structural(id) => Some(id),
                    _ => crate::inline_carrier_declaration(types.iter(), field)
                        .map(|declaration| declaration.id),
                }
            }
            DescriptorPathSegment::Case(id) if case_fields.is_none() => {
                let cases = owner
                    .shape
                    .sum_cases()
                    .ok_or("a loan case has no sum owner")?;
                let case = exactly_one(cases.iter(), |case| case.id == *id)
                    .ok_or("a loan case has no exact declaration")?;
                case_fields = Some(case.fields.as_slice());
                Some(owner.id)
            }
            DescriptorPathSegment::Referent if case_fields.is_none() => {
                if element.is_some() {
                    return Err("reference backing beneath an unresolved element selector");
                }
                let StructuralTypeShape::Reference { referent, .. } = owner.shape else {
                    return Err("a loan referent has no reference descriptor");
                };
                Some(referent)
            }
            DescriptorPathSegment::FixedIndex(_)
            | DescriptorPathSegment::RuntimeIndex { .. }
                if case_fields.is_none() =>
            {
                match owner.shape {
                    StructuralTypeShape::FixedArray { element, .. }
                    | StructuralTypeShape::ElementView { element, .. } => Some(element),
                    StructuralTypeShape::ByteSequence(_) => None,
                    _ => return Err("a loan index has no array or view owner"),
                }
            }
            DescriptorPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } if case_fields.is_none() => Some(
                crate::representation_view_declaration(
                    types.iter(),
                    *structural_type,
                    *stored_scalar,
                )
                .ok_or("a loan presentation has no exact declaration")?
                .id,
            ),
            // Extent observations name the descriptor's storage, not
            // its separately borrowed bytes.
            DescriptorPathSegment::Extent => return Ok(element),
            _ => return Err("a loan path has no exact typed projection"),
        };
    }
    Ok(element)
}

pub fn descriptor_location_type(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Option<StructuralTypeId> {
    resolved_path(types, root_type, path, |_, _, _| None)?.1
}
pub fn descriptor_path(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
) -> Option<Vec<StructuralPathSegment>> {
    Some(resolved_path(types, root_type, path, |_, _, _| None)?.0)
}
pub(crate) fn resolved_path(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    path: &[DescriptorPathSegment],
    mut constructor_element: impl FnMut(
        &[DescriptorPathSegment],
        StructuralTypeId,
        u64,
    ) -> Option<StructuralTypeId>,
) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
    let mut result = Vec::new();
    let mut tip = Some(root_type);
    let mut case_fields = None;
    for segment in path {
        let declaration = declaration(types, tip?)?;
        result.push(match segment {
            DescriptorPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } if case_fields.is_none() => {
                tip = Some(
                    crate::representation_view_declaration(
                        types.iter(),
                        *structural_type,
                        *stored_scalar,
                    )?
                    .id,
                );
                StructuralPathSegment::RepresentationView {
                    structural_type: *structural_type,
                    stored_scalar: *stored_scalar,
                }
            }
            DescriptorPathSegment::Referent if case_fields.is_none() => {
                let StructuralTypeShape::Reference { referent, .. } = declaration.shape
                else {
                    return None;
                };
                tip = Some(referent);
                StructuralPathSegment::Referent
            }
            DescriptorPathSegment::FixedIndex(index) if case_fields.is_none() => {
                tip = match declaration.shape {
                    StructuralTypeShape::ElementView {
                        access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                        ..
                    } => {
                        let prefix = &path[..result.len()];
                        Some(constructor_element(prefix, declaration.id, *index)?)
                    }
                    _ => fixed_element(&declaration.shape, *index)?,
                };
                StructuralPathSegment::FixedIndex(*index)
            }
            DescriptorPathSegment::Case(id) if case_fields.is_none() => {
                let cases = declaration.shape.sum_cases()?;
                let case = exactly_one(cases.iter(), |case| case.id == *id)?;
                case_fields = Some(case.fields.as_slice());
                StructuralPathSegment::Case(*id)
            }
            DescriptorPathSegment::Field(id) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(declaration)?,
                };
                let field = exactly_one(fields.iter(), |field| {
                    field.id == *id && !field.relevance.is_erased()
                })?;
                // A leaf field that states its carrier names its
                // exact declaration. Reading it back from the leaf
                // shape finds nothing as soon as two declarations
                // share that shape, as the byte views of two regions
                // do, and a path ending at such a field would then
                // have no type at all.
                tip = match field.field_type {
                    StructuralFieldType::Structural(id) => Some(id),
                    _ => crate::inline_carrier_declaration(types.iter(), field)
                        .or_else(|| {
                            field
                                .field_type
                                .canonical_leaf_type()
                                .and_then(|leaf| leaf.leaf_declaration(types.iter()))
                        })
                        .map(|declaration| declaration.id),
                };
                StructuralPathSegment::Field(field.identity)
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
        terminal_psi::record_or_mixed_shape_pattern!(fields) => Some(fields),
        _ => None,
    }
}

fn fixed_element(
    shape: &StructuralTypeShape,
    index: u64,
) -> Option<Option<StructuralTypeId>> {
    match shape {
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } if index < *length => Some(Some(*element)),
        StructuralTypeShape::ByteSequence(
            ByteSequenceCarrier::BoundedView { capacity, .. }
            | ByteSequenceCarrier::BoundedOwned { capacity },
        ) if index < *capacity => Some(None),
        _ => None,
    }
}
