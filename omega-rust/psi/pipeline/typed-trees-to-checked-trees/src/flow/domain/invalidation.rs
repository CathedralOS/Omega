mod matching;

use super::*;

use self::matching::matching_mutation_for_fact_place;

pub(crate) fn filter_contexts_after_place_mutations(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &DomainFacts,
    semantic_context_refs: &mut arena::Arena<FlowSemanticContextRef>,
    invalidation_segments: &mut arena::Arena<facts::PlaceSegment>,
    invalidations: &mut arena::Arena<FlowInvalidationFact>,
    source: arena::HandleSpan<FlowSemanticContextRef>,
    mutated_places: &[CanonicalPlace],
    invalidation_source: FlowInvalidationSource,
) -> arena::HandleSpan<FlowSemanticContextRef> {
    if mutated_places.is_empty() {
        return source;
    }

    let mut filtered = arena::HandleSpan::empty();
    let mut removed_any = false;
    let copied: Vec<_> = semantic_context_refs.span_or_empty(source).to_vec();
    for context_ref in copied {
        let context = semantic.contexts.get(context_ref.context);
        let mut invalidated_any = false;
        for fact_ref in semantic.refs.span_or_empty(context.facts) {
            let fact = semantic.facts.get(fact_ref.fact);
            let FactPlace::Place(place) = fact.place else {
                // An instantiated Boolean promise with an unresolved operand
                // dependency cannot survive a storage revision. Its producer
                // retains Unknown explicitly rather than inventing a place.
                if matches!(fact.payload, FactPayload::StorageDependency { .. }) {
                    invalidated_any = true;
                    removed_any = true;
                }
                continue;
            };
            let Some((mutated_place, dependency_segments)) = matching_mutation_for_fact_place(
                program,
                semantic,
                domain_dependencies,
                fact,
                place,
                mutated_places,
            ) else {
                continue;
            };

            invalidated_any = true;
            removed_any = true;
            invalidations.append(FlowInvalidationFact {
                source: invalidation_source,
                context: context_ref.context,
                fact: fact_ref.fact,
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
        }

        if !invalidated_any {
            semantic_context_refs.append_to_span(&mut filtered, context_ref);
        }
    }

    if removed_any { filtered } else { source }
}
