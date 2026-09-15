use super::{integer, value};
use crate::proofs::nonzero_divisor_certificate::{
    Proposition, PropositionContext, produce_checked_canonical_integer_proof,
    prove_canonical_integer_proposition,
};
use proof_admission::{ProofRule, accept_certificate, accept_certificate_with_machine_parameters};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
};
use std::collections::BTreeSet;

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
    let ProofRule::IntegerOrderSubstitution {
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
        ProofRule::IntegerOrderSubstitution { endpoint: 0, .. }
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
fn exact_division_goal_transports_affine_bound_through_cited_target_aliases() {
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
    let ProofRule::IntegerOrderSubstitution {
        relation,
        equality,
        endpoint,
    } = disjunct.rule
    else {
        panic!("the canonical target uses the outer substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
    let ProofRule::IntegerOrderSubstitution {
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
        ProofRule::IntegerOrderSubstitution { endpoint: 0, .. }
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
    // The general equality join composes beyond the specialized two-alias
    // producer. Every original equality remains an independently checked
    // dependency; the old search depth was not a language restriction.
    let assumptions = [
        positive_root_bound,
        outer_alias,
        inner_alias,
        Proposition::Equal(value(5, signed), value(6, signed)),
    ];
    let axioms = [Proposition::Equal(
        value(6, signed),
        ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
            .expect("exact add"),
    )];
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
        .expect("explicit equation transport composes all three target aliases");
    let acceptance = accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
        .expect("the original division question and premises independently check");
    assert!(
        acceptance
            .rules
            .contains(&proof_admission::AcceptedProofRule::ValueEqualityTransport)
    );
    assert_eq!(acceptance.assumptions.len(), assumptions.len());
    assert_eq!(acceptance.semantic_axioms.len(), axioms.len());
    for removed in 0..assumptions.len() {
        let mut missing = assumptions.clone();
        missing[removed] = Proposition::Truth;
        assert!(accept_certificate(&context, &goal, &missing, &axioms, &proof).is_err());
        assert!(prove_canonical_integer_proposition(&context, &goal, &missing, &axioms).is_none());
    }
    assert!(accept_certificate(&context, &goal, &assumptions, &[], &proof).is_err());
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
    let ProofRule::IntegerOrderSubstitution {
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
    let ProofRule::IntegerOrderSubstitution {
        relation,
        equality,
        endpoint,
    } = root_bound.rule
    else {
        panic!("the outer root alias uses endpoint substitution")
    };
    assert_eq!(endpoint, 1);
    assert!(matches!(equality.rule, ProofRule::Assumption { index: 0 }));
    let ProofRule::IntegerOrderSubstitution {
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
        ProofRule::IntegerOrderSubstitution { endpoint: 0, .. }
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
    let ProofRule::IntegerOrderSubstitution {
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
        ProofRule::IntegerOrderSubstitution { endpoint: 0, .. }
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
fn exact_division_goal_proves_three_through_fourteen_definition_affine_safe_divisors() {
    let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let context = PropositionContext::from_value_types((1..=17).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(signed),
        )
    }))
    .expect("seventeen i8 values");
    let exact_division_goal = |divisor: ScalarTerm| {
        Proposition::Disjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), integer(signed, -2)),
            Proposition::LessOrEqual(integer(signed, 1), divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(divisor, integer(signed, -1)),
                Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
            ]),
        ])
    };
    let three_step_goal = Proposition::LessOrEqual(integer(signed, 1), value(6, signed));
    let four_step_goal = exact_division_goal(value(7, signed));
    let five_step_goal = exact_division_goal(value(8, signed));
    let six_step_goal = exact_division_goal(value(2, signed));
    let seven_step_goal = exact_division_goal(value(9, signed));
    let eight_step_goal = exact_division_goal(value(10, signed));
    let nine_step_goal = exact_division_goal(value(11, signed));
    let ten_step_goal = exact_division_goal(value(12, signed));
    let eleven_step_goal = exact_division_goal(value(13, signed));
    let twelve_step_goal = exact_division_goal(value(14, signed));
    let thirteen_step_goal = exact_division_goal(value(15, signed));
    let fourteen_step_goal = exact_division_goal(value(16, signed));
    let fifteen_step_goal = exact_division_goal(value(17, signed));
    let three_step_root_bound = Proposition::LessOrEqual(integer(signed, -2), value(3, signed));
    let four_step_root_bound = Proposition::LessOrEqual(integer(signed, -3), value(3, signed));
    let five_step_root_bound = Proposition::LessOrEqual(integer(signed, -4), value(3, signed));
    let six_step_root_bound = Proposition::LessOrEqual(integer(signed, -5), value(3, signed));
    let seven_step_root_bound = Proposition::LessOrEqual(integer(signed, -6), value(3, signed));
    let eight_step_root_bound = Proposition::LessOrEqual(integer(signed, -7), value(3, signed));
    let nine_step_root_bound = Proposition::LessOrEqual(integer(signed, -8), value(3, signed));
    let ten_step_root_bound = Proposition::LessOrEqual(integer(signed, -9), value(3, signed));
    let eleven_step_root_bound = Proposition::LessOrEqual(integer(signed, -10), value(3, signed));
    let twelve_step_root_bound = Proposition::LessOrEqual(integer(signed, -11), value(3, signed));
    let thirteen_step_root_bound = Proposition::LessOrEqual(integer(signed, -12), value(3, signed));
    let fourteen_step_root_bound = Proposition::LessOrEqual(integer(signed, -13), value(3, signed));
    let fifteen_step_root_bound = Proposition::LessOrEqual(integer(signed, -14), value(3, signed));
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
            value(7, signed),
            ScalarTerm::exact_integer_add(signed, value(6, signed), integer(signed, 1))
                .expect("fourth exact add"),
        ),
        Proposition::Equal(
            value(8, signed),
            ScalarTerm::exact_integer_add(signed, value(7, signed), integer(signed, 1))
                .expect("fifth exact add"),
        ),
        Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(signed, value(8, signed), integer(signed, 1))
                .expect("sixth exact add"),
        ),
        Proposition::Equal(
            value(9, signed),
            ScalarTerm::exact_integer_add(signed, value(2, signed), integer(signed, 1))
                .expect("seventh exact add"),
        ),
        Proposition::Equal(
            value(10, signed),
            ScalarTerm::exact_integer_add(signed, value(9, signed), integer(signed, 1))
                .expect("eighth exact add"),
        ),
        Proposition::Equal(
            value(11, signed),
            ScalarTerm::exact_integer_add(signed, value(10, signed), integer(signed, 1))
                .expect("ninth exact add"),
        ),
        Proposition::Equal(
            value(12, signed),
            ScalarTerm::exact_integer_add(signed, value(11, signed), integer(signed, 1))
                .expect("tenth exact add"),
        ),
        Proposition::Equal(
            value(13, signed),
            ScalarTerm::exact_integer_add(signed, value(12, signed), integer(signed, 1))
                .expect("eleventh exact add"),
        ),
        Proposition::Equal(
            value(14, signed),
            ScalarTerm::exact_integer_add(signed, value(13, signed), integer(signed, 1))
                .expect("twelfth exact add"),
        ),
        Proposition::Equal(
            value(15, signed),
            ScalarTerm::exact_integer_add(signed, value(14, signed), integer(signed, 1))
                .expect("thirteenth exact add"),
        ),
        Proposition::Equal(
            value(16, signed),
            ScalarTerm::exact_integer_add(signed, value(15, signed), integer(signed, 1))
                .expect("fourteenth exact add"),
        ),
        Proposition::Equal(
            value(17, signed),
            ScalarTerm::exact_integer_add(signed, value(16, signed), integer(signed, 1))
                .expect("fifteenth exact add"),
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

    let four_step_proof = prove_canonical_integer_proposition(
        &context,
        &four_step_goal,
        std::slice::from_ref(&four_step_root_bound),
        &definitions,
    )
    .expect("four-definition affine word remains selectable");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = four_step_proof.rule else {
        panic!("four-definition affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = disjunct.rule else {
        panic!("four-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(7, signed));
    assert_eq!(witness.definition_axioms, vec![0, 1, 2, 3]);

    let five_step_proof = prove_canonical_integer_proposition(
        &context,
        &five_step_goal,
        std::slice::from_ref(&five_step_root_bound),
        &definitions,
    )
    .expect("five-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = five_step_proof.rule else {
        panic!("five-definition affine divisor selects one canonical arm")
    };
    assert_eq!(index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = disjunct.rule else {
        panic!("five-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(8, signed));
    assert_eq!(witness.definition_axioms, vec![0, 1, 2, 3, 4]);

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &five_step_goal,
            std::slice::from_ref(&five_step_root_bound),
            &definitions[..4],
        )
        .is_none(),
        "an incomplete five-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &five_step_goal,
            &[five_step_root_bound],
            &[
                definitions[4].clone(),
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed five-definition word cannot claim canonical custody",
    );
    let six_step_proof = prove_canonical_integer_proposition(
        &context,
        &six_step_goal,
        std::slice::from_ref(&six_step_root_bound),
        &definitions,
    )
    .expect("six-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &six_step_proof.rule else {
        panic!("six-definition affine divisor selects one canonical arm")
    };
    assert_eq!(*index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("six-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(2, signed));
    assert_eq!(witness.definition_axioms, vec![0, 1, 2, 3, 4, 5]);
    accept_certificate(
        &context,
        &six_step_goal,
        std::slice::from_ref(&six_step_root_bound),
        &definitions,
        &six_step_proof,
    )
    .expect("the checker independently replays the six-definition certificate");

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &six_step_goal,
            std::slice::from_ref(&six_step_root_bound),
            &definitions[..5],
        )
        .is_none(),
        "an incomplete six-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &six_step_goal,
            std::slice::from_ref(&six_step_root_bound),
            &[
                definitions[5].clone(),
                definitions[4].clone(),
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed six-definition word cannot claim canonical custody",
    );

    let mut redirected_definitions = definitions[..6].to_vec();
    redirected_definitions[5] = Proposition::Equal(
        value(9, signed),
        ScalarTerm::exact_integer_add(signed, value(8, signed), integer(signed, 1))
            .expect("redirected sixth exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &six_step_goal,
            std::slice::from_ref(&six_step_root_bound),
            &redirected_definitions,
        )
        .is_none(),
        "a redirected sixth definition cannot complete the target word",
    );
    assert!(
        accept_certificate(
            &context,
            &six_step_goal,
            std::slice::from_ref(&six_step_root_bound),
            &redirected_definitions,
            &six_step_proof,
        )
        .is_err(),
        "a certificate word cannot replay against stale definition evidence",
    );

    let seven_step_proof = prove_canonical_integer_proposition(
        &context,
        &seven_step_goal,
        std::slice::from_ref(&seven_step_root_bound),
        &definitions,
    )
    .expect("seven-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &seven_step_proof.rule else {
        panic!("seven-definition affine divisor selects one canonical arm")
    };
    assert_eq!(*index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("seven-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(9, signed));
    assert_eq!(witness.definition_axioms, vec![0, 1, 2, 3, 4, 5, 6]);
    accept_certificate(
        &context,
        &seven_step_goal,
        std::slice::from_ref(&seven_step_root_bound),
        &definitions,
        &seven_step_proof,
    )
    .expect("the checker independently replays the seven-definition certificate");

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &seven_step_goal,
            std::slice::from_ref(&seven_step_root_bound),
            &definitions[..6],
        )
        .is_none(),
        "an incomplete seven-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &seven_step_goal,
            std::slice::from_ref(&seven_step_root_bound),
            &[
                definitions[6].clone(),
                definitions[5].clone(),
                definitions[4].clone(),
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed seven-definition word cannot claim canonical custody",
    );

    let mut redirected_definitions = definitions[..7].to_vec();
    redirected_definitions[6] = Proposition::Equal(
        value(10, signed),
        ScalarTerm::exact_integer_add(signed, value(2, signed), integer(signed, 1))
            .expect("redirected seventh exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &seven_step_goal,
            std::slice::from_ref(&seven_step_root_bound),
            &redirected_definitions,
        )
        .is_none(),
        "a redirected seventh definition cannot complete the target word",
    );
    assert!(
        accept_certificate(
            &context,
            &seven_step_goal,
            std::slice::from_ref(&seven_step_root_bound),
            &redirected_definitions,
            &seven_step_proof,
        )
        .is_err(),
        "a seven-definition certificate cannot replay against stale definition evidence",
    );

    let eight_step_proof = prove_canonical_integer_proposition(
        &context,
        &eight_step_goal,
        std::slice::from_ref(&eight_step_root_bound),
        &definitions,
    )
    .expect("eight-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &eight_step_proof.rule else {
        panic!("eight-definition affine divisor selects one canonical arm")
    };
    assert_eq!(*index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("eight-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(10, signed));
    assert_eq!(witness.definition_axioms, vec![0, 1, 2, 3, 4, 5, 6, 7],);
    accept_certificate(
        &context,
        &eight_step_goal,
        std::slice::from_ref(&eight_step_root_bound),
        &definitions,
        &eight_step_proof,
    )
    .expect("the checker independently replays the eight-definition certificate");

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &eight_step_goal,
            std::slice::from_ref(&eight_step_root_bound),
            &definitions[..7],
        )
        .is_none(),
        "an incomplete eight-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &eight_step_goal,
            std::slice::from_ref(&eight_step_root_bound),
            &[
                definitions[7].clone(),
                definitions[6].clone(),
                definitions[5].clone(),
                definitions[4].clone(),
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed eight-definition word cannot claim canonical custody",
    );

    let mut redirected_definitions = definitions[..8].to_vec();
    redirected_definitions[7] = Proposition::Equal(
        value(11, signed),
        ScalarTerm::exact_integer_add(signed, value(9, signed), integer(signed, 1))
            .expect("redirected eighth exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &eight_step_goal,
            std::slice::from_ref(&eight_step_root_bound),
            &redirected_definitions,
        )
        .is_none(),
        "a redirected eighth definition cannot complete the target word",
    );
    assert!(
        accept_certificate(
            &context,
            &eight_step_goal,
            std::slice::from_ref(&eight_step_root_bound),
            &redirected_definitions,
            &eight_step_proof,
        )
        .is_err(),
        "an eight-definition certificate cannot replay against stale definition evidence",
    );

    let nine_step_proof = prove_canonical_integer_proposition(
        &context,
        &nine_step_goal,
        std::slice::from_ref(&nine_step_root_bound),
        &definitions,
    )
    .expect("nine-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &nine_step_proof.rule else {
        panic!("nine-definition affine divisor selects one canonical arm")
    };
    assert_eq!(*index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("nine-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(11, signed));
    assert_eq!(witness.definition_axioms, vec![0, 1, 2, 3, 4, 5, 6, 7, 8],);
    accept_certificate(
        &context,
        &nine_step_goal,
        std::slice::from_ref(&nine_step_root_bound),
        &definitions,
        &nine_step_proof,
    )
    .expect("the checker independently replays the nine-definition certificate");

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &nine_step_goal,
            std::slice::from_ref(&nine_step_root_bound),
            &definitions[..8],
        )
        .is_none(),
        "an incomplete nine-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &nine_step_goal,
            std::slice::from_ref(&nine_step_root_bound),
            &[
                definitions[8].clone(),
                definitions[7].clone(),
                definitions[6].clone(),
                definitions[5].clone(),
                definitions[4].clone(),
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed nine-definition word cannot claim canonical custody",
    );

    let mut redirected_definitions = definitions[..9].to_vec();
    redirected_definitions[8] = Proposition::Equal(
        value(12, signed),
        ScalarTerm::exact_integer_add(signed, value(10, signed), integer(signed, 1))
            .expect("redirected ninth exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &nine_step_goal,
            std::slice::from_ref(&nine_step_root_bound),
            &redirected_definitions,
        )
        .is_none(),
        "a redirected ninth definition cannot complete the target word",
    );
    assert!(
        accept_certificate(
            &context,
            &nine_step_goal,
            std::slice::from_ref(&nine_step_root_bound),
            &redirected_definitions,
            &nine_step_proof,
        )
        .is_err(),
        "a nine-definition certificate cannot replay against stale definition evidence",
    );

    let ten_step_proof = prove_canonical_integer_proposition(
        &context,
        &ten_step_goal,
        std::slice::from_ref(&ten_step_root_bound),
        &definitions,
    )
    .expect("ten-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &ten_step_proof.rule else {
        panic!("ten-definition affine divisor selects one canonical arm")
    };
    assert_eq!(*index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("ten-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(12, signed));
    assert_eq!(
        witness.definition_axioms,
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    );
    accept_certificate(
        &context,
        &ten_step_goal,
        std::slice::from_ref(&ten_step_root_bound),
        &definitions,
        &ten_step_proof,
    )
    .expect("the checker independently replays the ten-definition certificate");

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &ten_step_goal,
            std::slice::from_ref(&ten_step_root_bound),
            &definitions[..9],
        )
        .is_none(),
        "an incomplete ten-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &ten_step_goal,
            std::slice::from_ref(&ten_step_root_bound),
            &[
                definitions[9].clone(),
                definitions[8].clone(),
                definitions[7].clone(),
                definitions[6].clone(),
                definitions[5].clone(),
                definitions[4].clone(),
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed ten-definition word cannot claim canonical custody",
    );

    let mut redirected_definitions = definitions[..10].to_vec();
    redirected_definitions[9] = Proposition::Equal(
        value(13, signed),
        ScalarTerm::exact_integer_add(signed, value(11, signed), integer(signed, 1))
            .expect("redirected tenth exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &ten_step_goal,
            std::slice::from_ref(&ten_step_root_bound),
            &redirected_definitions,
        )
        .is_none(),
        "a redirected tenth definition cannot complete the target word",
    );
    assert!(
        accept_certificate(
            &context,
            &ten_step_goal,
            std::slice::from_ref(&ten_step_root_bound),
            &redirected_definitions,
            &ten_step_proof,
        )
        .is_err(),
        "a ten-definition certificate cannot replay against stale definition evidence",
    );

    let eleven_step_proof = prove_canonical_integer_proposition(
        &context,
        &eleven_step_goal,
        std::slice::from_ref(&eleven_step_root_bound),
        &definitions,
    )
    .expect("eleven-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &eleven_step_proof.rule else {
        panic!("eleven-definition affine divisor selects one canonical arm")
    };
    assert_eq!(*index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("eleven-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(13, signed));
    assert_eq!(
        witness.definition_axioms,
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    );
    accept_certificate(
        &context,
        &eleven_step_goal,
        std::slice::from_ref(&eleven_step_root_bound),
        &definitions,
        &eleven_step_proof,
    )
    .expect("the checker independently replays the eleven-definition certificate");

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &eleven_step_goal,
            std::slice::from_ref(&eleven_step_root_bound),
            &definitions[..10],
        )
        .is_none(),
        "an incomplete eleven-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &eleven_step_goal,
            std::slice::from_ref(&eleven_step_root_bound),
            &[
                definitions[10].clone(),
                definitions[9].clone(),
                definitions[8].clone(),
                definitions[7].clone(),
                definitions[6].clone(),
                definitions[5].clone(),
                definitions[4].clone(),
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed eleven-definition word cannot claim canonical custody",
    );

    let mut redirected_definitions = definitions[..11].to_vec();
    redirected_definitions[10] = Proposition::Equal(
        value(14, signed),
        ScalarTerm::exact_integer_add(signed, value(12, signed), integer(signed, 1))
            .expect("redirected eleventh exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &eleven_step_goal,
            std::slice::from_ref(&eleven_step_root_bound),
            &redirected_definitions,
        )
        .is_none(),
        "a redirected eleventh definition cannot complete the target word",
    );
    assert!(
        accept_certificate(
            &context,
            &eleven_step_goal,
            std::slice::from_ref(&eleven_step_root_bound),
            &redirected_definitions,
            &eleven_step_proof,
        )
        .is_err(),
        "an eleven-definition certificate cannot replay against stale definition evidence",
    );

    let twelve_step_proof = prove_canonical_integer_proposition(
        &context,
        &twelve_step_goal,
        std::slice::from_ref(&twelve_step_root_bound),
        &definitions,
    )
    .expect("twelve-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &twelve_step_proof.rule else {
        panic!("twelve-definition affine divisor selects one canonical arm")
    };
    assert_eq!(*index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("twelve-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(14, signed));
    assert_eq!(
        witness.definition_axioms,
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    );
    accept_certificate(
        &context,
        &twelve_step_goal,
        std::slice::from_ref(&twelve_step_root_bound),
        &definitions,
        &twelve_step_proof,
    )
    .expect("the checker independently replays the twelve-definition certificate");

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &twelve_step_goal,
            std::slice::from_ref(&twelve_step_root_bound),
            &definitions[..11],
        )
        .is_none(),
        "an incomplete twelve-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &twelve_step_goal,
            std::slice::from_ref(&twelve_step_root_bound),
            &[
                definitions[11].clone(),
                definitions[10].clone(),
                definitions[9].clone(),
                definitions[8].clone(),
                definitions[7].clone(),
                definitions[6].clone(),
                definitions[5].clone(),
                definitions[4].clone(),
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed twelve-definition word cannot claim canonical custody",
    );

    let mut redirected_definitions = definitions[..12].to_vec();
    redirected_definitions[11] = Proposition::Equal(
        value(15, signed),
        ScalarTerm::exact_integer_add(signed, value(13, signed), integer(signed, 1))
            .expect("redirected twelfth exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &twelve_step_goal,
            std::slice::from_ref(&twelve_step_root_bound),
            &redirected_definitions,
        )
        .is_none(),
        "a redirected twelfth definition cannot complete the target word",
    );
    assert!(
        accept_certificate(
            &context,
            &twelve_step_goal,
            std::slice::from_ref(&twelve_step_root_bound),
            &redirected_definitions,
            &twelve_step_proof,
        )
        .is_err(),
        "a twelve-definition certificate cannot replay against stale definition evidence",
    );

    let thirteen_step_proof = prove_canonical_integer_proposition(
        &context,
        &thirteen_step_goal,
        std::slice::from_ref(&thirteen_step_root_bound),
        &definitions,
    )
    .expect("thirteen-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &thirteen_step_proof.rule else {
        panic!("thirteen-definition affine divisor selects one canonical arm")
    };
    assert_eq!(*index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("thirteen-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(15, signed));
    assert_eq!(
        witness.definition_axioms,
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
    );
    accept_certificate(
        &context,
        &thirteen_step_goal,
        std::slice::from_ref(&thirteen_step_root_bound),
        &definitions,
        &thirteen_step_proof,
    )
    .expect("the checker independently replays the thirteen-definition certificate");

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &thirteen_step_goal,
            std::slice::from_ref(&thirteen_step_root_bound),
            &definitions[..12],
        )
        .is_none(),
        "an incomplete thirteen-definition word cannot prove divisor safety",
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &thirteen_step_goal,
            std::slice::from_ref(&thirteen_step_root_bound),
            &[
                definitions[12].clone(),
                definitions[11].clone(),
                definitions[10].clone(),
                definitions[9].clone(),
                definitions[8].clone(),
                definitions[7].clone(),
                definitions[6].clone(),
                definitions[5].clone(),
                definitions[4].clone(),
                definitions[3].clone(),
                definitions[2].clone(),
                definitions[1].clone(),
                definitions[0].clone(),
            ],
        )
        .is_none(),
        "a reversed thirteen-definition word cannot claim canonical custody",
    );

    let mut redirected_definitions = definitions[..13].to_vec();
    redirected_definitions[12] = Proposition::Equal(
        value(16, signed),
        ScalarTerm::exact_integer_add(signed, value(14, signed), integer(signed, 1))
            .expect("redirected thirteenth exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &thirteen_step_goal,
            std::slice::from_ref(&thirteen_step_root_bound),
            &redirected_definitions,
        )
        .is_none(),
        "a redirected thirteenth definition cannot complete the target word",
    );
    assert!(
        accept_certificate(
            &context,
            &thirteen_step_goal,
            std::slice::from_ref(&thirteen_step_root_bound),
            &redirected_definitions,
            &thirteen_step_proof,
        )
        .is_err(),
        "a thirteen-definition certificate cannot replay against stale definition evidence",
    );

    let fourteen_step_proof = prove_canonical_integer_proposition(
        &context,
        &fourteen_step_goal,
        std::slice::from_ref(&fourteen_step_root_bound),
        &definitions,
    )
    .expect("fourteen-definition affine word proves the positive divisor arm");
    let ProofRule::DisjunctionIntroduction { disjunct, index } = &fourteen_step_proof.rule else {
        panic!("fourteen-definition affine divisor selects one canonical arm")
    };
    assert_eq!(*index, 1);
    let ProofRule::IntegerAffineBound { witness, .. } = &disjunct.rule else {
        panic!("fourteen-definition affine divisor uses the affine-bound rule")
    };
    assert_eq!(witness.root, value(3, signed));
    assert_eq!(witness.target, value(16, signed));
    assert_eq!(
        witness.definition_axioms,
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13],
    );
    accept_certificate(
        &context,
        &fourteen_step_goal,
        std::slice::from_ref(&fourteen_step_root_bound),
        &definitions,
        &fourteen_step_proof,
    )
    .expect("the checker independently replays the fourteen-definition certificate");

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &fourteen_step_goal,
            std::slice::from_ref(&fourteen_step_root_bound),
            &definitions[..13],
        )
        .is_none(),
        "an incomplete fourteen-definition word cannot prove divisor safety",
    );
    let reversed_definitions = definitions[..14].iter().rev().cloned().collect::<Vec<_>>();
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &fourteen_step_goal,
            std::slice::from_ref(&fourteen_step_root_bound),
            &reversed_definitions,
        )
        .is_none(),
        "a reversed fourteen-definition word cannot claim canonical custody",
    );

    let mut redirected_definitions = definitions[..14].to_vec();
    redirected_definitions[13] = Proposition::Equal(
        value(17, signed),
        ScalarTerm::exact_integer_add(signed, value(15, signed), integer(signed, 1))
            .expect("redirected fourteenth exact add"),
    );
    assert!(
        prove_canonical_integer_proposition(
            &context,
            &fourteen_step_goal,
            std::slice::from_ref(&fourteen_step_root_bound),
            &redirected_definitions,
        )
        .is_none(),
        "a redirected fourteenth definition cannot complete the target word",
    );
    assert!(
        accept_certificate(
            &context,
            &fourteen_step_goal,
            std::slice::from_ref(&fourteen_step_root_bound),
            &redirected_definitions,
            &fourteen_step_proof,
        )
        .is_err(),
        "a fourteen-definition certificate cannot replay against stale definition evidence",
    );

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &fifteen_step_goal,
            std::slice::from_ref(&fifteen_step_root_bound),
            &definitions,
        )
        .is_none(),
        "a fifteen-definition word remains outside the bounded certificate frontier",
    );
}

#[test]
fn correlated_forbidden_root_producer_is_shared_by_exact_divide_and_remainder() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root = value(1, integer_type);
    let sixty_four = value(2, integer_type);
    let left_offset = value(3, integer_type);
    let negative_two = value(4, integer_type);
    let dividend = value(5, integer_type);
    let two = value(6, integer_type);
    let right_product = value(7, integer_type);
    let divisor = value(8, integer_type);
    let context = PropositionContext::from_value_types((1..=8).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("context");
    let axioms = [
        Proposition::Equal(sixty_four.clone(), integer(integer_type, 64)),
        Proposition::Equal(
            left_offset.clone(),
            ScalarTerm::exact_integer_add(integer_type, root.clone(), sixty_four.clone())
                .expect("dividend add"),
        ),
        Proposition::Equal(negative_two.clone(), integer(integer_type, -2)),
        Proposition::Equal(
            dividend.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, left_offset, negative_two)
                .expect("dividend multiply"),
        ),
        Proposition::Equal(two.clone(), integer(integer_type, 2)),
        Proposition::Equal(
            right_product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, root.clone(), two)
                .expect("divisor multiply"),
        ),
        Proposition::Equal(
            divisor.clone(),
            ScalarTerm::exact_integer_add(integer_type, right_product, integer(integer_type, 1))
                .expect("divisor add"),
        ),
    ];
    let assumptions = [
        Proposition::LessOrEqual(integer(integer_type, -1), root.clone()),
        Proposition::LessOrEqual(root.clone(), integer(integer_type, 0)),
    ];
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -2)),
        Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1)),
            Proposition::LessOrEqual(integer(integer_type, -127), dividend),
        ]),
    ]);
    let parameters = BTreeSet::from([ValueId::new(1).expect("root")]);

    let divide = produce_checked_canonical_integer_proof(
        &context,
        &goal,
        &assumptions,
        &axioms,
        &parameters,
    )
    .expect("same-root affine exact divide is certified");
    let remainder = produce_checked_canonical_integer_proof(
        &context,
        &goal,
        &assumptions,
        &axioms,
        &parameters,
    )
    .expect("same-root affine exact remainder uses the same definedness certificate");
    assert_eq!(divide, remainder);
    let ProofRule::IntegerCorrelatedForbiddenRoots { witness } = &divide.rule else {
        panic!("correlated divide/remainder uses its dedicated conversion")
    };
    assert_eq!(witness.definition_axiom_count, axioms.len());
    assert_eq!(witness.lower_bound_axiom, axioms.len());
    assert_eq!(witness.upper_bound_axiom, axioms.len() + 1);
    assert_eq!(
        witness
            .dividend
            .steps
            .iter()
            .map(|step| step.definition_axiom)
            .collect::<Vec<_>>(),
        vec![1, 3],
    );
    assert_eq!(
        witness
            .divisor
            .steps
            .iter()
            .map(|step| step.definition_axiom)
            .collect::<Vec<_>>(),
        vec![5, 6],
    );
    accept_certificate_with_machine_parameters(
        &context,
        &goal,
        &assumptions,
        &axioms,
        &parameters,
        &divide,
    )
    .expect("the kernel replays the producer-selected witness");

    assert!(
        accept_certificate_with_machine_parameters(
            &context,
            &goal,
            &assumptions,
            &axioms,
            &BTreeSet::new(),
            &divide,
        )
        .is_err(),
        "a value not reconstructed as a machine parameter cannot be a correlated root",
    );
    let mut redirected_axioms = axioms.clone();
    redirected_axioms[1] = Proposition::Equal(
        sixty_four,
        ScalarTerm::exact_integer_add(integer_type, root.clone(), value(2, integer_type))
            .expect("redirected dividend definition"),
    );
    assert!(
        accept_certificate_with_machine_parameters(
            &context,
            &goal,
            &assumptions,
            &redirected_axioms,
            &parameters,
            &divide,
        )
        .is_err(),
        "redirecting a pre-operation branch definition invalidates the witness",
    );
    let drifted_assumptions = [
        Proposition::LessOrEqual(integer(integer_type, -2), root.clone()),
        assumptions[1].clone(),
    ];
    assert!(
        accept_certificate_with_machine_parameters(
            &context,
            &goal,
            &drifted_assumptions,
            &axioms,
            &parameters,
            &divide,
        )
        .is_err(),
        "changing a selected signature endpoint invalidates the witness",
    );
}

