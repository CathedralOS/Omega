//! Retain reusable whole entry values independently of scalar proof reads.
//! A real state parameter supplies initialized storage; every selected prefix
//! stays owned, and canonical Copy plus unrestricted multiplicity authorize
//! duplication. Copy records may contain shared descriptors: copying those
//! retains their existing backing and lifetime rather than copying referents.
//! Exclusive descriptors and other dispositions still require their own
//! loan/transfer evidence and cannot acquire this entry-copy receipt.
//! This is source reuse permission, not disposal for a materialized carrier:
//! the call's access mode separately decides whether its operand is only lent.

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
    // Typed projection and bound receipts are retained on this whole root,
    // independently of the scalar leaves demanded by the declaration.
    let geometry = validation::structural_coordinate(
        program,
        program.state_parameters(state),
        coordinate.parameter,
        path,
    )?;
    let bounds = admitted.entry_indexes.span(coordinate.entry_indexes)?;
    for (path_step, step) in geometry.projections.iter().enumerate() {
        let validation::ScalarCoordinateProjection::Element {
            selector,
            collection_type,
            element_type,
            collection_path,
            extent,
            ..
        } = step
        else {
            continue;
        };
        let extent = extent.map(u64::try_from).transpose().ok()?;
        let mut matching = bounds.iter().filter(|bound| {
            bound.state == state.symbol
                && bound.parameter == coordinate.parameter
                && usize::try_from(bound.path_step).ok() == Some(path_step)
                && bound.selector == *selector
                && bound.collection_type == *collection_type
                && bound.element_type == *element_type
                && bound.collection_path == *collection_path
                && bound.extent == extent
        });
        matching.next()?;
        if matching.next().is_some() {
            return None;
        }
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
    {
        return None;
    }
    Some(checked_trees::CheckedRankApplicationOwnedCopy {
        formal,
        actual,
        actual_type,
    })
}
