use super::*;
use psi_core::{IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId};
use psi_proof_kernel::{PrimitiveJudgment, ProofRule};

fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(id).expect("value id"),
        ScalarType::Integer(integer_type),
    )
}

fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
    ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("integer")
}

fn two_value_context(integer_type: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap()
}

#[test]
fn signed_goal_prefers_negative_arm_and_tightens_requirement() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let divisor = value(1, integer_type);
    let negative_one = integer(integer_type, -1);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), negative_one),
        Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
    ]);
    let requirements = [Proposition::LessOrEqual(divisor, integer(integer_type, -2))];
    let proof = prove_canonical_integer_proposition(
        &PropositionContext::from_value_types([(
            ValueId::new(1).unwrap(),
            ScalarType::Integer(integer_type),
        )])
        .unwrap(),
        &goal,
        &requirements,
        &[],
    )
    .expect("negative bound proves nonzero");
    assert!(matches!(
        proof.rule,
        ProofRule::DisjunctionIntroduction { index: 0, .. }
    ));
}

#[test]
fn signed_goal_selects_positive_arm_from_exact_requirement() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let divisor = value(1, integer_type);
    let positive = Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone());
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor, integer(integer_type, -1)),
        positive.clone(),
    ]);
    let proof = prove_canonical_integer_proposition(
        &PropositionContext::from_value_types([(
            ValueId::new(1).unwrap(),
            ScalarType::Integer(integer_type),
        )])
        .unwrap(),
        &goal,
        &[positive],
        &[],
    )
    .expect("positive requirement proves nonzero");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("signed goal uses disjunction introduction")
    };
    assert_eq!(index, 1);
    assert!(matches!(disjunct.rule, ProofRule::Assumption { index: 0 }));
}

#[test]
fn literal_equality_substitution_uses_only_prior_fact() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let divisor = value(1, integer_type);
    let goal = Proposition::LessOrEqual(
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).unwrap(),
        divisor.clone(),
    );
    let facts = [Proposition::Equal(
        divisor,
        ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).unwrap(),
    )];
    let proof = prove_canonical_integer_proposition(
        &PropositionContext::from_value_types([(
            ValueId::new(1).unwrap(),
            ScalarType::Integer(integer_type),
        )])
        .unwrap(),
        &goal,
        &[],
        &facts,
    )
    .expect("literal equality proves nonzero");
    assert!(matches!(
        proof.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 1, .. }
    ));
    assert!(
        prove_canonical_integer_proposition(
            &PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(integer_type),
            )])
            .unwrap(),
            &goal,
            &[],
            &[],
        )
        .is_none()
    );
}

#[test]
fn exact_division_goal_composes_ordered_three_arm_and_joint_exception_proofs() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let dividend = value(1, integer_type);
    let divisor = value(2, integer_type);
    let negative_safe = Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -2));
    let positive_safe = Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone());
    let negative_one = Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1));
    let dividend_safe = Proposition::LessOrEqual(integer(integer_type, -127), dividend.clone());
    let goal = Proposition::Disjunction(vec![
        negative_safe.clone(),
        positive_safe,
        Proposition::Conjunction(vec![negative_one.clone(), dividend_safe.clone()]),
    ]);

    let context = two_value_context(integer_type);
    let negative = prove_canonical_integer_proposition(&context, &goal, &[negative_safe], &[])
        .expect("first exact-division arm is cited");
    assert!(matches!(
        negative.rule,
        ProofRule::DisjunctionIntroduction { index: 0, .. }
    ));

    let joint =
        prove_canonical_integer_proposition(&context, &goal, &[negative_one, dividend_safe], &[])
            .expect("joint -1/dividend exception is composed");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = joint.rule else {
        panic!("exact division uses disjunction introduction")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("joint exact bounds use conjunction introduction")
    };
    assert_eq!(conjuncts.len(), 2);
    assert!(matches!(
        conjuncts[0].rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(
        conjuncts[1].rule,
        ProofRule::Assumption { index: 1 }
    ));
    assert!(prove_canonical_integer_proposition(&context, &goal, &[], &[]).is_none());
}

#[test]
fn exact_division_goal_composes_runtime_negative_bound_and_landed_dividend() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let dividend = value(1, integer_type);
    let divisor = value(2, integer_type);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -2)),
        Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1)),
            Proposition::LessOrEqual(integer(integer_type, -127), dividend.clone()),
        ]),
    ]);
    let proof = prove_canonical_integer_proposition(
        &two_value_context(integer_type),
        &goal,
        &[Proposition::LessOrEqual(divisor, integer(integer_type, -1))],
        &[Proposition::Equal(dividend, integer(integer_type, -7))],
    )
    .expect("runtime negative bound and landed dividend prove the joint arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("mixed joint evidence selects its canonical disjunct")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("mixed joint evidence proves both canonical premises")
    };
    assert!(matches!(
        conjuncts[0].rule,
        ProofRule::Assumption { index: 0 }
    ));
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = &conjuncts[1].rule
    else {
        panic!("landed dividend proves its canonical floor by substitution")
    };
    assert_eq!(*endpoint, 1);
    assert!(matches!(
        relation.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    assert!(matches!(
        equality.rule,
        ProofRule::SemanticAxiom { index: 0 }
    ));
}

#[test]
fn exact_division_goal_cites_complete_retained_goal_or_arm() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let dividend = value(1, integer_type);
    let divisor = value(2, integer_type);
    let joint_arm = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1)),
        Proposition::LessOrEqual(integer(integer_type, -127), dividend.clone()),
    ]);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor, integer(integer_type, -2)),
        Proposition::LessOrEqual(integer(integer_type, 1), value(2, integer_type)),
        joint_arm.clone(),
    ]);
    let whole_goal = prove_canonical_integer_proposition(
        &two_value_context(integer_type),
        &goal,
        std::slice::from_ref(&goal),
        &[],
    )
    .expect("exact retained canonical goal is cited directly");
    assert!(matches!(
        whole_goal.rule,
        ProofRule::Assumption { index: 0 }
    ));

    let retained_arm = prove_canonical_integer_proposition(
        &two_value_context(integer_type),
        &goal,
        &[],
        std::slice::from_ref(&joint_arm),
    )
    .expect("exact retained canonical arm is introduced");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = retained_arm.rule else {
        panic!("retained canonical arm is wrapped by disjunction introduction")
    };
    assert_eq!(index, 2);
    assert!(matches!(
        disjunct.rule,
        ProofRule::SemanticAxiom { index: 0 }
    ));

    let redirected_arm = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(value(3, integer_type), integer(integer_type, -1)),
        Proposition::LessOrEqual(integer(integer_type, -127), dividend),
    ]);
    assert!(
        prove_canonical_integer_proposition(
            &two_value_context(integer_type),
            &goal,
            &[],
            &[redirected_arm],
        )
        .is_none(),
        "redirected canonical arm cannot prove the goal",
    );
}

#[test]
fn exact_division_goal_composes_literal_equality_requirements() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_divisor = value(2, unsigned);
    let unsigned_goal = Proposition::LessOrEqual(
        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
        unsigned_divisor.clone(),
    );
    let unsigned_proof = prove_canonical_integer_proposition(
        &two_value_context(unsigned),
        &unsigned_goal,
        &[Proposition::Equal(
            unsigned_divisor,
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5)).expect("safe u8 divisor"),
        )],
        &[],
    )
    .expect("literal equality requirement proves unsigned definedness");
    let ProofRule::IntegerLessOrEqualSubstitution {
        equality, endpoint, ..
    } = unsigned_proof.rule
    else {
        panic!("unsigned literal requirement uses equality substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 0 }));
    assert!(
        prove_canonical_integer_proposition(
            &two_value_context(unsigned),
            &unsigned_goal,
            &[Proposition::Equal(
                value(2, unsigned),
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0)).expect("zero u8 divisor"),
            )],
            &[],
        )
        .is_none(),
        "zero equality requirement cannot prove unsigned definedness",
    );
    assert!(
        prove_canonical_integer_proposition(
            &two_value_context(unsigned),
            &unsigned_goal,
            &[Proposition::Equal(
                value(1, unsigned),
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5))
                    .expect("redirected u8 divisor"),
            )],
            &[],
        )
        .is_none(),
        "wrong-operand equality requirement cannot prove unsigned definedness",
    );

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_dividend = value(1, signed);
    let signed_divisor = value(2, signed);
    let signed_goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), signed_dividend.clone()),
        ]),
    ]);
    let signed_proof = prove_canonical_integer_proposition(
        &two_value_context(signed),
        &signed_goal,
        &[
            Proposition::LessOrEqual(signed_divisor, integer(signed, -1)),
            Proposition::Equal(signed_dividend, integer(signed, -7)),
        ],
        &[],
    )
    .expect("runtime negative bound and dividend requirement prove the joint arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
        panic!("mixed requirement evidence selects the canonical joint arm")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("mixed requirement evidence proves both joint premises")
    };
    assert!(matches!(
        conjuncts[0].rule,
        ProofRule::Assumption { index: 0 }
    ));
    let ProofRule::IntegerLessOrEqualSubstitution { equality, .. } = &conjuncts[1].rule else {
        panic!("dividend requirement proves its floor by substitution")
    };
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
    assert!(
        prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            &[
                Proposition::LessOrEqual(value(2, signed), integer(signed, -1)),
                Proposition::Equal(
                    value(1, signed),
                    ScalarTerm::integer(signed, signed.minimum_value())
                        .expect("minimum i8 dividend"),
                ),
            ],
            &[],
        )
        .is_none(),
        "minimum dividend equality cannot prove the joint arm",
    );
}

