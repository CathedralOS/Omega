use super::*;
use crate::flow::{
    canonical_place_from_expression_in_state, expression_type_symbol, symbol_type_symbol,
};

pub(crate) fn canonical_place_to_fact_place_in_state(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<facts::PlaceHandle> {
    let canonical = canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )?;
    Some(append_place_with_segments(
        facts,
        canonical.root,
        &canonical.segments,
    ))
}

pub(crate) fn append_place_segment(
    facts: &mut FactPlan,
    base_place: facts::PlaceHandle,
    segment: facts::PlaceSegment,
) -> facts::PlaceHandle {
    let place = *facts.places.get(base_place);
    let mut segments: Vec<_> = facts.place_segments.span_or_empty(place.segments).to_vec();
    segments.push(segment);
    append_place_with_segments(facts, place.root, &segments)
}

pub(crate) fn resolve_place_member_symbol(
    program: &typed_trees::TypedTrees,
    facts: &FactPlan,
    place: facts::PlaceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let place = facts.places.get(place);
    let base_symbol = fact_place_type_symbol(program, facts, place)?;

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == base_symbol)
        && let Some(attached_data) = machine.attached_data.as_deref()
        && let Some(data) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == attached_data)
    {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == base_symbol)
    {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    None
}

fn fact_place_type_symbol(
    program: &typed_trees::TypedTrees,
    facts: &FactPlan,
    place: &facts::Place,
) -> Option<SymbolHandle> {
    let mut current = match place.root {
        facts::PlaceRoot::Symbol(symbol) => symbol_type_symbol(program, symbol)?,
        facts::PlaceRoot::Expression(expression) => expression_type_symbol(program, expression)?,
        _ => return None,
    };

    for segment in facts.place_segments.span_or_empty(place.segments) {
        match segment {
            facts::PlaceSegment::Case { .. } => {}
            facts::PlaceSegment::Field { symbol } => {
                current = symbol_type_symbol(program, *symbol)?;
            }
            facts::PlaceSegment::FixedIndex { .. }
            | facts::PlaceSegment::FixedRange { .. }
            | facts::PlaceSegment::Index { .. } => {
                return None;
            }
        }
    }

    Some(current)
}

pub(crate) fn append_place_with_segments(
    facts: &mut FactPlan,
    root: facts::PlaceRoot,
    segments: &[facts::PlaceSegment],
) -> facts::PlaceHandle {
    let place = facts.append_place(facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in segments {
        facts.push_place_segment(place, *segment);
    }
    place
}

pub(crate) fn append_place_from_name_path(
    facts: &mut FactPlan,
    path: &NamePath,
) -> facts::PlaceHandle {
    let place = facts.append_symbol_place(path.head_symbol());
    for symbol in path.member_symbols().iter().skip(1) {
        if symbol.is_valid() {
            facts.push_place_segment(place, facts::PlaceSegment::Field { symbol: *symbol });
        }
    }
    place
}
