use super::*;
use crate::{
    AcceptedProofRule, MathematicalCoreDecision, PrimitiveJudgment, ProofNode, ProofRule,
    accept_certificate, decide_primitive,
};
use semantic_vocabulary::{
    FloatIntegerConversion, IeeeFloatFormat, IntegerSign, IntegerType, ScalarType, ValueId,
};

fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).unwrap()
}
fn scalar(raw: u64, format: IeeeFloatFormat) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::IeeeFloat(format))
}
fn exact(source: ScalarTerm, target: IntegerType) -> Proposition {
    Proposition::IeeeFloatIntegerExact { source, target }
}
fn equal(source: ScalarTerm, literal: IeeeFloatValue) -> Proposition {
    let mut left = source;
    let mut right = ScalarTerm::IeeeFloatLiteral(literal);
    if left > right {
        std::mem::swap(&mut left, &mut right);
    }
    Proposition::ScalarIeeeFloatComparison {
        kind: IeeeFloatComparisonKind::Equal,
        format: literal.format(),
        left,
        right,
    }
}
fn primitive(goal: Proposition) -> ProofNode {
    ProofNode {
        conclusion: goal,
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedFloatIntegerRelation),
    }
}
fn certificate(
    context: &PropositionContext,
    goal: &Proposition,
    equation: &Proposition,
) -> ProofNode {
    ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::FloatIntegerConversion {
            premise: Box::new(ProofNode {
                conclusion: equation.clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
            computed: Some(Box::new(primitive(
                float_integer_conversion_substitution(context, goal, equation).unwrap(),
            ))),
        },
    }
}

#[test]
fn conversion_rule_checks_both_children_and_keeps_only_the_cited_equation() {
    let source = scalar(1, IeeeFloatFormat::Binary64);
    let context =
        PropositionContext::from_value_types([(ValueId::new(1).unwrap(), source.scalar_type())])
            .unwrap();
    let goal = exact(source.clone(), integer(IntegerSign::Signed, 32));
    let equation = equal(
        source.clone(),
        IeeeFloatValue::Binary64(0x4045_0000_0000_0000),
    ); // 42
    let axioms = [equation.clone(), Proposition::Truth];
    let proof = certificate(&context, &goal, &equation);
    let accepted = accept_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
    assert_eq!(accepted.semantic_axioms.len(), 1);
    assert_eq!(accepted.semantic_axioms[0].index, 0);
    assert!(accepted.assumptions.is_empty());
    assert!(
        accepted
            .rules
            .contains(&AcceptedProofRule::FloatIntegerConversion)
    );
    assert!(AcceptedProofRule::FloatIntegerConversion.is_provisional());
    assert!(matches!(
        accepted.mathematical_core,
        MathematicalCoreDecision::Refused(_)
    ));
    for child in [0, 1] {
        let mut forged = proof.clone();
        let ProofRule::FloatIntegerConversion { premise, computed } = &mut forged.rule else {
            unreachable!()
        };
        if child == 0 {
            premise.rule = ProofRule::SemanticAxiom { index: 1 };
        } else {
            computed.as_mut().unwrap().rule = ProofRule::Primitive(PrimitiveJudgment::Truth);
        }
        assert!(accept_certificate(&context, &goal, &[], &axioms, &forged).is_err());
    }
    for bits in [
        0,
        0x8000_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0x7ff8_0000_0000_0001,
    ] {
        assert!(
            float_integer_conversion_substitution(
                &context,
                &goal,
                &equal(source.clone(), IeeeFloatValue::Binary64(bits))
            )
            .is_err()
        );
    }
    let raw = Proposition::Equal(
        source.clone(),
        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x4045_0000_0000_0000)),
    );
    assert!(float_integer_conversion_substitution(&context, &goal, &raw).is_err());
    let mut changed = proof;
    let ProofRule::FloatIntegerConversion { computed, .. } = &mut changed.rule else {
        unreachable!()
    };
    computed.as_mut().unwrap().conclusion = exact(
        ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x4045_0000_0000_0000)),
        integer(IntegerSign::Signed, 16),
    );
    assert!(accept_certificate(&context, &goal, &[], &axioms, &changed).is_err());
}

fn graph(value: IeeeFloatValue, result: i128, saturating: bool) -> Proposition {
    let target = integer(IntegerSign::Signed, 32);
    let operation = FloatSemanticOperation::from_source_identity(
        "FloatSemantics",
        if saturating {
            "to_i32_saturating"
        } else {
            "to_i32"
        },
        &[numerics::float_semantics::FloatSemanticValueKind::Meaning],
        numerics::float_semantics::FloatSemanticValueKind::Integer(
            numerics::integer_carrier::FixedInteger::signed(32),
        ),
    )
    .unwrap();
    Proposition::FloatIntegerConversion(
        FloatIntegerConversion::new(
            ScalarTerm::IeeeFloatLiteral(value),
            ScalarTerm::Integer {
                scalar_type: target,
                value: IntegerValue::Signed(result),
            },
            operation.contract_identity(),
        )
        .unwrap(),
    )
}