#[test]
fn exact_division_goal_transports_exact_bound_across_endpoint_equality() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(unsigned)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(unsigned)),
    ])
    .expect("three u8 values");
    let unsigned_goal = Proposition::LessOrEqual(
        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
        value(2, unsigned),
    );
    let unsigned_proof = prove_canonical_integer_proposition(
        &unsigned_context,
        &unsigned_goal,
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                value(3, unsigned),
            ),
            Proposition::Equal(value(3, unsigned), value(2, unsigned)),
        ],
        &[],
    )
    .expect("exact intermediate bound transports to the unsigned divisor");
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = unsigned_proof.rule
    else {
        panic!("unsigned endpoint transport uses integer-order substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(relation.rule, ProofRule::Assumption { index: 0 }));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
    assert!(
        prove_canonical_integer_proposition(
            &unsigned_context,
            &unsigned_goal,
            &[Proposition::Equal(value(3, unsigned), value(2, unsigned),)],
            &[],
        )
        .is_none(),
        "endpoint equality without its bound cannot prove definedness",
    );

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
    ])
    .expect("three i8 values");
    let signed_divisor = value(2, signed);
    let signed_goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(signed_divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let signed_proof = prove_canonical_integer_proposition(
        &signed_context,
        &signed_goal,
        &[
            Proposition::LessOrEqual(value(3, signed), integer(signed, -2)),
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
        &[],
    )
    .expect("exact intermediate bound transports to the signed divisor");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
        panic!("signed endpoint transport selects its canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = disjunct.rule
    else {
        panic!("signed endpoint transport uses integer-order substitution")
    };
    assert_eq!(endpoint, 0);
    assert!(matches!(relation.rule, ProofRule::Assumption { index: 0 }));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));

    let dividend_transport_proof = prove_canonical_integer_proposition(
        &signed_context,
        &signed_goal,
        &[
            Proposition::LessOrEqual(value(2, signed), integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(3, signed)),
            Proposition::Equal(value(3, signed), value(1, signed)),
        ],
        &[],
    )
    .expect("exact intermediate floor transports to the signed dividend");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = dividend_transport_proof.rule
    else {
        panic!("dividend endpoint transport selects the canonical joint arm")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("dividend endpoint transport proves both joint premises")
    };
    assert!(matches!(
        conjuncts[0].rule,
        ProofRule::Assumption { index: 0 }
    ));
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = &conjuncts[1].rule
    else {
        panic!("dividend floor uses integer-order substitution")
    };
    assert_eq!(*endpoint, 1);
    assert!(matches!(relation.rule, ProofRule::Assumption { index: 1 }));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 2 }));
    assert!(
        prove_canonical_integer_proposition(
            &signed_context,
            &signed_goal,
            &[
                Proposition::LessOrEqual(value(2, signed), integer(signed, -1)),
                Proposition::LessOrEqual(integer(signed, -127), value(3, signed)),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
            &[],
        )
        .is_none(),
        "redirected equality cannot transport the dividend floor",
    );
}

