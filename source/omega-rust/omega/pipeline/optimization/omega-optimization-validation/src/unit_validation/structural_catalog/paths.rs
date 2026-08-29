use super::*;

pub(crate) fn structural_qualifications_match(
    carrier: StructuralTypeId,
    qualifications: &[StructuralDomainId],
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
        && qualifications.iter().all(|domain| {
            domains
                .get(domain)
                .is_some_and(|domain| domain.carrier == carrier)
        })
}

pub(crate) fn resolve_structural_path(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[psi_terminal::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    types.get(&structural_type)?;
    for segment in path {
        let declaration = types.get(&structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                psi_terminal::StructuralPathSegment::Field(identity),
                psi_terminal::StructuralTypeShape::Record { fields },
            ) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let psi_terminal::StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                psi_terminal::StructuralPathSegment::FixedIndex(index),
                psi_terminal::StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}
