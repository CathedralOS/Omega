use proof_admission::{
    BoundedTheoremError, Budget, Context, PrimitiveJudgment, ProofNode, ProofRule, Signature, Term,
    denote_bounded_implication_type, infer_sort, verify_bounded_theorem,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId,
};

fn fixture() -> (IntegerType, ValueId, ScalarTerm, ScalarTerm) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let id = ValueId::new(1).unwrap();
    (
        integer,
        id,
        ScalarTerm::value(id, ScalarType::Integer(integer)),
        ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap(),
    )
}

#[test]
fn the_expected_implication_is_formed_without_automatically_proving_it() {
    let (integer, id, subject, zero) = fixture();
    let conclusion = Proposition::LessThan(zero, subject);
    let expected = denote_bounded_implication_type(
        &[(id, ScalarType::Integer(integer))],
        &[],
        &[],
        &conclusion,
        &mut Budget::default(),
    )
    .expect("nonnegativity does not prove strict positivity, but the requirement has a type");
    let mut body = expected.ty;
    let mut binders = 0;
    while let Term::Pi { codomain, .. } = expected.arena.get(body) {
        binders += 1;
        body = codomain;
    }
    assert_eq!(binders, 3, "the u8 subject and both carrier bounds");
    assert!(matches!(expected.arena.get(body), Term::Squash { .. }));
}

#[test]
fn fixed_subjects_carrier_bounds_and_even_unused_premises_are_bound() {
    let (integer, id, subject, zero) = fixture();
    let premise = Proposition::LessThan(zero.clone(), subject.clone());
    let goal = Proposition::LessOrEqual(zero, subject);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
    };
    let mut theorem = verify_bounded_theorem(
        &[(id, ScalarType::Integer(integer))],
        &[],
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &proof,
        &mut Budget::default(),
    )
    .expect("the exact carrier supplies nonnegativity for arbitrary u8 inputs");
    let certificate = &theorem.denotation.certificate;
    assert!(certificate.context.is_empty());
    assert!(
        theorem.denotation.assumptions.is_empty(),
        "the derivation did not use the authored premise"
    );
    let mut body = certificate.expected;
    let mut domains = Vec::new();
    while let Term::Pi { domain, codomain } = theorem.denotation.arena.get(body) {
        domains.push(domain);
        body = codomain;
    }
    assert_eq!(
        domains.len(),
        4,
        "subject, two carrier bounds, and the full authored premise"
    );
    assert!(matches!(
        theorem.denotation.arena.get(domains[3]),
        Term::Squash { .. }
    ));
    assert!(matches!(
        theorem.denotation.arena.get(body),
        Term::Squash { .. }
    ));
    let context = Context::empty()
        .with_signature(Signature::from_declarations(certificate.signature.clone()));
    assert!(
        infer_sort(
            &mut theorem.denotation.arena,
            &context,
            certificate.expected,
            &mut Budget::default()
        )
        .unwrap()
        .is_strict()
    );
}

#[test]
fn the_same_carrier_cannot_prove_the_next_stricter_bound() {
    let (integer, id, subject, zero) = fixture();
    let goal = Proposition::LessThan(zero, subject);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
    };
    assert!(
        verify_bounded_theorem(
            &[(id, ScalarType::Integer(integer))],
            &[],
            &goal,
            &[],
            &[],
            &proof,
            &mut Budget::default()
        )
        .is_err()
    );
}

#[test]
fn a_duplicated_or_omitted_subject_cannot_change_the_theorem_telescope() {
    let (integer, id, subject, zero) = fixture();
    let goal = Proposition::LessOrEqual(zero, subject);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
    };
    let input = (id, ScalarType::Integer(integer));
    assert!(matches!(verify_bounded_theorem(
        &[input, input], &[], &goal, &[], &[], &proof, &mut Budget::default()
    ), Err(BoundedTheoremError::DuplicateSubject(found)) if found == id));
    assert!(
        verify_bounded_theorem(&[], &[], &goal, &[], &[], &proof, &mut Budget::default()).is_err()
    );
}

