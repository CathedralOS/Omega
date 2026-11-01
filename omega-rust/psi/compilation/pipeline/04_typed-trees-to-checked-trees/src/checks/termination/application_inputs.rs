//! Retain reusable whole entry values independently of scalar proof reads.
//! A real state parameter supplies initialized storage; every selected prefix
//! stays owned, and canonical Copy plus unrestricted multiplicity authorize
//! duplication. Other dispositions require their own loan/transfer evidence.

pub(super) fn owned_copy(
    program: &dyn validation::CheckingInput,
    admitted: &checked_trees::CheckedRankArrivalPlan,
    state: &typed_trees::state::State,
    formal: symbols::SymbolHandle,
    actual: checked_trees::CheckedRankCoordinateHandle,
) -> Option<checked_trees::CheckedRankApplicationOwnedCopy> {
    admitted.coordinates.is_valid(actual).then_some(())?;
    let coordinate = admitted.coordinates.get(actual);
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == coordinate.parameter)?;
    if parameter.is_const || parameter.relevance.is_erased() {
        return None;
    }
    let path = admitted.path_segments.span(coordinate.path)?;
    // A whole indexed root needs its own retained selector observation. The
    // scalar leaf roster cannot authorize re-evaluating that selector here.
    if path
        .iter()
        .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
    {
        return None;
    }
    for length in 0..=path.len() {
        let selected = validation::project_type_reference_from_segments(
            program,
            parameter.type_reference,
            &path[..length],
        )?;
        if validation::reference_shell(program, selected).is_some() {
            return None;
        }
    }
    let actual_type =
        validation::project_type_reference_from_segments(program, parameter.type_reference, path)?;
    if program.primitive_type_reference(actual_type).is_some()
        || !validation::reference_result_custody::source_value_is_copied(program, actual_type)
        || validation::reference_result_custody::result_multiplicity(program, actual_type)
            != language_semantics::Multiplicity::Unrestricted
        || !validation::has_contents(
            program,
            actual_type,
            &[],
            validation::ContentsRequirement::BorrowFree,
        )
    {
        return None;
    }
    Some(checked_trees::CheckedRankApplicationOwnedCopy {
        formal,
        actual,
        actual_type,
    })
}
