//! The kernel's verdict on a bound for the result of an exact multiply.
//!
//! No rule is specific to this: three existing rules compose. Given
//! `product = left * right` as semantic axiom 0 and the operand bounds as
//! assumptions,
//!
//! 1. `IntegerAffineBound` bounds the mathematical product term by the least
//!    or greatest of the four corner products (`map_direct_multiply_bound`);
//! 2. `ValueEqualityTransport` rewrites that term to the value the result
//!    equation defines;
//! 3. `IntegerOrderSubstitution` with a reflexive equality restates the
//!    mathematical order as the carrier-typed `product <= literal`.
//!
//! Every test goes through `verify_obligation` on the certificate route.

use proof_admission::{
    AcceptedFactRoute, CertificateEnvelope, EvidenceError, EvidenceRoute,
    IntegerAffineBoundConversionError, IntegerAffineWitness, MathematicalCoreDecision, Obligation,
    ObligationClass, PrimitiveJudgment, ProofError, ProofNode, ProofRule, ProofSystemMarker,
    verify_obligation,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ObligationId,
    Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};

const LEFT: u64 = 1;
const RIGHT: u64 = 2;
const PRODUCT: u64 = 3;
const OTHER: u64 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    Lower,
    Upper,
}

fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("integer type")
}

fn value(integer: IntegerType, id: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(id).expect("value"),
        ScalarType::Integer(integer),
    )
}

fn math_value(integer: IntegerType, id: u64) -> IntegerMathTerm {
    IntegerMathTerm::MathValue {
        source_type: integer,
        value: ValueId::new(id).expect("value"),
    }
}

fn literal(integer: IntegerType, value: IntegerValue) -> ScalarTerm {
    ScalarTerm::integer(integer, value).expect("literal in the carrier")
}

fn context(integer: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types([LEFT, RIGHT, PRODUCT, OTHER].map(|id| {
        (
            ValueId::new(id).expect("value"),
            ScalarType::Integer(integer),
        )
    }))
    .expect("context")
}

fn bound(integer: IntegerType, id: u64, side: Side, endpoint: IntegerValue) -> Proposition {
    match side {
        Side::Lower => Proposition::LessOrEqual(literal(integer, endpoint), value(integer, id)),
        Side::Upper => Proposition::LessOrEqual(value(integer, id), literal(integer, endpoint)),
    }
}

/// `[left >= .0, left <= .1, right >= .0, right <= .1]`, assumptions 0 to 3.
fn intervals(
    integer: IntegerType,
    left: (IntegerValue, IntegerValue),
    right: (IntegerValue, IntegerValue),
) -> Vec<Proposition> {
    vec![
        bound(integer, LEFT, Side::Lower, left.0),
        bound(integer, LEFT, Side::Upper, left.1),
        bound(integer, RIGHT, Side::Lower, right.0),
        bound(integer, RIGHT, Side::Upper, right.1),
    ]
}

fn multiply(integer: IntegerType, left: u64, right: u64) -> ScalarTerm {
    ScalarTerm::exact_integer_multiply(integer, value(integer, left), value(integer, right))
        .expect("exact multiply")
}

/// The result equation `product = left * right`.
fn result_equation(integer: IntegerType) -> Proposition {
    Proposition::Equal(value(integer, PRODUCT), multiply(integer, LEFT, RIGHT))
}

fn assumed(assumptions: &[Proposition], index: usize) -> ProofNode {
    ProofNode {
        conclusion: assumptions[index].clone(),
        rule: ProofRule::Assumption { index },
    }
}

fn truth() -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    }
}

/// The four operand bounds in the order the multiply image reads them: for
/// each operand the bound on the goal's own side first, then the other.
fn operand_bounds(assumptions: &[Proposition], side: Side) -> Vec<ProofNode> {
    let order = match side {
        Side::Lower => [0, 1, 2, 3],
        Side::Upper => [1, 0, 3, 2],
    };
    order.map(|index| assumed(assumptions, index)).to_vec()
}

fn math_order(term: IntegerMathTerm, side: Side, endpoint: IntegerValue) -> Proposition {
    let endpoint = IntegerMathTerm::literal(endpoint);
    match side {
        Side::Lower => Proposition::IntegerMathLessOrEqual(endpoint, term),
        Side::Upper => Proposition::IntegerMathLessOrEqual(term, endpoint),
    }
}

