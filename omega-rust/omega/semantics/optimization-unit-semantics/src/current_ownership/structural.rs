use super::*;

pub(super) fn place_structural_type(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    function
        .structural_parameters
        .iter()
        .find_map(|parameter| (parameter.place == place).then_some(parameter.structural_type))
        .or_else(|| {
            function
                .result
                .structural()
                .filter(|result| result.place == place)
                .map(|result| result.structural_type)
        })
        .or_else(|| {
            function
                .structural_places
                .iter()
                .find_map(|candidate| (candidate.id == place).then_some(candidate.kind))
                .and_then(|kind| match kind {
                    semantic_vocabulary::StructuralPlaceKind::OperationResult {
                        structural_type,
                        ..
                    }
                    | semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type,
                        ..
                    }
                    | semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                        structural_type,
                        ..
                    } => Some(structural_type),
                    semantic_vocabulary::StructuralPlaceKind::ProviderAttachment {
                        attachment,
                        ..
                    } => Some(attachment),
                    semantic_vocabulary::StructuralPlaceKind::Parameter { .. }
                    | semantic_vocabulary::StructuralPlaceKind::Result => None,
                })
        })
}

pub(super) fn projected_fixed_array_root_is_fully_consumed(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    frontier: &CurrentOwnership,
    place: PlaceId,
) -> bool {
    let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    else {
        return false;
    };
    if frontier
        .claims
        .values()
        .any(|claim| claim.input == Some(place))
    {
        return false;
    }
    let Some(StructuralTypeShape::FixedArray { element, length }) = structural_types
        .get(&parameter.structural_type)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let Some(length) = usize::try_from(*length).ok() else {
        return false;
    };
    if parameter.multiplicity != StructuralMultiplicity::Linear
        && (parameter.multiplicity != StructuralMultiplicity::Affine
            || parameter.is_self
            || parameter.access != StructuralAccess::Owned
            || !parameter.qualifications.is_empty()
            || length != 2
            || !matches!(
                structural_types
                    .get(element)
                    .map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Record { .. })
            ))
    {
        return false;
    }
    let Some(moved) = frontier.partial_custody_paths.get(&place) else {
        return false;
    };
    moved.len() == length
        && (0..length).all(|index| {
            moved.contains(&vec![StructuralPathSegment::FixedIndex(
                u64::try_from(index).expect("a usize index fits u64"),
            )])
        })
}