#[test]
fn a_singleton_conjunction_is_not_a_canonical_contract_conclusion() {
    let (integer, id, subject, zero) = fixture();
    let premise = Proposition::Disjunction(vec![
        Proposition::LessThan(subject.clone(), zero.clone()),
        Proposition::LessThan(zero.clone(), subject.clone()),
    ]);
    let conclusion = Proposition::Conjunction(vec![Proposition::LessThan(zero, subject)]);
    let refused = denote_bounded_implication_type(
        &[(id, ScalarType::Integer(integer))],
        &[],
        &[premise],
        &conclusion,
        &mut Budget::default(),
    )
    .err()
    .expect("the producer must preserve canonical proposition arity");
    assert!(format!("{refused:?}").contains("NonCanonicalConjunctionArity(1)"));
}

#[test]
fn a_hypothesis_binds_between_the_subjects_and_the_premises() {
    let (integer, id, subject, zero) = fixture();
    let inner = denote_bounded_implication_type(
        &[(id, ScalarType::Integer(integer))],
        &[],
        &[],
        &Proposition::LessOrEqual(zero.clone(), subject.clone()),
        &mut Budget::default(),
    )
    .unwrap();
    let goal = Proposition::LessOrEqual(zero, subject);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
    };
    let domains = |arena: &proof_admission::TermArena, mut body| {
        let mut domains = Vec::new();
        while let Term::Pi { domain, codomain } = arena.get(body) {
            domains.push(domain);
            body = codomain;
        }
        domains
    };
    let expected = denote_bounded_implication_type(
        &[(id, ScalarType::Integer(integer))],
        std::slice::from_ref(&inner),
        &[],
        &goal,
        &mut Budget::default(),
    )
    .unwrap();
    let expected_domains = domains(&expected.arena, expected.ty);
    assert_eq!(
        expected_domains.len(),
        4,
        "the subject, its two bounds and the hypothesis"
    );
    assert!(matches!(
        expected.arena.get(expected_domains[3]),
        Term::Pi { .. }
    ));
    let mut theorem = verify_bounded_theorem(
        &[(id, ScalarType::Integer(integer))],
        std::slice::from_ref(&inner),
        &goal,
        &[],
        &[],
        &proof,
        &mut Budget::default(),
    )
    .expect("the proof need not use its hypothesis");
    let certificate = &theorem.denotation.certificate;
    let proved = domains(&theorem.denotation.arena, certificate.expected);
    assert_eq!(proved.len(), 4, "the proof binds the same hypothesis");
    assert!(matches!(
        theorem.denotation.arena.get(proved[3]),
        Term::Pi { .. }
    ));
    let context = Context::empty()
        .with_signature(Signature::from_declarations(certificate.signature.clone()));
    infer_sort(
        &mut theorem.denotation.arena,
        &context,
        certificate.expected,
        &mut Budget::default(),
    )
    .expect("the hypothesis's declarations arrive with it");
}

/// `(λ (h : 0 < n). inr h) assumption`: the introduction sits inside an implication proof
/// that is applied on the spot, so the kernel infers the disjunction's pair rather than
/// checking it against the goal.
fn disjunction_under_an_applied_implication(index: usize) -> (Proposition, Proposition, ProofNode) {
    let (_, _, subject, zero) = fixture();
    let premise = Proposition::LessThan(zero.clone(), subject.clone());
    let goal =
        Proposition::Disjunction(vec![Proposition::LessThan(subject, zero), premise.clone()]);
    let node = |conclusion, rule| ProofNode { conclusion, rule };
    let introduced = node(
        goal.clone(),
        ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(node(premise.clone(), ProofRule::Assumption { index: 1 })),
            index,
        },
    );
    let implication = node(
        Proposition::Implication {
            premise: Box::new(premise.clone()),
            conclusion: Box::new(goal.clone()),
        },
        ProofRule::ImplicationIntroduction {
            body: Box::new(introduced),
        },
    );
    let proof = node(
        goal.clone(),
        ProofRule::ImplicationElimination {
            implication: Box::new(implication),
            premise: Box::new(node(premise.clone(), ProofRule::Assumption { index: 0 })),
        },
    );
    (premise, goal, proof)
}

