use super::{integer, two_value_context, value};
use crate::proofs::nonzero_divisor_certificate::{
    Proposition, PropositionContext, prove_canonical_integer_proposition,
};
use proof_admission::{PrimitiveJudgment, ProofRule, accept_certificate};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
};

#[test]
fn exact_add_goal_serializes_two_ordered_endpoint_proofs() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let sum = semantic_vocabulary::IntegerMathTerm::Add(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(1).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(2).unwrap(),
        }),
    );
    let context = two_value_context(integer_type);
    for (goal, assumptions) in [
        (
            Proposition::IntegerMathLessOrEqual(
                semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(-128)),
                sum.clone(),
            ),
            vec![
                Proposition::LessOrEqual(integer(integer_type, -100), left.clone()),
                Proposition::LessOrEqual(integer(integer_type, 20), right.clone()),
            ],
        ),
        (
            Proposition::IntegerMathLessOrEqual(
                sum.clone(),
                semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(127)),
            ),
            vec![
                Proposition::LessOrEqual(left.clone(), integer(integer_type, 100)),
                Proposition::LessOrEqual(right.clone(), integer(integer_type, 20)),
            ],
        ),
    ] {
        let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
            .expect("two operand endpoints prove canonical exact add");
        let ProofRule::IntegerAffineBound { root_bound, .. } = proof.rule else {
            panic!("tight exact-add endpoints need no relaxation")
        };
        let ProofRule::ConjunctionIntroduction(parts) = root_bound.rule else {
            panic!("direct exact add serializes both ordered endpoint children")
        };
        assert_eq!(parts.len(), 2);
        let mentions_endpoint = |proposition: &Proposition, operand: &ScalarTerm| {
            matches!(proposition, Proposition::LessOrEqual(endpoint, _) if endpoint == operand)
                || matches!(proposition, Proposition::LessOrEqual(_, endpoint) if endpoint == operand)
                || matches!(proposition, Proposition::Equal(endpoint, _) if endpoint == operand)
                || matches!(proposition, Proposition::Equal(_, endpoint) if endpoint == operand)
        };
        assert!(mentions_endpoint(&parts[0].conclusion, &left));
        assert!(mentions_endpoint(&parts[1].conclusion, &right));
    }

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &Proposition::IntegerMathLessOrEqual(
                sum,
                semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(127)),
            ),
            &[Proposition::LessOrEqual(left, integer(integer_type, 100),)],
            &[],
        )
        .is_none(),
        "omitting the second operand endpoint cannot certify the addition",
    );
}

#[test]
fn exact_add_goal_derives_embedded_literal_endpoints_in_both_orders() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = two_value_context(integer_type);
    let literal = semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(7));
    for (sum, assumption, truth_index) in [
        (
            semantic_vocabulary::IntegerMathTerm::Add(
                Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(1).unwrap(),
                }),
                Box::new(literal.clone()),
            ),
            Proposition::LessOrEqual(value(1, integer_type), integer(integer_type, 120)),
            1,
        ),
        (
            semantic_vocabulary::IntegerMathTerm::Add(
                Box::new(literal.clone()),
                Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: ValueId::new(2).unwrap(),
                }),
            ),
            Proposition::LessOrEqual(value(2, integer_type), integer(integer_type, 120)),
            0,
        ),
    ] {
        let goal = Proposition::IntegerMathLessOrEqual(
            sum,
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(127)),
        );
        let proof = prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&assumption),
            &[],
        )
        .expect("value plus embedded literal proves the canonical carrier endpoint");
        let ProofRule::IntegerAffineBound {
            root_bound,
            witness,
        } = proof.rule
        else {
            panic!("literal exact add uses direct checked endpoint mapping")
        };
        assert_eq!(
            witness.target.scalar_type(),
            ScalarType::Integer(integer_type)
        );
        let ProofRule::ConjunctionIntroduction(parts) = root_bound.rule else {
            panic!("literal exact add keeps ordered operand evidence")
        };
        assert!(matches!(
            parts[truth_index].rule,
            ProofRule::Primitive(PrimitiveJudgment::Truth)
        ));
        assert!(matches!(
            parts[1 - truth_index].rule,
            ProofRule::Assumption { index: 0 }
        ));
    }
}