#[test]
fn exact_division_goal_composes_complete_prior_fact_proofs() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_divisor = value(2, unsigned);
    let unsigned_goal = Proposition::LessOrEqual(
        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 literal"),
        unsigned_divisor.clone(),
    );
    let unsigned_direct = prove_canonical_integer_proposition(
        &two_value_context(unsigned),
        &unsigned_goal,
        std::slice::from_ref(&unsigned_goal),
        &[],
    )
    .expect("exact unsigned divisor floor is cited directly");
    assert!(matches!(
        unsigned_direct.rule,
        ProofRule::Assumption { index: 0 }
    ));
    let unsigned_stronger = prove_canonical_integer_proposition(
        &two_value_context(unsigned),
        &unsigned_goal,
        &[Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2)).expect("stronger u8 floor"),
            unsigned_divisor.clone(),
        )],
        &[],
    )
    .expect("stronger unsigned divisor floor composes transitively");
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = unsigned_stronger.rule
    else {
        panic!("stronger unsigned floor uses exact transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 0 }
    ));
    let unsigned_literal =
        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5)).expect("u8 literal");
    let unsigned_proof = prove_canonical_integer_proposition(
        &two_value_context(unsigned),
        &unsigned_goal,
        &[],
        &[Proposition::Equal(unsigned_divisor, unsigned_literal)],
    )
    .expect("landed positive literal proves unsigned definedness");
    assert!(matches!(
        unsigned_proof.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 1, .. }
    ));

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_dividend = value(1, signed);
    let signed_divisor = value(2, signed);
    let signed_goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), signed_dividend),
        ]),
    ]);
    let positive_divisor = Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone());
    let positive_proof = prove_canonical_integer_proposition(
        &two_value_context(signed),
        &signed_goal,
        std::slice::from_ref(&positive_divisor),
        &[],
    )
    .expect("exact signed positive-divisor arm is cited directly");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = positive_proof.rule else {
        panic!("signed positive divisor selects its canonical arm")
    };
    assert_eq!(index, 1);
    assert!(matches!(disjunct.rule, ProofRule::Assumption { index: 0 }));
    let stronger_positive_proof = prove_canonical_integer_proposition(
        &two_value_context(signed),
        &signed_goal,
        &[Proposition::LessOrEqual(
            integer(signed, 3),
            signed_divisor.clone(),
        )],
        &[],
    )
    .expect("stronger signed positive floor composes transitively");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = stronger_positive_proof.rule
    else {
        panic!("stronger positive floor selects its canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = disjunct.rule
    else {
        panic!("stronger signed positive floor uses exact transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 0 }
    ));
    let stronger_negative_proof = prove_canonical_integer_proposition(
        &two_value_context(signed),
        &signed_goal,
        &[Proposition::LessOrEqual(
            signed_divisor.clone(),
            integer(signed, -3),
        )],
        &[],
    )
    .expect("stronger signed negative ceiling composes transitively");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = stronger_negative_proof.rule
    else {
        panic!("stronger negative ceiling selects its canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = disjunct.rule
    else {
        panic!("stronger signed negative ceiling uses exact transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    let safe_fact = Proposition::Equal(signed_divisor.clone(), integer(signed, -3));
    let signed_proof = prove_canonical_integer_proposition(
        &two_value_context(signed),
        &signed_goal,
        &[],
        &[safe_fact],
    )
    .expect("landed negative literal proves signed definedness");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
        panic!("signed exact division proves one canonical disjunct")
    };
    assert_eq!(index, 0);
    assert!(matches!(
        disjunct.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 0, .. }
    ));

    for excluded in [0, -1] {
        assert!(
            prove_canonical_integer_proposition(
                &two_value_context(signed),
                &signed_goal,
                &[],
                &[Proposition::Equal(
                    signed_divisor.clone(),
                    integer(signed, excluded),
                )],
            )
            .is_none(),
            "signed literal {excluded} is not carrier-total",
        );
    }

    let exceptional_proof = prove_canonical_integer_proposition(
        &two_value_context(signed),
        &signed_goal,
        &[],
        &[
            Proposition::Equal(signed_divisor, integer(signed, -1)),
            Proposition::Equal(value(1, signed), integer(signed, -7)),
        ],
    )
    .expect("landed -1 and nonminimum dividend prove the exceptional arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = exceptional_proof.rule else {
        panic!("signed -1 exact division proves the joint exceptional arm")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("signed -1 arm proves both canonical bounds")
    };
    assert_eq!(conjuncts.len(), 2);
    assert!(
        conjuncts
            .iter()
            .all(|proof| matches!(proof.rule, ProofRule::IntegerLessOrEqualSubstitution { .. }))
    );

    let dividend_bound = Proposition::LessOrEqual(integer(signed, -127), value(1, signed));
    let retained_bound_proof = prove_canonical_integer_proposition(
        &two_value_context(signed),
        &signed_goal,
        std::slice::from_ref(&dividend_bound),
        &[Proposition::Equal(value(2, signed), integer(signed, -1))],
    )
    .expect("landed -1 and exact retained dividend bound prove the exceptional arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = retained_bound_proof.rule else {
        panic!("retained dividend bound selects the joint exceptional arm")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("retained dividend bound proves both canonical bounds")
    };
    assert!(matches!(
        conjuncts[0].rule,
        ProofRule::IntegerLessOrEqualSubstitution { .. }
    ));
    assert!(matches!(
        conjuncts[1].rule,
        ProofRule::Assumption { index: 0 }
    ));

    let stronger_bound_proof = prove_canonical_integer_proposition(
        &two_value_context(signed),
        &signed_goal,
        &[Proposition::LessOrEqual(
            integer(signed, -120),
            value(1, signed),
        )],
        &[Proposition::Equal(value(2, signed), integer(signed, -1))],
    )
    .expect("stronger retained dividend floor proves the exceptional arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = stronger_bound_proof.rule else {
        panic!("stronger retained bound selects the joint exceptional arm")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("stronger retained bound proves both canonical bounds")
    };
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = &conjuncts[1].rule
    else {
        panic!("canonical dividend floor follows by one checked transitivity step")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 0 }
    ));

    let retained_axiom_proof = prove_canonical_integer_proposition(
        &two_value_context(signed),
        &signed_goal,
        &[],
        &[
            Proposition::Equal(value(2, signed), integer(signed, -1)),
            dividend_bound,
        ],
    )
    .expect("pre-site exact dividend axiom proves the exceptional arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = retained_axiom_proof.rule else {
        panic!("pre-site dividend axiom selects the joint exceptional arm")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("pre-site dividend axiom proves both canonical bounds")
    };
    assert!(matches!(
        conjuncts[1].rule,
        ProofRule::SemanticAxiom { index: 1 }
    ));
}

#[test]
fn i1_exact_division_goal_requires_both_joint_bounds() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
    let dividend = value(1, integer_type);
    let divisor = value(2, integer_type);
    let divisor_negative = Proposition::LessOrEqual(divisor, integer(integer_type, -1));
    let dividend_nonnegative = Proposition::LessOrEqual(integer(integer_type, 0), dividend);
    let goal =
        Proposition::Conjunction(vec![divisor_negative.clone(), dividend_nonnegative.clone()]);
    assert!(
        prove_canonical_integer_proposition(
            &two_value_context(integer_type),
            &goal,
            std::slice::from_ref(&divisor_negative),
            &[],
        )
        .is_none()
    );
    let retained_bound_proof = prove_canonical_integer_proposition(
        &two_value_context(integer_type),
        &goal,
        &[divisor_negative.clone(), dividend_nonnegative.clone()],
        &[],
    )
    .expect("both exact i1 bounds prove the joint goal");
    let ProofRule::ConjunctionIntroduction(conjuncts) = retained_bound_proof.rule else {
        panic!("exact i1 bounds compose through conjunction introduction")
    };
    assert_eq!(conjuncts.len(), 2);
    assert!(matches!(
        conjuncts[0].rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(
        conjuncts[1].rule,
        ProofRule::Assumption { index: 1 }
    ));
    assert!(
        prove_canonical_integer_proposition(
            &two_value_context(integer_type),
            &goal,
            &[
                divisor_negative,
                Proposition::LessOrEqual(integer(integer_type, 0), value(3, integer_type),),
            ],
            &[],
        )
        .is_none(),
        "wrong-dividend bound cannot prove the joint goal",
    );

    let landed = [
        Proposition::Equal(value(2, integer_type), integer(integer_type, -1)),
        Proposition::Equal(value(1, integer_type), integer(integer_type, 0)),
    ];
    let proof =
        prove_canonical_integer_proposition(&two_value_context(integer_type), &goal, &[], &landed)
            .expect("landed i1 -1/zero pair proves exact definedness");
    assert!(matches!(
        proof.rule,
        ProofRule::ConjunctionIntroduction(ref conjuncts) if conjuncts.len() == 2
    ));
}

#[test]
fn i1_exact_division_goal_transports_both_joint_endpoints() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .expect("four i1 values");
    let goal = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(value(2, integer_type), integer(integer_type, -1)),
        Proposition::LessOrEqual(integer(integer_type, 0), value(1, integer_type)),
    ]);
    let assumptions = [
        Proposition::LessOrEqual(value(3, integer_type), integer(integer_type, -1)),
        Proposition::Equal(value(3, integer_type), value(2, integer_type)),
        Proposition::LessOrEqual(integer(integer_type, 0), value(4, integer_type)),
        Proposition::Equal(value(4, integer_type), value(1, integer_type)),
    ];
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
        .expect("both i1 joint endpoints transport independently");
    let ProofRule::ConjunctionIntroduction(conjuncts) = proof.rule else {
        panic!("i1 endpoint transport constructs the canonical conjunction")
    };
    assert_eq!(conjuncts.len(), 2);
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = &conjuncts[0].rule
    else {
        panic!("i1 divisor bound uses endpoint substitution")
    };
    assert_eq!(*endpoint, 0);
    assert!(matches!(relation.rule, ProofRule::Assumption { index: 0 }));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = &conjuncts[1].rule
    else {
        panic!("i1 dividend bound uses endpoint substitution")
    };
    assert_eq!(*endpoint, 1);
    assert!(matches!(relation.rule, ProofRule::Assumption { index: 2 }));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 3 }));

    let crossed = [
        Proposition::LessOrEqual(value(3, integer_type), integer(integer_type, -1)),
        Proposition::Equal(value(3, integer_type), value(1, integer_type)),
        Proposition::LessOrEqual(integer(integer_type, 0), value(4, integer_type)),
        Proposition::Equal(value(4, integer_type), value(2, integer_type)),
    ];
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &crossed, &[]).is_none(),
        "crossed i1 endpoint equalities cannot prove the joint goal",
    );
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &assumptions[..3], &[]).is_none(),
        "missing dividend equality cannot prove the joint goal",
    );
}

#[test]
fn exact_division_goal_nests_closed_transitivity_under_endpoint_transport() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(unsigned)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(unsigned)),
    ])
    .expect("three u8 values");
    let unsigned_goal = Proposition::LessOrEqual(
        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
        value(2, unsigned),
    );
    let unsigned_proof = prove_canonical_integer_proposition(
        &unsigned_context,
        &unsigned_goal,
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2))
                    .expect("stronger u8 floor"),
                value(3, unsigned),
            ),
            Proposition::Equal(value(3, unsigned), value(2, unsigned)),
        ],
        &[],
    )
    .expect("stronger intermediate bound transports to the unsigned divisor");
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = unsigned_proof.rule
    else {
        panic!("unsigned stronger endpoint transport uses substitution")
    };
    assert_eq!(endpoint, 1);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = relation.rule
    else {
        panic!("transported unsigned relation uses closed transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
    assert!(
        prove_canonical_integer_proposition(
            &unsigned_context,
            &unsigned_goal,
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0))
                        .expect("weak u8 floor"),
                    value(3, unsigned),
                ),
                Proposition::Equal(value(3, unsigned), value(2, unsigned)),
            ],
            &[],
        )
        .is_none(),
        "weak transported bound cannot prove unsigned definedness",
    );

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
    ])
    .expect("three i8 values");
    let signed_divisor = value(2, signed);
    let signed_goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(signed_divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let signed_proof = prove_canonical_integer_proposition(
        &signed_context,
        &signed_goal,
        &[
            Proposition::LessOrEqual(value(3, signed), integer(signed, -3)),
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
        &[],
    )
    .expect("stronger intermediate ceiling transports to the signed divisor");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
        panic!("signed stronger transport selects its canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation, endpoint, ..
    } = disjunct.rule
    else {
        panic!("signed stronger endpoint transport uses substitution")
    };
    assert_eq!(endpoint, 0);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = relation.rule
    else {
        panic!("transported signed relation uses closed transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
}

#[test]
fn exact_division_goal_nests_two_citation_transitivity_under_endpoint_transport() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let unsigned_context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(unsigned)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(unsigned)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(unsigned)),
    ])
    .expect("four u8 values");
    let unsigned_goal = Proposition::LessOrEqual(
        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
        value(2, unsigned),
    );
    let unsigned_proof = prove_canonical_integer_proposition(
        &unsigned_context,
        &unsigned_goal,
        &[
            Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                value(4, unsigned),
            ),
            Proposition::LessOrEqual(value(4, unsigned), value(3, unsigned)),
            Proposition::Equal(value(3, unsigned), value(2, unsigned)),
        ],
        &[],
    )
    .expect("two cited bounds transport to the unsigned divisor");
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = unsigned_proof.rule
    else {
        panic!("unsigned two-citation transport uses substitution")
    };
    assert_eq!(endpoint, 1);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = relation.rule
    else {
        panic!("transported unsigned relation uses two-citation transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 1 }
    ));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 2 }));
    assert!(
        prove_canonical_integer_proposition(
            &unsigned_context,
            &unsigned_goal,
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                    value(4, unsigned),
                ),
                Proposition::Equal(value(3, unsigned), value(2, unsigned)),
            ],
            &[],
        )
        .is_none(),
        "missing middle relation cannot prove transported definedness",
    );

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let signed_context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(signed)),
    ])
    .expect("four i8 values");
    let signed_divisor = value(2, signed);
    let signed_goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(signed_divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let signed_proof = prove_canonical_integer_proposition(
        &signed_context,
        &signed_goal,
        &[
            Proposition::LessOrEqual(value(3, signed), value(4, signed)),
            Proposition::LessOrEqual(value(4, signed), integer(signed, -2)),
            Proposition::Equal(value(3, signed), value(2, signed)),
        ],
        &[],
    )
    .expect("two cited bounds transport to the signed divisor");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
        panic!("signed two-citation transport selects its canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation, endpoint, ..
    } = disjunct.rule
    else {
        panic!("signed two-citation transport uses substitution")
    };
    assert_eq!(endpoint, 0);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = relation.rule
    else {
        panic!("transported signed relation uses two-citation transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 1 }
    ));
}

