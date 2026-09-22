use facts::{Fact, FactPayload, FactPlan};

use super::names::symbol_name;

pub(crate) fn borrow_access_label(
    program: &typed_trees::TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    access: &checked_trees::BorrowArgumentAccessFact,
) -> String {
    facts::canonical_place_label_from_parts(
        program,
        facts::PlaceRoot::Symbol(access.root_symbol),
        borrow.access_segments(access),
    )
}

pub(crate) fn semantic_fact_requirement_label(
    program: &typed_trees::TypedTrees,
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
                facts::FactPlace::Place(place) => place,
                _ => return "unknown domain membership".to_owned(),
            };
            let domain = if program.domain_definitions().iter().any(|domain| {
                domain.symbol == domain_symbol
                    && !typed_trees::domain::index_parameters(program, domain).is_empty()
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
                facts::FactPlace::Place(place) => place,
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
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    fact: &Fact,
) -> Option<String> {
    semantic.boolean_fact_label(program, fact)
}

pub(crate) fn joined_place_label(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &facts::Place,
    extra_segments: &[facts::PlaceSegment],
) -> String {
    let mut segments: Vec<_> = semantic
        .place_segments
        .span_or_empty(place.segments)
        .to_vec();
    segments.extend(extra_segments.iter().copied());
    facts::canonical_place_label_from_parts(program, place.root, &segments)
}