#[test]
fn exact_subtract_goal_serializes_independent_and_joint_guards() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(signed))),
    )
    .unwrap();
    let left = value(1, signed);
    let right = value(2, signed);
    let difference = semantic_vocabulary::IntegerMathTerm::Subtract(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: signed,
            value: ValueId::new(1).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: signed,
            value: ValueId::new(2).unwrap(),
        }),
    );
    let lower_goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(-128)),
        difference.clone(),
    );
    let independent = prove_canonical_integer_proposition(
        &context,
        &lower_goal,
        &[
            Proposition::LessOrEqual(integer(signed, -100), left.clone()),
            Proposition::LessOrEqual(right.clone(), integer(signed, 20)),
        ],
        &[],
    )
    .expect("oppositely oriented endpoints prove subtraction");
    let ProofRule::IntegerAffineBound { root_bound, .. } = independent.rule else {
        panic!("direct subtraction uses the checked affine rule")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::ConjunctionIntroduction(ref parts) if parts.len() == 2
    ));

    let complement = value(3, signed);
    let definition = Proposition::Equal(
        complement.clone(),
        ScalarTerm::exact_integer_add(signed, integer(signed, -128), right.clone()).unwrap(),
    );
    let correlated = prove_canonical_integer_proposition(
        &context,
        &lower_goal,
        &[Proposition::LessOrEqual(complement, left)],
        std::slice::from_ref(&definition),
    )
    .expect("MIN plus right guard proves subtraction lower bound");
    let ProofRule::IntegerAffineBound { witness, .. } = correlated.rule else {
        panic!("correlated subtraction uses the checked affine rule")
    };
    assert_eq!(witness.definition_axioms, vec![0]);
    assert_eq!(witness.literal_axioms, vec![None]);

    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let context = two_value_context(unsigned);
    let left = value(1, unsigned);
    let right = value(2, unsigned);
    let goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
        semantic_vocabulary::IntegerMathTerm::Subtract(
            Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: unsigned,
                value: ValueId::new(1).unwrap(),
            }),
            Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: unsigned,
                value: ValueId::new(2).unwrap(),
            }),
        ),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[Proposition::LessOrEqual(right.clone(), left)],
        &[],
    )
    .expect("unsigned right <= left guard proves subtraction");
    let ProofRule::IntegerAffineBound { witness, .. } = proof.rule else {
        panic!("unsigned joint guard uses the checked affine rule")
    };
    assert_eq!(witness.root, right);
    assert!(witness.definition_axioms.is_empty());
}

#[test]
fn direct_correlated_arithmetic_replays_authored_complement_expressions() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let context = two_value_context(integer_type);
    let literal = |value| {
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(value)).expect("u8 literal")
    };
    let add_goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::Add(
            Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(1).unwrap(),
            }),
            Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(2).unwrap(),
            }),
        ),
        semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
    );
    let complement =
        ScalarTerm::exact_integer_subtract(integer_type, literal(255), right.clone()).unwrap();
    let add_assumption = Proposition::LessOrEqual(left.clone(), complement.clone());
    let add = prove_canonical_integer_proposition(
        &context,
        &add_goal,
        std::slice::from_ref(&add_assumption),
        &[],
    )
    .expect("the authored direct MAX-right comparison proves exact addition");
    let ProofRule::IntegerAffineBound { witness, .. } = &add.rule else {
        panic!("direct complement replay uses the checked affine rule")
    };
    assert_eq!(witness.root, complement);
    assert!(witness.definition_axioms.is_empty());
    accept_certificate(
        &context,
        &add_goal,
        std::slice::from_ref(&add_assumption),
        &[],
        &add,
    )
    .expect("the kernel rejoins the direct complement expression");

    let multiply_goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::Multiply(
            Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(1).unwrap(),
            }),
            Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(2).unwrap(),
            }),
        ),
        semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
    );
    let quotient =
        ScalarTerm::exact_integer_divide(integer_type, literal(255), right.clone()).unwrap();
    let multiply_assumptions = [
        Proposition::LessOrEqual(literal(1), right.clone()),
        Proposition::LessOrEqual(left.clone(), quotient.clone()),
    ];
    let multiply =
        prove_canonical_integer_proposition(&context, &multiply_goal, &multiply_assumptions, &[])
            .expect("the authored direct MAX/right comparison proves exact multiplication");
    let ProofRule::IntegerAffineBound { witness, .. } = &multiply.rule else {
        panic!("direct quotient replay uses the checked affine rule")
    };
    assert_eq!(witness.root, quotient);
    assert!(witness.definition_axioms.is_empty());
    accept_certificate(
        &context,
        &multiply_goal,
        &multiply_assumptions,
        &[],
        &multiply,
    )
    .expect("the kernel rejoins the direct quotient expression");

    let drifted_add = Proposition::LessOrEqual(
        left.clone(),
        ScalarTerm::exact_integer_subtract(integer_type, literal(254), right.clone()).unwrap(),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &add_goal,
            std::slice::from_ref(&drifted_add),
            &[],
        )
        .is_none(),
        "a non-carrier complement endpoint remains fenced",
    );
    let drifted_multiply = [
        Proposition::LessOrEqual(literal(1), right.clone()),
        Proposition::LessOrEqual(
            left,
            ScalarTerm::exact_integer_divide(integer_type, literal(254), right).unwrap(),
        ),
    ];
    assert!(
        prove_canonical_integer_proposition(&context, &multiply_goal, &drifted_multiply, &[],)
            .is_none(),
        "a non-carrier quotient endpoint remains fenced",
    );

    let signed_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_left = value(1, signed_type);
    let signed_right = value(2, signed_type);
    let signed_context = two_value_context(signed_type);
    let subtract_goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::Subtract(
            Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: signed_type,
                value: ValueId::new(1).unwrap(),
            }),
            Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: signed_type,
                value: ValueId::new(2).unwrap(),
            }),
        ),
        semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(127)),
    );
    let signed_complement =
        ScalarTerm::exact_integer_add(signed_type, integer(signed_type, 127), signed_right.clone())
            .unwrap();
    let subtract_assumptions = [
        Proposition::LessOrEqual(signed_right.clone(), integer(signed_type, 0)),
        Proposition::LessOrEqual(signed_left, signed_complement.clone()),
    ];
    let subtract = prove_canonical_integer_proposition(
        &signed_context,
        &subtract_goal,
        &subtract_assumptions,
        &[],
    )
    .expect("the authored direct MAX+right comparison proves exact subtraction");
    let ProofRule::IntegerAffineBound { witness, .. } = &subtract.rule else {
        panic!("direct subtraction complement uses the checked affine rule")
    };
    assert_eq!(witness.root, signed_complement);
    assert!(witness.definition_axioms.is_empty());
    accept_certificate(
        &signed_context,
        &subtract_goal,
        &subtract_assumptions,
        &[],
        &subtract,
    )
    .expect("the kernel rejoins the direct subtraction complement");
}

