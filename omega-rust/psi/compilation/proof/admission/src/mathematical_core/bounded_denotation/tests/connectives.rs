use super::*;

/// Elaborate one `PredicateDenotation` node over the cited premise —
/// `Truth` premises are discharged by the primitive judgment, every
/// other premise is a cited assumption — and return the elaboration
/// so a test can inspect the interned rule axioms.
fn predicate_denotation_elaboration(
    premise: &Proposition,
    goal: &Proposition,
) -> Result<BoundedDenotation, BoundedDenotationError> {
    let (premise_proof, roster) = if matches!(premise, Proposition::Truth) {
        (
            ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            },
            Vec::new(),
        )
    } else {
        (assumption_proof(premise.clone(), 0), vec![premise.clone()])
    };
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(premise_proof),
        },
    };
    let context = PropositionContext::default();
    let machine_parameters = BTreeSet::new();
    let mut elaboration = Elaboration::new(&context, goal, &roster, &[], &machine_parameters)
        .expect("the normalized relation licenses the conversion");
    let term = elaboration.node(&proof)?;
    assert_eq!(
        elaboration.denotation.rule_axioms.len(),
        0,
        "{premise:?} -> {goal:?} derives without a rule-instance axiom",
    );
    let mut denoted = elaboration.finish(term);
    verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, &mut budget())
        .expect("the derived judgment re-decides in the kernel");
    Ok(denoted)
}

/// The closed `Int`-relation → `Id Two zero one` crossing: a decided
/// false order or identity refutes through the shared numeral-order
/// derivation and strict irreflexivity, then `EmptyElim` — the
/// `Id Two`/`Int` carrier crossing's Falsehood side, with no
/// rule-instance axiom.
#[test]
fn closed_false_integer_relations_refute_to_empty_elimination() {
    for premise in [
        Proposition::LessThan(literal(3), literal(2)),
        Proposition::LessThan(literal(2), literal(2)),
        Proposition::LessOrEqual(literal(3), literal(2)),
        Proposition::Equal(literal(2), literal(3)),
        Proposition::Equal(literal(3), literal(2)),
    ] {
        let denoted = predicate_denotation_elaboration(&premise, &Proposition::Falsehood)
            .expect("the closed-false relation denotes a refutation");
        assert!(
            matches!(
                denoted.arena.get(denoted.certificate.term),
                Term::EmptyElim { .. }
            ),
            "{premise:?} eliminates its checked `Empty` into Falsehood",
        );
    }
}

/// `Id Two zero one` itself is the discriminant case: `J`'s Boolean
/// discriminant lands on the goal type directly — the strict `Empty`
/// is no `Type 0` branch — so a Falsehood premise transports into
/// any licensed goal.
#[test]
fn the_falsehood_premise_transports_through_the_j_discriminant() {
    let goal = Proposition::Equal(literal(2), literal(3));
    let denoted = predicate_denotation_elaboration(&Proposition::Falsehood, &goal)
        .expect("the Falsehood premise denotes a discriminant elimination");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::IdElim { .. }
        ),
        "the evidence is the `J` discriminant landing on the goal",
    );
}

/// The `Id Two zero zero` → closed-true side: a Truth-normalized
/// goal is inhabited outright — `refl` for the reflexive identity,
/// the `eq_le`/`lt_le` roster laws or the numeral-order derivation
/// for a decided `IntLe`/`IntLt`, and the pair/tag constructions for
/// the connective denotations.
#[test]
fn closed_true_goals_are_inhabited_outright() {
    let denoted = predicate_denotation_elaboration(
        &Proposition::Equal(literal(2), literal(2)),
        &Proposition::Truth,
    )
    .expect("a closed-true identity crosses to Truth");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Refl { .. }
        ),
        "the `Id Two zero zero` goal is `refl`",
    );

    for goal in [
        Proposition::LessThan(literal(1), literal(2)),
        Proposition::LessOrEqual(literal(2), literal(2)),
        Proposition::Equal(literal(2), literal(2)),
    ] {
        predicate_denotation_elaboration(&Proposition::Truth, &goal)
            .expect("a Truth premise inhabits the closed-true relation");
    }
}

