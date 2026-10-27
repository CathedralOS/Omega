use validation::CanonicalPlace;

use crate::flow::append_place_segments;
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
    semantic: &mut FactPlan,
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
        |semantic, fact, place| {
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
    semantic: &mut FactPlan,
    semantic_context_refs: &mut arena::PagedArenaSet<FlowSemanticContextRef>,
    invalidation_segments: &mut arena::PagedArenaSet<facts::PlaceSegment>,
    invalidations: &mut arena::PagedArenaSet<FlowInvalidationFact>,
    source: arena::HandleSpan<FlowSemanticContextRef>,
    invalidation_source: FlowInvalidationSource,
    matching_mutation: impl Fn(
        &FactPlan,
        &facts::Fact,
        facts::PlaceHandle,
    ) -> Option<(
        &'mutation CanonicalPlace,
        &'dependency [facts::PlaceSegment],
    )>,
) -> arena::HandleSpan<FlowSemanticContextRef> {
    let mut filtered = arena::HandleSpan::empty();
    for source_handle in source.handles() {
        let reference = *semantic_context_refs.get(source_handle);
        let survival = context_survival(
            semantic,
            reference.context,
            &matching_mutation,
            &mut |fact, mutated_place, dependency_segments| {
                invalidations.append(FlowInvalidationFact {
                    source: invalidation_source,
                    context: reference.context,
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
        );
        match survival {
            ContextSurvival::Unchanged => {
                semantic_context_refs.append_to_span(&mut filtered, reference);
            }
            ContextSurvival::Dropped => {}
            ContextSurvival::Reduced(retained) => {
                let point = semantic.contexts.get(reference.context).point;
                let mut refs = arena::HandleSpan::empty();
                for fact in retained {
                    semantic.append_ref(&mut refs, fact);
                }
                let context = semantic.append_context(point, refs);
                semantic_context_refs
                    .append_to_span(&mut filtered, FlowSemanticContextRef { context });
            }
        }
    }
    filtered
}

enum ContextSurvival {
    Unchanged,
    Dropped,
    Reduced(Vec<facts::FactHandle>),
}

/// Facts that overlap the write leave the context. A storage dependency and
/// the fact it guards leave together. Every other fact in that context stays:
/// independent field domains are not one proposition.
fn context_survival<'mutation, 'dependency>(
    semantic: &FactPlan,
    context: facts::FactContextHandle,
    matching_mutation: &impl Fn(
        &FactPlan,
        &facts::Fact,
        facts::PlaceHandle,
    ) -> Option<(
        &'mutation CanonicalPlace,
        &'dependency [facts::PlaceSegment],
    )>,
    invalidated: &mut impl FnMut(facts::FactHandle, &CanonicalPlace, &[facts::PlaceSegment]),
) -> ContextSurvival {
    let fact_refs = semantic
        .refs
        .span_or_empty(semantic.contexts.get(context).facts);
    if fact_refs.iter().any(|reference| {
        let fact = semantic.facts.get(reference.fact);
        fact.place.as_place().is_none()
            && matches!(fact.payload, FactPayload::StorageDependency { .. })
    }) {
        return ContextSurvival::Dropped;
    }
    let mut dropped = Vec::new();
    for reference in fact_refs {
        let handle = reference.fact;
        let fact = semantic.facts.get(handle);
        let Some(place) = fact.place.as_place() else {
            continue;
        };
        let Some((mutation, dependencies)) = matching_mutation(semantic, fact, place) else {
            continue;
        };
        crate::collections::push_unique(&mut dropped, handle);
        if let FactPayload::StorageDependency { dependent } = fact.payload
            && !dropped.contains(&dependent)
            && !dependency_is_strict_prefix_of_dependent(semantic, place, dependent)
        {
            dropped.push(dependent);
        }
        invalidated(handle, mutation, dependencies);
    }
    if dropped.is_empty() {
        return ContextSurvival::Unchanged;
    }
    let mut changed = true;
    while changed {
        changed = false;
        for reference in fact_refs {
            let handle = reference.fact;
            let FactPayload::StorageDependency { dependent } = semantic.facts.get(handle).payload
            else {
                continue;
            };
            if dropped.contains(&dependent) && !dropped.contains(&handle) {
                dropped.push(handle);
                changed = true;
            }
        }
    }
    if dropped.len() == fact_refs.len() {
        ContextSurvival::Dropped
    } else {
        ContextSurvival::Reduced(
            fact_refs
                .iter()
                .map(|reference| reference.fact)
                .filter(|handle| !dropped.contains(handle))
                .collect(),
        )
    }
}

/// A dependency on an ancestor place overlaps every sibling write. That
/// overlap retires the ancestor guard. The dependent fact stays unless its
/// own place overlaps: `main.pause` does not change `main.name`.
fn dependency_is_strict_prefix_of_dependent(
    semantic: &FactPlan,
    dependency: facts::PlaceHandle,
    dependent: facts::FactHandle,
) -> bool {
    let Some(dependent) = semantic.facts.get(dependent).place.as_place() else {
        return false;
    };
    let dependency = semantic.places.get(dependency);
    let dependent = semantic.places.get(dependent);
    if dependency.root != dependent.root {
        return false;
    }
    let dependency_segments = semantic.place_segments.span_or_empty(dependency.segments);
    let dependent_segments = semantic.place_segments.span_or_empty(dependent.segments);
    dependent_segments.len() > dependency_segments.len()
        && dependent_segments.starts_with(dependency_segments)
}
