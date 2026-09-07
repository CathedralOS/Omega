use super::*;

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
