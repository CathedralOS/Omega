//! Whole-place assignment transports live predicates on its contained fields.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_copied_field_predicates(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    contexts: &FlowBuildContext,
    active: HandleSpan<FlowSemanticContextRef>,
    source: PlaceHandle,
    destination: PlaceHandle,
    point: ProgramPoint,
    references: &mut HandleSpan<facts::FactRef>,
) {
    let source_place = *semantic.places.get(source);
    let destination_place = *semantic.places.get(destination);
    if !matches!(source_place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
        || !matches!(destination_place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
    {
        return;
    }
    let source_segments = semantic
        .place_segments
        .span_or_empty(source_place.segments)
        .to_vec();
    let destination_segments = semantic
        .place_segments
        .span_or_empty(destination_place.segments)
        .to_vec();
    if !source_segments
        .iter()
        .chain(&destination_segments)
        .all(stable_segment)
    {
        return;
    }
    let facts: Vec<_> = contexts
        .contexts
        .semantic_context_refs
        .span_or_empty(active)
        .iter()
        .flat_map(|reference| {
            semantic
                .refs
                .span_or_empty(semantic.contexts.get(reference.context).facts)
        })
        .map(|reference| *semantic.facts.get(reference.fact))
        .collect();
    for fact in facts {
        let (domain, domain_symbol) = match fact.payload {
            FactPayload::DomainMembership {
                domain,
                domain_symbol,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain,
                domain_symbol,
                ..
            } => (domain, domain_symbol),
            _ => continue,
        };
        // A field predicate follows the copied value. Routed qualifications
        // require their own custody correspondence, not this predicate rule.
        if !program.domain_definitions().iter().any(|definition| {
            definition.symbol == domain_symbol
                && definition.establishment_routes.is_empty()
                && definition.alias.is_none()
                && definition.predicate_body.is_present()
        }) {
            continue;
        }
        let FactPlace::Place(place) = fact.place else {
            continue;
        };
        let place = semantic.places.get(place);
        let segments = semantic.place_segments.span_or_empty(place.segments);
        if place.root != source_place.root
            || segments.len() <= source_segments.len()
            || !segments.starts_with(&source_segments)
            || !segments.iter().all(stable_segment)
        {
            continue;
        }
        let suffix = segments[source_segments.len()..].to_vec();
        let copied_place = semantic.append_place(facts::Place {
            root: destination_place.root,
            segments: HandleSpan::empty(),
        });
        for segment in destination_segments.iter().chain(&suffix) {
            semantic.push_place_segment(copied_place, *segment);
        }
        let copied_fact = semantic.append_fact(Fact {
            place: FactPlace::Place(copied_place),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: fact.evidence,
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain,
                domain_symbol,
            },
        });
        semantic.append_ref(references, copied_fact);
    }
}

fn stable_segment(segment: &facts::PlaceSegment) -> bool {
    match segment {
        facts::PlaceSegment::Field { symbol } => symbol.is_valid(),
        facts::PlaceSegment::Case { variant } => variant.is_valid(),
        facts::PlaceSegment::FixedIndex { .. } => true,
        _ => false,
    }
}
