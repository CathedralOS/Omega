use super::{BoundsSource, ByteSequencePredicate, integer_literal_thresholds, meet};
use crate::flow::state_values::fields::FieldValue;
use checked_trees::expression::ExpressionHandle;
use symbols::SymbolHandle;
use typed_trees::statement::TransitionTargetHandle;

fn field(symbol: u32, literal: u32, predicates: Vec<ByteSequencePredicate>) -> FieldValue {
    let edge_potential = predicates.clone();
    FieldValue {
        root: facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(11)),
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

fn fixture() -> (
    typed_trees::TypedTrees,
    typed_trees::machine::Machine,
    typed_trees::state::State,
) {
    (
        typed_trees::TypedTrees::default(),
        typed_trees::machine::Machine::default(),
        typed_trees::state::State::default(),
    )
}

#[test]
fn deliveries_shrink_and_regrow_with_each_edges_latest_evidence() {
    let (program, machine, state) = fixture();
    let mut previous = vec![seeded(1, 2, ByteSequencePredicate::ALL.to_vec(), edge(9))];
    let mut incoming = field(1, 2, ByteSequencePredicate::ALL.to_vec());
    incoming.literal = ExpressionHandle::invalid();
    while incoming.predicates.pop().is_some() {
        meet(
            &program,
            &machine,
            &state,
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
        &state,
        &mut previous,
        &[field(1, 2, ByteSequencePredicate::ALL.to_vec())],
        edge(0),
        &integer_literal_thresholds(&program)
    ));
    assert_eq!(previous[0].predicates, ByteSequencePredicate::ALL.to_vec());
    assert_eq!(previous[0].literal, ExpressionHandle::from_arena_index(2));
}

#[test]
fn widening_reaches_late_authored_threshold_before_the_carrier() {
    let program = crate::tests::front_end::typed_program(
        "data Buffer { value: i32 in Wrapping; } machine Buffer::run(&mut self) {}",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let typed_trees::data::DataMember::Field(value) =
        &program.data_members(&program.data_definitions()[0])[0]
    else {
        panic!("fixture value field");
    };
    let mut incoming = field(value.symbol.arena_index(), 0, Vec::new());
    incoming.root = facts::PlaceRoot::Symbol(machine.symbol);
    incoming.segments = vec![facts::PlaceSegment::Field {
        symbol: value.symbol,
    }];
    incoming.literal = ExpressionHandle::invalid();
    let thresholds: Vec<_> = (0..=12).map(numerics::bignum::BigInt::from_u64).collect();
    let mut previous = Vec::new();
    for maximum in 0..=12 {
        incoming.integer_bounds = Some(facts::IntegerRange {
            minimum: numerics::bignum::BigInt::zero(),
            maximum: numerics::bignum::BigInt::from_u64(maximum),
        });
        meet(
            &program,
            machine,
            state,
            &mut previous,
            &[incoming.clone()],
            edge(1),
            &thresholds,
        );
        assert_eq!(previous[0].integer_bounds, incoming.integer_bounds);
    }
    // A guard-free counter exceeding every threshold still reaches the
    // primitive carrier. Replaying unchanged evidence must then settle.
    incoming.integer_bounds.as_mut().unwrap().maximum = numerics::bignum::BigInt::from_u64(13);
    assert!(meet(
        &program,
        machine,
        state,
        &mut previous,
        &[incoming.clone()],
        edge(1),
        &thresholds
    ));
    assert_eq!(
        previous[0].integer_bounds.as_ref().unwrap().maximum,
        numerics::bignum::BigInt::from_u64(i32::MAX as u64)
    );
    assert!(!meet(
        &program,
        machine,
        state,
        &mut previous,
        &[incoming],
        edge(1),
        &thresholds
    ));
}
