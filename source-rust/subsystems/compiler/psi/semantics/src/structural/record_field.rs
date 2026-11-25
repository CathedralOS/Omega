use crate::exactly_one::exactly_one;
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralCaseId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, find_structural_case, live_field_by_id,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SequenceFieldObservation {
    Length,
    Contents,
}

pub fn sequence_field_allows_observation<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    field: &terminal_psi::StructuralFieldType,
    observation: SequenceFieldObservation,
) -> bool {
    let access = if let Some(carrier) = field.byte_sequence_carrier() {
        match carrier {
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => StructuralAccess::Owned,
            terminal_psi::ByteSequenceCarrier::BorrowedView { access }
            | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. } => {
                access.unwrap_or(StructuralAccess::SharedBorrow)
            }
        }
    } else if let Some(identity) = field.as_structural() {
        let Some(declaration) = exactly_one(types, |row| row.id == identity) else {
            return false;
        };
        match declaration.shape {
            StructuralTypeShape::ElementView { access, .. }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access },
            ) => access.unwrap_or(StructuralAccess::SharedBorrow),
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
            ) => StructuralAccess::Owned,
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
            ) => access.unwrap_or(StructuralAccess::SharedBorrow),
            _ => return false,
        }
    } else {
        return false;
    };
    observation == SequenceFieldObservation::Length
        || access != StructuralAccess::WriteOnlyBorrow
}

pub struct RecordFieldCarrier {
    pub structural_type: StructuralTypeId,
    pub path: Vec<StructuralPathSegment>,
}

pub fn record_field_carrier<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<RecordFieldCarrier> {
    let (structural_type, runtime_path, _) = walk_record_fields(types.clone(), root, path)?;
    matches!(
        crate::structural_shape(types.clone(), structural_type)?,
        StructuralTypeShape::Record { .. } | StructuralTypeShape::Mixed { .. }
    )
    .then_some(RecordFieldCarrier {
        structural_type,
        path: runtime_path,
    })
}

pub struct CasePayloadCarrier<'types> {
    pub sum: StructuralTypeId,
    pub case: StructuralCaseId,
    pub path: Vec<StructuralPathSegment>,
    pub fields: &'types [StructuralFieldDeclaration],
}

pub fn case_payload_carrier<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<CasePayloadCarrier<'types>> {
    let (CanonicalStructuralPathSegment::Case(case), prefix) = path.split_last()? else {
        return None;
    };
    let (sum, runtime_path, _) = walk_record_fields(types.clone(), root, prefix)?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        crate::structural_shape(types.clone(), sum)?
    else {
        return None;
    };
    let selected = find_structural_case(cases, *case)?;
    Some(CasePayloadCarrier {
        sum,
        case: *case,
        path: runtime_path,
        fields: &selected.fields,
    })
}

pub fn observed_field_declaration<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<&'types StructuralFieldDeclaration> {
    let fields = if let Some(carrier) = case_payload_carrier(types.clone(), root, path) {
        carrier.fields
    } else {
        let carrier = record_field_carrier(types.clone(), root, path)?;
        let (terminal_psi::record_or_mixed_shape_pattern!(fields)) =
            crate::structural_shape(types.clone(), carrier.structural_type)?
        else {
            return None;
        };
        fields
    };
    live_field_by_id(fields, field)
}

pub fn static_field_projection<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<(
    Vec<CanonicalStructuralPathSegment>,
    &'types StructuralFieldDeclaration,
)> {
    let (prefix, selected_case) = match path.split_last() {
        Some((StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
        _ => (path, None),
    };
    let (mut canonical, carrier) =
        crate::canonical_static_projection(types.clone(), root, prefix)?;
    let fields = if let Some(case) = selected_case {
        let cases = carrier.shape.sum_cases()?;
        let selected = exactly_one(cases.iter(), |candidate| candidate.id == case)?;
        canonical.push(CanonicalStructuralPathSegment::Case(case));
        selected.fields.as_slice()
    } else {
        carrier.shape.direct_fields()?
    };
    let mut matching = fields
        .iter()
        .filter(|candidate| candidate.id == field && !candidate.relevance.is_erased());
    let field = matching.next()?;
    matching.next().is_none().then_some((canonical, field))
}

pub fn projected_field_declaration<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<&'types StructuralFieldDeclaration> {
    let fields = projected_fields(types, root, path)?;
    let mut matching = fields
        .iter()
        .filter(|candidate| candidate.id == field && !candidate.relevance.is_erased());
    let field = matching.next()?;
    matching.next().is_none().then_some(field)
}

pub fn runtime_structural_field<'types>(
    types: impl crate::StructuralDeclarations<'types>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'types StructuralFieldDeclaration> {
    let (StructuralPathSegment::Field(identity), parent) = path.split_last()? else {
        return None;
    };
    terminal_psi::live_field(projected_fields(types, root, parent)?, *identity)
}

/// An operation's storage path as the place table names it: each field
/// step by its declared identity, resolved against the prefix before it
/// (which may end in the case whose payload holds the field), and every
/// other step as it is. The lowered form and the checker both intern a
/// place from an operation's path through this one reading.
pub fn resolved_structural_path<'types>(
    types: impl crate::StructuralDeclarations<'types>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Result<Vec<terminal_psi::ResolvedStructuralPathSegment>, &'static str> {
    use terminal_psi::ResolvedStructuralPathSegment as Segment;
    let mut segments = Vec::with_capacity(path.len());
    for (position, segment) in path.iter().enumerate() {
        segments.push(match segment {
            StructuralPathSegment::Case(case) => Segment::Case(*case),
            StructuralPathSegment::FixedIndex(index) => Segment::FixedIndex(*index),
            StructuralPathSegment::RuntimeIndex { index, .. } => {
                Segment::RuntimeIndex { selector: *index }
            }
            StructuralPathSegment::Referent => Segment::Referent,
            // The view keeps the same backing and changes how later
            // fields are resolved. Retain it for the shared shape and
            // overlap judgments, rather than erasing it.
            StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } => Segment::RepresentationView {
                structural_type: *structural_type,
                stored_scalar: *stored_scalar,
            },
            StructuralPathSegment::Field(_) => Segment::Field(
                runtime_structural_field(types.clone(), root, &path[..=position])
                    .ok_or("a storage path that is not a path of fields and cases")?
                    .id,
            ),
            StructuralPathSegment::AllElements | StructuralPathSegment::FixedRange { .. } => {
                return Err(
                    "storage projection outside plain records and fixed array elements",
                );
            }
        });
    }
    Ok(segments)
}

