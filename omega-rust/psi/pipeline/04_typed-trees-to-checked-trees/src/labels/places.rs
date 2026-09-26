use crate::fact_plan::{Fact, FactPayload, FactPlan};

use super::names::symbol_name;

pub(crate) fn borrow_access_label(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    borrow: &crate::checked_trees::BorrowFacts,
    access: &crate::checked_trees::BorrowArgumentAccessFact,
) -> String {
    crate::fact_plan::canonical_place_label_from_parts(
        program,
        crate::fact_plan::PlaceRoot::Symbol(access.root_symbol),
        borrow.access_segments(access),
    )
}

pub(crate) fn semantic_fact_requirement_label(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &FactPlan,
    fact: &Fact,
) -> String {
    match fact.payload {
        FactPayload::ContractDomainMembership {
            domain_symbol,
            semantic_domain,
            ..
        }
        | FactPayload::DomainMembership {
            domain_symbol,
            semantic_domain,
            ..
        } => {
            let place = match fact.place {
                crate::fact_plan::FactPlace::Place(place) => place,
                _ => return "unknown domain membership".to_owned(),
            };
            let domain = if program.domain_definitions().iter().any(|domain| {
                domain.symbol == domain_symbol
                    && !symbol_resolved_trees_to_typed_trees::typed_trees::domain::index_parameters(
                        program, domain,
                    )
                    .is_empty()
            }) {
                program
                    .semantic_domains
                    .name(semantic_domain)
                    .map(str::to_owned)
                    .unwrap_or_else(|| symbol_name(program, domain_symbol))
            } else {
                symbol_name(program, domain_symbol)
            };
            format!("{} in {}", semantic.place_label(program, place), domain)
        }
        FactPayload::ContractCarryPermission { permission, .. }
        | FactPayload::CarryPermission { permission, .. } => {
            let place = match fact.place {
                crate::fact_plan::FactPlace::Place(place) => place,
                _ => return "unknown carry permission".to_owned(),
            };
            format!(
                "{} in {}",
                semantic.place_label(program, place),
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &FactPlan,
    fact: &Fact,
) -> Option<String> {
    semantic.boolean_fact_label(program, fact)
}

pub(crate) fn joined_place_label(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &crate::fact_plan::Place,
    extra_segments: &[crate::fact_plan::PlaceSegment],
) -> String {
    let mut segments: Vec<_> = semantic
        .place_segments
        .span_or_empty(place.segments)
        .to_vec();
    segments.extend(extra_segments.iter().copied());
    crate::fact_plan::canonical_place_label_from_parts(program, place.root, &segments)
}