#[test]
fn exact_multiply_goal_serializes_four_corners_and_negative_quotient_guard() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = value(1, integer_type);
    let right = value(2, integer_type);
    let quotient = value(3, integer_type);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let product = semantic_vocabulary::IntegerMathTerm::Multiply(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(1).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(2).unwrap(),
        }),
    );
    let lower_goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(-128)),
        product,
    );
    let direct = prove_canonical_integer_proposition(
        &context,
        &lower_goal,
        &[
            Proposition::LessOrEqual(integer(integer_type, -4), left.clone()),
            Proposition::LessOrEqual(left.clone(), integer(integer_type, 5)),
            Proposition::LessOrEqual(integer(integer_type, -3), right.clone()),
            Proposition::LessOrEqual(right.clone(), integer(integer_type, 2)),
        ],
        &[],
    )
    .expect("four signed corners prove the multiplication lower bound");
    let ProofRule::IntegerLessOrEqualTransitivity {
        middle_less_or_equal_right,
        ..
    } = direct.rule
    else {
        panic!("tight four-corner bound is relaxed to the carrier endpoint")
    };
    let ProofRule::IntegerAffineBound { root_bound, .. } = middle_less_or_equal_right.rule else {
        panic!("four corners use the checked affine boundary")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::ConjunctionIntroduction(ref parts) if parts.len() == 4
    ));

    let quotient_definition = Proposition::Equal(
        quotient.clone(),
        ScalarTerm::exact_integer_divide(integer_type, integer(integer_type, -128), right.clone())
            .unwrap(),
    );
    let correlated = prove_canonical_integer_proposition(
        &context,
        &lower_goal,
        &[
            Proposition::LessOrEqual(right, integer(integer_type, -2)),
            Proposition::LessOrEqual(left, quotient.clone()),
        ],
        std::slice::from_ref(&quotient_definition),
    )
    .expect("negative MIN/right guard proves the multiplication lower bound");
    let ProofRule::IntegerAffineBound {
        root_bound,
        witness,
    } = correlated.rule
    else {
        panic!("correlated multiplication uses the checked affine boundary")
    };
    assert_eq!(witness.definition_axioms, vec![0]);
    assert_eq!(witness.literal_axioms, vec![None]);
    assert!(matches!(
        root_bound.rule,
        ProofRule::ConjunctionIntroduction(ref parts) if parts.len() == 2
    ));
}

#[test]
fn exact_multiply_orients_a_landed_zero_for_both_target_directions() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let right = value(2, integer_type);
    let context = two_value_context(integer_type);
    let zero = integer(integer_type, 0);
    let zero_axiom = Proposition::Equal(right, zero);
    let product = semantic_vocabulary::IntegerMathTerm::Multiply(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(1).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(2).unwrap(),
        }),
    );
    for goal in [
        Proposition::IntegerMathLessOrEqual(
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(-128)),
            product.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            product.clone(),
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(127)),
        ),
    ] {
        let proof = prove_canonical_integer_proposition(
            &context,
            &goal,
            &[],
            std::slice::from_ref(&zero_axiom),
        )
        .expect("landed zero orients the exact product endpoint");
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = proof.rule
        else {
            panic!("the exact zero product is relaxed to the carrier endpoint")
        };
        let mapped = if matches!(
            left_less_or_equal_middle.rule,
            ProofRule::IntegerAffineBound { .. }
        ) {
            left_less_or_equal_middle
        } else {
            middle_less_or_equal_right
        };
        let ProofRule::IntegerAffineBound { root_bound, .. } = mapped.rule else {
            panic!("landed zero uses the checked direct multiply mapper")
        };
        let ProofRule::ConjunctionIntroduction(parts) = root_bound.rule else {
            panic!("direct multiply retains all four endpoint proofs")
        };
        assert!(
            parts
                .iter()
                .any(|part| matches!(part.rule, ProofRule::IntegerOrderSubstitution { .. }))
        );
    }
}

