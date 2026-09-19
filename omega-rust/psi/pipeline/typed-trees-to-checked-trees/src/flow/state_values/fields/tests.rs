use super::{BoundsSource, ByteSequencePredicate, meet};
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
fn field_meet_loses_literal_identity_but_keeps_common_predicates() {
    let (program, machine) = fixture();
    let predicates = ByteSequencePredicate::ALL.to_vec();
    let mut previous = vec![seeded(1, 2, predicates.clone(), edge(9))];
    assert!(meet(
        &program,
        &machine,
        &mut previous,
        &[field(1, 3, predicates.clone())],
        edge(0)
    ));
    assert!(!previous[0].literal.is_valid());
    assert_eq!(previous[0].predicates, predicates);
    // The same edge re-delivering the original literal restores it: the join
    // reflects each edge's latest delivery, not the worst ever seen.
    assert!(meet(
        &program,
        &machine,
        &mut previous,
        &[field(1, 2, predicates.clone())],
        edge(0)
    ));
    assert_eq!(previous[0].literal, ExpressionHandle::from_arena_index(2));
}

#[test]
fn missing_field_evidence_is_absorbing_and_exact_paths_do_not_merge() {
    let (program, machine) = fixture();
    let mut previous = vec![seeded(1, 2, ByteSequencePredicate::ALL.to_vec(), edge(9))];
    // An edge arriving without the field empties its join: that path carries
    // no evidence for it.
    assert!(meet(
        &program,
        &machine,
        &mut previous,
        &[field(2, 2, ByteSequencePredicate::ALL.to_vec())],
        edge(0)
    ));
    assert_eq!(previous.len(), 2);
    assert!(!previous[0].literal.is_valid());
    assert!(previous[0].predicates.is_empty());
    // The new field records every other predecessor as carrying no evidence,
    // so its own join starts empty too.
    assert!(previous[1].predicates.is_empty());
    // Both edges re-delivering the field let the join recover.
    assert!(meet(
        &program,
        &machine,
        &mut previous,
        &[field(1, 2, ByteSequencePredicate::ALL.to_vec())],
        edge(0)
    ));
    assert_eq!(previous[0].predicates, ByteSequencePredicate::ALL.to_vec());
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
        edge(0)
    ));
    assert_eq!(previous[0].predicates, ByteSequencePredicate::ALL.to_vec());
    assert_eq!(previous[0].literal, ExpressionHandle::from_arena_index(2));
}

fn range(minimum: u64, maximum: u64) -> facts::IntegerRange {
    facts::IntegerRange {
        minimum: numerics::bignum::BigInt::from_u64(minimum),
        maximum: numerics::bignum::BigInt::from_u64(maximum),
    }
}

#[test]
fn equal_integer_ranges_survive_and_differing_ranges_union() {
    let (program, machine) = fixture();
    let mut initial = field(1, 0, Vec::new());
    initial.integer_bounds = Some(range(0, 9));
    let mut previous = vec![initial.clone()];
    // An identical edge bound is a no-op.
    assert!(!meet(
        &program,
        &machine,
        &mut previous,
        &[initial.clone()],
        edge(0)
    ));
    // A wider bound arriving on a second edge unions into the join.
    let mut wider = initial.clone();
    wider.integer_bounds = Some(range(0, 19));
    assert!(meet(
        &program,
        &machine,
        &mut previous,
        &[wider.clone()],
        edge(1)
    ));
    assert_eq!(previous[0].integer_bounds, Some(range(0, 19)));
    // A repeated identical arrival is a no-op.
    assert!(!meet(&program, &machine, &mut previous, &[wider], edge(1)));
}

#[test]
fn integer_bounds_retighten_when_an_edges_delivery_shrinks() {
    let (program, machine) = fixture();
    let mut initial = field(1, 0, Vec::new());
    initial.integer_bounds = Some(range(0, 9));
    let mut previous = vec![initial.clone()];
    let mut wider = initial.clone();
    wider.integer_bounds = Some(range(0, 19));
    assert!(meet(&program, &machine, &mut previous, &[wider], edge(0)));
    assert_eq!(previous[0].integer_bounds, Some(range(0, 19)));
    // The same edge later proving the tighter interval retightens the join:
    // only the latest delivery per edge contributes, so a transient
    // over-approximation cannot wedge the row at a widened bound.
    assert!(meet(&program, &machine, &mut previous, &[initial], edge(0)));
    assert_eq!(previous[0].integer_bounds, Some(range(0, 9)));
}

