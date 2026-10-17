//! Stored source coordinates depend on reference bindings independently of payload.
//!
//! A projected scalar is read after its selector, but the collection descriptor
//! was evaluated before that selector. Sharing its canonical slot therefore
//! requires the descriptor prefixes to survive; a write to payload contents
//! alone does not redirect the completed read. Reference construction and
//! copied-value alias publication use this same typed prefix discovery.

use checked_trees::FlowOperandFact;
use facts::PlaceSegment;
use validation::CanonicalPlace;

pub(super) fn borrowed_bindings(
    program: &dyn validation::CheckingInput,
    capture: &FlowOperandFact,
    remaining: &[PlaceSegment],
) -> Option<Vec<CanonicalPlace>> {
    let mut sources = Vec::new();
    validation::expression_traversal::collect_expression_tree(
        program,
        capture.expression,
        &mut sources,
    );
    let (machine, body) = validation::find_state(program, capture.state_symbol)?;
    let produced_windows: Vec<_> = sources
        .iter()
        .filter(|source| {
            validation::has_builtin_subslice_meaning(program, machine, Some(body), **source)
        })
        .filter_map(|source| {
            validation::canonical_place_from_expression_in_state(
                program,
                capture.state_symbol,
                capture.statement_index,
                *source,
            )
        })
        .collect();
    let mut bindings = Vec::new();
    for source in sources {
        let Some(mut place) = validation::canonical_place_from_expression_in_state(
            program,
            capture.state_symbol,
            capture.statement_index,
            source,
        ) else {
            continue;
        };
        // Produced temporaries have no mutable stored binding. Their selected
        // source prefixes and selectors are visited independently by this walk.
        if !place
            .root
            .as_symbol()
            .is_some_and(|symbol| symbol.is_valid())
        {
            continue;
        }
        if source == capture.expression {
            place.extend_segments(remaining);
        }
        // A completed window is a descriptor value, not a replaceable
        // binding at the range-shaped source coordinate. Its original
        // descriptor prefixes are discovered independently. The caller retains
        // selector-value authority separately. Exact coordinate equality covers a borrow
        // of that completed window without exempting further projections.
        if produced_windows
            .iter()
            .any(|window| crate::flow::canonical_places_agree(program, window, &place))
        {
            continue;
        }
        let reference = validation::canonical_place_type_reference(
            program,
            capture.state_symbol,
            capture.statement_index,
            &place,
        )?;
        if crate::flow::operand_binds_live_view(program, reference) && !bindings.contains(&place) {
            bindings.push(place);
        }
    }
    Some(bindings)
}