#[test]
fn a_disjunction_introduced_inside_an_applied_implication_keeps_its_type() {
    let (integer, id, _, _) = fixture();
    let (premise, goal, proof) = disjunction_under_an_applied_implication(1);
    verify_bounded_theorem(
        &[(id, ScalarType::Integer(integer))],
        &[],
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &proof,
        &mut Budget::default(),
    )
    .expect("the pair is checked against the disjunction it introduces");
}

#[test]
fn a_disjunction_introduced_inside_an_applied_implication_still_names_its_disjunct() {
    let (integer, id, _, _) = fixture();
    let (premise, goal, proof) = disjunction_under_an_applied_implication(0);
    assert!(
        verify_bounded_theorem(
            &[(id, ScalarType::Integer(integer))],
            &[],
            &goal,
            std::slice::from_ref(&premise),
            &[],
            &proof,
            &mut Budget::default(),
        )
        .is_err()
    );
}

fn address_fixture() -> (ScalarType, ValueId, ScalarTerm, ScalarTerm) {
    let address = IntegerType::address(64).unwrap();
    let id = ValueId::new(1).unwrap();
    (
        ScalarType::Integer(address),
        id,
        ScalarTerm::value(id, ScalarType::Integer(address)),
        ScalarTerm::integer(address, IntegerValue::Unsigned(0)).unwrap(),
    )
}

#[test]
fn an_address_subject_binds_no_scalar_carrier_bound() {
    let (scalar, id, subject, zero) = address_fixture();
    let expected = denote_bounded_implication_type(
        &[(id, scalar)],
        &[],
        &[],
        &Proposition::LessOrEqual(zero, subject),
        &mut Budget::default(),
    )
    .expect("an address subject has a logical type");
    let mut body = expected.ty;
    let mut binders = 0;
    while let Term::Pi { codomain, .. } = expected.arena.get(body) {
        binders += 1;
        body = codomain;
    }
    assert_eq!(
        binders, 1,
        "the scalar carrier-bound judgment refuses address bounds, so none is assumed"
    );
}

#[test]
fn an_address_theorem_is_proved_from_its_premise() {
    let (scalar, id, subject, zero) = address_fixture();
    let address = scalar.as_integer().unwrap();
    let one = ScalarTerm::integer(address, IntegerValue::Unsigned(1)).unwrap();
    let premise = Proposition::Equal(subject.clone(), one.clone());
    let goal = Proposition::LessThan(zero.clone(), subject.clone());
    let node = |conclusion, rule| ProofNode { conclusion, rule };
    let proof = node(
        goal.clone(),
        ProofRule::IntegerStrictOrderTransitivity {
            left_to_middle: Box::new(node(
                Proposition::LessThan(zero, one.clone()),
                ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            )),
            middle_to_right: Box::new(node(
                Proposition::LessOrEqual(one.clone(), subject.clone()),
                ProofRule::IntegerOrderWeakening {
                    relation: Box::new(node(
                        Proposition::Equal(one, subject),
                        ProofRule::EqualitySymmetry {
                            equality: Box::new(node(
                                premise.clone(),
                                ProofRule::Assumption { index: 0 },
                            )),
                        },
                    )),
                },
            )),
        },
    );
    verify_bounded_theorem(
        &[(id, scalar)],
        &[],
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &proof,
        &mut Budget::default(),
    )
    .expect("the premise alone establishes the conclusion");
}

#[test]
fn an_address_theorem_cannot_assume_a_scalar_carrier_bound() {
    let (scalar, id, subject, zero) = address_fixture();
    let goal = Proposition::LessOrEqual(zero, subject);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
    };
    assert!(
        verify_bounded_theorem(
            &[(id, scalar)],
            &[],
            &goal,
            &[],
            &[],
            &proof,
            &mut Budget::default(),
        )
        .is_err()
    );
}