#[test]
fn exact_division_goal_nests_two_citation_dividend_floor_under_endpoint_transport() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(signed)),
    ])
    .expect("four i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let assumptions = [
        Proposition::LessOrEqual(value(2, signed), integer(signed, -1)),
        Proposition::LessOrEqual(integer(signed, -127), value(4, signed)),
        Proposition::LessOrEqual(value(4, signed), value(3, signed)),
        Proposition::Equal(value(3, signed), value(1, signed)),
    ];
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
        .expect("two cited bounds transport to the signed dividend");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("two-citation dividend transport selects the joint arm")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("two-citation dividend transport proves both joint premises")
    };
    assert!(matches!(
        conjuncts[0].rule,
        ProofRule::Assumption { index: 0 }
    ));
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = &conjuncts[1].rule
    else {
        panic!("dividend floor uses endpoint substitution")
    };
    assert_eq!(*endpoint, 1);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = &relation.rule
    else {
        panic!("transported dividend floor uses two-citation transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Assumption { index: 1 }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 2 }
    ));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 3 }));

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                assumptions[0].clone(),
                assumptions[1].clone(),
                assumptions[3].clone(),
            ],
            &[],
        )
        .is_none(),
        "missing middle bound cannot prove the transported dividend floor",
    );
}

#[test]
fn i1_exact_division_goal_nests_two_citation_transport_for_both_endpoints() {
    let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(6).unwrap(), ScalarType::Integer(i1)),
    ])
    .expect("six i1 values");
    let goal = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(value(2, i1), integer(i1, -1)),
        Proposition::LessOrEqual(integer(i1, 0), value(1, i1)),
    ]);
    let assumptions = [
        Proposition::LessOrEqual(value(3, i1), value(4, i1)),
        Proposition::LessOrEqual(value(4, i1), integer(i1, -1)),
        Proposition::Equal(value(3, i1), value(2, i1)),
        Proposition::LessOrEqual(integer(i1, 0), value(6, i1)),
        Proposition::LessOrEqual(value(6, i1), value(5, i1)),
        Proposition::Equal(value(5, i1), value(1, i1)),
    ];
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
        .expect("two cited bounds transport to both i1 operands");
    let ProofRule::ConjunctionIntroduction(conjuncts) = proof.rule else {
        panic!("i1 two-citation transport proves both canonical conjuncts")
    };
    for (conjunct, endpoint, first, second, equality) in
        [(&conjuncts[0], 0, 0, 1, 2), (&conjuncts[1], 1, 3, 4, 5)]
    {
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality: equality_proof,
            endpoint: actual_endpoint,
        } = &conjunct.rule
        else {
            panic!("i1 bound uses endpoint substitution")
        };
        assert_eq!(*actual_endpoint, endpoint);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = &relation.rule
        else {
            panic!("transported i1 bound uses two-citation transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index } if index == first
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index } if index == second
        ));
        assert!(matches!(
            equality_proof.rule,
            ProofRule::Assumption { index } if index == equality
        ));
    }

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                assumptions[0].clone(),
                assumptions[1].clone(),
                assumptions[2].clone(),
                assumptions[3].clone(),
                assumptions[5].clone(),
            ],
            &[],
        )
        .is_none(),
        "missing one middle bound cannot prove the complete i1 conjunction",
    );
}

#[test]
fn exact_division_goal_composes_two_citation_bounds_for_both_signed_joint_conjuncts() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(signed)),
    ])
    .expect("four i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let assumptions = [
        Proposition::LessOrEqual(value(2, signed), value(3, signed)),
        Proposition::LessOrEqual(value(3, signed), integer(signed, -1)),
        Proposition::LessOrEqual(integer(signed, -127), value(4, signed)),
        Proposition::LessOrEqual(value(4, signed), value(1, signed)),
    ];
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
        .expect("two cited bounds prove each signed joint conjunct");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("two-citation joint bounds select the canonical joint arm")
    };
    assert_eq!(index, 2);
    let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
        panic!("two-citation joint bounds prove both canonical conjuncts")
    };
    for (conjunct, first, second) in [(&conjuncts[0], 0, 1), (&conjuncts[1], 2, 3)] {
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = &conjunct.rule
        else {
            panic!("joint conjunct uses two-citation transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index } if index == first
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index } if index == second
        ));
    }

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                assumptions[0].clone(),
                assumptions[1].clone(),
                assumptions[2].clone(),
            ],
            &[],
        )
        .is_none(),
        "missing one citation cannot prove both joint conjuncts",
    );
}

#[test]
fn exact_division_goal_composes_two_exact_transitive_bound_citations() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(unsigned)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(unsigned)),
    ])
    .expect("three u8 values");
    let unsigned_one = ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one");
    let unsigned_goal = Proposition::LessOrEqual(unsigned_one.clone(), value(2, unsigned));
    let unsigned_proof = prove_canonical_integer_proposition(
        &context,
        &unsigned_goal,
        &[
            Proposition::LessOrEqual(unsigned_one, value(3, unsigned)),
            Proposition::LessOrEqual(value(3, unsigned), value(2, unsigned)),
        ],
        &[],
    )
    .expect("two exact unsigned bounds compose transitively");
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = unsigned_proof.rule
    else {
        panic!("two exact unsigned bounds use transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 1 }
    ));

    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
    ])
    .expect("three i8 values");
    let signed_dividend = value(1, signed);
    let signed_divisor = value(2, signed);
    let signed_goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), signed_dividend),
        ]),
    ]);
    let negative_proof = prove_canonical_integer_proposition(
        &context,
        &signed_goal,
        &[
            Proposition::LessOrEqual(signed_divisor, value(3, signed)),
            Proposition::LessOrEqual(value(3, signed), integer(signed, -2)),
        ],
        &[],
    )
    .expect("two exact signed negative bounds compose transitively");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative_proof.rule else {
        panic!("signed negative transitivity selects its canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = disjunct.rule
    else {
        panic!("two exact signed negative bounds use transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 1 }
    ));
}

#[test]
fn exact_division_goal_proves_single_definition_affine_safe_divisor() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=4).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("four i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let root_bound = Proposition::LessOrEqual(integer(signed, 0), value(3, signed));
    let definition = Proposition::Equal(
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
        value(2, signed),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&root_bound),
        std::slice::from_ref(&definition),
    )
    .expect("affine root bound proves the canonical positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("affine safe divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound {
        root_bound: child,
        witness,
    } = disjunct.rule
    else {
        panic!("affine safe divisor uses the checked affine-bound rule")
    };
    assert!(matches!(child.rule, ProofRule::Assumption { index: 0 }));
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(2, signed));
    assert_eq!(witness.definition_axioms, vec![0]);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[],
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "an affine definition without its root bound is not proof authority",
    );
    let redirected = Proposition::Equal(
        value(4, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("redirected exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &[root_bound], &[redirected],)
            .is_none(),
        "a definition for another target cannot prove divisor safety",
    );
}

#[test]
fn exact_division_goal_proves_uniquely_landed_affine_sibling() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let root_bound = Proposition::LessOrEqual(integer(signed, 0), value(3, signed));
    let landing = Proposition::Equal(value(4, signed), integer(signed, 1));
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), value(4, signed))
            .expect("exact add"),
    );

    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&root_bound),
        &[landing.clone(), definition.clone()],
    )
    .expect("the exact earlier sibling landing completes the affine word");
    let ProofRule::DisjunctionIntroduction { disjunct, index: 1 } = proof.rule else {
        panic!("landed affine sibling selects the positive divisor arm")
    };
    let ProofRule::IntegerAffineBound { witness, .. } = disjunct.rule else {
        panic!("landed affine sibling remains kernel-checked")
    };
    assert_eq!(witness.definition_axioms, [1]);
    assert_eq!(witness.literal_axioms, [Some(0)]);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&root_bound),
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "a value sibling without its landing is not literal authority",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&root_bound),
            &[landing.clone(), landing.clone(), definition.clone()],
        )
        .is_none(),
        "ambiguous sibling landings fail closed",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&root_bound),
            &[
                Proposition::Equal(value(5, signed), integer(signed, 1)),
                definition,
            ],
        )
        .is_none(),
        "a landing for another value cannot supply sibling custody",
    );
}

