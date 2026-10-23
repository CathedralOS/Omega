//! Owned value transfers preserve existing qualifications and IEEE predicates.
//! No declaration, equal carrier geometry, or matching label creates a fact.
//! Reference copies do not copy referent predicates. Call facts remain provisional
//! until the call/custody checker validates their exact producing invocation.

use arena::HandleSpan;
use checked_trees::FlowSemanticContextRef;
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceHandle, ProgramPoint};

pub(in crate::flow) fn append_owned_qualification_transfer(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    contexts: &crate::flow::FlowBuildContext,
    active: HandleSpan<FlowSemanticContextRef>,
    source_expression: typed_trees::expression::ExpressionHandle,
    source: PlaceHandle,
    destination: PlaceHandle,
    destination_type: typed_trees::types::TypeReferenceHandle,
    point: ProgramPoint,
    references: &mut HandleSpan<facts::FactRef>,
) {
    let ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        ..
    } = point
    else {
        return;
    };
    let Some((machine, state)) =
        validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
    else {
        return;
    };
    if !validation::value_has_builtin_coordinates(program, machine, state, source_expression) {
        return;
    }
    let source = *semantic.places.get(source);
    let destination = *semantic.places.get(destination);
    let source_segments = semantic
        .place_segments
        .span_or_empty(source.segments)
        .to_vec();
    let destination_segments = semantic
        .place_segments
        .span_or_empty(destination.segments)
        .to_vec();
    if !crate::flow::source_transfer_cases_selected(
        program,
        semantic,
        contexts,
        active,
        point,
        source.root,
        source_segments.clone(),
    ) {
        return;
    }
    let candidates = crate::flow::active_context_facts(contexts, semantic, active);
    for fact in candidates {
        let payload = if let Some((_, domain, domain_symbol, semantic_domain)) =
            fact.payload.domain_membership()
        {
            if !semantic_domain.is_valid()
                && validation::field_domains::domain_requires_provenance(program, domain_symbol)
            {
                continue;
            }
            FactPayload::DomainMembership {
                value: typed_trees::expression::ExpressionHandle::invalid(),
                domain,
                domain_symbol,
                semantic_domain,
            }
        } else if matches!(fact.payload, FactPayload::FloatPredicate { .. }) {
            fact.payload
        } else {
            continue;
        };
        let Some(place) = fact.place.as_place() else {
            continue;
        };
        let place = semantic.places.get(place);
        let segments = semantic.place_segments.span_or_empty(place.segments);
        // An independently live whole-element extent qualifies whichever
        // element this captured builtin read selects. A single candidate or
        // unresolved selector never supplies that universal premise.
        if place.root != source.root
            || segments.len() < source_segments.len()
            || !crate::flow::segments_cover_subject(
                program,
                &segments[..source_segments.len()],
                &source_segments,
            )
        {
            continue;
        }
        let relative_path = &segments[source_segments.len()..];
        // A copied reference is not a copy of its referent's qualifications.
        // Those remain tied to the independently checked reference origins;
        // freezing them under the new slot would survive source invalidation.
        if !(0..=relative_path.len()).all(|length| {
            let Some(mut reference) = validation::project_type_reference_from_segments(
                program,
                destination_type,
                &relative_path[..length],
            ) else {
                return false;
            };
            while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
                program.type_reference_table.type_reference(reference)
            {
                reference = *base_type;
            }
            !matches!(
                program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Reference { .. }
            )
        }) {
            continue;
        }
        if !relative_path.iter().chain(&destination_segments).all(|segment| {
            matches!(segment,
            facts::PlaceSegment::Field { symbol } if symbol.is_valid())
                || matches!(segment, facts::PlaceSegment::Case { variant } if variant.is_valid())
                || matches!(segment, facts::PlaceSegment::FixedIndex { .. })
        }) {
            continue;
        }
        let mut rebased = destination_segments.clone();
        rebased.extend_from_slice(relative_path);
        let place = crate::semantic::places::append_place(semantic, destination.root, &rebased);
        // The exact-place lane may already have moved this same fact and
        // recorded its qualification correspondence. Do not duplicate it
        // when the structural traversal has an empty relative path.
        if semantic
            .refs
            .span_or_empty(*references)
            .iter()
            .any(|reference| {
                let prior = semantic.facts.get(reference.fact);
                prior.evidence == fact.evidence
                    && matches!(prior.place, FactPlace::Place(prior_place)
                    if semantic.places_equal(prior_place, place))
                    && match (prior.payload, payload) {
                        (
                            FactPayload::FloatPredicate { predicate: left },
                            FactPayload::FloatPredicate { predicate: right },
                        ) => left == right,
                        (prior, transferred) => prior
                            .domain_membership()
                            .zip(transferred.domain_membership())
                            .is_some_and(
                                |(
                                    (_, _, left_symbol, left_identity),
                                    (_, _, right_symbol, right_identity),
                                )| {
                                    left_symbol == right_symbol && left_identity == right_identity
                                },
                            ),
                    }
            })
        {
            continue;
        }
        let transferred = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: fact.evidence,
            payload,
        });
        semantic.append_ref(references, transferred);
    }
}
