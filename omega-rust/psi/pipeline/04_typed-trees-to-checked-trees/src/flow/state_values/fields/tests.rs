use super::{BoundsSource, ByteSequencePredicate, integer_literal_thresholds, meet};
use crate::flow::state_values::fields::FieldValue;
use checked_trees::expression::ExpressionHandle;
use symbols::SymbolHandle;
use typed_trees::statement::TransitionTargetHandle;

fn field(symbol: u32, literal: u32, predicates: Vec<ByteSequencePredicate>) -> FieldValue {
    let edge_potential = predicates.clone();
    FieldValue {
        segments: vec![facts::PlaceSegment::Field {
            symbol: SymbolHandle::from_arena_index(symbol),
        }],
        literal: ExpressionHandle::from_arena_index(literal),
        predicates,
        integer_bounds: None,
        deliveries: Vec::new(),
        edge_potential,
        predicate_ceiling: Vec::new(),
        bounds_growth: 0,
        bounds_shrink: 0,
    }
}

fn seeded(
    symbol: u32,
    literal: u32,
    predicates: Vec<ByteSequencePredicate>,
    source: BoundsSource,
) -> FieldValue {
    let mut field = field(symbol, literal, predicates);
    field.seed_delivery(source);
    field
}

fn edge(ordinal: u32) -> BoundsSource {
    BoundsSource::transition(
        SymbolHandle::from_arena_index(7),
        TransitionTargetHandle::from_arena_index(ordinal),
    )
}

fn fixture() -> (typed_trees::TypedTrees, typed_trees::machine::Machine) {
    (
        typed_trees::TypedTrees::default(),
        typed_trees::machine::Machine::default(),
    )
}

#[test]
fn deliveries_shrink_and_regrow_with_each_edges_latest_evidence() {
    let (program, machine) = fixture();
    let mut previous = vec![seeded(1, 2, ByteSequencePredicate::ALL.to_vec(), edge(9))];
    let mut incoming = field(1, 2, ByteSequencePredicate::ALL.to_vec());
    incoming.literal = ExpressionHandle::invalid();
    while incoming.predicates.pop().is_some() {
        meet(
            &program,
            &machine,
            &mut previous,
            &[incoming.clone()],
            edge(0),
            &integer_literal_thresholds(&program),
        );
    }
    assert!(previous[0].predicates.is_empty());
    assert!(!previous[0].literal.is_valid());
    // A later pass re-delivering full evidence restores the join.
    assert!(meet(
        &program,
        &machine,
        &mut previous,
        &[field(1, 2, ByteSequencePredicate::ALL.to_vec())],
        edge(0),
        &integer_literal_thresholds(&program)
    ));
    assert_eq!(previous[0].predicates, ByteSequencePredicate::ALL.to_vec());
    assert_eq!(previous[0].literal, ExpressionHandle::from_arena_index(2));
}