#[test]
fn exact_division_goal_composes_landed_affine_source_through_partial_cast() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i16_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(i16_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i16_type)),
        (ValueId::new(6).unwrap(), ScalarType::Integer(i16_type)),
    ])
    .expect("mixed cast context");
    let divisor = value(2, i8_type);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(i8_type, -2)),
        Proposition::LessOrEqual(integer(i8_type, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(i8_type, -1)),
            Proposition::LessOrEqual(integer(i8_type, -127), value(1, i8_type)),
        ]),
    ]);
    let root_bound = Proposition::LessOrEqual(integer(i16_type, 0), value(3, i16_type));
    let landing = Proposition::Equal(value(4, i16_type), integer(i16_type, 1));
    let affine = Proposition::Equal(
        value(5, i16_type),
        ScalarTerm::exact_integer_add(i16_type, value(3, i16_type), value(4, i16_type))
            .expect("exact add"),
    );
    let cast = Proposition::Equal(
        value(2, i8_type),
        ScalarTerm::integer_exact_cast(i16_type, i8_type, value(5, i16_type)).expect("exact cast"),
    );

    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&root_bound),
        &[landing.clone(), affine.clone(), cast.clone()],
    )
    .expect("landed affine bound composes through one partial cast");
    let ProofRule::DisjunctionIntroduction { disjunct, index: 1 } = proof.rule else {
        panic!("affine-cast divisor selects the positive arm")
    };
    let ProofRule::IntegerCastBound {
        root_bound: affine_bound,
        witness: cast_witness,
    } = disjunct.rule
    else {
        panic!("outer cast custody is explicit")
    };
    let ProofRule::IntegerAffineBound {
        witness: affine_witness,
        ..
    } = affine_bound.rule
    else {
        panic!("cast child independently retains affine custody")
    };
    assert_eq!(affine_witness.definition_axioms, [1]);
    assert_eq!(affine_witness.literal_axioms, [Some(0)]);
    assert_eq!(cast_witness.definition_axioms, [2]);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&root_bound),
            &[affine.clone(), cast.clone()],
        )
        .is_none(),
        "the affine sibling landing remains mandatory",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&root_bound),
            &[cast, landing, affine],
        )
        .is_none(),
        "a cast cannot authorize definitions that land after it",
    );
}

#[test]
fn exact_division_goal_relaxes_stronger_affine_endpoint_bounds() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=3).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("three i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);

    let positive_root_bound = Proposition::LessOrEqual(integer(signed, 0), value(3, signed));
    let positive_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 2))
            .expect("exact add"),
    );
    let positive = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&positive_root_bound),
        std::slice::from_ref(&positive_definition),
    )
    .expect("a stronger positive affine bound relaxes to the canonical arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = positive.rule else {
        panic!("strong positive affine bound selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = disjunct.rule
    else {
        panic!("strong positive affine bound uses one closed bridge")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    let ProofRule::IntegerAffineBound { witness, .. } = middle_less_or_equal_right.rule else {
        panic!("the right transitivity child owns the positive affine bound")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.definition_axioms, vec![0]);

    let negative_root_bound = Proposition::LessOrEqual(value(3, signed), integer(signed, 0));
    let negative_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_subtract(signed, value(3, signed), integer(signed, 3))
            .expect("exact subtract"),
    );
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&negative_root_bound),
        std::slice::from_ref(&negative_definition),
    )
    .expect("a stronger negative affine bound relaxes to the canonical arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative.rule else {
        panic!("strong negative affine bound selects one canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = disjunct.rule
    else {
        panic!("strong negative affine bound uses one closed bridge")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::IntegerAffineBound { .. }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));

    let weak_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 0))
            .expect("exact add zero"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[positive_root_bound],
            &[weak_definition],
        )
        .is_none(),
        "a weaker mapped endpoint cannot reverse the closed bridge",
    );
}

#[test]
fn exact_division_goal_proves_landed_literal_affine_root() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=4).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("four i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let landed_root = Proposition::Equal(value(3, signed), integer(signed, 0));

    let positive_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
    );
    let positive = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&landed_root),
        std::slice::from_ref(&positive_definition),
    )
    .expect("a landed literal proves the positive affine divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = positive.rule else {
        panic!("landed positive affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound { root_bound, .. } = disjunct.rule else {
        panic!("landed positive divisor uses the affine-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("landed positive root uses endpoint substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(
        relation.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 0 }));

    let negative_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_subtract(signed, value(3, signed), integer(signed, 2))
            .expect("exact subtract"),
    );
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&landed_root),
        std::slice::from_ref(&negative_definition),
    )
    .expect("a landed literal proves the negative affine divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative.rule else {
        panic!("landed negative affine divisor selects one canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerAffineBound { root_bound, .. } = disjunct.rule else {
        panic!("landed negative divisor uses the affine-bound rule")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 0, .. }
    ));

    let unsafe_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 0))
            .expect("exact add zero"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&landed_root),
            std::slice::from_ref(&unsafe_definition),
        )
        .is_none(),
        "a landed zero divisor cannot prove either safe arm",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[Proposition::Equal(value(4, signed), integer(signed, 0))],
            &[positive_definition],
        )
        .is_none(),
        "a redirected landed literal cannot provide affine root custody",
    );
}

