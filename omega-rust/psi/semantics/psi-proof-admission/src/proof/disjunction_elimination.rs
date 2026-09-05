use super::*;
use psi_core::PropositionId;

fn atom(index: u64) -> Proposition {
    Proposition::Atom(PropositionId::new(index).expect("nonzero atom"))
}

fn assumption(conclusion: Proposition, index: usize) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::Assumption { index },
    }
}

fn fixture() -> (Vec<Proposition>, ProofNode) {
    let left = atom(1);
    let right = atom(2);
    let goal = atom(3);
    let disjunction = Proposition::Disjunction(vec![left.clone(), right.clone()]);
    let implications: Vec<_> = [left.clone(), right.clone()]
        .into_iter()
        .map(|premise| Proposition::Implication {
            premise: Box::new(premise),
            conclusion: Box::new(goal.clone()),
        })
        .collect();
    let branches = [left, right]
        .into_iter()
        .enumerate()
        .map(|(index, premise)| ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ImplicationElimination {
                implication: Box::new(assumption(implications[index].clone(), index + 1)),
                premise: Box::new(assumption(premise, 3)),
            },
        })
        .collect();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::DisjunctionElimination {
            disjunction: Box::new(assumption(disjunction.clone(), 0)),
            branches,
        },
    };
    (
        vec![
            disjunction,
            implications[0].clone(),
            implications[1].clone(),
        ],
        proof,
    )
}

fn check(
    assumptions: &[Proposition],
    proof: &ProofNode,
) -> Result<CertificateAcceptance, ProofError> {
    accept_certificate(
        &PropositionContext::default(),
        &proof.conclusion,
        assumptions,
        &[],
        proof,
    )
}

#[test]
fn ordered_branches_discharge_only_their_local_assumptions() {
    let (assumptions, proof) = fixture();
    let accepted = check(&assumptions, &proof).expect("both cases prove the same conclusion");
    assert!(
        accepted
            .rules
            .contains(&AcceptedProofRule::DisjunctionElimination)
    );
    assert_eq!(accepted.assumptions.len(), 3);
    for (index, premise) in accepted.assumptions.iter().enumerate() {
        assert_eq!(premise.index, index);
        assert_eq!(premise.proposition, assumptions[index]);
    }
}

#[test]
fn elimination_requires_every_branch_and_the_common_conclusion() {
    let (assumptions, proof) = fixture();
    for count in [0, 1, 3] {
        let mut malformed = proof.clone();
        let ProofRule::DisjunctionElimination { branches, .. } = &mut malformed.rule else {
            unreachable!()
        };
        branches.resize(count, branches[0].clone());
        assert_eq!(
            check(&assumptions, &malformed),
            Err(ProofError::DisjunctionArityMismatch)
        );
    }
    let mut wrong_conclusion = proof.clone();
    let ProofRule::DisjunctionElimination { branches, .. } = &mut wrong_conclusion.rule else {
        unreachable!()
    };
    branches[0] = assumption(atom(1), 3);
    assert_eq!(
        check(&assumptions, &wrong_conclusion),
        Err(ProofError::DisjunctionBranchConclusionMismatch)
    );

    let mut wrong_premise = proof;
    let ProofRule::DisjunctionElimination { disjunction, .. } = &mut wrong_premise.rule else {
        unreachable!()
    };
    **disjunction = assumption(assumptions[1].clone(), 1);
    assert_eq!(
        check(&assumptions, &wrong_premise),
        Err(ProofError::RulePremiseMismatch("disjunction elimination"))
    );
}

#[test]
fn branches_cannot_reuse_another_case_or_an_unproved_disjunction() {
    let (assumptions, proof) = fixture();
    let mut swapped = proof.clone();
    let ProofRule::DisjunctionElimination { branches, .. } = &mut swapped.rule else {
        unreachable!()
    };
    branches.swap(0, 1);
    assert_eq!(
        check(&assumptions, &swapped),
        Err(ProofError::AssumptionConclusionMismatch(3))
    );

    let mut leaked = proof.clone();
    let ProofRule::DisjunctionElimination { branches, .. } = &mut leaked.rule else {
        unreachable!()
    };
    let ProofRule::ImplicationElimination { premise, .. } = &mut branches[1].rule else {
        unreachable!()
    };
    premise.rule = ProofRule::Assumption { index: 4 };
    assert_eq!(
        check(&assumptions, &leaked),
        Err(ProofError::UnknownAssumption(4))
    );

    let mut unproved = proof;
    let ProofRule::DisjunctionElimination { disjunction, .. } = &mut unproved.rule else {
        unreachable!()
    };
    disjunction.rule = ProofRule::Assumption { index: 3 };
    assert_eq!(
        check(&assumptions, &unproved),
        Err(ProofError::UnknownAssumption(3))
    );
}

#[test]
fn nested_implication_and_case_scopes_keep_only_ambient_custody() {
    let disjunction = Proposition::Disjunction(vec![atom(1), atom(2)]);
    let goal = Proposition::Implication {
        premise: Box::new(disjunction.clone()),
        conclusion: Box::new(Proposition::Truth),
    };
    let branches = [atom(1), atom(2)]
        .into_iter()
        .map(|branch| {
            // An inner case analysis can cite the enclosing case assumption,
            // but neither assumption is an ambient certificate requirement.
            ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::DisjunctionElimination {
                    disjunction: Box::new(ProofNode {
                        conclusion: Proposition::Disjunction(vec![branch.clone(), atom(4)]),
                        rule: ProofRule::DisjunctionIntroduction {
                            disjunct: Box::new(assumption(branch, 1)),
                            index: 0,
                        },
                    }),
                    branches: vec![
                        ProofNode {
                            conclusion: Proposition::Truth,
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        };
                        2
                    ],
                },
            }
        })
        .collect();
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ImplicationIntroduction {
            body: Box::new(ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::DisjunctionElimination {
                    disjunction: Box::new(assumption(disjunction, 0)),
                    branches,
                },
            }),
        },
    };
    let accepted = accept_certificate(
        &PropositionContext::default(),
        &goal,
        &[],
        &[Proposition::Truth],
        &proof,
    )
    .expect("nested scopes");
    assert!(accepted.assumptions.is_empty());
    assert_eq!(
        accepted.semantic_axioms,
        vec![AcceptedPremise {
            index: 0,
            proposition: Proposition::Truth
        }]
    );
    assert!(
        accepted
            .rules
            .contains(&AcceptedProofRule::ImplicationIntroduction)
    );

    let identity = Proposition::Implication {
        premise: Box::new(atom(1)),
        conclusion: Box::new(atom(1)),
    };
    let proof = ProofNode {
        conclusion: identity,
        rule: ProofRule::ImplicationIntroduction {
            body: Box::new(assumption(atom(1), 0)),
        },
    };
    assert!(
        check(&[], &proof)
            .expect("discharged implication")
            .assumptions
            .is_empty()
    );
}
