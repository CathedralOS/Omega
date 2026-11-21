//! The remainder-within-dividend form of `IntegerAffineBound`, a provisional
//! rule (`wiki/spec/proofs/foundation.md`, "Admitting an arithmetic rule").
//!
//! For `result = dividend % divisor`, an exact remainder whose divisor is a
//! value and not a literal, the kernel concludes
//! `min(dividend, 0) <= result <= max(dividend, 0)` once the divisor is
//! proved positive or proved below minus one. The first test evaluates that
//! statement against computed arithmetic for every pair of `i8` and of `u8`
//! operands, including the pairs the side conditions exclude (a zero divisor,
//! and every divisor of minus one, which covers `MIN % -1`); the kernel must
//! refuse those. The rest remove one condition at a time.

use proof_admission::{
    AcceptedProofRule, IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule,
    accept_certificate, check_certificate, check_integer_affine_bound_conversion,
    check_integer_affine_witness, decide_primitive, map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

const DIVIDEND: u64 = 1;
const DIVISOR: u64 = 2;
const RESULT: u64 = 3;
const OTHER: u64 = 4;

fn carrier(sign: IntegerSign) -> IntegerType {
    IntegerType::new(sign, 8).unwrap()
}

fn value(identity: u64, integer: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer),
    )
}

fn literal(integer: IntegerType, number: i128) -> ScalarTerm {
    ScalarTerm::integer(
        integer,
        IntegerValue::for_sign(integer.sign(), number).unwrap(),
    )
    .unwrap()
}

fn context(integer: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types([DIVIDEND, DIVISOR, RESULT, OTHER].map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer),
        )
    }))
    .unwrap()
}

/// `result == dividend % divisor`, the operation's result equation.
fn definition(integer: IntegerType) -> Proposition {
    Proposition::Equal(
        value(RESULT, integer),
        ScalarTerm::exact_integer_remainder(
            integer,
            value(DIVIDEND, integer),
            value(DIVISOR, integer),
        )
        .unwrap(),
    )
}

fn witness(integer: IntegerType) -> IntegerAffineWitness {
    IntegerAffineWitness {
        root: value(DIVIDEND, integer),
        target: value(RESULT, integer),
        definition_axioms: vec![0],
        literal_axioms: vec![None],
    }
}

fn assumption(conclusion: Proposition, index: usize) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::Assumption { index },
    }
}

/// `conclusion` by the form, from proofs of the dividend's bound and of the
/// divisor's premise.
fn bounded(
    integer: IntegerType,
    conclusion: Proposition,
    dividend_bound: ProofNode,
    divisor_premise: ProofNode,
) -> ProofNode {
    let parts = vec![dividend_bound, divisor_premise];
    ProofNode {
        conclusion,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: Proposition::Conjunction(
                    parts.iter().map(|part| part.conclusion.clone()).collect(),
                ),
                rule: ProofRule::ConjunctionIntroduction(parts),
            }),
            witness: witness(integer),
        },
    }
}

/// The four facts that pin both operands: `a <= dividend`, `dividend <= a`,
/// `d <= divisor`, `divisor <= d`.
fn pinned(integer: IntegerType, dividend: i128, divisor: i128) -> [Proposition; 4] {
    let (a, d) = (literal(integer, dividend), literal(integer, divisor));
    [
        Proposition::LessOrEqual(a.clone(), value(DIVIDEND, integer)),
        Proposition::LessOrEqual(value(DIVIDEND, integer), a),
        Proposition::LessOrEqual(d.clone(), value(DIVISOR, integer)),
        Proposition::LessOrEqual(value(DIVISOR, integer), d),
    ]
}

/// `0 < divisor` from `0 < d` and `d <= divisor`, or `divisor < -1` from
/// `divisor <= d` and `d < -1`. The closed relation is false, and the kernel
/// refuses it, exactly when `d` does not satisfy the premise.
fn divisor_premise(
    integer: IntegerType,
    facts: &[Proposition; 4],
    d: i128,
    positive: bool,
) -> ProofNode {
    let closed = |left: i128, right: i128| ProofNode {
        conclusion: Proposition::LessThan(literal(integer, left), literal(integer, right)),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    if positive {
        ProofNode {
            conclusion: Proposition::LessThan(literal(integer, 0), value(DIVISOR, integer)),
            rule: ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: Box::new(closed(0, d)),
                middle_to_right: Box::new(assumption(facts[2].clone(), 2)),
            },
        }
    } else {
        ProofNode {
            conclusion: Proposition::LessThan(value(DIVISOR, integer), literal(integer, -1)),
            rule: ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: Box::new(assumption(facts[3].clone(), 3)),
                middle_to_right: Box::new(closed(d, -1)),
            },
        }
    }
}

