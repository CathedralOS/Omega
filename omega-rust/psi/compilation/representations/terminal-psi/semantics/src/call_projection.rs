//! Translate field coordinates between two already-matched argument types.

use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration, StructuralTypeShape};

/// Formation checks that an argument supplies its formal type. Two
/// instantiations can have different field IDs for the same declaration.
/// Translate the path before checking the observation at its own place.
pub fn translate(
    types: &[StructuralTypeDeclaration],
    mut source: StructuralTypeId,
    mut destination: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    use CanonicalStructuralPathSegment::{Case, Field, FixedIndex, Referent};
    use StructuralTypeShape::{FixedArray, Reference};

    let mut translated = Vec::with_capacity(path.len());
    let mut payload = None;
    for (position, segment) in path.iter().enumerate() {
        if source == destination && payload.is_none() {
            translated.extend_from_slice(&path[position..]);
            return Some(translated);
        }
        let left = terminal_psi::find_structural_type(types, source)?;
        let right = terminal_psi::find_structural_type(types, destination)?;
        let same_declaration = !left.declaration_identity.is_absent()
            && left.declaration_identity == right.declaration_identity;
        match *segment {
            Field(field) if same_declaration => {
                let (left_fields, right_fields) = match payload.take() {
                    Some(fields) => fields,
                    None => (left.shape.direct_fields()?, right.shape.direct_fields()?),
                };
                let field = left_fields.iter().find(|candidate| candidate.id == field)?;
                if field.declaration_identity.is_absent() || field.relevance.is_erased() {
                    return None;
                }
                let mut matching = right_fields.iter().filter(|candidate| {
                    candidate.declaration_identity == field.declaration_identity
                        && candidate.relevance == field.relevance
                });
                let corresponding = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                translated.push(Field(corresponding.id));
                if position + 1 < path.len() {
                    source = field.field_type.as_structural().or(field.inline_carrier)?;
                    destination = corresponding
                        .field_type
                        .as_structural()
                        .or(corresponding.inline_carrier)?;
                }
            }
            Case(case) if same_declaration && payload.is_none() => {
                let case = left
                    .shape
                    .sum_cases()?
                    .iter()
                    .find(|candidate| candidate.id == case)?;
                if case.declaration_identity.is_absent() {
                    return None;
                }
                let mut matching = right.shape.sum_cases()?.iter().filter(|candidate| {
                    candidate.declaration_identity == case.declaration_identity
                });
                let corresponding = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                translated.push(Case(corresponding.id));
                payload = Some((case.fields.as_slice(), corresponding.fields.as_slice()));
            }
            FixedIndex(index) if payload.is_none() => {
                let (
                    FixedArray {
                        element: left_element,
                        length: left_length,
                    },
                    FixedArray {
                        element: right_element,
                        length: right_length,
                    },
                ) = (&left.shape, &right.shape)
                else {
                    return None;
                };
                if index >= left_length.literal()? || index >= right_length.literal()? {
                    return None;
                }
                translated.push(FixedIndex(index));
                source = *left_element;
                destination = *right_element;
            }
            Referent if payload.is_none() => {
                let (
                    Reference {
                        referent: left_referent,
                        access: left_access,
                    },
                    Reference {
                        referent: right_referent,
                        access: right_access,
                    },
                ) = (&left.shape, &right.shape)
                else {
                    return None;
                };
                if left_access != right_access || *left_access == StructuralAccess::WriteOnlyBorrow
                {
                    return None;
                }
                translated.push(Referent);
                source = *left_referent;
                destination = *right_referent;
            }
            _ => return None,
        }
    }
    Some(translated)
}
