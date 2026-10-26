use validation::CanonicalPlace;

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
    if mutated_places.is_empty() {
        return None;
    }
    let place = semantic.places.get(fact_place);
    match place.root {
        facts::PlaceRoot::Unknown => return None,
        facts::PlaceRoot::Symbol(symbol) if !symbol.is_valid() => return None,
        facts::PlaceRoot::Expression(expression)
            if !program.expression_table.expression_is_valid(expression) =>
        {
            return None;
        }
        facts::PlaceRoot::Symbol(_)
        | facts::PlaceRoot::InvocationReferent { .. }
        | facts::PlaceRoot::InvocationOutput(_)
        | facts::PlaceRoot::Expression(_)
        | facts::PlaceRoot::TypeReference(_) => {}
    }
    let fact_root = place.root;
    if !mutated_places
        .iter()
        .any(|mutation| mutation.root == fact_root)
    {
        return None;
    }
    let fact_segments = semantic.place_segments.span_or_empty(place.segments);
    let is_case_value = matches!(fact.payload, facts::FactPayload::CaseValue { .. });
    let mut case_canonical: Option<CanonicalPlace> = None;
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
        if is_case_value {
            let fact_canonical_place = case_canonical.get_or_insert_with(|| CanonicalPlace {
                root: fact_root,
                segments: fact_segments.to_vec(),
            });
            if !crate::flow::operand_contexts::case_overwrite(
                program,
                fact_canonical_place,
                &mutation.segments,
            ) {
                continue;
            }
        }
        if let Some(dependency_segments) = matching_dependency_segments(
            program,
            domain_dependencies,
            dependency,
            fact_segments,
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
    fact_segments: &[facts::PlaceSegment],
    mutated_place: &CanonicalPlace,
) -> Option<&'a [facts::PlaceSegment]> {
    let Some(domain_dependency) = dependency else {
        return canonical_place_segments_may_overlap(
            program,
            fact_segments,
            &mutated_place.segments,
        )
        .then_some(&[]);
    };

    if domain_dependency.dependencies.is_empty() {
        return canonical_place_segments_may_overlap(
            program,
            fact_segments,
            &mutated_place.segments,
        )
        .then_some(&[]);
    }

    domain_dependencies
        .dependency_paths(domain_dependency)
        .find(|dependency_segments| {
            canonical_place_joined_segments_may_overlap(
                program,
                fact_segments,
                dependency_segments,
                &mutated_place.segments,
            )
        })
}
