use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralCaseId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
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
    let selected = cases.iter().find(|candidate| candidate.id == *case)?;
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
        let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
            &types
                .clone()
                .find(|declaration| declaration.id == carrier.structural_type)?
                .shape
        else {
            return None;
        };
        fields
    };
    fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())
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
    for (index, segment) in path.iter().enumerate() {
        if let CanonicalStructuralPathSegment::FixedIndex(element_index) = segment {
            // Fixed arrays retain every element identity through nested fields.
            // Views keep their existing terminal-index route: their bound is
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
                StructuralTypeShape::ElementView { element } if index + 1 == path.len() => {
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
    Some((structural_type, runtime_path, crosses_view))
}
