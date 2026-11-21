//! The integer-multiple remainder form of `IntegerAffineBound`, a provisional
//! rule (`wiki/spec/proofs/foundation.md`, "Admitting an arithmetic rule").
//!
//! For an exact remainder by a nonzero literal divisor (excluding signed `-1`),
//! when the dividend expands over exact add, subtract, multiply, widen and
//! exact cast into a polynomial whose every coefficient is divisible by the
//! divisor, the kernel concludes `0 <= remainder` and `remainder <= 0`. The
//! first test evaluates that statement over every `(value, divisor)` pair of
//! `i8` and of `u8` against computed arithmetic, including zero divisors,
//! signed `MIN % -1`, overflowing intermediate products, and non-multiples.

use proof_admission::{
    AcceptedProofRule, IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule,
    accept_certificate, check_certificate, check_integer_affine_bound_conversion,
    check_integer_affine_witness, decide_primitive,
};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};

const COUNT: u64 = 1;
const PRODUCT: u64 = 2;
const SUM: u64 = 3;
const NARROWED: u64 = 4;
const REMAINDER: u64 = 5;

fn carrier(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).unwrap()
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

fn truth() -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    }
}

fn assumption(conclusion: Proposition, index: usize) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::Assumption { index },
    }
}

fn range(sign: IntegerSign) -> (i128, i128) {
    match sign {
        IntegerSign::Signed => (i128::from(i8::MIN), i128::from(i8::MAX)),
        IntegerSign::Unsigned => (0, i128::from(u8::MAX)),
    }
}

fn computed_remainder_8bit(sign: IntegerSign, dividend: i128, divisor: i128) -> Option<i128> {
    match sign {
        IntegerSign::Signed => {
            let left = i8::try_from(dividend).ok()?;
            let right = i8::try_from(divisor).ok()?;
            left.checked_rem(right).map(i128::from)
        }
        IntegerSign::Unsigned => {
            let left = u8::try_from(dividend).ok()?;
            let right = u8::try_from(divisor).ok()?;
            left.checked_rem(right).map(i128::from)
        }
    }
}

#[test]
fn zero_remainder_agrees_with_computed_arithmetic_across_every_small_width_value() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let integer = carrier(sign, 8);
        let context = PropositionContext::from_value_types([
            (ValueId::new(COUNT).unwrap(), ScalarType::Integer(integer)),
            (ValueId::new(PRODUCT).unwrap(), ScalarType::Integer(integer)),
            (
                ValueId::new(REMAINDER).unwrap(),
                ScalarType::Integer(integer),
            ),
        ])
        .unwrap();
        let (minimum, maximum) = range(sign);
        let zero = literal(integer, 0);
        let mut accepted_pairs = 0u32;
        let mut refused_pairs = 0u32;

        // Exhaust all 65,536 `(a, d)` pairs of the 8-bit carrier for `a * d % d`
        // and near-miss `(a * 2 + 1) % d`.
        for divisor_number in minimum..=maximum {
            let divisor = literal(integer, divisor_number);
            let scaled =
                ScalarTerm::exact_integer_multiply(integer, value(COUNT, integer), divisor.clone())
                    .unwrap();
            let axioms = [
                Proposition::Equal(value(PRODUCT, integer), scaled.clone()),
                Proposition::Equal(
                    value(REMAINDER, integer),
                    ScalarTerm::exact_integer_remainder(
                        integer,
                        value(PRODUCT, integer),
                        divisor.clone(),
                    )
                    .unwrap(),
                ),
            ];
            let witness = IntegerAffineWitness {
                root: value(REMAINDER, integer),
                target: value(REMAINDER, integer),
                definition_axioms: vec![0, 1],
                literal_axioms: vec![None, None],
            };
            let checked = check_integer_affine_witness(&context, &axioms, &witness);
            let expected_admitted =
                divisor_number != 0 && !(sign == IntegerSign::Signed && divisor_number == -1);
            assert_eq!(
                checked.is_ok(),
                expected_admitted,
                "{sign:?} divisor {divisor_number}"
            );

            for operand in minimum..=maximum {
                let product = operand * divisor_number;
                let in_carrier = (minimum..=maximum).contains(&product);
                if let Ok(form) = &checked {
                    assert!(
                        check_integer_affine_bound_conversion(
                            form,
                            &Proposition::Truth,
                            &Proposition::LessOrEqual(value(REMAINDER, integer), zero.clone()),
                        )
                        .is_ok()
                    );
                    assert!(
                        check_integer_affine_bound_conversion(
                            form,
                            &Proposition::Truth,
                            &Proposition::LessOrEqual(zero.clone(), value(REMAINDER, integer)),
                        )
                        .is_ok()
                    );
                    if in_carrier {
                        assert_eq!(
                            computed_remainder_8bit(sign, product, divisor_number),
                            Some(0),
                            "{sign:?} operand {operand} divisor {divisor_number}"
                        );
                        accepted_pairs += 1;
                    }
                } else {
                    refused_pairs += 1;
                }

                // Inline expression without an operation axiom requires the
                // product to fit the 8-bit carrier, refusing every overflow.
                let inline_target =
                    ScalarTerm::exact_integer_remainder(integer, scaled.clone(), divisor.clone())
                        .unwrap();
                let inline_witness = IntegerAffineWitness {
                    root: inline_target.clone(),
                    target: inline_target,
                    definition_axioms: Vec::new(),
                    literal_axioms: Vec::new(),
                };
                if expected_admitted {
                    let inline_form =
                        check_integer_affine_witness(&context, &[], &inline_witness).unwrap();
                    let bounds = inline_form.multiple_remainder_bounds().unwrap();
                    assert_eq!(bounds.len(), 2);
                    let closed_math = IntegerMathTerm::Multiply(
                        Box::new(IntegerMathTerm::literal(
                            IntegerValue::for_sign(sign, operand).unwrap(),
                        )),
                        Box::new(IntegerMathTerm::literal(
                            IntegerValue::for_sign(sign, divisor_number).unwrap(),
                        )),
                    );
                    let lower_holds = decide_primitive(
                        &context,
                        &Proposition::IntegerMathLessOrEqual(
                            IntegerMathTerm::literal(integer.minimum_value().unwrap()),
                            closed_math.clone(),
                        ),
                        PrimitiveJudgment::ClosedIntegerRelation,
                    )
                    .is_ok();
                    let upper_holds = decide_primitive(
                        &context,
                        &Proposition::IntegerMathLessOrEqual(
                            closed_math,
                            IntegerMathTerm::literal(integer.maximum_value().unwrap()),
                        ),
                        PrimitiveJudgment::ClosedIntegerRelation,
                    )
                    .is_ok();
                    assert_eq!(lower_holds && upper_holds, in_carrier);
                }
            }
        }
        assert!(accepted_pairs > 0 && refused_pairs >= 256);
    }
}