/// The crossing reaches inside the connective denotations: a
/// conjunction with a refutable child projects the component into
/// `Empty`, and a Truth-normalized conjunction or disjunction goal
/// is built from its component inhabitants.
#[test]
fn the_crossing_composes_through_connective_denotations() {
    let premise = Proposition::Conjunction(vec![
        Proposition::LessThan(literal(3), literal(2)),
        Proposition::Equal(literal(2), literal(2)),
    ]);
    let denoted = predicate_denotation_elaboration(&premise, &Proposition::Falsehood)
        .expect("a conjunction with a refutable child denotes a projection");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::EmptyElim { .. }
        ),
        "the refuted component eliminates `Empty` into Falsehood",
    );

    let goal = Proposition::Conjunction(vec![
        Proposition::Equal(literal(2), literal(2)),
        Proposition::LessOrEqual(literal(2), literal(2)),
    ]);
    let denoted = predicate_denotation_elaboration(&Proposition::Truth, &goal)
        .expect("a Truth-normalized conjunction denotes a pair");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Pair { .. }
        ),
        "the conjunction's components pair",
    );

    let goal = Proposition::Disjunction(vec![
        Proposition::Equal(literal(2), literal(3)),
        Proposition::Truth,
    ]);
    let denoted = predicate_denotation_elaboration(&Proposition::Truth, &goal)
        .expect("a Truth-normalized disjunction denotes a tagged pair");
    let Term::Pair { first, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the disjunction's inhabited branch pairs with a tag");
    };
    assert_eq!(denoted.arena.get(first), Term::TwoOne);
}

/// The open Boolean-equality expansion — `Equal(¬a, b)` denoting
/// `Id Two (not a) b` into its normalized valuation disjunction —
/// derives by `caseTwo` case analysis on the two atoms: each literal
/// valuation either inhabits the expanded conjunction or refutes the
/// premise through the `J` discriminant, so no rule-instance axiom
/// enters the signature.
#[test]
fn an_open_boolean_expansion_denotes_by_case_analysis() {
    let (a_id, a) = boolean_value(1);
    let (b_id, b) = boolean_value(2);
    let context = PropositionContext::from_value_types([
        (a_id, ScalarType::Boolean),
        (b_id, ScalarType::Boolean),
    ])
    .expect("context");
    let premise = Proposition::Equal(ScalarTerm::boolean_not(a.clone()).expect("not"), b.clone());
    // The normalized `Equal(¬a, b)` expansion: the two
    // same-valuation conjunctions, disjoined.
    let goal = Proposition::Disjunction(vec![
        Proposition::Conjunction(vec![
            Proposition::Equal(a.clone(), ScalarTerm::Boolean(false)),
            Proposition::Equal(b.clone(), ScalarTerm::Boolean(true)),
        ]),
        Proposition::Conjunction(vec![
            Proposition::Equal(a.clone(), ScalarTerm::Boolean(true)),
            Proposition::Equal(b, ScalarTerm::Boolean(false)),
        ]),
    ]);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(assumption_proof(premise.clone(), 0)),
        },
    };
    let machine_parameters = BTreeSet::new();
    let mut elaboration = Elaboration::new(
        &context,
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &machine_parameters,
    )
    .expect("the Boolean expansion licenses the conversion");
    let term = elaboration
        .node(&proof)
        .expect("the open Boolean expansion denotes a case analysis");
    assert!(
        elaboration.denotation.rule_axioms.is_empty(),
        "the crossing derives without naming a rule-instance axiom",
    );
    let mut denoted = elaboration.finish(term);
    // The transport applies a `caseTwo` case-analysis function to the
    // cited premise; the atoms `a`/`b` are the whole signature.
    let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the expansion denotes an applied case-analysis function");
    };
    assert!(
        matches!(denoted.arena.get(function), Term::CaseTwo { .. }),
        "the transport function eliminates a Boolean atom",
    );
    assert_eq!(denoted.certificate.signature.len(), 2);
    verify_mathematical_certificate(&mut denoted.arena, &denoted.certificate, &mut budget())
        .expect("the derived judgment re-decides in the kernel");
}

