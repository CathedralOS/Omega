//! The quotient-at-most-dividend form of `IntegerAffineBound`, a provisional
//! rule (`wiki/spec/proofs/foundation.md`, "Admitting an arithmetic rule").
//!
//! For `result = dividend / divisor`, an exact quotient whose divisor is a
//! value, the kernel concludes `result <= dividend` once the dividend is
//! proved at least a nonnegative literal and the divisor proved positive.
//! The first test evaluates that against computed arithmetic for every pair
//! of `i8` and of `u8` operands, including the pairs the side conditions
//! exclude: a zero divisor, a negative divisor (with it `MIN / -1`), and a
//! negative dividend, where the statement is false for some pairs. The rest
//! check whole certificates at the edges and remove one condition at a time.

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

/// `result == dividend / divisor`, the operation's result equation.
fn definition(integer: IntegerType) -> Proposition {
    Proposition::Equal(
        value(RESULT, integer),
        ScalarTerm::exact_integer_divide(
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

/// `result <= dividend` by the form, from proofs of the two premises.
fn related(integer: IntegerType, nonnegative: ProofNode, positive: ProofNode) -> ProofNode {
    let parts = vec![nonnegative, positive];
    ProofNode {
        conclusion: conclusion(integer),
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

fn conclusion(integer: IntegerType) -> Proposition {
    Proposition::LessOrEqual(value(RESULT, integer), value(DIVIDEND, integer))
}

fn at_least(integer: IntegerType, lower: i128) -> Proposition {
    Proposition::LessOrEqual(literal(integer, lower), value(DIVIDEND, integer))
}

fn positive(integer: IntegerType, identity: u64) -> Proposition {
    Proposition::LessThan(literal(integer, 0), value(identity, integer))
}

fn range(sign: IntegerSign) -> (i128, i128) {
    match sign {
        IntegerSign::Signed => (i128::from(i8::MIN), i128::from(i8::MAX)),
        IntegerSign::Unsigned => (0, i128::from(u8::MAX)),
    }
}

#[test]
fn the_relation_agrees_with_the_computed_quotient_on_every_pair_of_small_operands() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let integer = carrier(sign);
        let context = context(integer);
        let axioms = [definition(integer)];
        let form = check_integer_affine_witness(&context, &axioms, &witness(integer)).unwrap();
        let (minimum, maximum) = range(sign);
        let holds = |instance: &Proposition| {
            decide_primitive(&context, instance, PrimitiveJudgment::ClosedIntegerRelation).is_ok()
        };
        let (mut proved, mut refused, mut false_when_refused) = (0u32, 0u32, 0u32);
        let mut above_when_refused = 0u32;
        for a in minimum..=maximum {
            // The dividend premise at `dividend == a`: the rule takes
            // `a <= dividend` exactly when `a` is nonnegative.
            let child =
                Proposition::Conjunction(vec![at_least(integer, a), positive(integer, DIVISOR)]);
            let dividend_allowed = map_integer_affine_bound(&form, &child).is_ok();
            assert_eq!(dividend_allowed, a >= 0, "{sign:?} {a}");
            if dividend_allowed {
                assert_eq!(
                    map_integer_affine_bound(&form, &child),
                    Ok(conclusion(integer))
                );
                // Nothing stronger: not strict, and not the other way.
                for stronger in [
                    Proposition::LessThan(value(RESULT, integer), value(DIVIDEND, integer)),
                    Proposition::LessOrEqual(value(DIVIDEND, integer), value(RESULT, integer)),
                ] {
                    assert!(
                        check_integer_affine_bound_conversion(&form, &child, &stronger).is_err()
                    );
                }
            }
            for d in minimum..=maximum {
                // The divisor premise at `divisor == d`, decided by the
                // kernel on literals and by arithmetic.
                let divisor_allowed = d > 0;
                assert_eq!(
                    holds(&Proposition::LessThan(
                        literal(integer, 0),
                        literal(integer, d)
                    )),
                    divisor_allowed,
                    "{sign:?} {d}"
                );
                // The machine's quotient where it has one: `/` on `i128`
                // truncates as the carrier does. A zero divisor has none,
                // and `MIN / -1` is not a value of the carrier.
                let quotient = (d != 0).then(|| a / d).filter(|q| *q <= maximum);
                if dividend_allowed && divisor_allowed {
                    assert!(quotient.unwrap() <= a, "{sign:?} {a} / {d}");
                    assert!(quotient.unwrap() >= 0, "{sign:?} {a} / {d}");
                    proved += 1;
                } else {
                    refused += 1;
                    if quotient.is_none_or(|quotient| quotient > a) {
                        false_when_refused += 1;
                    }
                    if quotient.is_some_and(|quotient| quotient > a) {
                        above_when_refused += 1;
                    }
                }
            }
        }
        assert_eq!(proved + refused, 256 * 256);
        match sign {
            // 128 nonnegative dividends by 127 positive divisors.
            IntegerSign::Signed => assert_eq!(proved, 128 * 127),
            // Every dividend by every divisor but zero.
            IntegerSign::Unsigned => assert_eq!(proved, 256 * 255),
        }
        // The exclusions are needed: among the refused pairs are ones with no
        // quotient and, on the signed carrier, ones whose quotient is above
        // the dividend (`-4 / 2`, `-1 / -1`).
        assert!(false_when_refused >= 256);
        // `-4 / 2 == -2` is one: a negative dividend below its quotient.
        assert_eq!(above_when_refused > 0, sign == IntegerSign::Signed);
    }
}

/// `a <= dividend`, `dividend <= a`, `d <= divisor`, `divisor <= d`.
fn pinned(integer: IntegerType, dividend: i128, divisor: i128) -> [Proposition; 4] {
    let (a, d) = (literal(integer, dividend), literal(integer, divisor));
    [
        Proposition::LessOrEqual(a.clone(), value(DIVIDEND, integer)),
        Proposition::LessOrEqual(value(DIVIDEND, integer), a),
        Proposition::LessOrEqual(d.clone(), value(DIVISOR, integer)),
        Proposition::LessOrEqual(value(DIVISOR, integer), d),
    ]
}

#[test]
fn whole_certificates_are_accepted_exactly_where_both_premises_are_provable() {
    // Both operands pinned by facts; `0 < divisor` derived from `0 < d` and
    // `d <= divisor`, which the kernel refuses unless `d` is positive. On the
    // operands at and next to the ends of the range and zero, which include
    // `MIN / -1`, `x / 0` and negative dividends.
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
                let divisor_premise = ProofNode {
                    conclusion: positive(integer, DIVISOR),
                    rule: ProofRule::IntegerStrictOrderTransitivity {
                        left_to_middle: Box::new(ProofNode {
                            conclusion: Proposition::LessThan(
                                literal(integer, 0),
                                literal(integer, d),
                            ),
                            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                        }),
                        middle_to_right: Box::new(assumption(facts[2].clone(), 2)),
                    },
                };
                let proof = related(integer, assumption(facts[0].clone(), 0), divisor_premise);
                let accepted =
                    check_certificate(&context, &proof.conclusion, &facts, &axioms, &proof).is_ok();
                assert_eq!(accepted, a >= 0 && d > 0, "{sign:?} {a} / {d}");
            }
        }
    }
}