#[test]
fn exact_left_shift_replays_a_prior_shift_rooted_at_one_exact_cast() {
    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let input = value(1, u64_type);
    let cast = value(2, u8_type);
    let first_count = value(3, i8_type);
    let first_shift = value(4, u8_type);
    let second_count = value(5, u16_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(u64_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(u8_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i8_type)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(u8_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(u16_type)),
    ])
    .unwrap();
    let assumptions = [Proposition::LessOrEqual(
        input.clone(),
        ScalarTerm::integer(u64_type, IntegerValue::Unsigned(31)).unwrap(),
    )];
    let axioms = [
        Proposition::Equal(
            cast.clone(),
            ScalarTerm::integer_exact_cast(u64_type, u8_type, input).unwrap(),
        ),
        Proposition::Equal(first_count.clone(), integer(i8_type, 1)),
        Proposition::Equal(
            first_shift,
            ScalarTerm::exact_integer_shift_left(u8_type, i8_type, cast, first_count).unwrap(),
        ),
        Proposition::Equal(
            second_count,
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(2)).unwrap(),
        ),
    ];
    prove_canonical_integer_proposition(
        &context,
        &Proposition::LessOrEqual(
            value(2, u8_type),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(31)).unwrap(),
        ),
        &assumptions,
        &axioms,
    )
    .expect("the exact cast replays the source bound");
    let goal = Proposition::IntegerMathLessOrEqual(
        semantic_vocabulary::IntegerMathTerm::ShiftLeft {
            value: Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: u8_type,
                value: ValueId::new(4).unwrap(),
            }),
            count: Box::new(semantic_vocabulary::IntegerMathTerm::MathValue {
                source_type: u16_type,
                value: ValueId::new(5).unwrap(),
            }),
        },
        semantic_vocabulary::IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
    );
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
        .expect("the second shift replays its prior shift through the exact cast");
    accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
        .expect("the kernel replays both landed counts and the exact cast root");

    let insufficient = [Proposition::LessOrEqual(
        value(1, u64_type),
        ScalarTerm::integer(u64_type, IntegerValue::Unsigned(32)).unwrap(),
    )];
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &insufficient, &axioms).is_none(),
        "an out-of-carrier search candidate is rejected rather than becoming proof authority",
    );
    let mut drifted_count = axioms.clone();
    drifted_count[1] = Proposition::Equal(value(3, i8_type), integer(i8_type, 2));
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &assumptions, &drifted_count)
            .is_none(),
        "drifting the earlier landed count invalidates the retained source endpoint",
    );
}
