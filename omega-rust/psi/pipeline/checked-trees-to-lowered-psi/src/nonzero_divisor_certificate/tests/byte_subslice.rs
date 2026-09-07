use super::*;

#[test]
fn guarded_tail_bounds_replay_the_index_literal_and_selected_length_bound() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = two_value_context(integer_type);
    let start = value(1, integer_type);
    let length = value(2, integer_type);
    let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).unwrap();
    let goal = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(start.clone(), length.clone()),
        Proposition::LessOrEqual(length.clone(), length.clone()),
    ]);
    // The branch reconstructs this discrete consequence independently of the
    // subslice. Its source-length provenance remains the artifact's obligation.
    let axioms = [
        Proposition::Equal(start.clone(), one.clone()),
        Proposition::LessOrEqual(one, length.clone()),
    ];
    let proof = prove_canonical_integer_proposition(&context, &goal, &[], &axioms)
        .expect("exact start identity plus the selected nonempty length bound");
    let acceptance = accept_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
    assert_eq!(acceptance.semantic_axioms.len(), 2);
    assert!(prove_canonical_integer_proposition(&context, &goal, &[], &axioms[..1]).is_none());
    assert!(accept_certificate(&context, &goal, &[], &axioms[..1], &proof).is_err());
    let reversed = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(length.clone(), start.clone()),
        Proposition::LessOrEqual(start, length),
    ]);
    assert!(prove_canonical_integer_proposition(&context, &reversed, &[], &axioms).is_none());
}

#[test]
fn empty_at_end_proves_both_canonical_reflexive_bounds_without_assumed_contents() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = two_value_context(integer_type);
    let length = value(2, integer_type);
    let goal = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(length.clone(), length.clone()),
        Proposition::LessOrEqual(length.clone(), length),
    ]);
    let proof = prove_canonical_integer_proposition(&context, &goal, &[], &[]).unwrap();
    let acceptance = accept_certificate(&context, &goal, &[], &[], &proof).unwrap();
    assert!(acceptance.semantic_axioms.is_empty());
    assert!(acceptance.assumptions.is_empty());
    assert!(matches!(proof.rule, ProofRule::ConjunctionIntroduction(ref legs) if legs.len() == 2));
}
