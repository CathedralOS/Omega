//! The exact subtree a restoration window vacates and reseats.
//!
//! `path` always selects storage. An optional field extends that path by its
//! declaration-local identity; without a field the path is already complete.
//! In particular, an index can select either a whole element or the parent of
//! a further field. Guessing from the last segment conflates those operations
//! and is ambiguous when different records reuse a field ID.
//!
//! This is geometry, not permission: consumers still authenticate the root,
//! runtime bounds, live loan, value custody and exact restoration debt.

use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// Declaration-derived complete projection and the subtree it selects.
pub struct StructuralWindowProjection {
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

/// Resolve a window below an already authenticated storage root. Reference
/// crossings describe geometry only; consumers must independently resolve the
/// live loan. Owning a carrier does not grant ownership of its referent.
pub fn structural_window_projection<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> Option<StructuralWindowProjection> {
    let declaration = |identity| {
        let mut matches = types.clone().filter(|candidate| candidate.id == identity);
        let selected = matches.next()?;
        matches.next().is_none().then_some(selected)
    };
    let mut selected = declaration(root)?;
    for segment in path {
        let child = match (segment, &selected.shape) {
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let mut matches = fields.iter().filter(|field| field.identity == *identity);
                let field = matches.next()?;
                if matches.next().is_some() || field.relevance.is_erased() {
                    return None;
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return None;
                };
                child
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            _ => return None,
        };
        selected = declaration(child)?;
    }
    let mut complete = path.to_vec();
    if let Some(identity) = field {
        let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
            &selected.shape
        else {
            return None;
        };
        let mut matches = fields.iter().filter(|field| field.id == identity);
        let field = matches.next()?;
        if matches.next().is_some() || field.relevance.is_erased() {
            return None;
        }
        let StructuralFieldType::Structural(child) = field.field_type else {
            return None;
        };
        selected = declaration(child)?;
        complete.push(StructuralPathSegment::Field(field.identity.clone()));
    }
    Some(StructuralWindowProjection {
        path: complete,
        structural_type: selected.id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{BindingRelevance, StructuralFieldDeclaration};

    fn declarations() -> Vec<StructuralTypeDeclaration> {
        let field = |name: &str, child| StructuralFieldDeclaration {
            // Reusing a declaration-local ID must not conflate the array
            // owner with the field inside its element.
            id: StructuralFieldId::new(1).unwrap(),
            identity: name.into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(StructuralTypeId::new(child).unwrap()),
        };
        [
            StructuralTypeShape::Record {
                fields: vec![field("elements", 2)],
            },
            StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(3).unwrap(),
                length: 2,
            },
            StructuralTypeShape::Record {
                fields: vec![field("member", 4)],
            },
            StructuralTypeShape::Record { fields: Vec::new() },
        ]
        .into_iter()
        .enumerate()
        .map(|(position, shape)| StructuralTypeDeclaration {
            id: StructuralTypeId::new(position as u64 + 1).unwrap(),
            identity: format!("WindowType{position}"),
            shape,
        })
        .collect()
    }

    #[test]
    fn window_element_and_its_field_have_distinct_complete_paths() {
        let types = declarations();
        let root = types[0].id;
        let path = vec![
            StructuralPathSegment::Field("elements".into()),
            StructuralPathSegment::FixedIndex(1),
        ];
        let element = structural_window_projection(types.iter(), root, &path, None).unwrap();
        let member =
            structural_window_projection(types.iter(), root, &path, StructuralFieldId::new(1))
                .unwrap();
        assert_eq!(element.structural_type, types[2].id);
        assert_eq!(element.path, path);
        assert_eq!(member.structural_type, types[3].id);
        assert_eq!(
            member.path,
            [path, vec![StructuralPathSegment::Field("member".into())]].concat()
        );
    }

    #[test]
    fn window_projection_rejects_wrong_field_and_out_of_bounds_element() {
        let types = declarations();
        let root = types[0].id;
        let mut path = vec![
            StructuralPathSegment::Field("elements".into()),
            StructuralPathSegment::FixedIndex(1),
        ];
        assert!(
            structural_window_projection(types.iter(), root, &path, StructuralFieldId::new(2))
                .is_none()
        );
        path[1] = StructuralPathSegment::FixedIndex(2);
        assert!(structural_window_projection(types.iter(), root, &path, None).is_none());
    }
}