/// Step 1: `left * right` over mathematical integers is bounded by
/// `endpoint`, from the conjunction of `parts`.
fn product_term_bound(
    integer: IntegerType,
    parts: Vec<ProofNode>,
    side: Side,
    endpoint: IntegerValue,
) -> ProofNode {
    let product = IntegerMathTerm::Multiply(
        Box::new(math_value(integer, LEFT)),
        Box::new(math_value(integer, RIGHT)),
    );
    ProofNode {
        conclusion: math_order(product, side, endpoint),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: Proposition::Conjunction(
                    parts.iter().map(|part| part.conclusion.clone()).collect(),
                ),
                rule: ProofRule::ConjunctionIntroduction(parts),
            }),
            witness: IntegerAffineWitness {
                root: value(integer, LEFT),
                target: multiply(integer, LEFT, RIGHT),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    }
}

/// Steps 2 and 3: carry a bound on the product term onto value `subject`
/// through semantic axiom 0, stated in the carrier.
fn onto_value(
    integer: IntegerType,
    term_bound: ProofNode,
    equation: &Proposition,
    subject: u64,
    side: Side,
    endpoint: IntegerValue,
) -> ProofNode {
    let subject_term = math_value(integer, subject);
    let transported = ProofNode {
        conclusion: math_order(subject_term.clone(), side, endpoint),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(term_bound),
            equalities: vec![ProofNode {
                conclusion: equation.clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }],
        },
    };
    ProofNode {
        conclusion: bound(integer, subject, side, endpoint),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(transported),
            equality: Box::new(ProofNode {
                conclusion: Proposition::IntegerMathEqual(subject_term.clone(), subject_term),
                rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
            }),
            endpoint: match side {
                Side::Lower => 1,
                Side::Upper => 0,
            },
        },
    }
}

/// The whole certificate for `product >= endpoint` or `product <= endpoint`.
fn result_bound(
    integer: IntegerType,
    assumptions: &[Proposition],
    side: Side,
    endpoint: IntegerValue,
) -> ProofNode {
    onto_value(
        integer,
        product_term_bound(integer, operand_bounds(assumptions, side), side, endpoint),
        &result_equation(integer),
        PRODUCT,
        side,
        endpoint,
    )
}

fn verdict(
    context: &PropositionContext,
    assumptions: &[Proposition],
    axioms: &[Proposition],
    proof: &ProofNode,
) -> Result<MathematicalCoreDecision, EvidenceError> {
    let fact = verify_obligation(
        context,
        &Obligation {
            id: ObligationId::new(1).expect("obligation"),
            proposition: proof.conclusion.clone(),
            class: ObligationClass::Derivable,
        },
        assumptions,
        axioms,
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(1).expect("identity"),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: proof.clone(),
        }),
    )?;
    let AcceptedFactRoute::CertificateDerived { acceptance, .. } = fact.route else {
        panic!("a certificate is accepted on the certificate route");
    };
    Ok(acceptance.mathematical_core)
}

/// Accepted by the rule checker and judged again by the mathematical core.
fn assert_proved(
    context: &PropositionContext,
    assumptions: &[Proposition],
    axioms: &[Proposition],
    proof: &ProofNode,
) {
    let decision = verdict(context, assumptions, axioms, proof);
    assert!(
        matches!(decision, Ok(MathematicalCoreDecision::Judged(_))),
        "{:?} must be proved, got {decision:?}",
        proof.conclusion
    );
}

fn assert_refused(
    context: &PropositionContext,
    assumptions: &[Proposition],
    axioms: &[Proposition],
    proof: &ProofNode,
    case: &str,
) {
    assert!(
        verdict(context, assumptions, axioms, proof).is_err(),
        "{case}: {:?} must be refused",
        proof.conclusion
    );
}

fn overflow_refusal(error: EvidenceError) -> bool {
    matches!(
        error,
        EvidenceError::Certificate(ProofError::IntegerAffineBoundConversion(
            IntegerAffineBoundConversionError::DirectMultiplyBoundOverflow
        ))
    )
}

