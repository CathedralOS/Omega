use mathematical_kernel::{
    Budget, Context, Declaration, MathematicalCertificate, Signature, StrictCertificateError, Term,
    TermArena, abstract_certificate_assumptions, check_type, infer_sort,
    strict_certificate_context,
};

#[test]
fn an_implication_keeps_both_premises_inside_its_strict_conclusion() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let subject = arena.insert(Term::Constant {
        declaration: 0,
        levels: Vec::new(),
    });
    let zero = arena.insert(Term::TwoZero);
    let one = arena.insert(Term::TwoOne);
    let first = arena.insert(Term::Id {
        ty: two,
        left: subject,
        right: zero,
    });
    let second = arena.insert(Term::Id {
        ty: two,
        left: subject,
        right: one,
    });
    let term = arena.insert(Term::Variable(0));
    let input = MathematicalCertificate::with_signature(
        vec![Declaration::assumption(0, two)],
        vec![first, second],
        term,
        second,
    );
    let strict = strict_certificate_context(&mut arena, &input, &mut Budget::default())
        .expect("each relevant premise is used only inside Strict elimination");
    assert_eq!(strict.signature, input.signature);
    assert_eq!(strict.context.len(), 2);
    let context = strict.context.iter().fold(
        Context::empty().with_signature(Signature::from_declarations(strict.signature.clone())),
        |context, premise| context.extend(*premise),
    );
    assert!(
        infer_sort(
            &mut arena,
            &context,
            strict.expected,
            &mut Budget::default()
        )
        .unwrap()
        .is_strict()
    );
    assert!(
        check_type(
            &mut arena,
            &context,
            strict.term,
            second,
            &mut Budget::default()
        )
        .is_err(),
        "Strict proof evidence cannot produce a relevant identity witness"
    );
    let closed =
        abstract_certificate_assumptions(&mut arena, &strict, &[0], &mut Budget::default())
            .unwrap();
    assert!(closed.certificate.signature.is_empty());
    assert!(closed.certificate.context.is_empty());
    assert!(
        infer_sort(
            &mut arena,
            &Context::empty(),
            closed.certificate.expected,
            &mut Budget::default(),
        )
        .unwrap()
        .is_strict()
    );
}

#[test]
fn a_conclusion_that_observes_the_witness_is_not_reinterpreted_as_a_fact() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let witness = arena.insert(Term::Variable(0));
    let expected = arena.insert(Term::Id {
        ty: two,
        left: witness,
        right: witness,
    });
    let term = arena.insert(Term::Refl {
        ty: two,
        value: witness,
    });
    let input = MathematicalCertificate::closed(vec![two], term, expected);
    assert!(matches!(
        strict_certificate_context(&mut arena, &input, &mut Budget::default()),
        Err(StrictCertificateError::DependentConclusion)
    ));
}

#[test]
fn a_dependent_premise_requires_its_own_full_interpretation() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let witness = arena.insert(Term::Variable(0));
    let relation = arena.insert(Term::Id {
        ty: two,
        left: witness,
        right: witness,
    });
    let zero = arena.insert(Term::TwoZero);
    let expected = arena.insert(Term::Id {
        ty: two,
        left: zero,
        right: zero,
    });
    let term = arena.insert(Term::Refl {
        ty: two,
        value: zero,
    });
    let input = MathematicalCertificate::closed(vec![two, relation], term, expected);
    assert!(matches!(
        strict_certificate_context(&mut arena, &input, &mut Budget::default()),
        Err(StrictCertificateError::DependentPremise(1))
    ));
}