/// `connective()` sorting derives a permuted conjunction with real
/// projections: `(b ∧ a) → (a ∧ b)` is `⟨snd e, fst e⟩`, not a
/// rule-instance axiom.
#[test]
fn a_permuted_conjunction_derives_through_its_projections() {
    let premise = Proposition::Conjunction(vec![atom(2), atom(1)]);
    let goal = Proposition::Conjunction(vec![atom(1), atom(2)]);
    let denoted = predicate_denotation_elaboration(&premise, &goal)
        .expect("the permuted conjunction derives");
    let Term::Pair { first, second } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the permuted conjunction re-pairs its projections");
    };
    assert!(matches!(denoted.arena.get(first), Term::Snd { .. }));
    assert!(matches!(denoted.arena.get(second), Term::Fst { .. }));
}

/// `connective()` deduplication and its single-child collapse: the
/// duplicated conjunction projects out the kept component, and the
/// reverse direction re-pairs the one premise.
#[test]
fn a_deduplicated_conjunction_projects_and_re_pairs() {
    let premise = Proposition::Conjunction(vec![atom(1), atom(1)]);
    predicate_denotation_elaboration(&premise, &atom(1))
        .expect("the duplicated conjunction projects");

    let goal = Proposition::Conjunction(vec![atom(1), atom(1)]);
    predicate_denotation_elaboration(&atom(1), &goal)
        .expect("the duplicated conclusion re-pairs the premise");
}

/// `connective()` flattening re-associates the nested `Σ`
/// denotation: `((a ∧ b) ∧ c) → (a ∧ (b ∧ c))` is a real projection
/// path, not an axiom.
#[test]
fn a_flattened_conjunction_re_associates_through_projections() {
    let premise = Proposition::Conjunction(vec![
        Proposition::Conjunction(vec![atom(1), atom(2)]),
        atom(3),
    ]);
    let goal = Proposition::Conjunction(vec![atom(1), atom(2), atom(3)]);
    let denoted = predicate_denotation_elaboration(&premise, &goal)
        .expect("the flattened conjunction derives");
    let Term::Pair { second, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the flattened conjunction re-pairs its components");
    };
    assert!(
        matches!(denoted.arena.get(second), Term::Pair { .. }),
        "the re-associated tail pairs the nested projections",
    );
}

/// A `Truth` conjunct drops: `(a ∧ ⊤) → a` is `fst e`, and the
/// reverse pairs the premise with `refl` at `Id Two zero zero`.
#[test]
fn a_truth_conjunct_drops_through_the_projection() {
    let premise = Proposition::Conjunction(vec![atom(1), Proposition::Truth]);
    let denoted =
        predicate_denotation_elaboration(&premise, &atom(1)).expect("the Truth conjunct drops");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Fst { .. }
        ),
        "the evidence is the surviving component's projection",
    );

    let goal = Proposition::Conjunction(vec![atom(1), Proposition::Truth]);
    let denoted = predicate_denotation_elaboration(&atom(1), &goal)
        .expect("the Truth conjunct pairs with its closed inhabitant");
    let Term::Pair { second, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the Truth-normalized goal pairs its components");
    };
    assert!(
        matches!(denoted.arena.get(second), Term::Refl { .. }),
        "the dropped Truth is refl at Id Two zero zero",
    );
}

