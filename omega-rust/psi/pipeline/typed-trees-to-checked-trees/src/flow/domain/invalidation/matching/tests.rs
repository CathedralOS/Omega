use super::*;

fn integer_expression(program: &mut typed_trees::TypedTrees, value: i64) -> ExpressionHandle {
    program
        .expression_table
        .insert(checked_trees::expression::ExpressionNode::Integer(
            numerics::literals::IntegerLiteral::from_value(value),
        ))
}

fn name_expression(program: &mut typed_trees::TypedTrees) -> ExpressionHandle {
    program
        .expression_table
        .insert(checked_trees::expression::ExpressionNode::Name(
            checked_trees::expression::TableNamePath::default(),
        ))
}

fn dependency_facts(
    domain_symbol: SymbolHandle,
    dependency_segments: &[facts::PlaceSegment],
) -> DomainFacts {
    let mut facts = DomainFacts::default();
    let mut segment_span = arena::HandleSpan::empty();
    for segment in dependency_segments {
        facts.segments.append_to_span(&mut segment_span, *segment);
    }
    let mut dependency_span = arena::HandleSpan::empty();
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
        evidence: Default::default(),
        payload: FactPayload::DomainMembership {
            value: ExpressionHandle::invalid(),
            domain: arena::HandleSpan::empty(),
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
    let mut program = typed_trees::TypedTrees::default();
    let zero = integer_expression(&mut program, 0);
    let one = integer_expression(&mut program, 1);
    let domains = dependency_facts(
        domain_symbol,
        &[facts::PlaceSegment::Field {
            symbol: value_symbol,
        }],
    );
    let fact = domain_membership_fact(domain_symbol);
    let fact_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![facts::PlaceSegment::Index { expression: zero }],
    };
    let mutated_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![
            facts::PlaceSegment::Index { expression: one },
            facts::PlaceSegment::Field { symbol: tag_symbol },
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
    let mut program = typed_trees::TypedTrees::default();
    let zero_left = integer_expression(&mut program, 0);
    let zero_right = integer_expression(&mut program, 0);
    let domains = dependency_facts(
        domain_symbol,
        &[facts::PlaceSegment::Field {
            symbol: value_symbol,
        }],
    );
    let fact = domain_membership_fact(domain_symbol);
    let fact_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![facts::PlaceSegment::Index {
            expression: zero_left,
        }],
    };
    let mutated_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![
            facts::PlaceSegment::Index {
                expression: zero_right,
            },
            facts::PlaceSegment::Field {
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
fn indexed_domain_dependency_preserves_shared_dynamic_index_disjoint_field() {
    // A domain fact over `self.entries[self.index].value` must survive a call that
    // mutates `self.entries[self.index].tag`: the index expression is the SAME dynamic
    // place, so the disjoint trailing field proves the two places cannot alias.
    let domain_symbol = SymbolHandle::from_arena_index(130);
    let entries_symbol = SymbolHandle::from_arena_index(131);
    let value_symbol = SymbolHandle::from_arena_index(132);
    let tag_symbol = SymbolHandle::from_arena_index(133);
    let mut program = typed_trees::TypedTrees::default();
    let shared_index = name_expression(&mut program);
    let domains = dependency_facts(
        domain_symbol,
        &[facts::PlaceSegment::Field {
            symbol: value_symbol,
        }],
    );
    let fact = domain_membership_fact(domain_symbol);
    let fact_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![facts::PlaceSegment::Index {
            expression: shared_index,
        }],
    };
    let mutated_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![
            facts::PlaceSegment::Index {
                expression: shared_index,
            },
            facts::PlaceSegment::Field { symbol: tag_symbol },
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
fn indexed_domain_dependency_invalidates_distinct_dynamic_index_same_field() {
    // Two DISTINCT occurrences of `self.index` produce distinct expression handles even
    // though they denote the same place. A same-field mutation through such an index must
    // still invalidate: we cannot prove the two dynamic indices disagree.
    let domain_symbol = SymbolHandle::from_arena_index(140);
    let entries_symbol = SymbolHandle::from_arena_index(141);
    let value_symbol = SymbolHandle::from_arena_index(142);
    let mut program = typed_trees::TypedTrees::default();
    let fact_index = name_expression(&mut program);
    let mutated_index = name_expression(&mut program);
    let domains = dependency_facts(
        domain_symbol,
        &[facts::PlaceSegment::Field {
            symbol: value_symbol,
        }],
    );
    let fact = domain_membership_fact(domain_symbol);
    let fact_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![facts::PlaceSegment::Index {
            expression: fact_index,
        }],
    };
    let mutated_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![
            facts::PlaceSegment::Index {
                expression: mutated_index,
            },
            facts::PlaceSegment::Field {
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
    let mut program = typed_trees::TypedTrees::default();
    let literal_zero = integer_expression(&mut program, 0);
    let unknown_index = name_expression(&mut program);
    let domains = dependency_facts(
        domain_symbol,
        &[facts::PlaceSegment::Field {
            symbol: value_symbol,
        }],
    );
    let fact = domain_membership_fact(domain_symbol);
    let fact_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![facts::PlaceSegment::Index {
            expression: literal_zero,
        }],
    };
    let mutated_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![
            facts::PlaceSegment::Index {
                expression: unknown_index,
            },
            facts::PlaceSegment::Field {
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
fn indexed_domain_dependency_preserves_disjoint_nested_field_under_same_index() {
    // Fact place `entries[0]` is in a domain whose dependency is the nested
    // path `inner.value`. A mutation of `entries[0].inner.tag` shares the index
    // and the `inner` prefix but diverges on the final field, so it must not
    // invalidate the indexed domain fact (regression guard for disjoint
    // mutation across a multi-segment dependency).
    let domain_symbol = SymbolHandle::from_arena_index(130);
    let entries_symbol = SymbolHandle::from_arena_index(131);
    let inner_symbol = SymbolHandle::from_arena_index(132);
    let value_symbol = SymbolHandle::from_arena_index(133);
    let tag_symbol = SymbolHandle::from_arena_index(134);
    let mut program = typed_trees::TypedTrees::default();
    let zero_left = integer_expression(&mut program, 0);
    let zero_right = integer_expression(&mut program, 0);
    let domains = dependency_facts(
        domain_symbol,
        &[
            facts::PlaceSegment::Field {
                symbol: inner_symbol,
            },
            facts::PlaceSegment::Field {
                symbol: value_symbol,
            },
        ],
    );
    let fact = domain_membership_fact(domain_symbol);
    let fact_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![facts::PlaceSegment::Index {
            expression: zero_left,
        }],
    };
    let mutated_place = CanonicalPlace {
        root: facts::PlaceRoot::Symbol(entries_symbol),
        segments: vec![
            facts::PlaceSegment::Index {
                expression: zero_right,
            },
            facts::PlaceSegment::Field {
                symbol: inner_symbol,
            },
            facts::PlaceSegment::Field { symbol: tag_symbol },
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