#[test]
fn cast_and_axiom_chains_certify_zero_remainder_and_record_provisional_rule() {
    let u64_ty = carrier(IntegerSign::Unsigned, 64);
    let u16_ty = carrier(IntegerSign::Unsigned, 16);
    let context = PropositionContext::from_value_types([
        (ValueId::new(COUNT).unwrap(), ScalarType::Integer(u64_ty)),
        (ValueId::new(PRODUCT).unwrap(), ScalarType::Integer(u64_ty)),
        (ValueId::new(SUM).unwrap(), ScalarType::Integer(u64_ty)),
        (ValueId::new(NARROWED).unwrap(), ScalarType::Integer(u16_ty)),
    ])
    .unwrap();

    let mul_expr =
        ScalarTerm::exact_integer_multiply(u64_ty, value(COUNT, u64_ty), literal(u64_ty, 2))
            .unwrap();
    let add_expr =
        ScalarTerm::exact_integer_add(u64_ty, value(PRODUCT, u64_ty), literal(u64_ty, 0)).unwrap();
    let cast_expr = ScalarTerm::IntegerExactCast {
        source_type: u64_ty,
        target_type: u16_ty,
        operand: Box::new(value(SUM, u64_ty)),
    };
    let axioms = [
        Proposition::Equal(value(PRODUCT, u64_ty), mul_expr),
        Proposition::Equal(value(SUM, u64_ty), add_expr),
        Proposition::Equal(value(NARROWED, u16_ty), cast_expr),
    ];
    let remainder =
        ScalarTerm::exact_integer_remainder(u16_ty, value(NARROWED, u16_ty), literal(u16_ty, 2))
            .unwrap();
    let zero = literal(u16_ty, 0);
    let witness = IntegerAffineWitness {
        root: remainder.clone(),
        target: remainder.clone(),
        definition_axioms: vec![0, 1, 2],
        literal_axioms: vec![None, None, None],
    };
    let upper_goal = Proposition::LessOrEqual(remainder.clone(), zero.clone());
    let lower_goal = Proposition::LessOrEqual(zero.clone(), remainder.clone());
    for goal in [upper_goal, lower_goal] {
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(truth()),
                witness: witness.clone(),
            },
        };
        let accepted = accept_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        assert!(
            accepted
                .rules
                .contains(&AcceptedProofRule::IntegerMultipleRemainder)
        );
        assert!(
            !accepted
                .rules
                .contains(&AcceptedProofRule::IntegerAffineBound)
        );
        assert!(AcceptedProofRule::IntegerMultipleRemainder.is_provisional());
        assert_eq!(accepted.semantic_axioms.len(), 3);
    }

    // Inline unfolded cast expression requires all exact representability bounds.
    let unfolded_dividend = ScalarTerm::IntegerExactCast {
        source_type: u64_ty,
        target_type: u16_ty,
        operand: Box::new(
            ScalarTerm::exact_integer_add(
                u64_ty,
                ScalarTerm::exact_integer_multiply(
                    u64_ty,
                    value(COUNT, u64_ty),
                    literal(u64_ty, 2),
                )
                .unwrap(),
                literal(u64_ty, 0),
            )
            .unwrap(),
        ),
    };
    let unfolded_remainder =
        ScalarTerm::exact_integer_remainder(u16_ty, unfolded_dividend, literal(u16_ty, 2)).unwrap();
    let inline_witness = IntegerAffineWitness {
        root: unfolded_remainder.clone(),
        target: unfolded_remainder.clone(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let form = check_integer_affine_witness(&context, &[], &inline_witness).unwrap();
    let required = form.multiple_remainder_bounds().unwrap().to_vec();
    assert_eq!(required.len(), 5);
    let children: Vec<ProofNode> = required
        .iter()
        .enumerate()
        .map(|(idx, prop)| assumption(prop.clone(), idx))
        .collect();
    let conjunction = ProofNode {
        conclusion: Proposition::Conjunction(required.clone()),
        rule: ProofRule::ConjunctionIntroduction(children),
    };
    let goal = Proposition::LessOrEqual(unfolded_remainder, zero);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(conjunction),
            witness: inline_witness.clone(),
        },
    };
    let accepted = accept_certificate(&context, &goal, &required, &[], &proof).unwrap();
    assert!(
        accepted
            .rules
            .contains(&AcceptedProofRule::IntegerMultipleRemainder)
    );
    assert_eq!(accepted.assumptions.len(), 5);

    // Forged `Truth` child without the required carrier bounds must reject.
    let forged = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(truth()),
            witness: inline_witness,
        },
    };
    assert!(check_certificate(&context, &goal, &required, &[], &forged).is_err());
}

