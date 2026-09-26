use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use std::collections::BTreeMap;

pub(crate) fn structural_qualifications_match(
    carrier: StructuralTypeId,
    qualifications: &[StructuralDomainId],
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> bool {
    !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
        && qualifications.iter().all(|domain| {
            domains
                .get(domain)
                .is_some_and(|domain| domain.carrier == carrier)
        })
}

pub(crate) fn structural_projected_qualifications_match(
    root: StructuralTypeId,
    qualifications: &[terminal_psi::StructuralPathQualification],
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> bool {
    !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
        && qualifications.iter().all(|qualification| {
            !qualification.path.is_empty()
                && resolve_structural_path(types, root, &qualification.path).is_some_and(
                    |projected| {
                        domains
                            .get(&qualification.domain)
                            .is_some_and(|domain| domain.carrier == projected)
                    },
                )
        })
}

pub(crate) fn resolve_structural_path(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    types.get(&structural_type)?;
    for segment in path {
        let declaration = types.get(&structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                terminal_psi::StructuralPathSegment::Field(identity),
                terminal_psi::StructuralTypeShape::Record { fields },
            ) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                match &field.field_type {
                    terminal_psi::StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape()?;
                        *types
                            .iter()
                            .find(|(_, declaration)| declaration.shape == shape)
                            .map(|(id, _)| id)?
                    }
                }
            }
            (
                terminal_psi::StructuralPathSegment::FixedIndex(index),
                terminal_psi::StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            (
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
                terminal_psi::StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}