fn projected_fields<'types>(
    types: impl crate::StructuralDeclarations<'types>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'types [StructuralFieldDeclaration]> {
    let (prefix, selected_case) = match path.split_last() {
        Some((StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
        _ => (path, None),
    };
    let carrier = crate::runtime_structural_path_tip(types, root, prefix)?;
    let fields = if let Some(case) = selected_case {
        let selected = exactly_one(carrier.shape.sum_cases()?.iter(), |candidate| {
            candidate.id == case
        })?;
        selected.fields.as_slice()
    } else {
        carrier.shape.direct_fields()?
    };
    Some(fields)
}

pub fn observed_field_backing_access<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
    access: StructuralAccess,
) -> Option<StructuralAccess> {
    let prefix = match path.split_last() {
        Some((CanonicalStructuralPathSegment::Case(_), prefix)) => prefix,
        _ => path,
    };
    let (_, _, crosses_view) = walk_record_fields(types, root, prefix)?;
    Some(
        if crosses_view && access != StructuralAccess::WriteOnlyBorrow {
            StructuralAccess::SharedBorrow
        } else {
            access
        },
    )
}

fn walk_record_fields<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<(StructuralTypeId, Vec<StructuralPathSegment>, bool)> {
    let mut structural_type = root;
    let mut runtime_path = Vec::with_capacity(path.len());
    let mut crosses_view = false;
    let mut payload_fields = None;
    for segment in path {
        let declaration = crate::structural_declaration(types.clone(), structural_type)?;
        if matches!(segment, CanonicalStructuralPathSegment::Referent) {
            if payload_fields.is_some() {
                return None;
            }
            let StructuralTypeShape::Reference { referent, access } = declaration.shape else {
                return None;
            };
            if access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                return None;
            }
            runtime_path.push(StructuralPathSegment::Referent);
            structural_type = referent;
            crosses_view = true;
            continue;
        }
        if let CanonicalStructuralPathSegment::Case(case) = segment {
            if payload_fields.is_some() {
                return None;
            }
            let cases = declaration.shape.sum_cases()?;
            payload_fields = Some(
                cases
                    .iter()
                    .find(|candidate| candidate.id == *case)?
                    .fields
                    .as_slice(),
            );
            runtime_path.push(StructuralPathSegment::Case(*case));
            continue;
        }
        if payload_fields.is_some()
            && !matches!(segment, CanonicalStructuralPathSegment::Field(_))
        {
            return None;
        }
        if let CanonicalStructuralPathSegment::FixedIndex(element_index) = segment {
            let declaration = crate::structural_declaration(types.clone(), structural_type)?;
            let element = match &declaration.shape {
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } => {
                    if *element_index >= *length {
                        return None;
                    }
                    *element
                }
                StructuralTypeShape::ElementView { element, .. } => {
                    crosses_view = true;
                    *element
                }
                _ => return None,
            };
            runtime_path.push(StructuralPathSegment::FixedIndex(*element_index));
            structural_type = element;
            continue;
        }
        let CanonicalStructuralPathSegment::Field(field) = segment else {
            return None;
        };
        let declaration = crate::structural_declaration(types.clone(), structural_type)?;
        let fields = match payload_fields.take() {
            Some(fields) => fields,
            None => declaration.shape.direct_fields()?,
        };
        let field = live_field_by_id(fields, *field)?;
        let child = field.field_type.as_structural()?;
        runtime_path.push(StructuralPathSegment::Field(field.identity));
        structural_type = child;
    }
    if payload_fields.is_some() {
        return None;
    }
    Some((structural_type, runtime_path, crosses_view))
}