#[test]
fn near_misses_wrapping_saturating_and_unused_axioms_are_refused() {
    let u64_ty = carrier(IntegerSign::Unsigned, 64);
    let i16_ty = carrier(IntegerSign::Signed, 16);
    let context = PropositionContext::from_value_types([
        (ValueId::new(COUNT).unwrap(), ScalarType::Integer(u64_ty)),
        (ValueId::new(PRODUCT).unwrap(), ScalarType::Integer(u64_ty)),
        (ValueId::new(NARROWED).unwrap(), ScalarType::Integer(i16_ty)),
    ])
    .unwrap();

    // Odd offset `count * 2 + 1` modulo 2 must reject.
    let odd = ScalarTerm::exact_integer_remainder(
        u64_ty,
        ScalarTerm::exact_integer_add(
            u64_ty,
            ScalarTerm::exact_integer_multiply(u64_ty, value(COUNT, u64_ty), literal(u64_ty, 2))
                .unwrap(),
            literal(u64_ty, 1),
        )
        .unwrap(),
        literal(u64_ty, 2),
    )
    .unwrap();
    assert!(
        check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root: odd.clone(),
                target: odd,
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        )
        .is_err()
    );

    // Wrapping and saturating multiples can lose parity and must reject.
    for policy_multiple in [
        ScalarTerm::wrapping_integer_multiply(u64_ty, value(COUNT, u64_ty), literal(u64_ty, 2))
            .unwrap(),
        ScalarTerm::saturating_integer_multiply(u64_ty, value(COUNT, u64_ty), literal(u64_ty, 2))
            .unwrap(),
    ] {
        let remainder =
            ScalarTerm::exact_integer_remainder(u64_ty, policy_multiple, literal(u64_ty, 2))
                .unwrap();
        assert!(
            check_integer_affine_witness(
                &context,
                &[],
                &IntegerAffineWitness {
                    root: remainder.clone(),
                    target: remainder,
                    definition_axioms: Vec::new(),
                    literal_axioms: Vec::new(),
                },
            )
            .is_err()
        );
    }

    // Signed divisor `-1` and zero divisor must reject.
    for bad_divisor in [0_i128, -1_i128] {
        let remainder = ScalarTerm::exact_integer_remainder(
            i16_ty,
            ScalarTerm::exact_integer_multiply(i16_ty, value(NARROWED, i16_ty), literal(i16_ty, 2))
                .unwrap(),
            literal(i16_ty, bad_divisor),
        )
        .unwrap();
        assert!(
            check_integer_affine_witness(
                &context,
                &[],
                &IntegerAffineWitness {
                    root: remainder.clone(),
                    target: remainder,
                    definition_axioms: Vec::new(),
                    literal_axioms: Vec::new(),
                },
            )
            .is_err()
        );
    }

    // Unused extra axiom in `definition_axioms` must reject.
    let axioms = [
        Proposition::Equal(
            value(PRODUCT, u64_ty),
            ScalarTerm::exact_integer_multiply(u64_ty, value(COUNT, u64_ty), literal(u64_ty, 2))
                .unwrap(),
        ),
        Proposition::Equal(value(NARROWED, i16_ty), literal(i16_ty, 4)),
    ];
    let remainder =
        ScalarTerm::exact_integer_remainder(u64_ty, value(PRODUCT, u64_ty), literal(u64_ty, 2))
            .unwrap();
    assert!(
        check_integer_affine_witness(
            &context,
            &axioms,
            &IntegerAffineWitness {
                root: remainder.clone(),
                target: remainder,
                definition_axioms: vec![0, 1],
                literal_axioms: vec![None, None],
            },
        )
        .is_err()
    );
}