#[test]
fn every_sign_combination_gets_the_tight_bound_and_nothing_tighter() {
    // The true extremes come from every point of the box, not from its
    // corners, so the oracle does not share the rule's argument.
    let integer = integer(IntegerSign::Signed, 16);
    let context = context(integer);
    let axioms = [result_equation(integer)];
    let shapes = [(-7_i128, -2_i128), (-3, 4), (2, 5)];
    for left in shapes {
        for right in shapes {
            let products = (left.0..=left.1)
                .flat_map(|a| (right.0..=right.1).map(move |b| a * b))
                .collect::<Vec<_>>();
            let least = *products.iter().min().expect("box");
            let greatest = *products.iter().max().expect("box");
            let assumptions = intervals(
                integer,
                (IntegerValue::Signed(left.0), IntegerValue::Signed(left.1)),
                (IntegerValue::Signed(right.0), IntegerValue::Signed(right.1)),
            );
            let claim = |side, endpoint| {
                result_bound(integer, &assumptions, side, IntegerValue::Signed(endpoint))
            };
            assert_proved(&context, &assumptions, &axioms, &claim(Side::Lower, least));
            assert_proved(
                &context,
                &assumptions,
                &axioms,
                &claim(Side::Upper, greatest),
            );
            let case = format!("{left:?} * {right:?}");
            for (side, tighter) in [(Side::Lower, least + 1), (Side::Upper, greatest - 1)] {
                assert_refused(
                    &context,
                    &assumptions,
                    &axioms,
                    &claim(side, tighter),
                    &case,
                );
            }
            // The corner on the wrong side is not the bound either.
            for (side, swapped) in [(Side::Lower, greatest), (Side::Upper, least)] {
                if least != greatest {
                    assert_refused(
                        &context,
                        &assumptions,
                        &axioms,
                        &claim(side, swapped),
                        &case,
                    );
                }
            }
        }
    }
}

#[test]
fn a_missing_operand_bound_is_refused_or_read_as_the_carrier_bound() {
    let integer = integer(IntegerSign::Signed, 16);
    let context = context(integer);
    let axioms = [result_equation(integer)];
    let signed = IntegerValue::Signed;
    let assumptions = intervals(integer, (signed(2), signed(5)), (signed(3), signed(10)));
    let greatest = signed(50);

    // Fewer than four parts is not an operand box.
    for dropped in 0..4 {
        let mut parts = operand_bounds(&assumptions, Side::Upper);
        parts.remove(dropped);
        let proof = onto_value(
            integer,
            product_term_bound(integer, parts, Side::Upper, greatest),
            &axioms[0],
            PRODUCT,
            Side::Upper,
            greatest,
        );
        assert_refused(&context, &assumptions, &axioms, &proof, "three parts");
    }

    // `Truth` in a slot stands for that operand's carrier endpoint. Without
    // an upper bound on either operand (slots 0 and 2) the product can reach
    // 32767 * 10 or 5 * 32767, so 50 is refused. Without a lower bound
    // (slots 1 and 3) the other operand is still positive, so the product
    // still cannot exceed 5 * 10 and 50 is proved.
    for slot in 0..4 {
        let mut parts = operand_bounds(&assumptions, Side::Upper);
        parts[slot] = truth();
        let proof = onto_value(
            integer,
            product_term_bound(integer, parts, Side::Upper, greatest),
            &axioms[0],
            PRODUCT,
            Side::Upper,
            greatest,
        );
        if slot % 2 == 0 {
            assert_refused(&context, &assumptions, &axioms, &proof, "no upper bound");
        } else {
            assert_proved(&context, &assumptions, &axioms, &proof);
        }
    }

    // With the left operand's upper bound unknown the image uses the
    // carrier maximum: 32767 * 10 is still a fact about the product term.
    let mut parts = operand_bounds(&assumptions, Side::Upper);
    parts[0] = truth();
    let term_bound = product_term_bound(integer, parts, Side::Upper, signed(327_670));
    assert_proved(&context, &assumptions, &axioms, &term_bound);

    // Without the result equation nothing reaches the result value.
    let proof = result_bound(integer, &assumptions, Side::Upper, greatest);
    assert_refused(&context, &assumptions, &[], &proof, "no result equation");
}