/// `0 <= dividend`, `0 < divisor`, and the certificate of `result <= dividend`.
fn small_case(integer: IntegerType) -> ([Proposition; 2], ProofNode) {
    let facts = [at_least(integer, 0), positive(integer, DIVISOR)];
    let proof = related(
        integer,
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
                .contains(&AcceptedProofRule::IntegerQuotientAtMostDividend)
        );
        assert!(
            !accepted
                .rules
                .contains(&AcceptedProofRule::IntegerAffineBound)
        );
        assert!(AcceptedProofRule::IntegerQuotientAtMostDividend.is_provisional());
        let cited: Vec<usize> = accepted
            .semantic_axioms
            .iter()
            .map(|axiom| axiom.index)
            .collect();
        assert_eq!(cited, [0], "the result equation stays a dependency");
    }
}

#[test]
fn a_missing_weaker_or_misdirected_premise_is_refused() {
    let integer = carrier(IntegerSign::Signed);
    let context = context(integer);
    let axioms = [definition(integer)];
    let divisor = value(DIVISOR, integer);
    let dividend = value(DIVIDEND, integer);
    let good = (at_least(integer, 0), positive(integer, DIVISOR));
    let refused = |facts: &[Proposition; 2]| {
        let proof = related(
            integer,
            assumption(facts[0].clone(), 0),
            assumption(facts[1].clone(), 1),
        );
        check_certificate(&context, &proof.conclusion, facts, &axioms, &proof).is_err()
    };
    assert!(!refused(&[good.0.clone(), good.1.clone()]));
    // A larger nonnegative lower bound is still a nonnegative dividend.
    assert!(!refused(&[at_least(integer, 5), good.1.clone()]));
    for divisor_premise in [
        // Admits a zero divisor.
        Proposition::LessOrEqual(literal(integer, 0), divisor.clone()),
        // A negative divisor, and with it `MIN / -1`.
        Proposition::LessThan(divisor.clone(), literal(integer, 0)),
        Proposition::LessThan(divisor.clone(), literal(integer, -1)),
        // About another value.
        positive(integer, OTHER),
        positive(integer, DIVIDEND),
        Proposition::Truth,
    ] {
        assert!(
            refused(&[good.0.clone(), divisor_premise.clone()]),
            "{divisor_premise:?}"
        );
    }
    for dividend_premise in [
        // Admits a negative dividend, where `-4 / 2 > -4`.
        at_least(integer, -1),
        // An upper bound says nothing of the sign.
        Proposition::LessOrEqual(dividend.clone(), literal(integer, 7)),
        // Strict, and about another value.
        Proposition::LessThan(literal(integer, 0), dividend.clone()),
        Proposition::LessOrEqual(literal(integer, 0), value(OTHER, integer)),
        Proposition::Truth,
    ] {
        assert!(
            refused(&[dividend_premise.clone(), good.1.clone()]),
            "{dividend_premise:?}"
        );
    }
    // The premises in the other order, one premise alone, or `Truth`.
    assert!(refused(&[good.1.clone(), good.0.clone()]));
    let facts = [good.0.clone(), good.1.clone()];
    for root_bound in [
        assumption(facts[0].clone(), 0),
        assumption(facts[1].clone(), 1),
        ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        },
    ] {
        let proof = ProofNode {
            conclusion: conclusion(integer),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(root_bound),
                witness: witness(integer),
            },
        };
        assert!(check_certificate(&context, &proof.conclusion, &facts, &axioms, &proof).is_err());
    }
}

