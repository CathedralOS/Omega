//! Look up structural declarations, fields and cases by their issued identities.

use super::{
    ByteSequenceCarrier, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};
use crate::TerminalIdentityIndex;
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId, StructuralTypeId};

pub fn field_by_identity(
    fields: &[StructuralFieldDeclaration],
    identity: TerminalIdentityIndex,
) -> Option<&StructuralFieldDeclaration> {
    fields.iter().find(|field| field.identity == identity)
}

pub fn live_field_by_identity(
    fields: &[StructuralFieldDeclaration],
    identity: TerminalIdentityIndex,
) -> Option<&StructuralFieldDeclaration> {
    fields
        .iter()
        .find(|field| field.identity == identity && !field.relevance.is_erased())
}

pub fn unique_structural_declaration<'d>(
    declarations: &'d [StructuralTypeDeclaration],
    id: StructuralTypeId,
    visiting: &[StructuralTypeId],
) -> Option<&'d StructuralTypeDeclaration> {
    if visiting.contains(&id) {
        return None;
    }
    let mut matches = declarations
        .iter()
        .filter(|declaration| declaration.id == id);
    let declaration = matches.next()?;
    matches.next().is_none().then_some(declaration)
}

pub fn structural_children(
    fields: &[StructuralFieldDeclaration],
) -> impl Iterator<Item = StructuralTypeId> + '_ {
    fields.iter().filter_map(|field| match field.field_type {
        StructuralFieldType::Structural(child) => Some(child),
        _ => None,
    })
}

pub fn live_field(
    fields: &[StructuralFieldDeclaration],
    identity: TerminalIdentityIndex,
) -> Option<&StructuralFieldDeclaration> {
    let mut matching = fields.iter().filter(|field| field.identity == identity);
    let field = matching.next()?;
    if matching.next().is_some()
        || field.relevance.is_erased()
        || fields
            .iter()
            .filter(|candidate| candidate.id == field.id)
            .count()
            != 1
    {
        return None;
    }
    Some(field)
}

pub fn find_structural_type(
    types: &[StructuralTypeDeclaration],
    id: StructuralTypeId,
) -> Option<&StructuralTypeDeclaration> {
    crate::find_by_identity(types, id, |declaration| declaration.id)
}

pub fn structural_type_contains_reference(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
) -> bool {
    structural_type_contains_reference_by(|id| find_structural_type(types, id), root)
}

pub fn structural_type_contains_reference_by<'types>(
    lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    structural_type_contains_by(
        lookup,
        root,
        |shape| matches!(shape, StructuralTypeShape::Reference { .. }),
        |_| false,
    )
}

pub fn structural_type_has_borrowed_storage_by<'types>(
    lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    structural_type_contains_by(
        lookup,
        root,
        StructuralTypeShape::is_borrowed_descriptor,
        |field| {
            matches!(
                field.byte_sequence_carrier(),
                Some(
                    ByteSequenceCarrier::BorrowedView { .. }
                        | ByteSequenceCarrier::BoundedView { .. }
                )
            )
        },
    )
}

fn structural_type_contains_by<'types>(
    lookup: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
    root: StructuralTypeId,
    contains_shape: impl Fn(&StructuralTypeShape) -> bool,
    contains_field: impl Fn(&StructuralFieldType) -> bool,
) -> bool {
    let mut pending = vec![root];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = lookup(current) else {
            continue;
        };
        if contains_shape(&declaration.shape) {
            return true;
        }
        let mut push_fields = |fields: &[StructuralFieldDeclaration]| {
            pending.extend(structural_children(fields));
            fields
                .iter()
                .any(|field| !field.relevance.is_erased() && contains_field(&field.field_type))
        };
        match &declaration.shape {
            // A parameter's bounds do not exclude borrowed contents.
            // Neither query may treat an unknown payload as empty.
            StructuralTypeShape::TypeParameter => return true,
            StructuralTypeShape::Record { fields, .. } => {
                if push_fields(fields) {
                    return true;
                }
            }
            StructuralTypeShape::Sum { cases } => {
                if cases.iter().any(|case| push_fields(&case.fields)) {
                    return true;
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                if push_fields(fields) || cases.iter().any(|case| push_fields(&case.fields)) {
                    return true;
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if *length != crate::ArrayExtent::Literal(0) {
                    pending.push(*element);
                }
            }
            StructuralTypeShape::ElementView { element, .. } => pending.push(*element),
            StructuralTypeShape::Opaque
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::AtomicCell(_)
            | StructuralTypeShape::ByteSequence(_) => {}
        }
    }
    false
}

pub fn structural_case_by_identity(
    cases: &[StructuralCaseDeclaration],
    identity: TerminalIdentityIndex,
) -> Option<&StructuralCaseDeclaration> {
    cases.iter().find(|case| case.identity == identity)
}

pub fn structural_type_by_identity(
    types: &[StructuralTypeDeclaration],
    identity: TerminalIdentityIndex,
) -> Option<&StructuralTypeDeclaration> {
    types
        .iter()
        .find(|declaration| declaration.identity == identity)
}

pub fn live_field_by_id(
    fields: &[StructuralFieldDeclaration],
    id: StructuralFieldId,
) -> Option<&StructuralFieldDeclaration> {
    fields
        .iter()
        .find(|field| field.id == id && !field.relevance.is_erased())
}

pub fn live_field_entry(
    fields: &[StructuralFieldDeclaration],
    identity: TerminalIdentityIndex,
) -> Option<(usize, &StructuralFieldDeclaration)> {
    fields
        .iter()
        .filter(|field| !field.relevance.is_erased())
        .enumerate()
        .find(|(_, field)| field.identity == identity)
}

pub fn find_structural_case(
    cases: &[StructuralCaseDeclaration],
    id: StructuralCaseId,
) -> Option<&StructuralCaseDeclaration> {
    cases.iter().find(|case| case.id == id)
}

pub fn structural_case_initializer_path(
    shape: &StructuralTypeShape,
    selected_case: StructuralCaseId,
    field_ordinal: usize,
    field: StructuralFieldId,
) -> Option<Vec<crate::StructuralPathSegment>> {
    let (common, cases) = match shape {
        StructuralTypeShape::Sum { cases } => (&[][..], cases),
        StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), cases),
        _ => return None,
    };
    let selected = find_structural_case(cases, selected_case)?;
    let (declaration, is_payload) = common
        .iter()
        .map(|declaration| (declaration, false))
        .chain(
            selected
                .fields
                .iter()
                .map(|declaration| (declaration, true)),
        )
        .filter(|(declaration, _)| !declaration.relevance.is_erased())
        .nth(field_ordinal)?;
    if declaration.id != field {
        return None;
    }
    let mut path = Vec::new();
    if is_payload {
        path.push(crate::StructuralPathSegment::Case(selected_case));
    }
    path.push(crate::StructuralPathSegment::Field(declaration.identity));
    Some(path)
}

pub fn structural_case_position(
    cases: &[StructuralCaseDeclaration],
    id: StructuralCaseId,
) -> Option<usize> {
    cases.iter().position(|case| case.id == id)
}