#[test]
fn bounds_about_another_value_are_refused() {
    let integer = integer(IntegerSign::Signed, 16);
    let context = context(integer);
    let signed = IntegerValue::Signed;
    let greatest = signed(50);
    let assumptions = intervals(integer, (signed(2), signed(5)), (signed(3), signed(10)));
    let axioms = [result_equation(integer)];

    // The left operand's interval is stated about a value that is not an
    // operand.
    let mut elsewhere = assumptions.clone();
    elsewhere[0] = bound(integer, OTHER, Side::Lower, signed(2));
    elsewhere[1] = bound(integer, OTHER, Side::Upper, signed(5));
    let proof = result_bound(integer, &elsewhere, Side::Upper, greatest);
    assert_refused(&context, &elsewhere, &axioms, &proof, "foreign operand");

    // The bound is claimed for a value the result equation does not define.
    let proof = onto_value(
        integer,
        product_term_bound(
            integer,
            operand_bounds(&assumptions, Side::Upper),
            Side::Upper,
            greatest,
        ),
        &axioms[0],
        OTHER,
        Side::Upper,
        greatest,
    );
    assert_refused(&context, &assumptions, &axioms, &proof, "foreign result");

    // The cited equation defines the product of other operands.
    let other_product = [Proposition::Equal(
        value(integer, PRODUCT),
        multiply(integer, LEFT, OTHER),
    )];
    let proof = onto_value(
        integer,
        product_term_bound(
            integer,
            operand_bounds(&assumptions, Side::Upper),
            Side::Upper,
            greatest,
        ),
        &other_product[0],
        PRODUCT,
        Side::Upper,
        greatest,
    );
    assert_refused(
        &context,
        &assumptions,
        &other_product,
        &proof,
        "foreign multiply",
    );

    // The equation is an add of the same operands, not a multiply.
    let sum = [Proposition::Equal(
        value(integer, PRODUCT),
        ScalarTerm::exact_integer_add(integer, value(integer, LEFT), value(integer, RIGHT))
            .expect("exact add"),
    )];
    let proof = onto_value(
        integer,
        product_term_bound(
            integer,
            operand_bounds(&assumptions, Side::Upper),
            Side::Upper,
            greatest,
        ),
        &sum[0],
        PRODUCT,
        Side::Upper,
        greatest,
    );
    assert_refused(&context, &assumptions, &sum, &proof, "an add equation");
}

#[test]
fn an_operand_needs_one_bound_on_each_side() {
    let integer = integer(IntegerSign::Signed, 16);
    let context = context(integer);
    let axioms = [result_equation(integer)];
    let signed = IntegerValue::Signed;
    let assumptions = intervals(integer, (signed(2), signed(5)), (signed(3), signed(10)));
    // Both slots of an operand filled from the same side, and the two sides
    // in the order of the other goal direction.
    for order in [[0, 0, 3, 2], [1, 1, 3, 2], [1, 0, 2, 2], [0, 1, 3, 2]] {
        let parts = order.map(|index| assumed(&assumptions, index)).to_vec();
        for endpoint in [6, 15, 20, 50] {
            let proof = onto_value(
                integer,
                product_term_bound(integer, parts.clone(), Side::Upper, signed(endpoint)),
                &axioms[0],
                PRODUCT,
                Side::Upper,
                signed(endpoint),
            );
            assert_refused(&context, &assumptions, &axioms, &proof, "slot sides");
        }
    }
}

#[test]
fn an_inverted_interval_proves_only_what_its_contradictory_hypotheses_entail() {
    // `5 <= left` and `left <= 2` hold for no integer. The kernel does not
    // look for that: it reads four endpoints and takes the corner extreme,
    // here 5 * 10. The verdict is an implication from hypotheses no run
    // satisfies, so nothing false about a reachable state follows; a claim
    // other than the corner extreme is still refused.
    let integer = integer(IntegerSign::Signed, 16);
    let context = context(integer);
    let axioms = [result_equation(integer)];
    let signed = IntegerValue::Signed;
    let assumptions = intervals(integer, (signed(5), signed(2)), (signed(3), signed(10)));
    assert_proved(
        &context,
        &assumptions,
        &axioms,
        &result_bound(integer, &assumptions, Side::Upper, signed(50)),
    );
    assert_proved(
        &context,
        &assumptions,
        &axioms,
        &result_bound(integer, &assumptions, Side::Lower, signed(6)),
    );
    for (side, endpoint) in [(Side::Upper, 49), (Side::Upper, 20), (Side::Lower, 7)] {
        assert_refused(
            &context,
            &assumptions,
            &axioms,
            &result_bound(integer, &assumptions, side, signed(endpoint)),
            "inverted interval",
        );
    }
}

