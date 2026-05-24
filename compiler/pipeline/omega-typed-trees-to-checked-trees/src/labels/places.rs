use omega_facts::{Fact, FactPayload, FactPlan};

use super::names::symbol_name;

pub(crate) fn semantic_fact_requirement_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    fact: &Fact,
) -> String {
    match fact.payload {
        FactPayload::ContractDomainMembership { domain_symbol, .. }
        | FactPayload::DomainMembership { domain_symbol, .. } => {
            let place = match fact.place {
                omega_facts::FactPlace::Place(place) => place,
                _ => return "unknown domain membership".to_owned(),
            };
            let place = semantic.places.get(place);
            format!(
                "{} in {}",
                requirement_place_label(program, semantic, place),
                symbol_name(program, domain_symbol)
            )
        }
        FactPayload::ContractBooleanExpression { expression, .. }
        | FactPayload::BooleanExpression(expression) => {
            program.expression_table.display_name(expression)
        }
        _ => "unknown contract fact".to_owned(),
    }
}

pub(crate) fn requirement_place_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
) -> String {
    canonical_place_label(program, semantic, place)
}

pub(crate) fn canonical_place_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
) -> String {
    canonical_place_label_from_parts(
        program,
        place.root,
        semantic.place_segments.span_or_empty(place.segments),
    )
}

pub(crate) fn joined_place_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
    extra_segments: &[omega_facts::PlaceSegment],
) -> String {
    let mut segments: Vec<_> = semantic
        .place_segments
        .span_or_empty(place.segments)
        .iter()
        .copied()
        .collect();
    segments.extend(extra_segments.iter().copied());
    canonical_place_label_from_parts(program, place.root, &segments)
}

pub(crate) fn canonical_place_label_from_parts(
    program: &omega_typed_trees::TypedTrees,
    root: omega_facts::PlaceRoot,
    segments: &[omega_facts::PlaceSegment],
) -> String {
    let mut label = match root {
        omega_facts::PlaceRoot::Unknown => "unknown".to_owned(),
        omega_facts::PlaceRoot::Symbol(symbol) => symbol_name(program, symbol),
        omega_facts::PlaceRoot::Expression(expression) => {
            program.expression_table.display_name(expression)
        }
        omega_facts::PlaceRoot::TypeReference(type_reference) => {
            program.display_type_reference(type_reference)
        }
    };

    for segment in segments {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_name(program, *symbol));
            }
            omega_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&program.expression_table.display_name(*expression));
                label.push(']');
            }
        }
    }

    label
}