#[test]
fn exact_multiply_zero_accepts_a_definition_local_prefix_carrier_only_when_oriented() {
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(u16_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i8_type)),
    ])
    .unwrap();
    let axioms = [
        Proposition::Equal(
            value(2, i8_type),
            ScalarTerm::integer_exact_cast(u16_type, i8_type, value(1, u16_type))
                .expect("exact cast"),
        ),
        Proposition::Equal(value(3, i8_type), integer(i8_type, -2)),
        Proposition::Equal(
            value(4, i8_type),
            ScalarTerm::exact_integer_multiply(i8_type, value(2, i8_type), value(3, i8_type))
                .expect("definition-local prefix product"),
        ),
        Proposition::Equal(value(5, i8_type), integer(i8_type, 0)),
    ];
    let product = semantic_vocabulary::IntegerMathTerm::Multiply(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i8_type,
            value: ValueId::new(4).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i8_type,
            value: ValueId::new(5).unwrap(),
        }),
    );
    let goal = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i8::MIN.into())),
            product.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            product,
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i8::MAX.into())),
        ),
    ]);
    let proof = prove_canonical_integer_proposition(&context, &goal, &[], &axioms)
        .expect("oriented zero combines with a definition-local carrier marker");
    accept_certificate(&context, &goal, &[], &axioms, &proof)
        .expect("the checker derives the carrier endpoint and replays the oriented zero");

    let mut nonzero_axioms = axioms.clone();
    nonzero_axioms[3] = Proposition::Equal(value(5, i8_type), integer(i8_type, 2));
    assert!(
        accept_certificate(&context, &goal, &[], &nonzero_axioms, &proof).is_err(),
        "a nonzero unoriented pair cannot reuse the zero-product certificate",
    );
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &[], &nonzero_axioms).is_none(),
        "two carrier-only nonzero endpoint pairs fail closed",
    );
}

#[test]
fn exact_multiply_replays_direct_cast_endpoints_and_rejects_redirected_custody() {
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let target_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let source = value(1, source_type);
    let cast = value(2, target_type);
    let factor = value(3, target_type);
    let other_source = value(4, source_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(source_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(target_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(target_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(source_type)),
    ])
    .unwrap();
    let cast_definition = Proposition::Equal(
        cast,
        ScalarTerm::integer_exact_cast(source_type, target_type, source).expect("exact cast"),
    );
    let source_bound = Proposition::LessOrEqual(
        value(1, source_type),
        ScalarTerm::integer(source_type, IntegerValue::Unsigned(64)).expect("u16 bound"),
    );
    let factor_definition = Proposition::Equal(factor, integer(target_type, -2));
    let product = semantic_vocabulary::IntegerMathTerm::Multiply(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: target_type,
            value: ValueId::new(2).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: target_type,
            value: ValueId::new(3).unwrap(),
        }),
    );
    let goal = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(-128)),
            product.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            product,
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(127)),
        ),
    ]);
    let axioms = [cast_definition, factor_definition.clone()];
    for cast_goal in [
        Proposition::LessOrEqual(integer(target_type, 0), value(2, target_type)),
        Proposition::LessOrEqual(value(2, target_type), integer(target_type, 64)),
    ] {
        assert!(
            crate::proofs::nonzero_divisor_certificate::cast_selection::prove(
                &context,
                &cast_goal,
                std::slice::from_ref(&source_bound),
                &axioms,
            )
            .is_some(),
            "the existing cast selector proves {cast_goal:?}",
        );
    }
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&source_bound),
        &axioms,
    )
    .expect("source-bounded cast endpoints orient the exact product");
    accept_certificate(
        &context,
        &goal,
        std::slice::from_ref(&source_bound),
        &axioms,
        &proof,
    )
    .expect("the checker replays the direct cast endpoint custody");

    let redirected_axioms = [
        Proposition::Equal(
            value(2, target_type),
            ScalarTerm::integer_exact_cast(source_type, target_type, other_source)
                .expect("redirected exact cast"),
        ),
        factor_definition,
    ];
    assert!(
        accept_certificate(
            &context,
            &goal,
            std::slice::from_ref(&source_bound),
            &redirected_axioms,
            &proof,
        )
        .is_err(),
        "redirecting the cited cast source invalidates the serialized witness",
    );
    let drifted_source_bound = Proposition::LessOrEqual(
        value(1, source_type),
        ScalarTerm::integer(source_type, IntegerValue::Unsigned(65)).expect("drifted u16 bound"),
    );
    assert!(
        accept_certificate(
            &context,
            &goal,
            std::slice::from_ref(&drifted_source_bound),
            &axioms,
            &proof,
        )
        .is_err(),
        "changing the cited cast-root endpoint invalidates the serialized witness",
    );
}

