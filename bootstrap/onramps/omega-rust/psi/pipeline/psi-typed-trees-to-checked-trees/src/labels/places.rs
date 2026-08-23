use psi_facts::{Fact, FactPayload, FactPlan};

use super::names::symbol_name;

pub(crate) fn borrow_access_label(
    program: &psi_typed_trees::TypedTrees,
    borrow: &psi_checked_trees::BorrowFacts,
    access: &psi_checked_trees::BorrowArgumentAccessFact,
) -> String {
    canonical_place_label_from_parts(
        program,
        psi_facts::PlaceRoot::Symbol(access.root_symbol),
        borrow.access_segments(access),
    )
}

pub(crate) fn semantic_fact_requirement_label(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    fact: &Fact,
) -> String {
    match fact.payload {
        FactPayload::ContractDomainMembership { domain_symbol, .. }
        | FactPayload::DomainMembership { domain_symbol, .. } => {
            let place = match fact.place {
                psi_facts::FactPlace::Place(place) => place,
                _ => return "unknown domain membership".to_owned(),
            };
            let place = semantic.places.get(place);
            format!(
                "{} in {}",
                requirement_place_label(program, semantic, place),
                symbol_name(program, domain_symbol)
            )
        }
        FactPayload::ContractCarryPermission { permission, .. }
        | FactPayload::CarryPermission { permission, .. } => {
            let place = match fact.place {
                psi_facts::FactPlace::Place(place) => place,
                _ => return "unknown carry permission".to_owned(),
            };
            let place = semantic.places.get(place);
            format!(
                "{} in {}",
                requirement_place_label(program, semantic, place),
                permission.name()
            )
        }
        FactPayload::ContractBooleanExpression { .. } | FactPayload::BooleanExpression(_) => {
            semantic_boolean_fact_label(program, semantic, fact)
                .unwrap_or_else(|| "unknown boolean expression".to_owned())
        }
        FactPayload::ContractPropositionApplication { .. }
        | FactPayload::PropositionApplication { .. } => semantic
            .proposition_fact_label(program, fact)
            .unwrap_or_else(|| "unknown proposition application".to_owned()),
        _ => "unknown contract fact".to_owned(),
    }
}

/// Canonical caller-term label for a semantic boolean fact. Declaration facts
/// render their typed expression directly; flow-instantiated operator/call
/// facts use the substitution record owned by the fact plan.
pub(crate) fn semantic_boolean_fact_label(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    fact: &Fact,
) -> Option<String> {
    semantic.boolean_fact_label(program, fact)
}

pub(crate) fn requirement_place_label(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &psi_facts::Place,
) -> String {
    canonical_place_label(program, semantic, place)
}

pub(crate) fn canonical_place_label(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &psi_facts::Place,
) -> String {
    canonical_place_label_from_parts(
        program,
        place.root,
        semantic.place_segments.span_or_empty(place.segments),
    )
}

pub(crate) fn joined_place_label(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &psi_facts::Place,
    extra_segments: &[psi_facts::PlaceSegment],
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
    program: &psi_typed_trees::TypedTrees,
    root: psi_facts::PlaceRoot,
    segments: &[psi_facts::PlaceSegment],
) -> String {
    let mut label = match root {
        psi_facts::PlaceRoot::Unknown => "unknown".to_owned(),
        psi_facts::PlaceRoot::Symbol(symbol) => symbol_name(program, symbol),
        psi_facts::PlaceRoot::Expression(expression) => {
            program.expression_table.display_name(expression)
        }
        psi_facts::PlaceRoot::TypeReference(type_reference) => {
            program.display_type_reference(type_reference)
        }
    };

    for segment in segments {
        match segment {
            psi_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_name(program, *symbol));
            }
            psi_facts::PlaceSegment::Case { variant } => {
                label.push_str("::");
                label.push_str(&symbol_name(program, *variant));
            }
            psi_facts::PlaceSegment::FixedIndex { index } => {
                label.push('[');
                label.push_str(&index.to_string());
                label.push(']');
            }
            psi_facts::PlaceSegment::FixedRange { start, end } => {
                label.push('[');
                label.push_str(&start.to_string());
                label.push_str("..");
                label.push_str(&end.to_string());
                label.push(']');
            }
            psi_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&program.expression_table.display_name(*expression));
                label.push(']');
            }
        }
    }

    label
}