#[test]
fn exact_division_goal_maps_checked_contiguous_cast_root_bound() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let context = PropositionContext::from_value_types([
        (
            ValueId::new(1).expect("dividend"),
            ScalarType::Integer(i8_type),
        ),
        (
            ValueId::new(2).expect("divisor"),
            ScalarType::Integer(i8_type),
        ),
        (
            ValueId::new(3).expect("root"),
            ScalarType::Integer(i16_type),
        ),
        (
            ValueId::new(4).expect("middle"),
            ScalarType::Integer(i16_type),
        ),
        (
            ValueId::new(5).expect("wide root"),
            ScalarType::Integer(i32_type),
        ),
        (
            ValueId::new(6).expect("redirected root"),
            ScalarType::Integer(i32_type),
        ),
        (
            ValueId::new(7).expect("redirected bound"),
            ScalarType::Integer(i32_type),
        ),
        (
            ValueId::new(8).expect("third alias"),
            ScalarType::Integer(i32_type),
        ),
    ])
    .expect("cast values");
    let divisor = value(2, i8_type);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(i8_type, -2)),
        Proposition::LessOrEqual(integer(i8_type, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(i8_type, -1)),
            Proposition::LessOrEqual(integer(i8_type, -127), value(1, i8_type)),
        ]),
    ]);
    let cast = Proposition::Equal(
        value(2, i8_type),
        ScalarTerm::integer_exact_cast(i16_type, i8_type, value(3, i16_type))
            .expect("partial exact cast"),
    );
    let positive_bound = Proposition::LessOrEqual(integer(i16_type, 1), value(3, i16_type));
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&positive_bound),
        std::slice::from_ref(&cast),
    )
    .expect("one checked cast maps the positive root bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("cast-bound divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerCastBound {
        root_bound,
        witness,
    } = disjunct.rule
    else {
        panic!("cast-bound divisor uses the cast-bound rule")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert_eq!(witness.root, value(3, i16_type));
    assert_eq!(witness.target, value(2, i8_type));
    assert_eq!(witness.definition_axioms, vec![0]);

    let negative_bound = Proposition::LessOrEqual(value(3, i16_type), integer(i16_type, -2));
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&negative_bound),
        std::slice::from_ref(&cast),
    )
    .expect("one checked cast maps the negative root bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative.rule else {
        panic!("negative cast-bound divisor selects one canonical arm")
    };
    assert_eq!(index, 0);
    assert!(matches!(disjunct.rule, ProofRule::IntegerCastBound { .. }));

    assert!(
        prove_canonical_integer_proposition(&context, &goal, &[], std::slice::from_ref(&cast),)
            .is_none(),
        "the cast definition does not supply root-bound authority",
    );
    let wide_bound = Proposition::LessOrEqual(integer(i32_type, 1), value(5, i32_type));
    let first_cast = Proposition::Equal(
        value(4, i16_type),
        ScalarTerm::integer_exact_cast(i32_type, i16_type, value(5, i32_type))
            .expect("first partial cast"),
    );
    let second_cast = Proposition::Equal(
        value(2, i8_type),
        ScalarTerm::integer_exact_cast(i16_type, i8_type, value(4, i16_type))
            .expect("second partial cast"),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&wide_bound),
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("the complete checked cast spine maps the root bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("cast-chain divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerCastBound { witness, .. } = disjunct.rule else {
        panic!("cast-chain divisor uses the cast-bound rule")
    };
    assert_eq!(witness.definition_axioms, vec![0, 1]);

    let landed_positive = Proposition::Equal(value(5, i32_type), integer(i32_type, 1));
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&landed_positive),
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("one exact landed root literal supplies cast-chain custody");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("landed cast-chain divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerCastBound { root_bound, .. } = disjunct.rule else {
        panic!("landed cast-chain divisor uses the cast-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("landed root bound uses one exact substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(
        relation.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 0 }));

    let landed_negative = Proposition::Equal(value(5, i32_type), integer(i32_type, -2));
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&landed_negative),
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("one exact landed negative literal supplies cast-chain custody");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative.rule else {
        panic!("negative landed cast-chain divisor selects one canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerCastBound { root_bound, .. } = disjunct.rule else {
        panic!("negative landed chain uses the cast-bound rule")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 0, .. }
    ));

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[Proposition::Equal(value(6, i32_type), integer(i32_type, 1))],
            &[first_cast.clone(), second_cast.clone()],
        )
        .is_none(),
        "a redirected landed literal does not supply root custody",
    );
    let stronger_positive = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[Proposition::Equal(value(5, i32_type), integer(i32_type, 2))],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("a closed stronger positive root literal proves the canonical arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = stronger_positive.rule else {
        panic!("stronger positive root selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerCastBound { root_bound, .. } = disjunct.rule else {
        panic!("stronger positive root uses the cast-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution { relation, .. } = root_bound.rule else {
        panic!("stronger positive root uses one substitution")
    };
    assert_eq!(
        relation.conclusion,
        Proposition::LessOrEqual(integer(i32_type, 1), integer(i32_type, 2)),
    );

    let stronger_negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[Proposition::Equal(
            value(5, i32_type),
            integer(i32_type, -3),
        )],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("a closed stronger negative root literal proves the canonical arm");
    let ProofRule::DisjunctionIntroduction { index, .. } = stronger_negative.rule else {
        panic!("stronger negative root selects one canonical arm")
    };
    assert_eq!(index, 0);

    for weak in [0, -1] {
        assert!(
            prove_canonical_integer_proposition(
                &context,
                &goal,
                &[Proposition::Equal(
                    value(5, i32_type),
                    integer(i32_type, weak),
                )],
                &[first_cast.clone(), second_cast.clone()],
            )
            .is_none(),
            "a weaker landed root cannot justify either canonical arm",
        );
    }

    let root_alias = Proposition::Equal(value(5, i32_type), value(6, i32_type));
    let alias_positive_bound = Proposition::LessOrEqual(integer(i32_type, 1), value(6, i32_type));
    let alias_positive = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[alias_positive_bound.clone(), root_alias.clone()],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("one exact root alias transports its directly cited positive bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = alias_positive.rule else {
        panic!("aliased positive root selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerCastBound { root_bound, .. } = disjunct.rule else {
        panic!("aliased positive root uses the cast-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("aliased root bound uses one substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(relation.rule, ProofRule::Assumption { index: 0 }));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));

    let alias_negative_bound = Proposition::LessOrEqual(value(6, i32_type), integer(i32_type, -2));
    let alias_negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[alias_negative_bound, root_alias.clone()],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("one exact root alias transports its directly cited negative bound");
    let ProofRule::DisjunctionIntroduction { index, .. } = alias_negative.rule else {
        panic!("aliased negative root selects one canonical arm")
    };
    assert_eq!(index, 0);

    let stronger_alias_positive_bound =
        Proposition::LessOrEqual(integer(i32_type, 2), value(6, i32_type));
    let stronger_alias_positive = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[stronger_alias_positive_bound, root_alias.clone()],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("one closed bridge strengthens the directly cited alias bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = stronger_alias_positive.rule
    else {
        panic!("stronger alias bound selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerCastBound { root_bound, .. } = disjunct.rule else {
        panic!("stronger alias bound uses the cast-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("stronger alias bound substitutes the root")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = relation.rule
    else {
        panic!("stronger alias bound uses one closed transitivity bridge")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 0 }
    ));

    let stronger_alias_negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[
            Proposition::LessOrEqual(value(6, i32_type), integer(i32_type, -3)),
            root_alias.clone(),
        ],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("one closed bridge strengthens the negative alias bound");
    let ProofRule::DisjunctionIntroduction { index, .. } = stronger_alias_negative.rule else {
        panic!("stronger negative alias bound selects one canonical arm")
    };
    assert_eq!(index, 0);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&alias_positive_bound),
            &[first_cast.clone(), second_cast.clone()],
        )
        .is_none(),
        "an alias bound without its exact equality does not reach the cast root",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                Proposition::LessOrEqual(integer(i32_type, 1), value(7, i32_type)),
                root_alias.clone(),
            ],
            &[first_cast.clone(), second_cast.clone()],
        )
        .is_none(),
        "a redirected bound cannot ride an unrelated root equality",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                Proposition::LessOrEqual(integer(i32_type, 0), value(6, i32_type)),
                root_alias.clone(),
            ],
            &[first_cast.clone(), second_cast.clone()],
        )
        .is_none(),
        "the fixed alias sibling does not strengthen a weaker bound",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                Proposition::LessOrEqual(value(6, i32_type), integer(i32_type, -1)),
                root_alias.clone(),
            ],
            &[first_cast.clone(), second_cast.clone()],
        )
        .is_none(),
        "a weaker negative alias bound cannot justify either canonical arm",
    );

    let alias_landed_positive = Proposition::Equal(value(6, i32_type), integer(i32_type, 2));
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[root_alias.clone(), alias_landed_positive.clone()],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("one root alias may land one stronger literal before the cast");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("literal-via-alias selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerCastBound { root_bound, .. } = disjunct.rule else {
        panic!("literal-via-alias uses the cast-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation: alias_bound,
        equality: root_equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("literal-via-alias substitutes the root")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(
        root_equality.rule,
        ProofRule::Assumption { index: 0 }
    ));
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality: literal_equality,
        endpoint,
    } = alias_bound.rule
    else {
        panic!("literal-via-alias substitutes the alias")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(
        relation.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));
    assert!(matches!(
        literal_equality.rule,
        ProofRule::Assumption { index: 1 }
    ));

    let alias_landed_negative = Proposition::Equal(value(6, i32_type), integer(i32_type, -3));
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[root_alias.clone(), alias_landed_negative],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("one root alias may land one stronger negative literal");
    let ProofRule::DisjunctionIntroduction { index, .. } = negative.rule else {
        panic!("negative literal-via-alias selects one canonical arm")
    };
    assert_eq!(index, 0);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&alias_landed_positive),
            &[first_cast.clone(), second_cast.clone()],
        )
        .is_none(),
        "a landed alias literal without the root equality cannot reach the cast",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                root_alias.clone(),
                Proposition::Equal(value(7, i32_type), integer(i32_type, 2)),
            ],
            &[first_cast.clone(), second_cast.clone()],
        )
        .is_none(),
        "a redirected landed literal cannot bind the selected alias",
    );
    for weak in [0, -1] {
        assert!(
            prove_canonical_integer_proposition(
                &context,
                &goal,
                &[
                    root_alias.clone(),
                    Proposition::Equal(value(6, i32_type), integer(i32_type, weak)),
                ],
                &[first_cast.clone(), second_cast.clone()],
            )
            .is_none(),
            "a weaker landed alias literal cannot justify either arm",
        );
    }

    let middle_alias = Proposition::Equal(value(6, i32_type), value(7, i32_type));
    let two_alias_bound = Proposition::LessOrEqual(integer(i32_type, 1), value(7, i32_type));
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[
            two_alias_bound.clone(),
            middle_alias.clone(),
            root_alias.clone(),
        ],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("exactly two root aliases transport one directly cited bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("two-alias cast bound selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerCastBound { root_bound, .. } = disjunct.rule else {
        panic!("two-alias bound uses the cast-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation: middle_bound,
        equality: outer_equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("two-alias bound substitutes the cast root")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(
        outer_equality.rule,
        ProofRule::Assumption { index: 2 }
    ));
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality: inner_equality,
        endpoint,
    } = middle_bound.rule
    else {
        panic!("two-alias bound substitutes the middle alias")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(relation.rule, ProofRule::Assumption { index: 0 }));
    assert!(matches!(
        inner_equality.rule,
        ProofRule::Assumption { index: 1 }
    ));

    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[
            Proposition::LessOrEqual(value(7, i32_type), integer(i32_type, -2)),
            middle_alias.clone(),
            root_alias.clone(),
        ],
        &[first_cast.clone(), second_cast.clone()],
    )
    .expect("two aliases transport one directly cited negative bound");
    let ProofRule::DisjunctionIntroduction { index, .. } = negative.rule else {
        panic!("negative two-alias bound selects one canonical arm")
    };
    assert_eq!(index, 0);

    for rejected in [
        vec![two_alias_bound.clone(), root_alias.clone()],
        vec![
            two_alias_bound.clone(),
            Proposition::Equal(value(6, i32_type), value(8, i32_type)),
            root_alias.clone(),
        ],
        vec![
            Proposition::LessOrEqual(integer(i32_type, 0), value(7, i32_type)),
            middle_alias.clone(),
            root_alias.clone(),
        ],
        vec![
            Proposition::LessOrEqual(integer(i32_type, 1), value(8, i32_type)),
            Proposition::Equal(value(7, i32_type), value(8, i32_type)),
            middle_alias.clone(),
            root_alias.clone(),
        ],
    ] {
        assert!(
            prove_canonical_integer_proposition(
                &context,
                &goal,
                &rejected,
                &[first_cast.clone(), second_cast.clone()],
            )
            .is_none(),
            "missing, redirected, weaker, or third-link facts remain outside the fixed sibling",
        );
    }

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&wide_bound),
            &[second_cast.clone(), first_cast.clone()],
        )
        .is_none(),
        "a source-reversed cast ledger cannot be reordered by the producer",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&wide_bound),
            &[first_cast, second_cast.clone(), second_cast],
        )
        .is_none(),
        "duplicate target definitions reject instead of selecting authority",
    );
}