#[test]
fn exact_multiply_replays_a_computed_multiply_root_through_a_cast_chain() {
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(i64_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i64_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i64_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(u64_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i32_type)),
        (ValueId::new(6).unwrap(), ScalarType::Integer(i32_type)),
    ])
    .unwrap();
    let assumptions = [
        Proposition::LessOrEqual(integer(i64_type, -536_870_912), value(1, i64_type)),
        Proposition::LessOrEqual(value(1, i64_type), integer(i64_type, 0)),
    ];
    let axioms = [
        Proposition::Equal(value(3, i64_type), integer(i64_type, -2)),
        Proposition::Equal(
            value(2, i64_type),
            ScalarTerm::exact_integer_multiply(i64_type, value(1, i64_type), value(3, i64_type))
                .expect("computed product"),
        ),
        Proposition::Equal(
            value(4, u64_type),
            ScalarTerm::integer_exact_cast(i64_type, u64_type, value(2, i64_type))
                .expect("first exact cast"),
        ),
        Proposition::Equal(
            value(5, i32_type),
            ScalarTerm::integer_exact_cast(u64_type, i32_type, value(4, u64_type))
                .expect("second exact cast"),
        ),
        Proposition::Equal(value(6, i32_type), integer(i32_type, -2)),
    ];
    let product = semantic_vocabulary::IntegerMathTerm::Multiply(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i32_type,
            value: ValueId::new(5).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i32_type,
            value: ValueId::new(6).unwrap(),
        }),
    );
    let goal = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i32::MIN.into())),
            product.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            product,
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i32::MAX.into())),
        ),
    ]);
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
        .expect("the computed multiply endpoint replays through the exact cast chain");
    accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
        .expect("the checker replays both multiply and cast custody");

    let drifted_assumptions = [
        Proposition::LessOrEqual(integer(i64_type, -536_870_911), value(1, i64_type)),
        assumptions[1].clone(),
    ];
    assert!(
        accept_certificate(&context, &goal, &drifted_assumptions, &axioms, &proof).is_err(),
        "changing the computed root bound invalidates the nested witness",
    );
}

#[test]
fn exact_multiply_replays_a_computed_multiply_root_through_a_widen_chain() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(i16_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i32_type)),
        (ValueId::new(6).unwrap(), ScalarType::Integer(i32_type)),
        (ValueId::new(7).unwrap(), ScalarType::Integer(i8_type)),
    ])
    .unwrap();
    let assumptions = [
        Proposition::LessOrEqual(integer(i8_type, -63), value(1, i8_type)),
        Proposition::LessOrEqual(value(1, i8_type), integer(i8_type, 64)),
    ];
    let axioms = [
        Proposition::Equal(value(3, i8_type), integer(i8_type, -2)),
        Proposition::Equal(
            value(2, i8_type),
            ScalarTerm::exact_integer_multiply(i8_type, value(1, i8_type), value(3, i8_type))
                .expect("computed product"),
        ),
        Proposition::Equal(
            value(4, i16_type),
            ScalarTerm::integer_widen(i8_type, i16_type, value(2, i8_type))
                .expect("first widening"),
        ),
        Proposition::Equal(
            value(5, i32_type),
            ScalarTerm::integer_widen(i16_type, i32_type, value(4, i16_type))
                .expect("second widening"),
        ),
        Proposition::Equal(value(6, i32_type), integer(i32_type, -2)),
    ];
    let product = semantic_vocabulary::IntegerMathTerm::Multiply(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i32_type,
            value: ValueId::new(5).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i32_type,
            value: ValueId::new(6).unwrap(),
        }),
    );
    let goal = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i32::MIN.into())),
            product.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            product,
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i32::MAX.into())),
        ),
    ]);
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
        .expect("the computed multiply endpoint replays through the widening chain");
    accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
        .expect("the checker replays both multiply and widening custody");

    let mut redirected_axioms = axioms.clone();
    redirected_axioms[2] = Proposition::Equal(
        value(4, i16_type),
        ScalarTerm::integer_widen(i8_type, i16_type, value(7, i8_type))
            .expect("redirected widening"),
    );
    assert!(
        accept_certificate(&context, &goal, &assumptions, &redirected_axioms, &proof).is_err(),
        "redirecting the widening source invalidates the nested witness",
    );
    let drifted_assumptions = [
        Proposition::LessOrEqual(integer(i8_type, -62), value(1, i8_type)),
        assumptions[1].clone(),
    ];
    assert!(
        accept_certificate(&context, &goal, &drifted_assumptions, &axioms, &proof).is_err(),
        "changing the computed root bound invalidates the widening witness",
    );
}

