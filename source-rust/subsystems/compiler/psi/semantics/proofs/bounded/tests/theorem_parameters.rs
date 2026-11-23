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
        &[input, input], &goal, &[], &[], &proof, &mut Budget::default()
    ), Err(BoundedTheoremError::DuplicateSubject(found)) if found == id));
    assert!(verify_bounded_theorem(&[], &goal, &[], &[], &proof, &mut Budget::default()).is_err());
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
        &[premise],
        &conclusion,
        &mut Budget::default(),
    )
    .err()
    .expect("the producer must preserve canonical proposition arity");
    assert!(format!("{refused:?}").contains("NonCanonicalConjunctionArity(1)"));
}