fn upper(integer: IntegerType, endpoint: i128) -> Proposition {
    Proposition::LessOrEqual(value(RESULT, integer), literal(integer, endpoint))
}

fn lower(integer: IntegerType, endpoint: i128) -> Proposition {
    Proposition::LessOrEqual(literal(integer, endpoint), value(RESULT, integer))
}

fn range(sign: IntegerSign) -> (i128, i128) {
    match sign {
        IntegerSign::Signed => (i128::from(i8::MIN), i128::from(i8::MAX)),
        IntegerSign::Unsigned => (0, i128::from(u8::MAX)),
    }
}

#[test]
fn the_bounds_agree_with_the_computed_remainder_on_every_pair_of_small_operands() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let integer = carrier(sign);
        let context = context(integer);
        let axioms = [definition(integer)];
        let form = check_integer_affine_witness(&context, &axioms, &witness(integer)).unwrap();
        let (minimum, maximum) = range(sign);
        let (dividend, divisor) = (value(DIVIDEND, integer), value(DIVISOR, integer));
        // The two divisor premises the rule takes, and each one's instance
        // at a concrete divisor, which the kernel decides on literals.
        type Instance = fn(IntegerType, i128) -> Proposition;
        let mut premises: Vec<(Proposition, Instance)> = vec![(
            Proposition::LessThan(literal(integer, 0), divisor.clone()),
            |integer, d| Proposition::LessThan(literal(integer, 0), literal(integer, d)),
        )];
        if sign == IntegerSign::Signed {
            premises.push((
                Proposition::LessThan(divisor.clone(), literal(integer, -1)),
                |integer, d| Proposition::LessThan(literal(integer, d), literal(integer, -1)),
            ));
        }
        let holds = |instance: &Proposition| {
            decide_primitive(&context, instance, PrimitiveJudgment::ClosedIntegerRelation).is_ok()
        };
        let (mut proved, mut refused) = (0u32, 0u32);
        for a in minimum..=maximum {
            let (high, low) = (a.max(0), a.min(0));
            for (premise, _) in &premises {
                // From `dividend == a`, stated as its two bounds, the rule
                // concludes exactly `low <= result` and `result <= high`.
                let from =
                    |bound: Proposition| Proposition::Conjunction(vec![bound, premise.clone()]);
                let above = from(Proposition::LessOrEqual(
                    dividend.clone(),
                    literal(integer, a),
                ));
                let below = from(Proposition::LessOrEqual(
                    literal(integer, a),
                    dividend.clone(),
                ));
                assert_eq!(
                    map_integer_affine_bound(&form, &above),
                    Ok(upper(integer, high))
                );
                assert_eq!(
                    map_integer_affine_bound(&form, &below),
                    Ok(lower(integer, low))
                );
                for (bound, tighter) in [
                    (&above, (high > minimum).then(|| upper(integer, high - 1))),
                    (&below, (low < maximum).then(|| lower(integer, low + 1))),
                ] {
                    if let Some(tighter) = tighter {
                        assert!(
                            check_integer_affine_bound_conversion(&form, bound, &tighter).is_err()
                        );
                    }
                }
            }
            for d in minimum..=maximum {
                // The side condition, decided by the kernel and by
                // arithmetic: some premise holds at this divisor.
                let mut admitted = false;
                for (index, (_, instance)) in premises.iter().enumerate() {
                    let allowed = if index == 0 { d > 0 } else { d < -1 };
                    assert_eq!(holds(&instance(integer, d)), allowed, "{sign:?} {d}");
                    admitted |= allowed;
                }
                if !admitted {
                    // A zero divisor, and every divisor of minus one, which
                    // covers `MIN % -1`: no premise holds, nothing is concluded.
                    assert!(d == 0 || d == -1, "{sign:?} {a} % {d}");
                    refused += 1;
                    continue;
                }
                // The statement, against the remainder the machine computes.
                // `%` on `i128` truncates as the carrier does, and no pair
                // here overflows it.
                let remainder = a % d;
                assert!(low <= remainder && remainder <= high, "{sign:?} {a} % {d}");
                proved += 1;
            }
        }
        let excluded = if sign == IntegerSign::Signed { 2 } else { 1 };
        assert_eq!(refused, 256 * excluded);
        assert_eq!(proved + refused, 256 * 256);
    }
}

