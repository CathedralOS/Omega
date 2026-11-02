use super::{BigInt, Engine, Polynomial};
use crate::proof_contracts::contract_entailment::inductive_judgment::apply_argument_map;
use std::collections::BTreeMap;

#[test]
fn opaque_rank_dependencies_survive_two_symbolic_arrivals() {
    let program = typed_trees::TypedTrees::default();
    let constant = |value| Polynomial::constant(BigInt::from_i64(value));
    let atom = |name: &str| Polynomial::atom(name.to_owned());
    let mut source = Engine::for_explicit_bindings(&program);
    let remainder = source
        .integer_remainder(atom("input"), atom("divisor"), true)
        .unwrap();
    let rank = source.integer_quotient(remainder, constant(2)).unwrap();
    let captured = source.capture_integer_denotation(&rank).unwrap();

    let mut arrival = Engine::for_explicit_bindings(&program);
    let first = arrival
        .instantiate_integer_denotation(
            &captured,
            &BTreeMap::from([
                ("input".to_owned(), atom("first-input")),
                ("divisor".to_owned(), atom("first-divisor")),
            ]),
        )
        .unwrap();
    let mut arguments = BTreeMap::from([
        ("first-input".to_owned(), atom("second-input")),
        ("first-divisor".to_owned(), atom("second-divisor")),
    ]);
    arrival.extend_argument_map_over_opaque_terms(&mut arguments);
    let second = apply_argument_map(&first, &arguments).unwrap();
    let saved = arrival.capture_integer_denotation(&second).unwrap();
    assert_eq!(saved.leaf_atoms(), &["second-divisor", "second-input"]);

    // No quotient is constant at either symbolic arrival. Both signs retain
    // truncation toward zero when the saved dependencies finally become known.
    for (input, divisor, expected) in [
        (17, 5, 1),
        (-17, 5, -1),
        (17, -5, 1),
        (-17, -5, -1),
        (35, 6, 2),
    ] {
        let mut replay = Engine::for_explicit_bindings(&program);
        let value = replay
            .instantiate_integer_denotation(
                &saved,
                &BTreeMap::from([
                    ("second-input".to_owned(), constant(input)),
                    ("second-divisor".to_owned(), constant(divisor)),
                ]),
            )
            .unwrap();
        let difference = value.sub(&constant(expected));
        assert!(replay.prove_at_least(&difference, &BigInt::zero()));
        assert!(replay.prove_at_least(&difference.neg(), &BigInt::zero()));
    }
    let mut incomplete = Engine::for_explicit_bindings(&program);
    assert!(
        incomplete
            .instantiate_integer_denotation(
                &saved,
                &BTreeMap::from([("second-input".to_owned(), constant(17))]),
            )
            .is_none()
    );
}
