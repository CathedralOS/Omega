use crate::flow::CanonicalPlace;
use crate::flow::domain::invalidation::matching::domain_membership_matching_dependency;
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
