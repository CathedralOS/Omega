use super::*;

pub(crate) fn filter_contexts_after_place_mutations(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &DomainFacts,
    semantic_context_refs: &mut omega_core::arena::Arena<FlowSemanticContextRef>,
    invalidation_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    invalidations: &mut omega_core::arena::Arena<FlowInvalidationFact>,
    source: omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    mutated_places: &[CanonicalPlace],
    invalidation_source: FlowInvalidationSource,
) -> omega_core::arena::HandleSpan<FlowSemanticContextRef> {
    if mutated_places.is_empty() {
        return source;
    }

    let mut filtered = omega_core::arena::HandleSpan::empty();
    let mut removed_any = false;
    let copied: Vec<_> = semantic_context_refs
        .span_or_empty(source)
        .iter()
        .copied()
        .collect();
    for context_ref in copied {
        let context = semantic.contexts.get(context_ref.context);
        let mut invalidated_any = false;
        for fact_ref in semantic.refs.span_or_empty(context.facts) {
            let fact = semantic.facts.get(fact_ref.fact);
            let FactPlace::Place(place) = fact.place else {
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

fn matching_mutation_for_fact_place<'a, 'b>(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &'a DomainFacts,
    fact: &Fact,
    fact_place: omega_facts::PlaceHandle,
    mutated_places: &'b [CanonicalPlace],
) -> Option<(&'b CanonicalPlace, &'a [omega_facts::PlaceSegment])> {
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

        if fact_canonical_place.root == mutated_place.root
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
    program: &omega_typed_trees::TypedTrees,
    domain_dependencies: &'a DomainFacts,
    fact: &Fact,
    fact_place: &CanonicalPlace,
    mutated_place: &CanonicalPlace,
) -> Option<&'a [omega_facts::PlaceSegment]> {
    let domain_symbol = match fact.payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
        _ => return None,
    };

    if fact_place.root != mutated_place.root {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn integer_expression(
        program: &mut omega_typed_trees::TypedTrees,
        value: i64,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert(omega_checked_trees::expression::ExpressionNode::Integer(
                value,
            ))
    }

    fn name_expression(program: &mut omega_typed_trees::TypedTrees) -> ExpressionHandle {
        program
            .expression_table
            .insert(omega_checked_trees::expression::ExpressionNode::Name(
                omega_checked_trees::expression::TableNamePath::default(),
            ))
    }

    fn dependency_facts(
        domain_symbol: SymbolHandle,
        dependency_segments: &[omega_facts::PlaceSegment],
    ) -> DomainFacts {
        let mut facts = DomainFacts::default();
        let mut segment_span = omega_core::arena::HandleSpan::empty();
        for segment in dependency_segments {
            facts.segments.append_to_span(&mut segment_span, *segment);
        }
        let mut dependency_span = omega_core::arena::HandleSpan::empty();
        facts.dependency_paths.append_to_span(
            &mut dependency_span,
            DomainDependencyPathFact {
                segments: segment_span,
            },
        );
        facts.dependencies.append(DomainDependencyFact {
            domain_symbol,
            dependencies: dependency_span,
        });
        facts
    }

    fn domain_membership_fact(domain_symbol: SymbolHandle) -> Fact {
        Fact {
            place: FactPlace::Unknown,
            point: ProgramPoint::Global,
            origin: FactOrigin::ProofObligation,
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: omega_core::arena::HandleSpan::empty(),
                domain_symbol,
            },
        }
    }

    #[test]
    fn indexed_domain_dependency_ignores_disjoint_literal_index_mutations() {
        let domain_symbol = SymbolHandle::from_arena_index(100);
        let entries_symbol = SymbolHandle::from_arena_index(101);
        let value_symbol = SymbolHandle::from_arena_index(102);
        let tag_symbol = SymbolHandle::from_arena_index(103);
        let mut program = omega_typed_trees::TypedTrees::default();
        let zero = integer_expression(&mut program, 0);
        let one = integer_expression(&mut program, 1);
        let domains = dependency_facts(
            domain_symbol,
            &[omega_facts::PlaceSegment::Field {
                symbol: value_symbol,
            }],
        );
        let fact = domain_membership_fact(domain_symbol);
        let fact_place = CanonicalPlace {
            root: omega_facts::PlaceRoot::Symbol(entries_symbol),
            segments: vec![omega_facts::PlaceSegment::Index { expression: zero }],
        };
        let mutated_place = CanonicalPlace {
            root: omega_facts::PlaceRoot::Symbol(entries_symbol),
            segments: vec![
                omega_facts::PlaceSegment::Index { expression: one },
                omega_facts::PlaceSegment::Field { symbol: tag_symbol },
            ],
        };

        assert!(
            domain_membership_matching_dependency(
                &program,
                &domains,
                &fact,
                &fact_place,
                &mutated_place,
            )
            .is_none()
        );
    }

    #[test]
    fn indexed_domain_dependency_invalidates_same_literal_index_dependency_mutations() {
        let domain_symbol = SymbolHandle::from_arena_index(110);
        let entries_symbol = SymbolHandle::from_arena_index(111);
        let value_symbol = SymbolHandle::from_arena_index(112);
        let mut program = omega_typed_trees::TypedTrees::default();
        let zero_left = integer_expression(&mut program, 0);
        let zero_right = integer_expression(&mut program, 0);
        let domains = dependency_facts(
            domain_symbol,
            &[omega_facts::PlaceSegment::Field {
                symbol: value_symbol,
            }],
        );
        let fact = domain_membership_fact(domain_symbol);
        let fact_place = CanonicalPlace {
            root: omega_facts::PlaceRoot::Symbol(entries_symbol),
            segments: vec![omega_facts::PlaceSegment::Index {
                expression: zero_left,
            }],
        };
        let mutated_place = CanonicalPlace {
            root: omega_facts::PlaceRoot::Symbol(entries_symbol),
            segments: vec![
                omega_facts::PlaceSegment::Index {
                    expression: zero_right,
                },
                omega_facts::PlaceSegment::Field {
                    symbol: value_symbol,
                },
            ],
        };

        assert!(
            domain_membership_matching_dependency(
                &program,
                &domains,
                &fact,
                &fact_place,
                &mutated_place,
            )
            .is_some()
        );
    }

    #[test]
    fn indexed_domain_dependency_is_conservative_for_unknown_indices() {
        let domain_symbol = SymbolHandle::from_arena_index(120);
        let entries_symbol = SymbolHandle::from_arena_index(121);
        let value_symbol = SymbolHandle::from_arena_index(122);
        let mut program = omega_typed_trees::TypedTrees::default();
        let literal_zero = integer_expression(&mut program, 0);
        let unknown_index = name_expression(&mut program);
        let domains = dependency_facts(
            domain_symbol,
            &[omega_facts::PlaceSegment::Field {
                symbol: value_symbol,
            }],
        );
        let fact = domain_membership_fact(domain_symbol);
        let fact_place = CanonicalPlace {
            root: omega_facts::PlaceRoot::Symbol(entries_symbol),
            segments: vec![omega_facts::PlaceSegment::Index {
                expression: literal_zero,
            }],
        };
        let mutated_place = CanonicalPlace {
            root: omega_facts::PlaceRoot::Symbol(entries_symbol),
            segments: vec![
                omega_facts::PlaceSegment::Index {
                    expression: unknown_index,
                },
                omega_facts::PlaceSegment::Field {
                    symbol: value_symbol,
                },
            ],
        };

        assert!(
            domain_membership_matching_dependency(
                &program,
                &domains,
                &fact,
                &fact_place,
                &mutated_place,
            )
            .is_some()
        );
    }
}
