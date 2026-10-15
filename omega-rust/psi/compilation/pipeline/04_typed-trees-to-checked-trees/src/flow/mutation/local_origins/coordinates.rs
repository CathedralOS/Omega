//! Transport frozen structural origins without replaying selector expressions.

use validation::CanonicalPlace;
use validation::canonical_place_type_reference;

use crate::flow::canonical_place_from_symbol;

use crate::flow::mutation::local_origins::place_from_origin_path;
use crate::flow::place_segment_has_unresolved_identity;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OriginPrecision {
    Exact,
    /// Every structural dimension is retained, but an element is unknown.
    MaySelect,
    /// Only an enclosing region survived. Descendant projection is unsound.
    Enclosing,
}

/// Precision separates complete structural projection from exact identity.
/// A runtime element selector retains its known field suffix for conservative
/// overlap, but cannot feed value transport or
/// returned-reference evidence re-anchoring as a current selector value.
pub(crate) fn origin_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    origin: &validation::LocalWriteOrigin,
) -> Option<(CanonicalPlace, OriginPrecision)> {
    let source = origin.source.as_ref()?;
    if source.collection_coarse
        && let Some(place) = structural_origin(program, state, statement_index, source)
    {
        return Some(place);
    }
    place_from_origin_path(program, state, statement_index, &source.path).map(|place| {
        (
            place,
            if source.collection_coarse {
                OriginPrecision::Enclosing
            } else {
                OriginPrecision::Exact
            },
        )
    })
}

fn structural_origin(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    source: &validation::LocalWriteSource,
) -> Option<(CanonicalPlace, OriginPrecision)> {
    let mut place = canonical_place_from_symbol(source.root)?;
    canonical_place_type_reference(program, state.symbol, statement_index, &place)?;
    let mut exact = true;
    for segment in &source.segments {
        if place_segment_has_unresolved_identity(*segment) {
            return None;
        }
        if let facts::PlaceSegment::Index { expression } = segment
            && (!program.expression_table.expression_is_valid(*expression)
                || program.expression_table.range_node(*expression).is_some())
        {
            // A window retains the collection dimension; an erased selector
            // does not establish whether it was a window or one element.
            // Stop before that coordinate rather than shifting later fields
            // or indexes onto the wrong dimension of the backing storage.
            return Some((place, OriginPrecision::Enclosing));
        }
        // Overlap treats a runtime selector as any element, independently of
        // its later value. Keep the known descendant fields: an unknown row
        // does not make a write to its second field touch its first field.
        // Exact consumers must still refuse this captured selector identity.
        exact &= !matches!(segment, facts::PlaceSegment::Index { .. });
        place.segments.push(*segment);
        crate::flow::compose_captured_view_segments(&mut place.segments)?;
        origin_type_reference(program, state, statement_index, &place)?;
    }
    Some((
        place,
        if exact {
            OriginPrecision::Exact
        } else {
            OriginPrecision::MaySelect
        },
    ))
}

/// Value and descriptor equality require the original resolved namespace.
/// Conservative write paths may name enclosing bytes, but a recast can change
/// element count and field interpretation without changing their address.
pub(super) fn exact_origin_place(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    origin: &validation::LocalWriteOrigin,
) -> Option<CanonicalPlace> {
    let source = origin.source.as_ref()?;
    if !source.recasts.is_empty() || source.representation.is_some() {
        return None;
    }
    let (place, precision) = structural_origin(program, state, statement_index, source)?;
    (precision == OriginPrecision::Exact).then_some(place)
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