#[test]
fn exact_multiply_replays_source_local_cast_bounds_through_an_affine_suffix() {
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(i16_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(6).unwrap(), ScalarType::Integer(i8_type)),
    ])
    .unwrap();
    let assumptions = [
        Proposition::LessOrEqual(integer(i16_type, -66), value(1, i16_type)),
        Proposition::LessOrEqual(value(1, i16_type), integer(i16_type, 61)),
    ];
    let axioms = [
        Proposition::Equal(
            value(2, i8_type),
            ScalarTerm::integer_exact_cast(i16_type, i8_type, value(1, i16_type))
                .expect("exact cast"),
        ),
        Proposition::Equal(value(3, i8_type), integer(i8_type, 3)),
        Proposition::Equal(value(6, i8_type), integer(i8_type, 4)),
        Proposition::Equal(
            value(4, i8_type),
            ScalarTerm::exact_integer_add(i8_type, value(2, i8_type), value(3, i8_type))
                .expect("affine suffix"),
        ),
        Proposition::Equal(value(5, i8_type), integer(i8_type, -2)),
    ];
    let product = semantic_vocabulary::IntegerMathTerm::Multiply(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i8_type,
            value: ValueId::new(4).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i8_type,
            value: ValueId::new(5).unwrap(),
        }),
    );
    let goal = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i8::MIN.into())),
            product.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            product,
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i8::MAX.into())),
        ),
    ]);
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
        .expect("source-local cast bounds replay through the affine suffix");
    accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
        .expect("the checker replays cast and affine custody");

    let drifted_assumptions = [
        Proposition::LessOrEqual(integer(i16_type, -65), value(1, i16_type)),
        assumptions[1].clone(),
    ];
    assert!(
        accept_certificate(&context, &goal, &drifted_assumptions, &axioms, &proof).is_err(),
        "changing the source-local cast bound invalidates the nested witness",
    );
    let mut redirected_axioms = axioms.clone();
    redirected_axioms[3] = Proposition::Equal(
        value(4, i8_type),
        ScalarTerm::exact_integer_add(i8_type, value(2, i8_type), value(6, i8_type))
            .expect("redirected affine suffix"),
    );
    assert!(
        accept_certificate(&context, &goal, &assumptions, &redirected_axioms, &proof).is_err(),
        "redirecting the affine suffix literal invalidates the nested witness",
    );
}

#[test]
fn exact_multiply_replays_affine_cast_affine_endpoints_and_rejects_each_redirect() {
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=16).map(|id| {
        let integer_type = if matches!(id, 8..=11 | 13..=16) {
            i8_type
        } else {
            i16_type
        };
        (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))
    }))
    .unwrap();
    let assumptions = [
        Proposition::LessOrEqual(integer(i16_type, -32), value(1, i16_type)),
        Proposition::LessOrEqual(value(1, i16_type), integer(i16_type, 28)),
    ];
    let axioms = [
        Proposition::Equal(value(2, i16_type), integer(i16_type, 3)),
        Proposition::Equal(
            value(3, i16_type),
            ScalarTerm::exact_integer_add(i16_type, value(1, i16_type), value(2, i16_type))
                .expect("pre-cast add"),
        ),
        Proposition::Equal(value(4, i16_type), integer(i16_type, -2)),
        Proposition::Equal(
            value(5, i16_type),
            ScalarTerm::exact_integer_multiply(i16_type, value(3, i16_type), value(4, i16_type))
                .expect("pre-cast multiply"),
        ),
        Proposition::Equal(value(6, i16_type), integer(i16_type, 1)),
        Proposition::Equal(
            value(7, i16_type),
            ScalarTerm::exact_integer_subtract(i16_type, value(5, i16_type), value(6, i16_type))
                .expect("pre-cast subtract"),
        ),
        Proposition::Equal(
            value(8, i8_type),
            ScalarTerm::integer_exact_cast(i16_type, i8_type, value(7, i16_type))
                .expect("exact cast"),
        ),
        Proposition::Equal(value(9, i8_type), integer(i8_type, 1)),
        Proposition::Equal(
            value(10, i8_type),
            ScalarTerm::exact_integer_add(i8_type, value(8, i8_type), value(9, i8_type))
                .expect("post-cast add"),
        ),
        Proposition::Equal(value(11, i8_type), integer(i8_type, 2)),
        Proposition::Equal(value(12, i16_type), integer(i16_type, 0)),
        Proposition::Equal(value(13, i8_type), integer(i8_type, 2)),
        Proposition::Equal(value(14, i8_type), integer(i8_type, -2)),
        Proposition::Equal(
            value(15, i8_type),
            ScalarTerm::exact_integer_multiply(i8_type, value(10, i8_type), value(14, i8_type))
                .expect("negative post-cast multiply"),
        ),
        Proposition::Equal(value(16, i8_type), integer(i8_type, 1)),
    ];
    let product = semantic_vocabulary::IntegerMathTerm::Multiply(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i8_type,
            value: ValueId::new(10).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i8_type,
            value: ValueId::new(11).unwrap(),
        }),
    );
    let goal = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i8::MIN.into())),
            product.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            product,
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i8::MAX.into())),
        ),
    ]);
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
        .expect("definition-local affine/cast/affine endpoints orient the product");
    accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
        .expect("the checker replays both affine witnesses and cast custody");

    let mut redirected_pre_cast = axioms.clone();
    redirected_pre_cast[1] = Proposition::Equal(
        value(3, i16_type),
        ScalarTerm::exact_integer_add(i16_type, value(12, i16_type), value(2, i16_type))
            .expect("redirected pre-cast add"),
    );
    assert!(
        accept_certificate(&context, &goal, &assumptions, &redirected_pre_cast, &proof,).is_err(),
        "redirecting the pre-cast affine root invalidates the serialized witness",
    );
    let drifted_assumptions = [
        Proposition::LessOrEqual(integer(i16_type, -33), value(1, i16_type)),
        assumptions[1].clone(),
    ];
    assert!(
        accept_certificate(&context, &goal, &drifted_assumptions, &axioms, &proof).is_err(),
        "changing a cited root endpoint invalidates the serialized witness",
    );
    let mut redirected_suffix = axioms.clone();
    redirected_suffix[8] = Proposition::Equal(
        value(10, i8_type),
        ScalarTerm::exact_integer_add(i8_type, value(8, i8_type), value(13, i8_type))
            .expect("redirected post-cast add"),
    );
    assert!(
        accept_certificate(&context, &goal, &assumptions, &redirected_suffix, &proof).is_err(),
        "redirecting the post-cast affine literal invalidates the serialized witness",
    );

    let difference = semantic_vocabulary::IntegerMathTerm::Subtract(
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i8_type,
            value: ValueId::new(15).unwrap(),
        }),
        Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: i8_type,
            value: ValueId::new(16).unwrap(),
        }),
    );
    let subtract_goal = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i8::MIN.into())),
            difference.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            difference,
            semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Signed(i8::MAX.into())),
        ),
    ]);
    let subtract_proof =
        prove_canonical_integer_proposition(&context, &subtract_goal, &assumptions, &axioms)
            .expect("negative cast-prefix multiply endpoints orient the outer subtraction");
    accept_certificate(
        &context,
        &subtract_goal,
        &assumptions,
        &axioms,
        &subtract_proof,
    )
    .expect("the checker replays sign-directed cast-prefix endpoints through subtraction");
}

