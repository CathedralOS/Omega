use mathematical_kernel::{
    Budget, CertificateAbstractionError, Context, Declaration, Level, MathematicalCertificate,
    MathematicalJudgmentType, Sort, Term, TermArena, TermHandle, abstract_certificate_assumptions,
    abstract_judgment_type, certificate_assumption_closure, check_type,
};

fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

fn equality(
    arena: &mut TermArena,
    ty: TermHandle,
    left: TermHandle,
    right: TermHandle,
) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

#[test]
fn forming_a_required_type_does_not_supply_its_unproved_conclusion() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let subject = constant(&mut arena, 0);
    let one = arena.insert(Term::TwoOne);
    let conclusion = equality(&mut arena, two, subject, one);
    let expected = abstract_judgment_type(
        &mut arena,
        MathematicalJudgmentType {
            signature: &[Declaration::assumption(0, two)],
            level_arity: 0,
            context: &[],
            ty: conclusion,
        },
        &[0],
        &mut Budget::default(),
    )
    .expect("the whole implication has a type without an inhabitant");
    assert!(expected.signature.is_empty());
    let bound = arena.insert(Term::Variable(0));
    let reflexivity = arena.insert(Term::Refl {
        ty: two,
        value: bound,
    });
    let wrong = arena.insert(Term::Lambda {
        domain: two,
        body: reflexivity,
    });
    assert!(
        check_type(
            &mut arena,
            &Context::empty(),
            wrong,
            expected.ty,
            &mut Budget::default()
        )
        .is_err(),
        "a subject need not equal One"
    );
}

#[test]
fn arbitrary_subjects_and_their_premise_become_explicit_binders() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let left = constant(&mut arena, 0);
    let right = constant(&mut arena, 1);
    let relation = equality(&mut arena, two, left, right);
    let proof = arena.insert(Term::Variable(0));
    let input = MathematicalCertificate::with_signature(
        vec![
            Declaration::assumption(0, two),
            Declaration::assumption(0, two),
        ],
        vec![relation],
        proof,
        relation,
    );
    let output =
        abstract_certificate_assumptions(&mut arena, &input, &[0, 1], &mut Budget::default())
            .unwrap();
    assert!(output.certificate.signature.is_empty());
    assert!(output.certificate.context.is_empty());
    assert_eq!(output.retained_declarations, [None, None]);
    let Term::Pi { domain, codomain } = arena.get(output.certificate.expected) else {
        panic!("first subject")
    };
    assert!(matches!(arena.get(domain), Term::Two));
    let Term::Pi { codomain, .. } = arena.get(codomain) else {
        panic!("second subject")
    };
    let Term::Pi { domain, codomain } = arena.get(codomain) else {
        panic!("the exact premise")
    };
    assert!(matches!(arena.get(domain), Term::Id { .. }));
    assert!(arena.structurally_equal_shifted(domain, codomain, 1));
}

#[test]
fn independent_theory_keeps_its_assumption_identity() {
    let mut arena = TermArena::new();
    let universe = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
    let carrier = constant(&mut arena, 0);
    let subject = constant(&mut arena, 1);
    let relation = equality(&mut arena, carrier, subject, subject);
    let proof = arena.insert(Term::Refl {
        ty: carrier,
        value: subject,
    });
    let input = MathematicalCertificate::with_signature(
        vec![
            Declaration::assumption(0, universe),
            Declaration::assumption(0, carrier),
        ],
        Vec::new(),
        proof,
        relation,
    );
    let output =
        abstract_certificate_assumptions(&mut arena, &input, &[1], &mut Budget::default()).unwrap();
    assert_eq!(output.retained_declarations, [Some(0), None]);
    assert_eq!(
        certificate_assumption_closure(&arena, &output.certificate)
            .into_iter()
            .collect::<Vec<_>>(),
        [0]
    );
}

#[test]
fn a_dependent_definition_follows_its_subject_without_a_new_grant() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let subject = constant(&mut arena, 0);
    let copied = constant(&mut arena, 1);
    let relation = equality(&mut arena, two, copied, copied);
    let proof = arena.insert(Term::Refl {
        ty: two,
        value: copied,
    });
    let input = MathematicalCertificate::with_signature(
        vec![
            Declaration::assumption(0, two),
            Declaration::definition(0, two, subject),
        ],
        Vec::new(),
        proof,
        relation,
    );
    let output =
        abstract_certificate_assumptions(&mut arena, &input, &[0], &mut Budget::default()).unwrap();
    assert!(output.certificate.signature.is_empty());
    assert!(certificate_assumption_closure(&arena, &output.certificate).is_empty());
}

#[test]
fn a_dependent_assumption_cannot_stay_global_after_its_subject_is_bound() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let subject = constant(&mut arena, 0);
    let relation = equality(&mut arena, two, subject, subject);
    let claim = constant(&mut arena, 1);
    let input = MathematicalCertificate::with_signature(
        vec![
            Declaration::assumption(0, two),
            Declaration::assumption(0, relation),
        ],
        Vec::new(),
        claim,
        relation,
    );
    assert!(matches!(
        abstract_certificate_assumptions(&mut arena, &input, &[0], &mut Budget::default()),
        Err(CertificateAbstractionError::DependentAssumption(1))
    ));
    assert!(matches!(
        abstract_certificate_assumptions(&mut arena, &input, &[1, 0], &mut Budget::default()),
        Err(CertificateAbstractionError::LaterBinder(0))
    ));
    let explicit =
        abstract_certificate_assumptions(&mut arena, &input, &[0, 1], &mut Budget::default())
            .unwrap();
    assert!(explicit.certificate.signature.is_empty());
}

#[test]
fn a_wrong_proof_is_not_repaired_by_quantifying_its_subject() {
    let mut arena = TermArena::new();
    let two = arena.insert(Term::Two);
    let subject = constant(&mut arena, 0);
    let relation = equality(&mut arena, two, subject, subject);
    let input = MathematicalCertificate::with_signature(
        vec![Declaration::assumption(0, two)],
        Vec::new(),
        subject,
        relation,
    );
    assert!(matches!(
        abstract_certificate_assumptions(&mut arena, &input, &[0], &mut Budget::default()),
        Err(CertificateAbstractionError::Kernel(_))
    ));
}
