//! Transport frozen structural origins without replaying selector expressions.

use validation::CanonicalPlace;
use validation::canonical_place_type_reference;

use crate::flow::canonical_place_from_symbol;

use crate::flow::mutation::local_origins::place_from_origin_path;
use crate::flow::place_segment_has_unresolved_identity;

/// The boolean reports whether every source segment survived the projection;
/// an unresolved runtime index truncates the place, and only an exact result
/// may feed value transport such as returned-reference evidence re-anchoring.
pub(crate) fn origin_place(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
            _ => *segment,
        };
        place.segments.push(segment);
        crate::flow::compose_captured_view_segments(&mut place.segments)?;
        origin_type_reference(program, state, statement_index, &place)?;
    }
    Some((place, true))
}

/// A frozen window is an exact descriptor origin, not an unknown element.
/// It retains its collection dimension; the type query below checks the
/// underlying carrier without pretending the window selected one child.
fn origin_type_reference(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    place: &CanonicalPlace,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let mut carrier = place.clone();
    if let Some(facts::PlaceSegment::FixedRange { start, end }) = carrier.segments.last().copied() {
        if start > end {
            return None;
        }
        carrier.segments.pop();
        let reference =
            canonical_place_type_reference(program, state.symbol, statement_index, &carrier)?;
        validation::collection_element_type_reference(program, reference)?;
        Some(reference)
    } else {
        canonical_place_type_reference(program, state.symbol, statement_index, place)
    }
}

/// The sole whole-local write origin of `local_symbol` before `statement`,
/// restricted to referents rooted outside the local's own storage. The range
/// referent lookup and the projected-binding walk share this admission.
pub(crate) fn sole_local_referent_origin(
    resolver: &validation::CallFrameResolver<'_>,
    machine: &typed_trees::machine::Machine,
    statement: &typed_trees::statement::StatementNode,
    local_symbol: symbols::SymbolHandle,
) -> Option<validation::LocalWriteOrigin> {
    let mut referents = resolver
        .local_write_origins_before_statement(machine, statement)?
        .into_iter()
        .filter(|origin| {
            origin.local_symbol == local_symbol
                && origin.local_segments.is_empty()
                && origin
                    .source
                    .as_ref()
                    .is_none_or(|source| source.root != local_symbol)
        });
    let origin = referents.next()?;
    (referents.next().is_none()).then_some(origin)
}
