//! Canonical reader evidence plus exact sums justified by the selected operand guards.

use super::super::fixtures;
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, IntegerAffineWitness, ObligationClass,
    PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerMathTerm, IntegerValue, Proposition, ScalarTerm, ScalarType,
};
use terminal_psi::TerminalModule;
use terminal_verifier::{ObligationEvidence, ProofBundle};

fn citation(proposition: &Proposition, axioms: &[Proposition]) -> ProofNode {
    ProofNode {
        conclusion: proposition.clone(),
        rule: ProofRule::SemanticAxiom {
            index: axioms
                .iter()
                .position(|axiom| axiom == proposition)
                .expect("the exact selected guard or constant definition is reconstructed"),
        },
    }
}

fn operand_bound(operand: &ScalarTerm, axioms: &[Proposition]) -> ProofNode {
    let ScalarType::Integer(integer) = operand.scalar_type() else {
        panic!("exact sum operand is an integer");
    };
    let maximum = ScalarTerm::integer(integer, IntegerValue::Unsigned(1024)).unwrap();
    let goal = Proposition::LessOrEqual(operand.clone(), maximum.clone());
    if axioms.contains(&goal) {
        return citation(&goal, axioms);
    }
    let (guard, equality) = axioms
        .iter()
        .find_map(|axiom| {
            let Proposition::LessOrEqual(left, right) = axiom else {
                return None;
            };
            if left != operand {
                return None;
            }
            let equality = Proposition::Equal(right.clone(), maximum.clone());
            axioms.contains(&equality).then_some((axiom, equality))
        })
        .expect("each add operand is guarded against the defined 1024 constant");
    ProofNode {
        conclusion: goal,
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(citation(guard, axioms)),
            equality: Box::new(citation(&equality, axioms)),
            endpoint: 1,
        },
    }
}

fn exact_sum(goal: &Proposition, axioms: &[Proposition]) -> ProofNode {
    let Proposition::IntegerMathLessOrEqual(sum, maximum) = goal else {
        panic!("unsigned exact sum has one canonical mathematical upper bound");
    };
    let IntegerMathTerm::Add(left, right) = sum else {
        panic!("exact sum");
    };
    let IntegerMathTerm::MathValue {
        source_type: integer,
        value: left,
    } = left.as_ref()
    else {
        panic!("runtime left operand");
    };
    let IntegerMathTerm::MathValue {
        source_type: right_type,
        value: right,
    } = right.as_ref()
    else {
        panic!("runtime right operand");
    };
    assert_eq!(integer, right_type);
    assert_eq!(maximum, &IntegerMathTerm::literal(integer.maximum_value()));
    let left = ScalarTerm::value(*left, ScalarType::Integer(*integer));
    let right = ScalarTerm::value(*right, ScalarType::Integer(*integer));
    let bounds = vec![operand_bound(&left, axioms), operand_bound(&right, axioms)];
    let tight = IntegerMathTerm::literal(IntegerValue::Unsigned(2048));
    let bounded_sum = ProofNode {
        conclusion: Proposition::IntegerMathLessOrEqual(sum.clone(), tight.clone()),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: Proposition::Conjunction(
                    bounds
                        .iter()
                        .map(|bound| bound.conclusion.clone())
                        .collect(),
                ),
                rule: ProofRule::ConjunctionIntroduction(bounds),
            }),
            witness: IntegerAffineWitness {
                root: left.clone(),
                target: ScalarTerm::exact_integer_add(*integer, left, right).unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    };
    ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(bounded_sum),
            middle_less_or_equal_right: Box::new(ProofNode {
                conclusion: Proposition::IntegerMathLessOrEqual(tight, maximum.clone()),
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
        },
    }
}

pub(super) fn for_module(module: &TerminalModule) -> ProofBundle {
    let reader = fixtures::byte_view_read_module();
    assert_eq!(
        module.machines[0], reader.machines[0],
        "callee and its guarded read stay unchanged"
    );
    let mut proof = fixtures::byte_view_read_proof(&reader);
    let reconstructed = terminal_verifier::reconstruct_terminal_obligations(module).unwrap();
    for site in reconstructed.obligations() {
        assert!(site.canonical_certificate);
        assert_eq!(site.obligation.class, ObligationClass::Derivable);
        if site.obligation.id == proof.evidence[0].obligation {
            continue;
        }
        proof.evidence.push(ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(site.obligation.id.get()).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: exact_sum(&site.obligation.proposition, &site.semantic_axioms),
            }),
        });
    }
    terminal_verifier::verify_module(module, &proof, &AdmissionProfile::default())
        .expect("the selected guards prove all reads and exact sums without admission");
    proof
}
