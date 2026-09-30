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
            // A view's extent is its stored runtime length: the index step
            // selects an element whose bound the owning operation carries
            // as its verified obligation.
            (
                terminal_psi::StructuralPathSegment::FixedIndex(_),
                terminal_psi::StructuralTypeShape::ElementView { element },
            ) => *element,
            (
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
                terminal_psi::StructuralTypeShape::ElementView { element },
            ) => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

/// Whether a leaf copy mints a record out of a byte carrier: `path` ends at
/// a literal index inside a `u8` fixed array whose remaining elements host
/// `expected`'s fixed scalar layout — the same window the verifier's
/// `byte_carrier_leaf_copy` admission proved. A runtime-selected index
/// cannot prove the wider `index + extent` bound, so it stays outside this
/// arm.
pub(crate) fn byte_carrier_leaf_window(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    expected: StructuralTypeId,
) -> bool {
    let Some((terminal_psi::StructuralPathSegment::FixedIndex(index), prefix)) = path.split_last()
    else {
        return false;
    };
    let Some(parent) = resolve_structural_path(types, root, prefix) else {
        return false;
    };
    let Some(declaration) = types.get(&parent) else {
        return false;
    };
    let terminal_psi::StructuralTypeShape::FixedArray { element, length } = &declaration.shape
    else {
        return false;
    };
    let byte_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .expect("u8 is valid"),
    );
    if !types.values().any(|candidate| {
        candidate.id == *element
            && candidate.shape == terminal_psi::StructuralTypeShape::PrimitiveScalar(byte_type)
    }) {
        return false;
    };
    let Some(layout) =
        terminal_semantics::byte_carrier_scalar_layout(types.values().copied(), expected)
    else {
        return false;
    };
    index
        .checked_add(layout.size)
        .is_some_and(|end| end <= *length)
}