#[test]
fn whole_certificates_are_accepted_exactly_where_the_divisor_premise_is_provable() {
    // The same statement through `check_certificate`, with both operands
    // pinned by facts and the divisor premise derived from them, on the
    // operands at and next to the ends of the range and zero.
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let integer = carrier(sign);
        let context = context(integer);
        let axioms = [definition(integer)];
        let (minimum, maximum) = range(sign);
        let edges: Vec<i128> = [minimum, minimum + 1, -2, -1, 0, 1, 2, maximum - 1, maximum]
            .into_iter()
            .filter(|edge| (minimum..=maximum).contains(edge))
            .collect();
        for &a in &edges {
            for &d in &edges {
                let facts = pinned(integer, a, d);
                let accepts = |proof: &ProofNode| {
                    check_certificate(&context, &proof.conclusion, &facts, &axioms, proof).is_ok()
                };
                for positive in [true, false] {
                    if !positive && sign == IntegerSign::Unsigned {
                        continue;
                    }
                    let premise = || divisor_premise(integer, &facts, d, positive);
                    let above = bounded(
                        integer,
                        upper(integer, a.max(0)),
                        assumption(facts[1].clone(), 1),
                        premise(),
                    );
                    let below = bounded(
                        integer,
                        lower(integer, a.min(0)),
                        assumption(facts[0].clone(), 0),
                        premise(),
                    );
                    let allowed = if positive { d > 0 } else { d < -1 };
                    assert_eq!(accepts(&above), allowed, "{sign:?} {a} % {d}");
                    assert_eq!(accepts(&below), allowed, "{sign:?} {a} % {d}");
                }
            }
        }
    }
}

/// `dividend <= 7`, `0 < divisor`, and the certificate of `result <= 7`.
fn small_case(integer: IntegerType) -> ([Proposition; 2], ProofNode) {
    let facts = [
        Proposition::LessOrEqual(value(DIVIDEND, integer), literal(integer, 7)),
        Proposition::LessThan(literal(integer, 0), value(DIVISOR, integer)),
    ];
    let proof = bounded(
        integer,
        upper(integer, 7),
        assumption(facts[0].clone(), 0),
        assumption(facts[1].clone(), 1),
    );
    (facts, proof)
}

#[test]
fn an_acceptance_records_the_provisional_row_and_its_definition() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let integer = carrier(sign);
        let (facts, proof) = small_case(integer);
        let axioms = [definition(integer)];
        let accepted = accept_certificate(
            &context(integer),
            &proof.conclusion,
            &facts,
            &axioms,
            &proof,
        )
        .unwrap();
        assert!(
            accepted
                .rules
                .contains(&AcceptedProofRule::IntegerRemainderWithinDividend)
        );
        assert!(
            !accepted
                .rules
                .contains(&AcceptedProofRule::IntegerAffineBound)
        );
        assert!(accepted.rules.iter().any(|rule| rule.is_provisional()));
        let cited: Vec<usize> = accepted
            .semantic_axioms
            .iter()
            .map(|axiom| axiom.index)
            .collect();
        assert_eq!(cited, [0], "the result equation stays a dependency");
    }
}

