//! Canonical carrier addresses authenticated against the actual type catalog.
use super::{DescriptorLocation, DescriptorPathSegment};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// Resolve retained declaration identities without recovering authority from
/// field spelling. Case liveness remains the operation verifier's obligation.
pub fn canonical_descriptor_location(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<DescriptorLocation> {
    let path = path
        .iter()
        .map(|segment| match segment {
            CanonicalStructuralPathSegment::Field(field) => DescriptorPathSegment::Field(*field),
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                DescriptorPathSegment::FixedIndex(*index)
            }
            CanonicalStructuralPathSegment::Case(case) => DescriptorPathSegment::Case(*case),
        })
        .collect::<Vec<_>>();
    resolved_path(types, root_type, &path)?;
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
                let cases = declaration.shape.sum_cases()?;
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

/// A runtime index does not mint one descriptor slot. Over a fixed array, or a
/// reference whose referent is one, every in-range element is a possible
/// storage origin. The caller still proves the index; this only names the
/// backing the pointer may address. A further runtime index, or an array
/// longer than the replay's visit budget, stays unknown rather than a prefix.
pub fn fixed_runtime_index_alternatives(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<DescriptorLocation>> {
    let index_at = path
        .iter()
        .position(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))?;
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
        StructuralTypeShape::FixedArray { length, .. } => Some(*length),
        StructuralTypeShape::ByteSequence(
            ByteSequenceCarrier::BoundedView { capacity, .. }
            | ByteSequenceCarrier::BoundedOwned { capacity },
        ) => Some(*capacity),
        StructuralTypeShape::Reference { referent, .. } => indexed_carrier_length(types, *referent),
        _ => None,
    }
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
                let StructuralTypeShape::Reference { referent, .. } = declaration.shape else {
                    return None;
                };
                tip = Some(referent);
                StructuralPathSegment::Referent
            }
            DescriptorPathSegment::FixedIndex(index) if case_fields.is_none() => {
                tip = fixed_element(&declaration.shape, *index)?;
                StructuralPathSegment::FixedIndex(*index)
            }
            DescriptorPathSegment::Case(id) if case_fields.is_none() => {
                let cases = declaration.shape.sum_cases()?;
                let mut matching = cases.iter().filter(|case| case.id == *id);
                case_fields = Some(matching.next()?.fields.as_slice());
                if matching.next().is_some() {
                    return None;
                }
                StructuralPathSegment::Case(*id)
            }
            DescriptorPathSegment::Field(id) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(declaration)?,
                };
                let mut matching = fields
                    .iter()
                    .filter(|field| field.id == *id && !field.relevance.is_erased());
                let field = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                tip = match field.field_type {
                    StructuralFieldType::Structural(id) => Some(id),
                    _ => None,
                };
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
        terminal_psi::record_or_mixed_shape_pattern!(fields) => Some(fields),
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
