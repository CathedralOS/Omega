use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralCaseId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, find_structural_case, live_field_by_id,
};

/// The original record carrier reached by exact declaration-local field IDs.
/// The runtime path names come from those declarations, never from source spelling.
pub struct RecordFieldCarrier {
    pub structural_type: StructuralTypeId,
    pub path: Vec<StructuralPathSegment>,
}

/// Resolve record-field scalar observation carriers; the final scalar field is
/// deliberately separate. Record fields and literal fixed-array indices
/// compose in declaration order, including fields below indexed records.
/// The carrier may also be a mixed shape, whose common fields sit directly on
/// it beside the tag like a record's and whatever case is active: its
/// payload fields instead need `case_payload_carrier`'s final `Case` step.
/// Access, ownership and scalar leaf validation remain the consuming
/// operation's independent obligations.
pub fn record_field_carrier<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<RecordFieldCarrier> {
    let (structural_type, runtime_path, _) = walk_record_fields(types.clone(), root, path)?;
    matches!(
        types
            .clone()
            .find(|declaration| declaration.id == structural_type)?
            .shape,
        StructuralTypeShape::Record { .. } | StructuralTypeShape::Mixed { .. }
    )
    .then_some(RecordFieldCarrier {
        structural_type,
        path: runtime_path,
    })
}

/// The payload namespace a case-qualified scalar observation reads: record
/// fields reach a sum, and one final `Case` step names the declared case
/// whose payload fields the observed leaf belongs to.
///
/// The `Case` step adds no storage component. A sum stores only its active
/// case's payload, overlaid at the sum's own place, so `path` ends at the sum
/// and the case is a claim about which overlay is live. Resolving the carrier
/// is not evidence of that claim: the verifier must prove the case active from
/// dominating membership facts, and an interpreter must check the stored
/// discriminator, before either observes a payload field.
pub struct CasePayloadCarrier<'types> {
    pub sum: StructuralTypeId,
    pub case: StructuralCaseId,
    pub path: Vec<StructuralPathSegment>,
    pub fields: &'types [StructuralFieldDeclaration],
}

/// Resolve `record fields… + Case` to the selected case's payload namespace.
/// Only the last segment may be a `Case`; a payload read below a nested
/// payload member is a leaf copy's job, not a scalar observation carrier.
pub fn case_payload_carrier<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<CasePayloadCarrier<'types>> {
    let (CanonicalStructuralPathSegment::Case(case), prefix) = path.split_last()? else {
        return None;
    };
    let (sum, runtime_path, _) = walk_record_fields(types.clone(), root, prefix)?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) = &types
        .clone()
        .find(|declaration| declaration.id == sum)?
        .shape
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

/// The relevant declaration one scalar field observation names below `root`:
/// a field of the record `path` reaches, a common field of the mixed shape it
/// reaches, or a payload field of the case its final `Case` step selects.
/// Erased declarations are never observed.
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
        let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = &types
            .clone()
            .find(|declaration| declaration.id == carrier.structural_type)?
            .shape
        else {
            return None;
        };
        fields
    };
    live_field_by_id(fields, field)
}

/// Resolve one static field access without interpreting its operation. Common
/// fields and selected payloads share declaration-local identity; a final Case
/// qualifies the separate leaf field, while earlier Case/Field pairs descend
/// through structural payloads. This supplies geometry only, never tag or
/// mutation authority, and never treats an atomic cell as an ordinary scalar.
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
    let (mut canonical, carrier) = crate::canonical_static_projection(types.clone(), root, prefix)?;
    let fields = if let Some(case) = selected_case {
        let cases = carrier.shape.sum_cases()?;
        let mut matching = cases.iter().filter(|candidate| candidate.id == case);
        let selected = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
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

/// Resolve the declared field of an explicit projection, including stored
/// reference crossings. This supplies field identity and type only: unlike a
/// canonical storage path, it proves neither backing identity nor authority.
/// A final Case is the receiver's payload namespace, not a separate record.
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

/// Resolve a path's final field by portable identity, without inventing a
/// nominal type for an inline carrier. This shares namespace selection with
/// field-ID observations; neither query proves live membership or loan access.
pub fn runtime_structural_field<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'types StructuralFieldDeclaration> {
    let (StructuralPathSegment::Field(identity), parent) = path.split_last()? else {
        return None;
    };
    terminal_psi::live_field(projected_fields(types, root, parent)?, *identity)
}

fn projected_fields<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<&'types [StructuralFieldDeclaration]> {
    let (prefix, selected_case) = match path.split_last() {
        Some((StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
        _ => (path, None),
    };
    let carrier = crate::runtime_structural_path_tip(types, root, prefix)?;
    let fields = if let Some(case) = selected_case {
        let mut matching = carrier
            .shape
            .sum_cases()?
            .iter()
            .filter(|candidate| candidate.id == case);
        let selected = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        selected.fields.as_slice()
    } else {
        carrier.shape.direct_fields()?
    };
    Some(fields)
}

/// Preserve backing authority when observing a field through a canonical path.
/// Owning an element-view descriptor, even inside an owned record, grants no
/// exclusive access to the elements it references. A caller must separately
/// validate the field and the operation's access requirements.
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

/// Walk declaration-local record fields and literal array indices from `root`.
/// Returns the reached type and its
/// runtime path; the caller decides which shape it must have.
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
        let declaration = types
            .clone()
            .find(|declaration| declaration.id == structural_type)?;
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
            continue;
        }
        if payload_fields.is_some() && !matches!(segment, CanonicalStructuralPathSegment::Field(_))
        {
            return None;
        }
        if let CanonicalStructuralPathSegment::FixedIndex(element_index) = segment {
            // Fixed arrays retain every element identity through nested fields.
            // Views use the same element/member namespace, but their bound is
            // a live length, not the declaration-owned extent checked here.
            let declaration = types
                .clone()
                .find(|declaration| declaration.id == structural_type)?;
            let element = match &declaration.shape {
                StructuralTypeShape::FixedArray { element, length } => {
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
        let declaration = types
            .clone()
            .find(|declaration| declaration.id == structural_type)?;
        let fields = match payload_fields.take() {
            Some(fields) => fields,
            None => declaration.shape.direct_fields()?,
        };
        let field = live_field_by_id(fields, *field)?;
        let child = field.field_type.as_structural()?;
        runtime_path.push(StructuralPathSegment::Field(field.identity.clone()));
        structural_type = child;
    }
    if payload_fields.is_some() {
        return None;
    }
    Some((structural_type, runtime_path, crosses_view))
}
