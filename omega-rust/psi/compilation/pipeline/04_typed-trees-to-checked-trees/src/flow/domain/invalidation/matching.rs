use validation::CanonicalPlace;

use crate::flow::canonical_place_from_semantic_place;
use crate::flow::canonical_place_joined_segments_may_overlap;
use crate::flow::canonical_place_segments_may_overlap;
use checked_trees::{DomainDependencyFact, DomainFacts};
use facts::{Fact, FactPlan};

pub(in crate::flow) fn matching_mutation_for_fact_place<'a, 'b>(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    domain_dependencies: &'a DomainFacts,
    fact: &Fact,
    fact_place: facts::PlaceHandle,
    mutated_places: &'b [CanonicalPlace],
) -> Option<(&'b CanonicalPlace, &'a [facts::PlaceSegment])> {
    let place = semantic.places.get(fact_place);
    let fact_canonical_place = canonical_place_from_semantic_place(program, semantic, place)?;
    let fact_root = fact_canonical_place.root;
    let dependency = match fact.payload {
        facts::domain_membership_symbol_pattern!(domain_symbol) => {
            domain_dependencies.dependency_fact(domain_symbol)
        }
        _ => None,
    };

    for mutation in mutated_places {
        if fact_root != mutation.root {
            continue;
        }
        // The selected case observes the discriminator, not its payload.
        // Replacing this value (or an ancestor) retires the observation;
        // writing a resolved descendant cannot change the enclosing tag.
        // Unresolved paths retain the ordinary conservative overlap law.
        if matches!(fact.payload, facts::FactPayload::AssignedCase { .. })
            && mutation.segments.len() > fact_canonical_place.segments.len()
            && mutation
                .segments
                .iter()
                .chain(&fact_canonical_place.segments)
                .all(|segment| !crate::flow::place_segment_has_unresolved_identity(*segment))
        {
            continue;
        }
        if let Some(dependency_segments) = matching_dependency_segments(
            program,
            domain_dependencies,
            dependency,
            &fact_canonical_place,
            mutation,
        ) {
            return Some((mutation, dependency_segments));
        }
    }

    None
}

fn matching_dependency_segments<'a>(
    program: &dyn ::validation::CheckingInput,
    domain_dependencies: &'a DomainFacts,
    dependency: Option<&'a DomainDependencyFact>,
    fact_place: &CanonicalPlace,
    mutated_place: &CanonicalPlace,
) -> Option<&'a [facts::PlaceSegment]> {
    let Some(domain_dependency) = dependency else {
        return canonical_place_segments_may_overlap(
            program,
            &fact_place.segments,
            &mutated_place.segments,
        )
        .then_some(&[]);
    };

    if domain_dependency.dependencies.is_empty() {
        return canonical_place_segments_may_overlap(
            program,
            &fact_place.segments,
            &mutated_place.segments,
        )
        .then_some(&[]);
    }

    domain_dependencies
        .dependency_paths(domain_dependency)
        .find(|dependency_segments| {
            canonical_place_joined_segments_may_overlap(
                program,
                &fact_place.segments,
                dependency_segments,
                &mutated_place.segments,
            )
        })
}