#[test]
fn a_corner_product_beyond_the_kernels_integers_is_refused() {
    // Endpoints are a sign and a 128-bit magnitude; the corner product is a
    // checked multiply of magnitudes. 2^128 - 2^64 is the last that fits.
    let integer = integer(IntegerSign::Unsigned, 128);
    let context = context(integer);
    let axioms = [result_equation(integer)];
    let unsigned = IntegerValue::Unsigned;
    let two_64 = 1_u128 << 64;

    let fits = intervals(
        integer,
        (unsigned(0), unsigned(two_64 - 1)),
        (unsigned(0), unsigned(two_64)),
    );
    assert_proved(
        &context,
        &fits,
        &axioms,
        &result_bound(
            integer,
            &fits,
            Side::Upper,
            unsigned(u128::MAX - two_64 + 1),
        ),
    );

    let beyond = intervals(
        integer,
        (unsigned(0), unsigned(two_64)),
        (unsigned(0), unsigned(two_64)),
    );
    // Neither the wrapped product (0), the saturated one, nor any other
    // endpoint is accepted, on either side.
    for (side, endpoint) in [
        (Side::Upper, 0),
        (Side::Upper, u128::MAX),
        (Side::Upper, two_64),
        (Side::Lower, 0),
    ] {
        let term_bound = product_term_bound(
            integer,
            operand_bounds(&beyond, side),
            side,
            unsigned(endpoint),
        );
        let refusal = verdict(&context, &beyond, &axioms, &term_bound)
            .expect_err("a corner product does not fit");
        assert!(overflow_refusal(refusal.clone()), "{refusal:?}");
        assert_refused(
            &context,
            &beyond,
            &axioms,
            &result_bound(integer, &beyond, side, unsigned(endpoint)),
            "corner overflow",
        );
    }

    // The same for a negative corner of the widest signed carrier.
    let integer = self::integer(IntegerSign::Signed, 128);
    let context = self::context(integer);
    let axioms = [result_equation(integer)];
    let signed = IntegerValue::Signed;
    let wide = intervals(
        integer,
        (signed(i128::MIN), signed(0)),
        (signed(0), signed(i128::MAX)),
    );
    for (side, endpoint) in [(Side::Lower, i128::MIN), (Side::Upper, 0)] {
        let term_bound =
            product_term_bound(integer, operand_bounds(&wide, side), side, signed(endpoint));
        let refusal = verdict(&context, &wide, &axioms, &term_bound)
            .expect_err("a corner product does not fit");
        assert!(overflow_refusal(refusal.clone()), "{refusal:?}");
    }
}

#[test]
fn a_bound_beyond_the_carrier_leaves_the_overflow_obligation_unproved() {
    // In `u8`, operands up to 20 give products up to 400. That is a fact
    // about the unbounded product, and it is all the kernel grants: the
    // exact multiply's own obligation `left * right <= 255` stays unproved.
    let integer = integer(IntegerSign::Unsigned, 8);
    let context = context(integer);
    let unsigned = IntegerValue::Unsigned;
    let assumptions = intervals(
        integer,
        (unsigned(0), unsigned(20)),
        (unsigned(0), unsigned(20)),
    );
    let parts = || operand_bounds(&assumptions, Side::Upper);

    let unbounded_fact = product_term_bound(integer, parts(), Side::Upper, unsigned(400));
    assert_proved(&context, &assumptions, &[], &unbounded_fact);

    let representable = product_term_bound(integer, parts(), Side::Upper, unsigned(255));
    assert_refused(&context, &assumptions, &[], &representable, "overflow goal");

    // Nor does it follow by weakening 400 to 255.
    let Proposition::IntegerMathLessOrEqual(product, four_hundred) = &unbounded_fact.conclusion
    else {
        unreachable!()
    };
    let carrier_maximum = IntegerMathTerm::literal(unsigned(255));
    let weakened = ProofNode {
        conclusion: Proposition::IntegerMathLessOrEqual(product.clone(), carrier_maximum.clone()),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(unbounded_fact.clone()),
            middle_less_or_equal_right: Box::new(ProofNode {
                conclusion: Proposition::IntegerMathLessOrEqual(
                    four_hundred.clone(),
                    carrier_maximum,
                ),
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
        },
    };
    assert_refused(&context, &assumptions, &[], &weakened, "400 <= 255");

    // 400 is not a `u8`, so no carrier-typed bound on the result can even
    // state it; the result keeps only its carrier bound.
    assert!(ScalarTerm::integer(integer, unsigned(400)).is_err());
    let axioms = [result_equation(integer)];
    let proof = result_bound(integer, &assumptions, Side::Upper, unsigned(255));
    assert_refused(&context, &assumptions, &axioms, &proof, "result at 255");

    // Operands that do fit prove both: 15 * 17 = 255.
    let fitting = intervals(
        integer,
        (unsigned(0), unsigned(15)),
        (unsigned(0), unsigned(17)),
    );
    assert_proved(
        &context,
        &fitting,
        &[],
        &product_term_bound(
            integer,
            operand_bounds(&fitting, Side::Upper),
            Side::Upper,
            unsigned(255),
        ),
    );
    assert_proved(
        &context,
        &fitting,
        &axioms,
        &result_bound(integer, &fitting, Side::Upper, unsigned(255)),
    );
}