#[test]
fn exact_multiply_maps_one_immediate_remainder_range_through_an_affine_suffix() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root = value(1, integer_type);
    let remainder = value(2, integer_type);
    let added = value(3, integer_type);
    let unsigned = |value| {
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(value)).expect("u8 literal")
    };
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let axioms = [
        Proposition::Equal(
            remainder.clone(),
            ScalarTerm::exact_integer_remainder(integer_type, root, unsigned(64)).unwrap(),
        ),
        Proposition::Equal(
            added.clone(),
            ScalarTerm::exact_integer_add(integer_type, remainder, unsigned(1)).unwrap(),
        ),
    ];
    let goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::Multiply(
            Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(3).unwrap(),
            }),
            Box::new(semantic_vocabulary::IntegerMathTerm::literal(
                IntegerValue::Unsigned(2),
            )),
        ),
        semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
    );
    let proof = prove_canonical_integer_proposition(&context, &goal, &[], &axioms)
        .expect("the checked remainder hull maps through the one-step affine suffix");
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        ..
    } = proof.rule
    else {
        panic!("the strongest mapped product endpoint is relaxed to the carrier maximum")
    };
    let ProofRule::IntegerAffineBound { root_bound, .. } = left_less_or_equal_middle.rule else {
        panic!("the outer product replays the checked four-endpoint certificate")
    };
    let ProofRule::ConjunctionIntroduction(parts) = root_bound.rule else {
        panic!("the outer product retains four ordered endpoint children")
    };
    assert!(parts.iter().any(|part| matches!(
        part.rule,
        ProofRule::IntegerAffineBound {
            root_bound: ref remainder_bound,
            ..
        } if matches!(remainder_bound.rule, ProofRule::IntegerAffineBound { .. })
    )));
}

#[test]
fn unsigned_affine_exact_cast_bound_uses_existing_ordered_transform_rule() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let root = value(1, integer_type);
    let added = value(2, integer_type);
    let target = value(3, integer_type);
    let unsigned = |value| {
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(value)).expect("u16 literal")
    };
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let assumptions = [Proposition::LessOrEqual(root.clone(), unsigned(126))];
    let axioms = [
        Proposition::Equal(
            added.clone(),
            ScalarTerm::exact_integer_add(integer_type, root, unsigned(1)).unwrap(),
        ),
        Proposition::Equal(
            target,
            ScalarTerm::exact_integer_multiply(integer_type, added, unsigned(2)).unwrap(),
        ),
    ];
    let goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(3).unwrap(),
        },
        semantic_vocabulary::IntegerMathTerm::IntegerLiteral(
            semantic_vocabulary::IntegerMathLiteral::new(false, 255).unwrap(),
        ),
    );
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
        .expect("unsigned affine endpoint maps to the exact-cast carrier bound");
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        ..
    } = proof.rule
    else {
        panic!("unsigned affine endpoint uses closed strengthening")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::IntegerAffineBound { .. }
    ));
}

