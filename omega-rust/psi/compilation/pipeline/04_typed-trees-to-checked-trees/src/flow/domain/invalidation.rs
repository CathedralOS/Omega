use validation::CanonicalPlace;

use crate::flow::append_place_segments;
use crate::flow::reference_spans;
use checked_trees::{
    DomainFacts, FlowInvalidationFact, FlowInvalidationSource, FlowSemanticContextRef,
};
use facts::{FactPayload, FactPlan};
mod matching;

pub(in crate::flow) use self::matching::matching_mutation_for_fact_place;

/// Query the same storage/dependent-domain invalidation law without publishing
/// a second flow trace. Cleanup selections carry existing context identities.
pub(crate) fn context_survives_place_mutations(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    domain_dependencies: &DomainFacts,
    context: facts::FactContextHandle,
    mutated_places: &[CanonicalPlace],
) -> bool {
    if mutated_places.is_empty() {
        return true;
    }
    visit_context_invalidations(
        semantic,
        context,
        &|fact, place| {
            matching_mutation_for_fact_place(
                program,
                semantic,
                domain_dependencies,
                fact,
                place,
                mutated_places,
            )
        },
        &mut |_, _, _| {},
    )
}

fn visit_context_invalidations<'mutation, 'dependency>(
    semantic: &FactPlan,
    context: facts::FactContextHandle,
    matching_mutation: &impl Fn(
        &facts::Fact,
        facts::PlaceHandle,
    ) -> Option<(
        &'mutation CanonicalPlace,
        &'dependency [facts::PlaceSegment],
    )>,
    invalidated: &mut impl FnMut(facts::FactHandle, &CanonicalPlace, &[facts::PlaceSegment]),
) -> bool {
    let mut invalidated_any = false;
    for fact_ref in semantic
        .refs
        .span_or_empty(semantic.contexts.get(context).facts)
    {
        let fact = semantic.facts.get(fact_ref.fact);
        let Some(place) = fact.place.as_place() else {
            if matches!(fact.payload, FactPayload::StorageDependency { .. }) {
                invalidated_any = true;
            }
            continue;
        };
        if let Some((mutation, dependencies)) = matching_mutation(fact, place) {
            invalidated_any = true;
            invalidated(fact_ref.fact, mutation, dependencies);
        }
    }
    !invalidated_any
}

pub(crate) fn filter_contexts_after_place_mutations(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    domain_dependencies: &DomainFacts,
    semantic_context_refs: &mut arena::PagedArenaSet<FlowSemanticContextRef>,
    invalidation_segments: &mut arena::PagedArenaSet<facts::PlaceSegment>,
    invalidations: &mut arena::PagedArenaSet<FlowInvalidationFact>,
    source: arena::HandleSpan<FlowSemanticContextRef>,
    mutated_places: &[CanonicalPlace],
    invalidation_source: FlowInvalidationSource,
) -> arena::HandleSpan<FlowSemanticContextRef> {
    if mutated_places.is_empty() {
        return source;
    }
    filter_contexts_after_mutations(
        semantic,
        semantic_context_refs,
        invalidation_segments,
        invalidations,
        source,
        invalidation_source,
        |fact, place| {
            matching_mutation_for_fact_place(
                program,
                semantic,
                domain_dependencies,
                fact,
                place,
                mutated_places,
            )
        },
    )
}

/// The observation owner supplies the matching footprint; publication remains
/// one law for declared storage and separately authorized content.
pub(in crate::flow) fn filter_contexts_after_mutations<'mutation, 'dependency>(
    semantic: &FactPlan,
    semantic_context_refs: &mut arena::PagedArenaSet<FlowSemanticContextRef>,
    invalidation_segments: &mut arena::PagedArenaSet<facts::PlaceSegment>,
    invalidations: &mut arena::PagedArenaSet<FlowInvalidationFact>,
    source: arena::HandleSpan<FlowSemanticContextRef>,
    invalidation_source: FlowInvalidationSource,
    matching_mutation: impl Fn(
        &facts::Fact,
        facts::PlaceHandle,
    ) -> Option<(
        &'mutation CanonicalPlace,
        &'dependency [facts::PlaceSegment],
    )>,
) -> arena::HandleSpan<FlowSemanticContextRef> {
    reference_spans::filter_flow_references(semantic_context_refs, source, |context_ref| {
        visit_context_invalidations(
            semantic,
            context_ref.context,
            &matching_mutation,
            &mut |fact, mutated_place, dependency_segments| {
                invalidations.append(FlowInvalidationFact {
                    source: invalidation_source,
                    context: context_ref.context,
                    fact,
                    mutated_root: mutated_place.root,
                    mutated_segments: append_place_segments(
                        invalidation_segments,
                        &mutated_place.segments,
                    ),
                    dependency_segments: append_place_segments(
                        invalidation_segments,
                        dependency_segments,
                    ),
                });
            },
        )
    })
}
