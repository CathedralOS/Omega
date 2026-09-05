use super::*;

#[test]
fn signed_division_composes_both_retained_nonzero_cases() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let context = two_value_context(integer_type);
    let dividend = value(1, integer_type);
    let divisor = value(2, integer_type);
    let negative = Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1));
    let positive = Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone());
    let nonzero = Proposition::Disjunction(vec![negative.clone(), positive.clone()]);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor, integer(integer_type, -2)),
        positive,
        Proposition::Conjunction(vec![
            negative,
            Proposition::LessOrEqual(integer(integer_type, -127), dividend.clone()),
        ]),
    ]);
    let facts = [
        Proposition::Equal(dividend, integer(integer_type, 7)),
        nonzero,
    ];
    let proof = prove_canonical_integer_proposition(&context, &goal, &[], &facts)
        .expect("both divisor signs prove the exact quotient is representable");
    let ProofRule::DisjunctionElimination { branches, .. } = &proof.rule else {
        panic!("no single sign was proved")
    };
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().all(|branch| branch.conclusion == goal));
    let mut missing_branch = proof.clone();
    let ProofRule::DisjunctionElimination { branches, .. } = &mut missing_branch.rule else {
        unreachable!()
    };
    branches.pop();
    assert!(accept_certificate(&context, &goal, &[], &facts, &missing_branch).is_err());
    assert!(prove_canonical_integer_proposition(&context, &goal, &[], &facts[..1]).is_none());
}

#[test]
fn independent_retained_cases_compose_without_leaking_sibling_assumptions() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let context = two_value_context(integer_type);
    let outside = |id, magnitude: i128| {
        let operand = value(id, integer_type);
        Proposition::Disjunction(vec![
            Proposition::LessOrEqual(operand.clone(), integer(integer_type, -magnitude)),
            Proposition::LessOrEqual(integer(integer_type, magnitude), operand),
        ])
    };
    let goal = Proposition::Conjunction(vec![outside(1, 1), outside(2, 1)]);
    let facts = [outside(1, 2), outside(2, 2)];
    let proof = prove_canonical_integer_proposition(&context, &goal, &[], &facts)
        .expect("independent cases retain both parameters");
    let ProofRule::ConjunctionIntroduction(conjuncts) = &proof.rule else {
        panic!("independent goals do not form a Cartesian product of their cases")
    };
    assert!(
        conjuncts
            .iter()
            .all(|proof| matches!(proof.rule, ProofRule::DisjunctionElimination { .. }))
    );
    for missing in 0..2 {
        let remaining = [facts[1 - missing].clone()];
        assert!(prove_canonical_integer_proposition(&context, &goal, &[], &remaining).is_none());
    }
    let only_negative = Proposition::LessOrEqual(value(1, integer_type), integer(integer_type, -1));
    assert!(prove_canonical_integer_proposition(&context, &only_negative, &[], &facts).is_none());
}