#[test]
fn landed_remainder_exact_cast_bound_retains_checked_root_custody() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let root = value(1, integer_type);
    let target = value(2, integer_type);
    let other = value(3, integer_type);
    let unsigned = |value| {
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(value)).expect("u16 literal")
    };
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let root_bound = Proposition::LessOrEqual(root.clone(), unsigned(u16::MAX.into()));
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::exact_integer_remainder(integer_type, root.clone(), unsigned(64)).unwrap(),
    );
    let goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: ValueId::new(2).unwrap(),
        },
        semantic_vocabulary::IntegerMathTerm::IntegerLiteral(
            semantic_vocabulary::IntegerMathLiteral::new(false, 127).unwrap(),
        ),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&root_bound),
        std::slice::from_ref(&definition),
    )
    .expect("landed remainder hull proves the exact-cast carrier bound");
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        ..
    } = proof.rule
    else {
        panic!("remainder hull uses closed strengthening")
    };
    let ProofRule::IntegerAffineBound { root_bound, .. } = left_less_or_equal_middle.rule else {
        panic!("remainder hull uses the existing ordered transform")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::Assumption { index: 0 }
    ));

    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[],
        std::slice::from_ref(&definition),
    )
    .expect("a landed nonzero remainder permits an explicit Truth root child");
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        ..
    } = proof.rule
    else {
        panic!("Truth-root remainder hull uses closed strengthening")
    };
    let ProofRule::IntegerAffineBound { root_bound, .. } = left_less_or_equal_middle.rule else {
        panic!("Truth-root remainder hull uses the checked ordered transform")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::Primitive(PrimitiveJudgment::Truth)
    ));

    let zero_divisor = Proposition::Equal(
        target,
        ScalarTerm::exact_integer_remainder(integer_type, other, unsigned(0)).unwrap(),
    );
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &[], &[zero_divisor],).is_none(),
        "a zero-divisor remainder cannot claim a total carrier image",
    );
}

#[test]
fn nested_shift_then_cast_bound_composes_existing_checked_rules() {
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let root = value(1, i64_type);
    let shifted = value(2, i64_type);
    let target = value(3, u64_type);
    let signed =
        |value| ScalarTerm::integer(i64_type, IntegerValue::Signed(value)).expect("i64 literal");
    let unsigned_count = ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).unwrap();
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(i64_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i64_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(u64_type)),
    ])
    .unwrap();
    let assumptions = [Proposition::LessOrEqual(
        root.clone(),
        signed(4_294_967_294),
    )];
    let axioms = [
        Proposition::Equal(
            shifted.clone(),
            ScalarTerm::exact_integer_shift_right(i64_type, u16_type, root, unsigned_count)
                .unwrap(),
        ),
        Proposition::Equal(
            target,
            ScalarTerm::integer_exact_cast(i64_type, u64_type, shifted).unwrap(),
        ),
    ];
    let goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::MathValue {
            source_type: u64_type,
            value: ValueId::new(3).unwrap(),
        },
        semantic_vocabulary::IntegerMathTerm::IntegerLiteral(
            semantic_vocabulary::IntegerMathLiteral::new(false, 2_147_483_647).unwrap(),
        ),
    );
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
        .expect("checked shift source bound composes through the checked cast");
    let ProofRule::IntegerCastBound { root_bound, .. } = proof.rule else {
        panic!("outer exact-cast word retains IntegerCastBound")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::IntegerAffineBound { .. }
    ));
}

#[test]
fn signed_exact_shift_count_uses_checked_conjunction_introduction() {
    let count_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let count = value(1, count_type);
    let lower = Proposition::LessOrEqual(integer(count_type, 0), count.clone());
    let upper = Proposition::LessOrEqual(count.clone(), integer(count_type, 63));
    let goal = Proposition::Conjunction(vec![lower.clone(), upper.clone()]);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(count_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(count_type)),
    ])
    .unwrap();
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&upper),
        std::slice::from_ref(&lower),
    )
    .expect("complete exact count bounds prove the canonical conjunction");
    let ProofRule::ConjunctionIntroduction(conjuncts) = proof.rule else {
        panic!("signed exact count uses conjunction introduction")
    };
    assert_eq!(conjuncts.len(), 2);
    assert!(matches!(
        conjuncts[0].rule,
        ProofRule::SemanticAxiom { index: 0 }
    ));
    assert!(matches!(
        conjuncts[1].rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(
        prove_canonical_integer_proposition(&context, &goal, std::slice::from_ref(&upper), &[],)
            .is_none(),
        "missing lower-bound custody cannot prove the count goal",
    );
    let redirected = Proposition::LessOrEqual(value(2, count_type), integer(count_type, 63));
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &[redirected], &[lower]).is_none(),
        "a different count identity cannot prove the count goal",
    );
}