#[test]
fn exact_division_goal_lands_affine_root_literal_through_one_alias() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let root_alias = Proposition::Equal(value(3, signed), value(4, signed));
    let landed_alias = Proposition::Equal(value(4, signed), integer(signed, 0));
    let positive_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[root_alias.clone(), landed_alias.clone()],
        std::slice::from_ref(&positive_definition),
    )
    .expect("one alias transports exact literal custody to the affine root");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("alias-landed affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound { root_bound, .. } = disjunct.rule else {
        panic!("alias-landed divisor uses the affine-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("the root alias uses the outer endpoint substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 0 }));
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = relation.rule
    else {
        panic!("the landed alias uses the inner endpoint substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
    assert!(matches!(
        relation.rule,
        ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
    ));

    let negative_definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_subtract(signed, value(3, signed), integer(signed, 2))
            .expect("exact subtract"),
    );
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[root_alias.clone(), landed_alias.clone()],
        std::slice::from_ref(&negative_definition),
    )
    .expect("one alias transports an upper literal bound to the affine root");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative.rule else {
        panic!("negative alias-landed divisor selects one canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerAffineBound { root_bound, .. } = disjunct.rule else {
        panic!("negative alias-landed divisor uses the affine-bound rule")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 0, .. }
    ));

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&landed_alias),
            std::slice::from_ref(&positive_definition),
        )
        .is_none(),
        "a landed alias without the root equality has no affine custody",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                root_alias.clone(),
                Proposition::Equal(value(5, signed), integer(signed, 0)),
            ],
            std::slice::from_ref(&positive_definition),
        )
        .is_none(),
        "a redirected landing cannot establish the root alias",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                root_alias,
                Proposition::Equal(value(4, signed), value(5, signed)),
                Proposition::Equal(value(5, signed), integer(signed, 0)),
            ],
            &[positive_definition],
        )
        .is_none(),
        "a second value alias is outside the fixed literal-landing family",
    );
}

#[test]
fn exact_division_goal_transports_affine_bound_through_target_alias() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let target_alias = Proposition::Equal(value(4, signed), value(2, signed));

    let positive_root_bound = Proposition::LessOrEqual(integer(signed, 0), value(3, signed));
    let positive_definition = Proposition::Equal(
        value(4, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
    );
    let positive = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[positive_root_bound.clone(), target_alias.clone()],
        std::slice::from_ref(&positive_definition),
    )
    .expect("one target alias transports the positive affine bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = positive.rule else {
        panic!("target-aliased positive divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = disjunct.rule
    else {
        panic!("target-aliased positive bound uses endpoint substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(
        relation.rule,
        ProofRule::IntegerAffineBound { .. }
    ));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));

    let negative_root_bound = Proposition::LessOrEqual(value(3, signed), integer(signed, 0));
    let negative_definition = Proposition::Equal(
        value(4, signed),
        ScalarTerm::exact_integer_subtract(signed, value(3, signed), integer(signed, 2))
            .expect("exact subtract"),
    );
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[negative_root_bound, target_alias.clone()],
        std::slice::from_ref(&negative_definition),
    )
    .expect("one target alias transports the negative affine bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative.rule else {
        panic!("target-aliased negative divisor selects one canonical arm")
    };
    assert_eq!(index, 0);
    assert!(matches!(
        disjunct.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 0, .. }
    ));

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&positive_root_bound),
            std::slice::from_ref(&positive_definition),
        )
        .is_none(),
        "an affine alias bound without its target equality cannot prove the goal",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                positive_root_bound,
                Proposition::Equal(value(4, signed), value(5, signed)),
            ],
            &[positive_definition],
        )
        .is_none(),
        "a redirected target equality cannot transport the affine bound",
    );
}

#[test]
fn exact_division_goal_transports_affine_bound_through_two_target_aliases() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=6).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("six i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let outer_alias = Proposition::Equal(value(2, signed), value(4, signed));
    let inner_alias = Proposition::Equal(value(4, signed), value(5, signed));
    let positive_root_bound = Proposition::LessOrEqual(integer(signed, 0), value(3, signed));
    let positive_definition = Proposition::Equal(
        value(5, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[
            positive_root_bound.clone(),
            outer_alias.clone(),
            inner_alias.clone(),
        ],
        std::slice::from_ref(&positive_definition),
    )
    .expect("two exact target aliases transport the positive affine bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("two-target-alias divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = disjunct.rule
    else {
        panic!("the canonical target uses the outer substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = relation.rule
    else {
        panic!("the middle target uses the inner substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 2 }));
    assert!(matches!(
        relation.rule,
        ProofRule::IntegerAffineBound { .. }
    ));

    let negative_root_bound = Proposition::LessOrEqual(value(3, signed), integer(signed, 0));
    let negative_definition = Proposition::Equal(
        value(5, signed),
        ScalarTerm::exact_integer_subtract(signed, value(3, signed), integer(signed, 2))
            .expect("exact subtract"),
    );
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[
            negative_root_bound,
            outer_alias.clone(),
            inner_alias.clone(),
        ],
        std::slice::from_ref(&negative_definition),
    )
    .expect("two exact target aliases transport the negative affine bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative.rule else {
        panic!("negative two-target-alias divisor selects one canonical arm")
    };
    assert_eq!(index, 0);
    assert!(matches!(
        disjunct.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 0, .. }
    ));

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[positive_root_bound.clone(), outer_alias.clone()],
            std::slice::from_ref(&positive_definition),
        )
        .is_none(),
        "a missing inner target equality cannot reach the affine bound",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                positive_root_bound.clone(),
                outer_alias.clone(),
                Proposition::Equal(value(4, signed), value(6, signed)),
            ],
            std::slice::from_ref(&positive_definition),
        )
        .is_none(),
        "a redirected inner target equality cannot reach the affine bound",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                positive_root_bound,
                outer_alias,
                inner_alias,
                Proposition::Equal(value(5, signed), value(6, signed)),
            ],
            &[Proposition::Equal(
                value(6, signed),
                ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1),)
                    .expect("exact add"),
            )],
        )
        .is_none(),
        "a third target alias is outside the fixed two-equality family",
    );
}

#[test]
fn exact_division_goal_proves_alias_substituted_affine_root_bound() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let alias_equality = Proposition::Equal(value(3, signed), value(4, signed));
    let alias_bound = Proposition::LessOrEqual(integer(signed, 0), value(4, signed));
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[alias_equality.clone(), alias_bound.clone()],
        std::slice::from_ref(&definition),
    )
    .expect("an exact alias transports the affine root bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("alias-substituted affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound {
        root_bound,
        witness,
    } = disjunct.rule
    else {
        panic!("alias-substituted divisor uses the affine-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("the affine root bound uses exact endpoint substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(relation.rule, ProofRule::Assumption { index: 1 }));
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 0 }));
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.definition_axioms, vec![0]);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&alias_bound),
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "an alias bound without its equality has no root custody",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&alias_equality),
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "an alias equality without its bound has no root custody",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                Proposition::Equal(value(5, signed), value(4, signed)),
                alias_bound,
            ],
            &[definition],
        )
        .is_none(),
        "a redirected equality cannot transport the affine root bound",
    );
}

