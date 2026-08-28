use super::*;

pub(super) fn matching_mutation_for_fact_place<'a, 'b>(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &'a DomainFacts,
    fact: &Fact,
    fact_place: psi_facts::PlaceHandle,
    mutated_places: &'b [CanonicalPlace],
) -> Option<(&'b CanonicalPlace, &'a [psi_facts::PlaceSegment])> {
    let place = semantic.places.get(fact_place);
    let fact_canonical_place = canonical_place_from_semantic_place(program, semantic, place)?;

    for mutated_place in mutated_places {
        let is_domain_membership = matches!(
            fact.payload,
            FactPayload::DomainMembership { .. } | FactPayload::ContractDomainMembership { .. }
        );
        if let Some(dependency_segments) = domain_membership_matching_dependency(
            program,
            domain_dependencies,
            fact,
            &fact_canonical_place,
            mutated_place,
        ) {
            return Some((mutated_place, dependency_segments));
        }

        if is_domain_membership {
            continue;
        }

        if canonical_place_roots_match(program, fact_canonical_place.root, mutated_place.root)
            && canonical_place_segments_may_overlap(
                program,
                &fact_canonical_place.segments,
                &mutated_place.segments,
            )
        {
            return Some((mutated_place, &[]));
        }
    }

    None
}

fn domain_membership_matching_dependency<'a>(
    program: &psi_typed_trees::TypedTrees,
    domain_dependencies: &'a DomainFacts,
    fact: &Fact,
    fact_place: &CanonicalPlace,
    mutated_place: &CanonicalPlace,
) -> Option<&'a [psi_facts::PlaceSegment]> {
    let domain_symbol = match fact.payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
        _ => return None,
    };

    if !canonical_place_roots_match(program, fact_place.root, mutated_place.root) {
        return None;
    }

    let Some(domain_dependency) = domain_dependencies.dependency_fact(domain_symbol) else {
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

fn canonical_place_roots_match(
    program: &psi_typed_trees::TypedTrees,
    left: psi_facts::PlaceRoot,
    right: psi_facts::PlaceRoot,
) -> bool {
    crate::flow::normalized_event_place_root(program, left)
        == crate::flow::normalized_event_place_root(program, right)
}

#[cfg(test)]
mod tests;
