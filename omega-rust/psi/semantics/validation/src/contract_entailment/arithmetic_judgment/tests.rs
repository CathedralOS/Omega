use super::*;

#[test]
fn disequality_strengthens_only_an_independently_proven_integer_direction() {
    let program = TypedTrees::default();
    let remaining = Polynomial::atom("remaining".to_owned());
    let floor = Polynomial::atom("floor".to_owned());
    for operator in [BinaryOperator::GreaterOrEqual, BinaryOperator::LessOrEqual] {
        for reverse in [false, true] {
            let mut engine = Engine::for_proof_integer_formation(&program);
            let mut comparisons = vec![
                (BinaryOperator::NotEqual, remaining.clone(), floor.clone()),
                (operator, remaining.clone(), floor.clone()),
            ];
            if reverse {
                comparisons.reverse();
            }
            assert!(engine.install_hypotheses(comparisons));
            let difference = if operator == BinaryOperator::GreaterOrEqual {
                remaining.sub(&floor)
            } else {
                floor.sub(&remaining)
            };
            assert!(engine.prove_at_least(&difference, &BigInt::from_i64(1)));
            assert!(!engine.prove_at_least(&difference, &BigInt::from_i64(2)));
            assert!(!engine.prove_at_least(&difference.neg(), &BigInt::zero()));
            assert!(!engine.requires_unsatisfiable);
        }
    }
    let mut unknown = Engine::for_proof_integer_formation(&program);
    assert!(unknown.install_hypotheses(vec![(
        BinaryOperator::NotEqual,
        remaining.clone(),
        floor.clone()
    ),]));
    assert!(!unknown.prove_at_least(&remaining.sub(&floor), &BigInt::from_i64(1)));
    assert!(!unknown.prove_at_least(&floor.sub(&remaining), &BigInt::from_i64(1)));
    assert!(!unknown.requires_unsatisfiable);
    let mut contradictory = Engine::for_proof_integer_formation(&program);
    assert!(contradictory.install_hypotheses(vec![
        (BinaryOperator::NotEqual, remaining.clone(), floor.clone()),
        (BinaryOperator::Equal, remaining, floor),
    ]));
    assert!(contradictory.requires_unsatisfiable);
}

#[test]
fn constant_multiplication_preserves_one_sided_bounds() {
    let positive = Interval {
        low: Some(BigInt::from_i64(2)),
        high: None,
    };
    let one = Interval::constant(BigInt::from_i64(1));
    assert_eq!(one.multiply(&positive), positive);
    assert_eq!(positive.multiply(&one), positive);
    let negative = Interval::constant(BigInt::from_i64(-3));
    let reflected = Interval {
        low: None,
        high: Some(BigInt::from_i64(-6)),
    };
    assert_eq!(negative.multiply(&positive), reflected);
    assert_eq!(positive.multiply(&negative), reflected);
    let zero = Interval::constant(BigInt::zero());
    assert_eq!(Interval::unbounded().multiply(&zero), zero);
    assert_eq!(zero.multiply(&Interval::unbounded()), zero);
}

#[test]
fn stored_difference_can_add_two_independently_lower_bounded_atoms() {
    let program = TypedTrees::default();
    let mut engine = Engine::for_proof_integer_formation(&program);
    let ceiling = Polynomial::atom("ceiling".to_owned());
    let bound = Polynomial::atom("bound".to_owned());
    let position = Polynomial::atom("position".to_owned());
    let amount = Polynomial::atom("amount".to_owned());
    assert!(engine.install_hypotheses(vec![
        (
            BinaryOperator::GreaterOrEqual,
            ceiling.clone(),
            bound.clone()
        ),
        (
            BinaryOperator::GreaterOrEqual,
            position.clone(),
            Polynomial::default()
        ),
        (
            BinaryOperator::GreaterOrEqual,
            amount.clone(),
            Polynomial::constant(BigInt::from_i64(1))
        ),
    ]));
    let room = ceiling.sub(&bound).add(&position).add(&amount);
    assert!(engine.prove_at_least(&room, &BigInt::from_i64(1)));
    assert!(!engine.prove_at_least(&room, &BigInt::from_i64(2)));
    assert!(!engine.prove_at_least(
        &ceiling.sub(&bound).add(&position).sub(&amount),
        &BigInt::zero()
    ));
}

#[test]
fn stored_difference_and_positive_residual_prove_three_atom_bound() {
    let program = TypedTrees::default();
    let mut engine = Engine::for_proof_integer_formation(&program);
    let remaining = Polynomial::atom("remaining".to_owned());
    let ceiling = Polynomial::atom("ceiling".to_owned());
    let step = Polynomial::atom("step".to_owned());
    assert!(engine.install_hypotheses(vec![
        (
            BinaryOperator::LessOrEqual,
            remaining.clone(),
            ceiling.clone()
        ),
        (
            BinaryOperator::GreaterOrEqual,
            step.clone(),
            Polynomial::constant(BigInt::from_i64(1)),
        ),
    ]));
    let next_room = ceiling.sub(&remaining).add(&step);
    assert!(!engine.prove_base_lower_bound(&next_room, &BigInt::from_i64(1)));
    assert!(engine.prove_at_least(&next_room, &BigInt::from_i64(1)));
    assert!(!engine.prove_at_least(&next_room, &BigInt::from_i64(2)));
    assert!(!engine.prove_at_least(&ceiling.sub(&remaining).sub(&step), &BigInt::zero(),));
}

#[test]
fn residual_addition_retains_the_stored_bound_offset() {
    let program = TypedTrees::default();
    let mut engine = Engine::for_proof_integer_formation(&program);
    let remaining = Polynomial::atom("remaining".to_owned());
    let ceiling = Polynomial::atom("ceiling".to_owned());
    let step = Polynomial::atom("step".to_owned());
    assert!(engine.install_hypotheses(vec![
        (
            BinaryOperator::GreaterOrEqual,
            ceiling.sub(&remaining),
            Polynomial::constant(BigInt::from_i64(-2)),
        ),
        (
            BinaryOperator::GreaterOrEqual,
            step.clone(),
            Polynomial::constant(BigInt::from_i64(1)),
        ),
    ]));
    let next_room = ceiling.sub(&remaining).add(&step);
    assert!(engine.prove_at_least(&next_room, &BigInt::from_i64(-1)));
    assert!(!engine.prove_at_least(&next_room, &BigInt::zero()));
}
