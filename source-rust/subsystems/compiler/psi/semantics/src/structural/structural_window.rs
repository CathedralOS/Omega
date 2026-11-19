use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

pub struct StructuralWindowProjection {
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

pub fn structural_window_origins<'parameters>(
    parameters: impl Iterator<Item = &'parameters terminal_psi::StructuralParameterDeclaration>
    + Clone,
    origins: impl IntoIterator<Item = PlaceId>,
    required_access: terminal_psi::StructuralAccess,
    mut live_storage: impl FnMut(PlaceId) -> bool,
) -> bool {
    let mut found_origin = false;
    for root in origins {
        found_origin = true;
        if !live_storage(root)
            && !parameters.clone().any(|parameter| {
                parameter.place == root
                    && required_access.admits_referent(parameter.access)
                    && parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            })
        {
            return false;
        }
    }
    found_origin
}

pub fn structural_window_selects_child(
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> bool {
    field.is_some()
        || path
            .iter()
            .rev()
            .take_while(|segment| !matches!(segment, StructuralPathSegment::Referent))
            .any(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RuntimeIndex { .. }
                )
            })
}

pub fn structural_window_projection<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> Option<StructuralWindowProjection> {
    let mut complete = path.to_vec();
    if let Some(identity) = field {
        let field = crate::projected_field_declaration(types.clone(), root, path, identity)?;
        field.field_type.as_structural()?;
        complete.push(StructuralPathSegment::Field(field.identity));
    }
    let structural_type = crate::structural::primitive_place::projection_tip_with_policy(
        types.clone(),
        root,
        &complete,
        |current, segment| match segment {
            StructuralPathSegment::Field(_)
            | StructuralPathSegment::Case(_)
            | StructuralPathSegment::Referent => true,
            StructuralPathSegment::FixedIndex(_) => current.is_some_and(|current| {
                types.clone().any(|declaration| {
                    declaration.id == current
                        && matches!(
                            declaration.shape,
                            StructuralTypeShape::FixedArray {
                                length: terminal_psi::ArrayExtent::Literal(_),
                                ..
                            }
                        )
                })
            }),
            StructuralPathSegment::RuntimeIndex { .. } => current.is_some_and(|current| {
                types.clone().any(|declaration| {
                    declaration.id == current
                        && matches!(
                            declaration.shape,
                            StructuralTypeShape::FixedArray { .. }
                                | StructuralTypeShape::ElementView { .. }
                        )
                })
            }),
            _ => false,
        },
    )??;
    Some(StructuralWindowProjection {
        path: complete,
        structural_type,
    })
}