#[test]
fn missing_integer_bounds_widen_the_join_to_the_carrier() {
    let (program, machine) = fixture();
    let mut initial = field(1, 0, Vec::new());
    initial.integer_bounds = Some(range(0, 9));
    let mut previous = vec![initial.clone()];
    // No declared carrier resolves in the empty fixture, so a bound-less edge
    // leaves the row with no bound evidence.
    let mut incoming = initial.clone();
    incoming.integer_bounds = None;
    assert!(meet(
        &program,
        &machine,
        &mut previous,
        &[incoming],
        edge(0)
    ));
    // The row survives with no bound: a carrier-less field contributes no
    // bound evidence rather than vanishing.
    assert_eq!(previous.len(), 1);
    assert!(previous[0].integer_bounds.is_none());
}

#[test]
fn potential_only_changes_propagate_through_missing_call_deliveries() {
    let (program, machine) = fixture();
    let mut incoming = field(1, 0, Vec::new());
    incoming.edge_potential = ByteSequencePredicate::ALL.to_vec();
    let mut initial = incoming.clone();
    initial.seed_delivery(edge(0));
    let mut previous = vec![initial];
    let call = BoundsSource::invocation(SymbolHandle::from_arena_index(7), 0, 0);

    // The call carries no row, but must still be recorded as a predecessor.
    assert!(!meet(&program, &machine, &mut previous, &[], call));
    assert_eq!(previous[0].deliveries.len(), 2);
    assert_eq!(previous[0].predicate_ceiling(), ByteSequencePredicate::ALL);

    // No literal, predicate, or bound changed: potential alone refutes the
    // co-inductive claim and must invalidate the stored ceiling.
    incoming.edge_potential.clear();
    assert!(meet(
        &program,
        &machine,
        &mut previous,
        &[incoming],
        edge(0)
    ));
    assert!(previous[0].predicate_ceiling().is_empty());

    // The same absent call row now forwards the changed ceiling. Its stored
    // delivery changes even though the joined result is already empty.
    assert!(!meet(&program, &machine, &mut previous, &[], call));
    let (_, delivery) = previous[0]
        .deliveries
        .iter()
        .find(|(source, _)| *source == call)
        .expect("call delivery");
    assert!(delivery.potential.is_empty());
}

#[test]
fn repeated_delivery_preserves_widening_until_the_evidence_changes() {
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    let source = "data Counter { count: u64; }
                  machine Counter::tick(&mut self) { let limit: u64 = 10; }";
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let program = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = program.machines().first().expect("counter machine");
    let counter = program.data_definitions().first().expect("counter data");
    let symbol = program
        .data_members(counter)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field.symbol),
            _ => None,
        })
        .expect("count field");
    let mut initial = field(1, 0, Vec::new());
    initial.segments = vec![facts::PlaceSegment::Field { symbol }];
    initial.integer_bounds = Some(range(0, 0));
    initial.seed_delivery(edge(0));
    let mut previous = vec![initial.clone()];
    let mut incoming = initial;
    for maximum in 1..=4 {
        incoming.integer_bounds = Some(range(0, maximum));
        assert!(meet(
            &program,
            machine,
            &mut previous,
            &[incoming.clone()],
            edge(0)
        ));
    }
    assert_eq!(previous[0].integer_bounds, Some(range(0, 10)));
    assert!(
        !meet(
            &program,
            machine,
            &mut previous,
            &[incoming.clone()],
            edge(0)
        ),
        "identical edge evidence must not undo widening and dirty the state"
    );
    assert_eq!(previous[0].integer_bounds, Some(range(0, 10)));

    // A genuinely tighter delivery still improves the live join; avoiding
    // duplicate work must not make old widening an absorbing unknown.
    incoming.integer_bounds = Some(range(0, 2));
    assert!(meet(&program, machine, &mut previous, &[incoming], edge(0)));
    assert_eq!(previous[0].integer_bounds, Some(range(0, 2)));
}
