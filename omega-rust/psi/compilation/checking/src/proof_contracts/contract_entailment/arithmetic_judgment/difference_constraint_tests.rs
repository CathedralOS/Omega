use super::{BigInt, BinaryOperator, Engine, Polynomial};

#[test]
fn difference_constraints_match_bounded_integer_models() {
    let program = typed_trees::TypedTrees::default();
    let x = Polynomial::atom("x".to_owned());
    let y = Polynomial::atom("y".to_owned());
    let constant = |value| Polynomial::constant(BigInt::from_i64(value));
    let polynomial = |left, right| {
        x.checked_mul(&constant(left))
            .unwrap()
            .add(&y.checked_mul(&constant(right)).unwrap())
    };
    let clauses = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)]
        .into_iter()
        .flat_map(|(left, right)| [-1, 0, 1, 3].map(|minimum| (left, right, minimum)))
        .collect::<Vec<_>>();
    for first in &clauses {
        for second in &clauses {
            let mut comparisons = vec![
                (BinaryOperator::GreaterOrEqual, x.clone(), constant(-2)),
                (BinaryOperator::LessOrEqual, x.clone(), constant(2)),
                (BinaryOperator::GreaterOrEqual, y.clone(), constant(-2)),
                (BinaryOperator::LessOrEqual, y.clone(), constant(2)),
            ];
            for &(left, right, minimum) in [first, second] {
                comparisons.push((
                    BinaryOperator::GreaterOrEqual,
                    polynomial(left, right),
                    constant(minimum),
                ));
            }
            let has_model = (-2..=2).any(|x| {
                (-2..=2).any(|y| {
                    [first, second]
                        .iter()
                        .all(|&&(left, right, minimum)| left * x + right * y >= minimum)
                })
            });
            let mut engine = Engine::for_explicit_bindings(&program);
            assert_eq!(
                engine.integer_comparisons_unsatisfiable(comparisons),
                !has_model,
                "{first:?}, {second:?}",
            );
        }
    }
}

#[test]
fn polynomial_and_opaque_bounds_keep_general_contradiction_search() {
    let program = typed_trees::TypedTrees::default();
    let constant = |value| Polynomial::constant(BigInt::from_i64(value));
    let x = Polynomial::atom("x".to_owned());
    let mut scaled = Engine::for_explicit_bindings(&program);
    assert!(scaled.integer_comparisons_unsatisfiable(vec![
        (
            BinaryOperator::GreaterOrEqual,
            x.checked_mul(&constant(2)).unwrap(),
            constant(6),
        ),
        (BinaryOperator::LessOrEqual, x, constant(2)),
    ]));

    // Refresh derives quotient == 5 after the matrix was closed with quotient <= 4.
    let mut opaque = Engine::for_explicit_bindings(&program);
    opaque.opaque_terms.push((
        "quotient".to_owned(),
        super::OpaqueTerm::Quotient {
            dividend: constant(10),
            divisor: constant(2),
        },
    ));
    assert!(opaque.integer_comparisons_unsatisfiable(vec![(
        BinaryOperator::LessOrEqual,
        Polynomial::atom("quotient".to_owned()),
        constant(4),
    )]));
}
