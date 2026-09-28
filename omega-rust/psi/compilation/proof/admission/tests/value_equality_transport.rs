use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, accept_certificate};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};

fn context() -> PropositionContext {
    PropositionContext::from_value_types(
        (1..=32).map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Boolean)),
    )
    .unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean)
}

fn equal(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::boolean_equal(left, right).unwrap()
}

fn truth() -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    }
}

fn transport(goal: Proposition, equalities: Vec<ProofNode>) -> ProofNode {
    ProofNode {
        conclusion: goal,
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(truth()),
            equalities,
        },
    }
}

fn fixture() -> (Proposition, Vec<Proposition>, ProofNode) {
    let equations = vec![
        Proposition::Truth,
        Proposition::Equal(value(3), equal(value(1), value(2))),
        Proposition::Equal(value(6), equal(value(4), value(5))),
        Proposition::Equal(value(7), equal(value(3), value(6))),
    ];
    let goal = Proposition::Equal(
        value(7),
        equal(equal(value(1), value(2)), equal(value(4), value(5))),
    );
    let proof = transport(
        goal.clone(),
        equations
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, conclusion)| ProofNode {
                conclusion: conclusion.clone(),
                rule: ProofRule::SemanticAxiom { index },
            })
            .collect(),
    );
    (goal, equations, proof)
}

#[test]
fn ambient_equations_cannot_replace_missing_or_unproved_children() {
    let (goal, equations, original) = fixture();
    for mutation in 0..5 {
        let mut proof = original.clone();
        let ProofRule::ValueEqualityTransport {
            premise,
            equalities,
        } = &mut proof.rule
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                equalities.remove(0);
            }
            1 => equalities.clear(),
            2 => equalities[0].rule = ProofRule::SemanticAxiom { index: 2 },
            3 => equalities[0].rule = ProofRule::Primitive(PrimitiveJudgment::Truth),
            4 => premise.conclusion = Proposition::Falsehood,
            _ => unreachable!(),
        }
        assert!(
            accept_certificate(&context(), &goal, &[], &equations, &proof).is_err(),
            "mutation {mutation}"
        );
    }
    let mut changed = equations.clone();
    changed[1] = Proposition::Equal(value(3), equal(value(1), value(4)));
    assert!(accept_certificate(&context(), &goal, &[], &changed, &original).is_err());
}

#[test]
fn guarded_or_unoriented_equations_are_not_definitions() {
    let equality = Proposition::Equal(value(2), ScalarTerm::boolean_not(value(1)).unwrap());
    let goal = Proposition::Equal(
        ScalarTerm::boolean_not(value(2)).unwrap(),
        ScalarTerm::boolean_not(ScalarTerm::boolean_not(value(1)).unwrap()).unwrap(),
    );
    let Proposition::Equal(left, right) = &equality else {
        unreachable!()
    };
    for original in [
        Proposition::Implication {
            premise: Box::new(Proposition::Falsehood),
            conclusion: Box::new(equality.clone()),
        },
        Proposition::Equal(right.clone(), left.clone()),
    ] {
        let proof = transport(
            goal.clone(),
            vec![ProofNode {
                conclusion: original.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }],
        );
        assert!(accept_certificate(&context(), &goal, &[original], &[], &proof).is_err());
    }
    let reversed = Proposition::Equal(right.clone(), left.clone());
    let proof = transport(
        goal.clone(),
        vec![ProofNode {
            conclusion: equality,
            rule: ProofRule::EqualitySymmetry {
                equality: Box::new(ProofNode {
                    conclusion: reversed.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        }],
    );
    assert!(accept_certificate(&context(), &goal, &[reversed], &[], &proof).is_ok());
}

#[test]
fn cyclic_and_expanding_equations_fail_with_bounded_work() {
    for equations in [
        vec![
            Proposition::Equal(value(1), value(2)),
            Proposition::Equal(value(2), value(1)),
        ],
        (1..=20)
            .map(|identity| {
                Proposition::Equal(
                    value(identity),
                    equal(value(identity + 1), value(identity + 1)),
                )
            })
            .collect(),
    ] {
        let goal = Proposition::Equal(value(1), value(1));
        let proof = transport(
            goal.clone(),
            equations
                .iter()
                .enumerate()
                .map(|(index, conclusion)| ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::SemanticAxiom { index },
                })
                .collect(),
        );
        assert!(accept_certificate(&context(), &goal, &[], &equations, &proof).is_err());
    }
}

#[test]
fn borrowed_equations_are_bounded_before_cloning_or_traversing_the_whole_roster() {
    use proof_admission::{PredicateDenotationError, check_value_equality_denotation};
    let equation = Proposition::Equal(value(1), value(2));
    // An unbounded borrowed iterator must terminate at the shared input limit,
    // without first materializing a roster or cloning all of its terms.
    assert_eq!(
        check_value_equality_denotation(
            &context(),
            &Proposition::Truth,
            std::iter::repeat(&equation)
        ),
        Err(PredicateDenotationError::ResourceLimitExceeded),
    );
    let mut deep = value(2);
    for _ in 0..65 {
        deep = ScalarTerm::BooleanNot {
            operand: Box::new(deep),
        };
    }
    let equation = Proposition::Equal(value(1), deep);
    assert_eq!(
        check_value_equality_denotation(&context(), &Proposition::Truth, [&equation]),
        Err(PredicateDenotationError::ResourceLimitExceeded),
    );
}