#[test]
fn a_missing_weaker_or_misdirected_divisor_premise_is_refused() {
    let integer = carrier(IntegerSign::Signed);
    let context = context(integer);
    let axioms = [definition(integer)];
    let dividend_bound = Proposition::LessOrEqual(value(DIVIDEND, integer), literal(integer, 7));
    let divisor = value(DIVISOR, integer);
    for premise in [
        // Admits a zero divisor.
        Proposition::LessOrEqual(literal(integer, 0), divisor.clone()),
        // Admits minus one, and with it `MIN % -1`.
        Proposition::LessThan(divisor.clone(), literal(integer, 0)),
        Proposition::LessOrEqual(divisor.clone(), literal(integer, -1)),
        // About another value.
        Proposition::LessThan(literal(integer, 0), value(OTHER, integer)),
        Proposition::LessThan(literal(integer, 0), value(DIVIDEND, integer)),
        Proposition::Truth,
    ] {
        let facts = [dividend_bound.clone(), premise.clone()];
        let proof = bounded(
            integer,
            upper(integer, 7),
            assumption(facts[0].clone(), 0),
            assumption(facts[1].clone(), 1),
        );
        assert!(
            check_certificate(&context, &proof.conclusion, &facts, &axioms, &proof).is_err(),
            "{premise:?}"
        );
    }
    // No divisor premise at all: the dividend's bound alone, or `Truth`.
    let facts = [dividend_bound.clone()];
    for root_bound in [
        assumption(dividend_bound.clone(), 0),
        ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        },
    ] {
        let proof = ProofNode {
            conclusion: upper(integer, 7),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(root_bound),
                witness: witness(integer),
            },
        };
        assert!(check_certificate(&context, &proof.conclusion, &facts, &axioms, &proof).is_err());
    }
}

#[test]
fn a_changed_dividend_bound_definition_or_conclusion_is_refused() {
    let integer = carrier(IntegerSign::Signed);
    let context = context(integer);
    let (facts, proof) = small_case(integer);
    let axioms = [definition(integer)];
    check_certificate(&context, &proof.conclusion, &facts, &axioms, &proof).unwrap();

    let refused = |facts: &[Proposition], axioms: &[Proposition], proof: &ProofNode| {
        check_certificate(&context, &proof.conclusion, facts, axioms, proof).is_err()
    };
    // A strict bound on the dividend has no image.
    let strict = [
        Proposition::LessThan(value(DIVIDEND, integer), literal(integer, 7)),
        facts[1].clone(),
    ];
    let proof_from = |facts: &[Proposition; 2], conclusion: Proposition| {
        bounded(
            integer,
            conclusion,
            assumption(facts[0].clone(), 0),
            assumption(facts[1].clone(), 1),
        )
    };
    assert!(refused(
        &strict,
        &axioms,
        &proof_from(&strict, upper(integer, 7))
    ));
    // A bound on another value is not a bound on the dividend.
    let other = [
        Proposition::LessOrEqual(value(OTHER, integer), literal(integer, 7)),
        facts[1].clone(),
    ];
    assert!(refused(
        &other,
        &axioms,
        &proof_from(&other, upper(integer, 7))
    ));
    // An upper bound gives no lower bound, and a negative upper bound still
    // leaves zero reachable.
    assert!(refused(
        &facts,
        &axioms,
        &proof_from(&facts, lower(integer, 0))
    ));
    let negative = [
        Proposition::LessOrEqual(value(DIVIDEND, integer), literal(integer, -3)),
        facts[1].clone(),
    ];
    assert!(refused(
        &negative,
        &axioms,
        &proof_from(&negative, upper(integer, -3))
    ));
    assert!(!refused(
        &negative,
        &axioms,
        &proof_from(&negative, upper(integer, 0))
    ));
    // The result equation must be the cited axiom, and an exact remainder of
    // these operands in this order.
    assert!(refused(&facts, &[], &proof));
    let swapped = [Proposition::Equal(
        value(RESULT, integer),
        ScalarTerm::exact_integer_remainder(
            integer,
            value(DIVISOR, integer),
            value(DIVIDEND, integer),
        )
        .unwrap(),
    )];
    assert!(refused(&facts, &swapped, &proof));
    let quotient = [Proposition::Equal(
        value(RESULT, integer),
        ScalarTerm::exact_integer_divide(
            integer,
            value(DIVIDEND, integer),
            value(DIVISOR, integer),
        )
        .unwrap(),
    )];
    assert!(refused(&facts, &quotient, &proof));
    let wrapping = [Proposition::Equal(
        value(RESULT, integer),
        ScalarTerm::wrapping_integer_remainder(
            integer,
            value(DIVIDEND, integer),
            value(DIVISOR, integer),
        )
        .unwrap(),
    )];
    assert!(refused(&facts, &wrapping, &proof));
}