#[test]
fn exactness_is_stronger_than_the_named_graph_and_resources_never_mean_false() {
    let target = integer(IntegerSign::Signed, 32);
    for (bits, result, eligible) in [
        (0x3ff8_0000_0000_0000, 1, false), // 1.5 truncates
        (0x8000_0000_0000_0000, 0, false), // negative zero loses meaning
        (0, 0, true),
        (0xc1e0_0000_0000_0000, -2147483648, true),
        (0x41df_ffff_ffc0_0000, 2147483647, true),
    ] {
        let value = IeeeFloatValue::Binary64(bits);
        assert_eq!(
            closed_float_integer_relation(&exact(ScalarTerm::IeeeFloatLiteral(value), target))
                .unwrap(),
            Some(eligible)
        );
        assert_eq!(
            closed_float_integer_relation(&graph(value, result, false)).unwrap(),
            Some(true)
        );
    }
    for bits in [
        0x7ff8_0000_0000_0001,
        0x7ff0_0000_0000_0000,
        0x41e0_0000_0000_0000,
    ] {
        let value = IeeeFloatValue::Binary64(bits);
        assert_eq!(
            closed_float_integer_relation(&graph(value, 0, false)).unwrap(),
            Some(false)
        );
        assert_eq!(
            closed_float_integer_relation(&exact(ScalarTerm::IeeeFloatLiteral(value), target))
                .unwrap(),
            Some(false)
        );
    }
    assert!(matches!(
        closed_with_endpoint_budget(
            &graph(IeeeFloatValue::Binary64(0x7ff0_0000_0000_0000), 0, true),
            0
        ),
        Err(FloatIntegerError::Semantic(
            FloatSemanticEvaluationError::IntegerConversion(
                FloatToIntegerError::MaterializationLimit { .. }
            )
        ))
    ));
    assert_eq!(
        closed_float_integer_relation(&exact(scalar(1, IeeeFloatFormat::Binary64), target))
            .unwrap(),
        None
    );
}

// Independent complete miniature IEEE model: sign1/exponent2/fraction2, bias1.
// Integer quarters encode finite values. Neither the oracle nor the exact
// embedding below calls Omega conversion kernels or host floating arithmetic.
#[derive(Clone, Copy, Debug)]
enum Mini {
    Finite(i32),
    Zero(bool),
    Infinity,
    NaN,
}
fn mini(bits: u32) -> Mini {
    let negative = bits & 16 != 0;
    let exponent = bits >> 2 & 3;
    let fraction = bits & 3;
    if exponent == 3 {
        return if fraction == 0 {
            Mini::Infinity
        } else {
            Mini::NaN
        };
    }
    if exponent == 0 && fraction == 0 {
        return Mini::Zero(negative);
    }
    let quarters = if exponent == 0 {
        fraction
    } else {
        (4 + fraction) << (exponent - 1)
    } as i32;
    Mini::Finite(if negative { -quarters } else { quarters })
}
fn embedded(bits: u32, format: IeeeFloatFormat) -> IeeeFloatValue {
    let (fraction_bits, exponent_bits, bias) = match format {
        IeeeFloatFormat::Binary32 => (23, 8, 127),
        IeeeFloatFormat::Binary64 => (52, 11, 1023),
    };
    let sign = if bits & 16 != 0 {
        1_u64 << (fraction_bits + exponent_bits)
    } else {
        0
    };
    let infinity = ((1_u64 << exponent_bits) - 1) << fraction_bits;
    let value = match mini(bits) {
        Mini::Zero(_) => sign,
        Mini::Infinity => sign | infinity,
        Mini::NaN => sign | infinity | (u64::from(bits & 3) << (fraction_bits - 2)),
        Mini::Finite(quarters) => {
            let magnitude = u64::from(quarters.unsigned_abs());
            let leading = 63 - magnitude.leading_zeros();
            let exponent = leading as i32 - 2;
            let significand = magnitude << (fraction_bits - leading);
            sign | (((exponent + bias) as u64) << fraction_bits)
                | (significand & ((1_u64 << fraction_bits) - 1))
        }
    };
    match format {
        IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(value as u32),
        IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(value),
    }
}

