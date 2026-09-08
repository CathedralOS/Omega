use super::*;

pub(super) fn insert_owned_result(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    frontier: &mut CurrentOwnership,
    place: PlaceId,
    multiplicity: StructuralMultiplicity,
) -> Result<(), OptimizationUnitValidationError> {
    if frontier.owned_places.insert(place, multiplicity).is_some() {
        return Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive {
            machine: function.machine,
            block,
            node,
            place,
        });
    }
    Ok(())
}

pub(super) fn reject_live_linear_claim(
    function: &PsiOptimizationFunction,
    block: BlockId,
    frontier: &CurrentOwnership,
) -> Result<(), OptimizationUnitValidationError> {
    if let Some(claim) = frontier.claims.iter().find_map(|(claim, live)| {
        (live.multiplicity == Some(StructuralMultiplicity::Linear)).then_some(*claim)
    }) {
        return Err(
            OptimizationUnitValidationError::CurrentLinearClaimAtReturn {
                machine: function.machine,
                block,
                claim,
            },
        );
    }
    Ok(())
}

pub(super) fn expected_trivial_affine_discards(
    function: &PsiOptimizationFunction,
    frontier: &CurrentOwnership,
) -> Vec<PlaceId> {
    // Match Terminal's cleanup schedule: completed affine results precede
    // named locals and parameters, each in reverse establishment order.
    let mut operation_results = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. }
                if frontier.owned_places.get(&place.id)
                    == Some(&StructuralMultiplicity::Affine)
                    && !frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(place.id)) =>
            {
                Some((producer, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    operation_results.sort_by_key(|(producer, _)| std::cmp::Reverse(*producer));
    let mut output = operation_results
        .into_iter()
        .map(|(_, place)| place)
        .collect::<Vec<_>>();
    let mut locals = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if frontier.owned_places.contains_key(&place.id) => {
                Some((declaration_ordinal, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    locals.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    output.extend(locals.into_iter().map(|(_, place)| place));
    output.extend(
        function
            .structural_parameters
            .iter()
            .rev()
            .filter_map(|parameter| {
                (parameter.multiplicity == StructuralMultiplicity::Affine
                    && frontier.owned_places.contains_key(&parameter.place)
                    && !frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(parameter.place))
                    && !function
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == parameter.place))
                .then_some(parameter.place)
            }),
    );
    output
}

pub(super) fn apply_edge_trivial_affine_discards(
    function: &PsiOptimizationFunction,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    discards: &[PlaceId],
) -> Result<(), OptimizationUnitValidationError> {
    let eligible = expected_trivial_affine_discards(function, frontier);
    let mut next = 0;
    for eligible_place in eligible {
        if discards.get(next) == Some(&eligible_place) {
            next += 1;
        }
    }
    if next != discards.len()
        || discards
            .iter()
            .any(|place| frontier.partial_custody_paths.contains_key(place))
    {
        return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
            machine: function.machine,
            block,
        });
    }
    for place in discards {
        frontier.owned_places.remove(place);
    }
    Ok(())
}
