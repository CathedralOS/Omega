use super::{
    matching_dependency_segments, matching_mutation_for_fact_place, prepare_mutated_places,
};
use crate::flow::CanonicalPlace;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{DomainDependencyFact, DomainDependencyPathFact, DomainFacts};
use facts::{Fact, FactOrigin, FactPayload, FactPlace, ProgramPoint};
use symbols::SymbolHandle;

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
            semantic_domain: Default::default(),
            value: ExpressionHandle::invalid(),
            domain: arena::HandleSpan::empty(),
            domain_symbol,
        },
    }
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
        matching_dependency_segments(
            &program,
            &domains,
            domains.dependency_fact(domain_symbol),
            &fact_place,
            &mutated_place,
        )
        .is_none()
    );
}

#[test]
fn prepared_mutations_preserve_self_root_order_and_dependency_evidence() {
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::{machine::Machine, signature::StateParameter, state::State};

    let mut symbols = SymbolTableBuilder::new();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Borrowed("root"));
    let machines = symbols.insert_children(
        root,
        [(SymbolKind::Machine, SymbolNameRef::Borrowed("owner"))],
    );
    let owner = SymbolTableBuilder::child_handles(machines).next().unwrap();
    let children = symbols.insert_children(
        owner,
        [
            (SymbolKind::State, SymbolNameRef::Borrowed("run")),
            (SymbolKind::Field, SymbolNameRef::Borrowed("value")),
            (SymbolKind::Field, SymbolNameRef::Borrowed("tag")),
        ],
    );
    let mut children = SymbolTableBuilder::child_handles(children);
    let state_symbol = children.next().unwrap();
    let value = children.next().unwrap();
    let tag = children.next().unwrap();
    let parameters = symbols.insert_children(
        state_symbol,
        [(SymbolKind::Parameter, SymbolNameRef::Borrowed("self"))],
    );
    let receiver = SymbolTableBuilder::child_handles(parameters)
        .next()
        .unwrap();
    let mut program = typed_trees::TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let mut state = State {
        symbol: state_symbol,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: receiver,
            is_self: true,
            ..Default::default()
        },
    );
    let mut machine = Machine {
        symbol: owner,
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let domain = SymbolHandle::from_arena_index(130);
    let dependency = [facts::PlaceSegment::Field { symbol: value }];
    let domains = dependency_facts(domain, &dependency);
    let mut fact = domain_membership_fact(domain);
    let mut semantic = facts::FactPlan::default();
    for fact_root in [receiver, owner] {
        let place = semantic.places.append(facts::Place {
            root: facts::PlaceRoot::Symbol(fact_root),
            segments: arena::HandleSpan::empty(),
        });
        fact.place = FactPlace::Place(place);
        for matching_roots in [[receiver, owner], [owner, receiver]] {
            let mutations = [
                CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(owner),
                    segments: vec![facts::PlaceSegment::Field { symbol: tag }],
                },
                CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(matching_roots[0]),
                    segments: dependency.to_vec(),
                },
                CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(matching_roots[1]),
                    segments: dependency.to_vec(),
                },
            ];
            let prepared = prepare_mutated_places(&program, &mutations);
            assert_eq!(
                matching_mutation_for_fact_place(
                    &program, &semantic, &domains, &fact, place, &prepared,
                ),
                Some((&mutations[1], dependency.as_slice())),
                "comparison normalization must preserve the first original mutation"
            );
            assert!(
                matching_mutation_for_fact_place(
                    &program,
                    &semantic,
                    &domains,
                    &fact,
                    place,
                    &prepared[..1],
                )
                .is_none(),
                "a disjoint dependency field must remain live"
            );
            let ordinary = Fact {
                payload: FactPayload::AssignedValue {
                    value: ExpressionHandle::invalid(),
                },
                ..fact
            };
            assert_eq!(
                matching_mutation_for_fact_place(
                    &program, &semantic, &domains, &ordinary, place, &prepared,
                ),
                Some((&mutations[0], &[][..])),
                "ordinary whole-root facts use base-place overlap"
            );
        }
    }
}
