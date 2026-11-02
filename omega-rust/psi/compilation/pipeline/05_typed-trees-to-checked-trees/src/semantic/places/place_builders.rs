use crate::places::place_at;
use validation::{data_member_symbol_by_name, expression_type_symbol, symbol_type_symbol};

use arena::HandleSpan;
use checked_trees::expression::{ExpressionHandle, NamePath};
use facts::{FactPlan, Place, PlaceHandle, PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;

pub(crate) fn canonical_place_to_fact_place_in_state(
    program: &dyn ::validation::CheckingInput,
    facts: &mut FactPlan,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<PlaceHandle> {
    let canonical = place_at(program, state_symbol, statement_index, expression)?;
    Some(append_place(facts, canonical.root, &canonical.segments))
}

pub(crate) fn append_place_segment(
    facts: &mut FactPlan,
    base_place: PlaceHandle,
    segment: PlaceSegment,
) -> PlaceHandle {
    let place = *facts.places.get(base_place);
    let mut segments: Vec<_> = facts.place_segments.span_or_empty(place.segments).to_vec();
    segments.push(segment);
    append_place(facts, place.root, &segments)
}

pub(crate) fn resolve_place_member_symbol(
    program: &dyn ::validation::CheckingInput,
    facts: &FactPlan,
    place: PlaceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let place = facts.places.get(place);
    let base_symbol = fact_place_type_symbol(program, facts, place)?;

    if let Some(machine) = program.machine_by_symbol(base_symbol)
        && let Some(data) = program.attached_data_definition(machine)
    {
        if let Some(symbol) = data_member_symbol_by_name(program, data, member_name) {
            return Some(symbol);
        }
    }

    if let Some(data) = program.data_definition_by_symbol(base_symbol) {
        if let Some(symbol) = data_member_symbol_by_name(program, data, member_name) {
            return Some(symbol);
        }
    }

    None
}

fn fact_place_type_symbol(
    program: &dyn ::validation::CheckingInput,
    facts: &FactPlan,
    place: &Place,
) -> Option<SymbolHandle> {
    let mut current = match place.root {
        PlaceRoot::Symbol(symbol) => symbol_type_symbol(program, symbol)?,
        PlaceRoot::Expression(expression) => expression_type_symbol(program, expression)?,
        _ => return None,
    };

    for segment in facts.place_segments.span_or_empty(place.segments) {
        match segment {
            PlaceSegment::Case { .. } => {}
            PlaceSegment::Field { symbol } => {
                current = symbol_type_symbol(program, *symbol)?;
            }
            PlaceSegment::FixedIndex { .. }
            | PlaceSegment::FixedRange { .. }
            | PlaceSegment::OpaqueRange
            | PlaceSegment::Index { .. } => {
                return None;
            }
        }
    }

    Some(current)
}

pub(crate) fn append_place(
    facts: &mut FactPlan,
    root: PlaceRoot,
    segments: &[PlaceSegment],
) -> PlaceHandle {
    let place = facts.append_place(Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in segments {
        facts.push_place_segment(place, *segment);
    }
    place
}

pub(crate) fn append_place_from_name_path(facts: &mut FactPlan, path: &NamePath) -> PlaceHandle {
    let place = facts.append_symbol_place(path.head_symbol());
    for symbol in path.member_symbols().iter().skip(1) {
        if symbol.is_valid() {
            facts.push_place_segment(place, PlaceSegment::Field { symbol: *symbol });
        }
    }
    place
}