/// `connective()` sorting across the tagged-sum denotation: the
/// permuted disjunction eliminates `e` and re-tags each branch —
/// `caseTwo(M, λh.⟨one, h⟩, λh.⟨zero, h⟩, fst e) (snd e)` — rather
/// than a rule-instance axiom.
#[test]
fn a_permuted_disjunction_derives_through_retagging() {
    let premise = Proposition::Disjunction(vec![atom(2), atom(1)]);
    let goal = Proposition::Disjunction(vec![atom(1), atom(2)]);
    let denoted = predicate_denotation_elaboration(&premise, &goal)
        .expect("the permuted disjunction derives");
    let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the disjunction eliminates the premise pair");
    };
    let Term::CaseTwo {
        zero_branch,
        one_branch,
        ..
    } = denoted.arena.get(function)
    else {
        panic!("the eliminator case-splits the tag");
    };
    let Term::Lambda { body, .. } = denoted.arena.get(zero_branch) else {
        panic!("the zero branch binds its payload");
    };
    let Term::Pair { first, .. } = denoted.arena.get(body) else {
        panic!("the zero branch re-tags into the goal");
    };
    assert_eq!(
        denoted.arena.get(first),
        Term::TwoOne,
        "the premise's first disjunct is the goal's rest",
    );
    let Term::Lambda { body, .. } = denoted.arena.get(one_branch) else {
        panic!("the one branch binds its payload");
    };
    let Term::Pair { first, .. } = denoted.arena.get(body) else {
        panic!("the one branch re-tags into the goal");
    };
    assert_eq!(
        denoted.arena.get(first),
        Term::TwoZero,
        "the premise's rest disjunct is the goal's first",
    );
}

/// A `Falsehood` disjunct drops: `(a ∨ ⊥) → a` case-splits the tag,
/// keeps the `a` payload, and refutes the `Id Two zero one` branch
/// through the `J` discriminant — and the reverse tags the premise
/// `zero` without ever inhabiting the dropped branch.
#[test]
fn a_falsehood_disjunct_drops_through_the_eliminator() {
    let premise = Proposition::Disjunction(vec![atom(1), Proposition::Falsehood]);
    let denoted =
        predicate_denotation_elaboration(&premise, &atom(1)).expect("the Falsehood disjunct drops");
    let Term::Apply { function, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the disjunction eliminates the premise pair");
    };
    let Term::CaseTwo { one_branch, .. } = denoted.arena.get(function) else {
        panic!("the eliminator case-splits the tag");
    };
    let Term::Lambda { body, .. } = denoted.arena.get(one_branch) else {
        panic!("the Falsehood branch binds its payload");
    };
    assert!(
        matches!(
            denoted.arena.get(body),
            Term::IdElim { .. } | Term::EmptyElim { .. }
        ),
        "the Falsehood payload refutes into the goal",
    );

    let goal = Proposition::Disjunction(vec![atom(1), Proposition::Falsehood]);
    let denoted = predicate_denotation_elaboration(&atom(1), &goal)
        .expect("the surviving disjunct tags zero");
    let Term::Pair { first, .. } = denoted.arena.get(denoted.certificate.term) else {
        panic!("the disjunction introduction pairs a tag");
    };
    assert_eq!(denoted.arena.get(first), Term::TwoZero);
}

/// Normalization under an implication premise derives
/// contravariantly: `((b ∧ a) → c) → ((a ∧ b) → c)` coerces the
/// domain through the same projection crossing inside the `λ`'s
/// applied body — the nested connective never becomes an axiom.
#[test]
fn an_implication_domain_derives_contravariantly() {
    let premise = Proposition::Implication {
        premise: Box::new(Proposition::Conjunction(vec![atom(2), atom(1)])),
        conclusion: Box::new(atom(3)),
    };
    let goal = Proposition::Implication {
        premise: Box::new(Proposition::Conjunction(vec![atom(1), atom(2)])),
        conclusion: Box::new(atom(3)),
    };
    let denoted = predicate_denotation_elaboration(&premise, &goal)
        .expect("the implication's normalized domain derives");
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::Lambda { .. }
        ),
        "the crossing stays inside the function body",
    );
}
