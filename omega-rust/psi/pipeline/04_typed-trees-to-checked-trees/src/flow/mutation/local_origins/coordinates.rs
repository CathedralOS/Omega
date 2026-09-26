//! Transport frozen structural origins without replaying selector expressions.
use crate::flow::CanonicalPlace;
use crate::flow::canonical_place_from_symbol;
use crate::flow::canonical_place_type_reference;
use crate::flow::mutation::local_origins::place_from_origin_path;
use crate::flow::place_segment_has_unresolved_identity;

/// The boolean reports whether every source segment survived the projection;
/// an unresolved runtime index truncates the place, and only an exact result
/// may feed value transport such as returned-reference evidence re-anchoring.
pub(crate) fn origin_place(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    origin: &validation::LocalWriteOrigin,
) -> Option<(CanonicalPlace, bool)> {
    if origin.collection_coarse
        && let Some(place) = structural_origin(program, state, statement_index, origin)
    {
        return Some(place);
    }
    place_from_origin_path(program, state, statement_index, &origin.source_path)
        .map(|place| (place, !origin.collection_coarse))
}

fn structural_origin(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    origin: &validation::LocalWriteOrigin,
) -> Option<(CanonicalPlace, bool)> {
    let mut place = canonical_place_from_symbol(origin.source_root)?;
    // Existing storage-frame consumers retain the authored self parameter,
    // not the machine's display name, as their caller namespace.
    if crate::semantic::calls::find_state_with_machine(program, state.symbol)
        .is_some_and(|(machine, _)| machine.symbol == origin.source_root)
        && let Some(receiver) = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
    {
        place.root = facts::PlaceRoot::Symbol(receiver.symbol);
    }
    canonical_place_type_reference(program, state.symbol, statement_index, &place)?;
    for segment in &origin.source_segments {
        if place_segment_has_unresolved_identity(*segment) {
            return None;
        }
        // A runtime selector was evaluated at binding formation. Its current
        // expression value cannot narrow a later write through that binding.
        let segment = match segment {
            facts::PlaceSegment::Index { .. } => return Some((place, false)),
            facts::PlaceSegment::FixedRange { .. } => return None,
            _ => *segment,
        };
        place.segments.push(segment);
        canonical_place_type_reference(program, state.symbol, statement_index, &place)?;
    }
    canonical_place_type_reference(program, state.symbol, statement_index, &place)?;
    Some((place, true))
}
