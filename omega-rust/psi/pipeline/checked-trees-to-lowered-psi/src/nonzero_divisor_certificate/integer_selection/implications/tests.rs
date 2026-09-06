use super::*;
use proof_admission::check_certificate;
use semantic_vocabulary::{PropositionContext, ScalarTerm, ScalarType, ValueId};

fn value(index: u64) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(index).unwrap(), ScalarType::Boolean)
}

fn polarity(index: u64, positive: bool) -> Proposition {
    Proposition::Equal(value(index), ScalarTerm::boolean(positive))
}

fn implication(premise: Proposition, conclusion: Proposition) -> Proposition {
    Proposition::Implication {
        premise: Box::new(premise),
        conclusion: Box::new(conclusion),
    }
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types(
        (1..=4).map(|index| (ValueId::new(index).unwrap(), ScalarType::Boolean)),
    )
    .unwrap()
}

fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    axioms: &[Proposition],
) -> Option<ProofNode> {
    super::super::build(&context(), goal, assumptions, axioms)
}

#[test]
fn implication_chains_replay_exact_citations_and_goal_orientation() {
    let goal = Proposition::Equal(ScalarTerm::boolean(true), value(4));
    let assumptions = [polarity(1, true)];
    let axioms = [
        implication(polarity(1, true), polarity(2, false)),
        Proposition::Conjunction(vec![
            Proposition::Truth,
            implication(polarity(2, false), polarity(3, true)),
        ]),
        Proposition::Equal(value(4), value(3)),
    ];
    let proof = prove(&goal, &assumptions, &axioms).expect("two implications and one exact alias");
    check_certificate(&context(), &goal, &assumptions, &axioms, &proof).unwrap();
    for index in 0..axioms.len() {
        let mut changed = axioms.clone();
        changed[index] = Proposition::Truth;
        assert!(check_certificate(&context(), &goal, &assumptions, &changed, &proof).is_err());
        assert!(prove(&goal, &assumptions, &changed).is_none());
    }
    assert!(check_certificate(&context(), &goal, &[], &axioms, &proof).is_err());
    assert!(prove(&goal, &[polarity(1, false)], &axioms).is_none());
}

#[test]
fn implication_cycles_and_conditional_laws_supply_no_ambient_premise() {
    let goal = polarity(2, true);
    let cycle = [
        implication(polarity(1, true), goal.clone()),
        implication(goal.clone(), polarity(1, true)),
    ];
    assert!(prove(&goal, &[], &cycle).is_none());
    let conditional = [Proposition::Disjunction(vec![
        cycle[0].clone(),
        Proposition::Truth,
    ])];
    assert!(prove(&goal, &[polarity(1, true)], &conditional).is_none());
    let proof = prove(&goal, &[polarity(1, true)], &cycle).unwrap();
    check_certificate(&context(), &goal, &[polarity(1, true)], &cycle, &proof).unwrap();
}

#[test]
fn each_selected_path_proves_its_own_implication_premise() {
    let goal = polarity(3, true);
    let assumptions = [Proposition::Disjunction(vec![
        polarity(1, true),
        polarity(2, false),
    ])];
    let axioms = [
        implication(polarity(1, true), goal.clone()),
        implication(polarity(2, false), goal.clone()),
    ];
    let proof =
        prove(&goal, &assumptions, &axioms).expect("either path establishes the same result");
    check_certificate(&context(), &goal, &assumptions, &axioms, &proof).unwrap();
    assert!(prove(&goal, &assumptions, &axioms[..1]).is_none());
}