#[test]
fn a_changed_definition_or_conclusion_is_refused() {
    let integer = carrier(IntegerSign::Signed);
    let context = context(integer);
    let (facts, proof) = small_case(integer);
    let axioms = [definition(integer)];
    check_certificate(&context, &proof.conclusion, &facts, &axioms, &proof).unwrap();
    let refused = |axioms: &[Proposition], proof: &ProofNode| {
        check_certificate(&context, &proof.conclusion, &facts, axioms, proof).is_err()
    };
    // Not strict, not reversed, and not against the divisor.
    for stronger in [
        Proposition::LessThan(value(RESULT, integer), value(DIVIDEND, integer)),
        Proposition::LessOrEqual(value(DIVIDEND, integer), value(RESULT, integer)),
        Proposition::LessOrEqual(value(RESULT, integer), value(DIVISOR, integer)),
    ] {
        let mut changed = proof.clone();
        changed.conclusion = stronger;
        assert!(refused(&axioms, &changed));
    }
    // The result equation must be the cited axiom: an exact quotient of
    // these operands in this order.
    assert!(refused(&[], &proof));
    let operation = |term: ScalarTerm| [Proposition::Equal(value(RESULT, integer), term)];
    let (a, d) = (value(DIVIDEND, integer), value(DIVISOR, integer));
    assert!(refused(
        &operation(ScalarTerm::exact_integer_divide(integer, d.clone(), a.clone()).unwrap()),
        &proof
    ));
    assert!(refused(
        &operation(ScalarTerm::wrapping_integer_divide(integer, a.clone(), d.clone()).unwrap()),
        &proof
    ));
    // A remainder under the same witness is the other form, which takes
    // other premises and concludes a literal bound.
    assert!(refused(
        &operation(ScalarTerm::exact_integer_remainder(integer, a, d).unwrap()),
        &proof
    ));
}
