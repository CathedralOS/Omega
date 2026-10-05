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
    let source = origin.source.as_ref()?;
    if source.collection_coarse
        && let Some(place) = structural_origin(program, state, statement_index, source)
    {
        return Some(place);
    }
    place_from_origin_path(program, state, statement_index, &source.path)
        .map(|place| (place, !source.collection_coarse))
}

fn structural_origin(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    source: &validation::LocalWriteSource,
) -> Option<(CanonicalPlace, bool)> {
    let mut place = canonical_place_from_symbol(source.root)?;
    canonical_place_type_reference(program, state.symbol, statement_index, &place)?;
    for segment in &source.segments {
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