#[test]
fn exact_division_goal_transports_bound_through_two_affine_root_aliases() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=7).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("seven i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let root_to_middle_alias = Proposition::Equal(value(3, signed), value(4, signed));
    let middle_to_bound_alias = Proposition::Equal(value(4, signed), value(5, signed));
    let lower_bound = Proposition::LessOrEqual(integer(signed, 0), value(5, signed));
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[
            root_to_middle_alias.clone(),
            middle_to_bound_alias.clone(),
            lower_bound.clone(),
        ],
        std::slice::from_ref(&definition),
    )
    .expect("two exact aliases transport the affine root lower bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("two-alias affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound { root_bound, .. } = disjunct.rule else {
        panic!("two-alias divisor uses the affine-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("the outer root alias uses endpoint substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 0 }));
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = relation.rule
    else {
        panic!("the inner root alias uses endpoint substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
    assert!(matches!(relation.rule, ProofRule::Assumption { index: 2 }));

    let upper_bound = Proposition::LessOrEqual(value(5, signed), integer(signed, -3));
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[
            root_to_middle_alias.clone(),
            middle_to_bound_alias.clone(),
            upper_bound,
        ],
        std::slice::from_ref(&definition),
    )
    .expect("two exact aliases transport the affine root upper bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative.rule else {
        panic!("negative two-alias affine divisor selects one canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerAffineBound { root_bound, .. } = disjunct.rule else {
        panic!("negative two-alias divisor uses the affine-bound rule")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 0, .. }
    ));

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[root_to_middle_alias.clone(), lower_bound.clone()],
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "a missing inner equality cannot establish root custody",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                root_to_middle_alias.clone(),
                Proposition::Equal(value(4, signed), value(6, signed)),
                lower_bound.clone(),
            ],
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "a redirected second equality cannot reach the bound alias",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                root_to_middle_alias,
                middle_to_bound_alias,
                Proposition::Equal(value(5, signed), value(6, signed)),
                Proposition::LessOrEqual(integer(signed, 0), value(6, signed)),
            ],
            &[definition],
        )
        .is_none(),
        "a third alias is outside the fixed two-equality custody family",
    );
}

#[test]
fn exact_division_goal_transports_transitive_bound_to_affine_root_alias() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=6).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("six i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let root_alias = Proposition::Equal(value(3, signed), value(4, signed));
    let lower_to_middle = Proposition::LessOrEqual(integer(signed, 0), value(5, signed));
    let middle_to_alias = Proposition::LessOrEqual(value(5, signed), value(4, signed));
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[
            root_alias.clone(),
            lower_to_middle.clone(),
            middle_to_alias.clone(),
        ],
        std::slice::from_ref(&definition),
    )
    .expect("two citations transport a lower bound through one root alias");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("transitively aliased affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound { root_bound, .. } = disjunct.rule else {
        panic!("transitively aliased divisor uses the affine-bound rule")
    };
    let ProofRule::IntegerLessOrEqualSubstitution {
        relation,
        equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("the transitive alias root bound uses endpoint substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 0 }));
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = relation.rule
    else {
        panic!("the substituted relation uses exactly two order citations")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Assumption { index: 1 }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 2 }
    ));

    let alias_to_middle = Proposition::LessOrEqual(value(4, signed), value(5, signed));
    let middle_to_ceiling = Proposition::LessOrEqual(value(5, signed), integer(signed, -3));
    let negative = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[root_alias.clone(), alias_to_middle, middle_to_ceiling],
        std::slice::from_ref(&definition),
    )
    .expect("two citations transport an upper bound through one root alias");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = negative.rule else {
        panic!("negative transitively aliased divisor selects one canonical arm")
    };
    assert_eq!(index, 0);
    let ProofRule::IntegerAffineBound { root_bound, .. } = disjunct.rule else {
        panic!("negative transitively aliased divisor uses the affine-bound rule")
    };
    assert!(matches!(
        root_bound.rule,
        ProofRule::IntegerLessOrEqualSubstitution { endpoint: 0, .. }
    ));

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[lower_to_middle.clone(), middle_to_alias.clone()],
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "a transitive alias bound without its equality has no root custody",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                root_alias.clone(),
                lower_to_middle.clone(),
                Proposition::LessOrEqual(value(6, signed), value(4, signed)),
            ],
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "a disconnected middle cannot establish the alias bound",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                Proposition::Equal(value(3, signed), value(6, signed)),
                lower_to_middle,
                middle_to_alias,
            ],
            &[definition],
        )
        .is_none(),
        "a redirected equality cannot transport the bound to the affine root",
    );
}

#[test]
fn exact_division_goal_proves_transitively_reconstructed_affine_root_bound() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=5).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("five i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let lower_to_middle = Proposition::LessOrEqual(integer(signed, 0), value(4, signed));
    let middle_to_root = Proposition::LessOrEqual(value(4, signed), value(3, signed));
    let definition = Proposition::Equal(
        value(2, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
    );
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        &[lower_to_middle.clone(), middle_to_root.clone()],
        std::slice::from_ref(&definition),
    )
    .expect("two exact order citations reconstruct the affine root bound");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("transitive affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound {
        root_bound,
        witness,
    } = disjunct.rule
    else {
        panic!("transitive divisor uses the affine-bound rule")
    };
    let ProofRule::IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle,
        middle_less_or_equal_right,
    } = root_bound.rule
    else {
        panic!("the affine root bound uses exact transitivity")
    };
    assert!(matches!(
        left_less_or_equal_middle.rule,
        ProofRule::Assumption { index: 0 }
    ));
    assert!(matches!(
        middle_less_or_equal_right.rule,
        ProofRule::Assumption { index: 1 }
    ));
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.definition_axioms, vec![0]);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&lower_to_middle),
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "the first transitive leg alone has no affine root custody",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&middle_to_root),
            std::slice::from_ref(&definition),
        )
        .is_none(),
        "the second transitive leg alone has no affine root custody",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                lower_to_middle,
                Proposition::LessOrEqual(value(5, signed), value(3, signed)),
            ],
            &[definition],
        )
        .is_none(),
        "disconnected bounds cannot reconstruct affine root custody",
    );
}

#[test]
fn exact_division_goal_proves_two_definition_affine_safe_divisor() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=4).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("four i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let root_bound = Proposition::LessOrEqual(integer(signed, -1), value(3, signed));
    let definitions = [
        Proposition::Equal(
            value(4, signed),
            ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
                .expect("first exact add"),
        ),
        Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(signed, value(4, signed), integer(signed, 1))
                .expect("second exact add"),
        ),
    ];
    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&root_bound),
        &definitions,
    )
    .expect("two-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("two-definition affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound {
        root_bound: child,
        witness,
    } = disjunct.rule
    else {
        panic!("two-definition affine divisor uses the affine-bound rule")
    };
    assert!(matches!(child.rule, ProofRule::Assumption { index: 0 }));
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(2, signed));
    assert_eq!(witness.definition_axioms, vec![0, 1]);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&root_bound),
            &definitions[..1],
        )
        .is_none(),
        "an incomplete definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[root_bound],
            &[definitions[1].clone(), definitions[0].clone()],
        )
        .is_none(),
        "a reversed definition word cannot claim canonical custody",
    );
}

#[test]
fn exact_division_goal_proves_three_and_four_definition_affine_safe_divisors() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=6).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("six i8 values");
    let divisor = value(2, signed);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
        Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor, integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
        ]),
    ]);
    let three_step_goal = Proposition::LessOrEqual(integer(signed, 1), value(6, signed));
    let three_step_root_bound = Proposition::LessOrEqual(integer(signed, -2), value(3, signed));
    let four_step_root_bound = Proposition::LessOrEqual(integer(signed, -3), value(3, signed));
    let definitions = [
        Proposition::Equal(
            value(4, signed),
            ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
                .expect("first exact add"),
        ),
        Proposition::Equal(
            value(5, signed),
            ScalarTerm::exact_integer_add(signed, value(4, signed), integer(signed, 1))
                .expect("second exact add"),
        ),
        Proposition::Equal(
            value(6, signed),
            ScalarTerm::exact_integer_add(signed, value(5, signed), integer(signed, 1))
                .expect("third exact add"),
        ),
        Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(signed, value(6, signed), integer(signed, 1))
                .expect("fourth exact add"),
        ),
    ];

    let three_step_proof = prove_canonical_integer_proposition(
        &context,
        &three_step_goal,
        std::slice::from_ref(&three_step_root_bound),
        &definitions,
    )
    .expect("three-definition affine word remains selectable first");
    let ProofRule::IntegerAffineBound { witness, .. } = three_step_proof.rule else {
        panic!("three-definition affine bound uses the affine-bound rule")
    };
    assert_eq!(witness.target, value(6, signed));
    assert_eq!(witness.definition_axioms, vec![0, 1, 2]);

    let proof = prove_canonical_integer_proposition(
        &context,
        &goal,
        std::slice::from_ref(&four_step_root_bound),
        &definitions,
    )
    .expect("four-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
        panic!("four-definition affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = disjunct.rule else {
        panic!("four-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(2, signed));
    assert_eq!(witness.definition_axioms, vec![0, 1, 2, 3]);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&four_step_root_bound),
            &definitions[..3],
        )
        .is_none(),
        "an incomplete four-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[four_step_root_bound],
            &[
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed four-definition word cannot claim canonical custody",
    );
}
