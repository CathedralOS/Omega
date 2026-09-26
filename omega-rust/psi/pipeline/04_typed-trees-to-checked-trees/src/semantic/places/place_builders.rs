use crate::checked_trees::expression::{ExpressionHandle, NamePath};
use crate::fact_plan::FactPlan;
use crate::flow::{
    canonical_place_from_expression_in_state, expression_type_symbol, symbol_type_symbol,
};
use arena::HandleSpan;
use symbols::SymbolHandle;

pub(crate) fn canonical_place_to_fact_place_in_state(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &mut FactPlan,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<crate::fact_plan::PlaceHandle> {
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
    base_place: crate::fact_plan::PlaceHandle,
    segment: crate::fact_plan::PlaceSegment,
) -> crate::fact_plan::PlaceHandle {
    let place = *facts.places.get(base_place);
    let mut segments: Vec<_> = facts.place_segments.span_or_empty(place.segments).to_vec();
    segments.push(segment);
    append_place_with_segments(facts, place.root, &segments)
}

pub(crate) fn resolve_place_member_symbol(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &FactPlan,
    place: crate::fact_plan::PlaceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let place = facts.places.get(place);
    let base_symbol = fact_place_type_symbol(program, facts, place)?;

    if let Some(machine) = crate::lookup::machine_by_symbol(program, base_symbol)
        && let Some(data) = program.attached_data_definition(machine)
    {
        for member in program.data_members(data) {
            match member {
                symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Field(
                    field,
                ) if field.name.as_str() == member_name => {
                    return Some(field.symbol);
                }
                symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Variant(
                    variant,
                ) if variant.name.as_str() == member_name => {
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
                symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Field(
                    field,
                ) if field.name.as_str() == member_name => {
                    return Some(field.symbol);
                }
                symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember::Variant(
                    variant,
                ) if variant.name.as_str() == member_name => {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    None
}

fn fact_place_type_symbol(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &FactPlan,
    place: &crate::fact_plan::Place,
) -> Option<SymbolHandle> {
    let mut current = match place.root {
        crate::fact_plan::PlaceRoot::Symbol(symbol) => symbol_type_symbol(program, symbol)?,
        crate::fact_plan::PlaceRoot::Expression(expression) => {
            expression_type_symbol(program, expression)?
        }
        _ => return None,
    };

    for segment in facts.place_segments.span_or_empty(place.segments) {
        match segment {
            crate::fact_plan::PlaceSegment::Case { .. } => {}
            crate::fact_plan::PlaceSegment::Field { symbol } => {
                current = symbol_type_symbol(program, *symbol)?;
            }
            crate::fact_plan::PlaceSegment::FixedIndex { .. }
            | crate::fact_plan::PlaceSegment::FixedRange { .. }
            | crate::fact_plan::PlaceSegment::Index { .. } => {
                return None;
            }
        }
    }

    Some(current)
}

pub(crate) fn append_place_with_segments(
    facts: &mut FactPlan,
    root: crate::fact_plan::PlaceRoot,
    segments: &[crate::fact_plan::PlaceSegment],
) -> crate::fact_plan::PlaceHandle {
    let place = facts.append_place(crate::fact_plan::Place {
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
) -> crate::fact_plan::PlaceHandle {
    let place = facts.append_symbol_place(path.head_symbol());
    for symbol in path.member_symbols().iter().skip(1) {
        if symbol.is_valid() {
            facts.push_place_segment(
                place,
                crate::fact_plan::PlaceSegment::Field { symbol: *symbol },
            );
        }
    }
    place
}