#[test]
fn conversion_certificates_match_all_independent_miniature_encodings() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        for (sign, lower, upper) in [(IntegerSign::Signed, -2, 1), (IntegerSign::Unsigned, 0, 3)] {
            let target = integer(sign, 2);
            let source = scalar(1, format);
            let context = PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                source.scalar_type(),
            )])
            .unwrap();
            let mut successes = 0;
            for bits in 0..32 {
                let expected = match mini(bits) {
                    Mini::Zero(negative) => !negative,
                    Mini::Finite(quarters) => {
                        quarters % 4 == 0 && (lower..=upper).contains(&(quarters / 4))
                    }
                    Mini::Infinity | Mini::NaN => false,
                };
                let literal = embedded(bits, format);
                let closed = exact(ScalarTerm::IeeeFloatLiteral(literal), target);
                assert_eq!(
                    decide_primitive(
                        &context,
                        &closed,
                        PrimitiveJudgment::ClosedFloatIntegerRelation
                    )
                    .is_ok(),
                    expected,
                    "{format:?} {sign:?} bits{bits}"
                );
                if let Mini::Finite(_) = mini(bits) {
                    let goal = exact(source.clone(), target);
                    let equation = equal(source.clone(), literal);
                    let proof = certificate(&context, &goal, &equation);
                    assert_eq!(
                        accept_certificate(&context, &goal, &[], &[equation], &proof).is_ok(),
                        expected
                    );
                }
                // Exact eligibility must independently justify every original
                // converter precondition, including its IEEE NaN exclusion.
                let closed_source = ScalarTerm::IeeeFloatLiteral(literal);
                for goal in semantic_vocabulary::float_integer_preconditions(
                    closed_source,
                    semantic_vocabulary::fixed_integer_type(target).unwrap(),
                )
                .unwrap()
                {
                    let proof = ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::FloatIntegerConversion {
                            premise: Box::new(primitive(closed.clone())),
                            computed: None,
                        },
                    };
                    assert_eq!(
                        accept_certificate(&context, &goal, &[], &[], &proof).is_ok(),
                        expected
                    );
                    if expected {
                        decide_primitive(
                            &context,
                            &goal,
                            PrimitiveJudgment::ClosedIeeeFloatRelation,
                        )
                        .unwrap();
                    }
                }
                successes += usize::from(expected);
            }
            assert_eq!(
                successes, 4,
                "all32 encodings; mini carriers test arithmetic, not sealed named declarations"
            );
        }
    }
}

#[test]
fn converter_preconditions_need_the_checked_exact_source_and_destination() {
    let source = scalar(1, IeeeFloatFormat::Binary64);
    let target = integer(IntegerSign::Signed, 32);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), source.scalar_type()),
        (ValueId::new(2).unwrap(), source.scalar_type()),
    ])
    .unwrap();
    let eligibility = exact(source.clone(), target);
    let goal = semantic_vocabulary::float_integer_preconditions(
        source.clone(),
        semantic_vocabulary::fixed_integer_type(target).unwrap(),
    )
    .unwrap()
    .remove(1);
    let make = |premise: &Proposition| ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::FloatIntegerConversion {
            premise: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            computed: None,
        },
    };
    assert!(
        accept_certificate(
            &context,
            &goal,
            std::slice::from_ref(&eligibility),
            &[],
            &make(&eligibility)
        )
        .is_ok()
    );
    for wrong in [
        exact(scalar(2, IeeeFloatFormat::Binary64), target),
        exact(source.clone(), integer(IntegerSign::Unsigned, 32)),
        equal(source.clone(), IeeeFloatValue::Binary64(0)),
    ] {
        assert!(
            accept_certificate(
                &context,
                &goal,
                std::slice::from_ref(&wrong),
                &[],
                &make(&wrong)
            )
            .is_err()
        );
    }
}

#[test]
fn converter_lower_bound_switches_only_when_minimum_minus_one_rounds_to_minimum() {
    for (format, bits, source, inclusive) in [
        (
            IeeeFloatFormat::Binary32,
            32,
            IeeeFloatValue::Binary32(0xcf00_0000),
            true,
        ),
        (
            IeeeFloatFormat::Binary64,
            32,
            IeeeFloatValue::Binary64(0xc1e0_0000_0000_0000),
            false,
        ),
        (
            IeeeFloatFormat::Binary64,
            64,
            IeeeFloatValue::Binary64(0xc3e0_0000_0000_0000),
            true,
        ),
    ] {
        let target = integer(IntegerSign::Signed, bits);
        let term = ScalarTerm::IeeeFloatLiteral(source);
        assert_eq!(term.scalar_type(), ScalarType::IeeeFloat(format));
        let conditions = semantic_vocabulary::float_integer_preconditions(
            term,
            semantic_vocabulary::fixed_integer_type(target).unwrap(),
        )
        .unwrap();
        let Proposition::ScalarIeeeFloatComparison { kind, .. } = conditions[2] else {
            unreachable!()
        };
        assert_eq!(kind == IeeeFloatComparisonKind::LessOrEqual, inclusive);
        for condition in conditions {
            decide_primitive(
                &PropositionContext::from_value_types([]).unwrap(),
                &condition,
                PrimitiveJudgment::ClosedIeeeFloatRelation,
            )
            .unwrap();
        }
    }
}
