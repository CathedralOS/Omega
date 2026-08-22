//! Focused certificates for canonical fixed-integer order propositions.
//!
//! This producer deliberately consumes only machine requirements and facts
//! reconstructed before the operation site. It never sees the operation's own
//! result equation, so the certificate cannot justify the operation with a
//! fact produced by that same operation.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_certificate,
    check_integer_affine_witness,
};

#[derive(Clone, Copy)]
enum Citation {
    Assumption(usize),
    SemanticAxiom(usize),
}

/// Build the recursive certificate shape shared by canonical integer goals.
///
/// This is deliberately not an affine or interval analyzer. It composes exact
/// prior citations and the small checked order rules; producers for richer
/// families must still materialize proofs of the atomic leaves.
pub(super) fn prove_canonical_integer_proposition(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let proof = build_canonical_integer_proposition(context, goal, assumptions, semantic_axioms)?;
    check_certificate(context, goal, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}

fn build_canonical_integer_proposition(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        if fact == goal {
            return Some(citation.proof(fact));
        }
    }
    match goal {
        Proposition::Truth => Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        }),
        Proposition::LessOrEqual(_, _) => {
            prove_integer_bound(context, goal, assumptions, semantic_axioms)
        }
        Proposition::Conjunction(conjuncts) => Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ConjunctionIntroduction(
                conjuncts
                    .iter()
                    .map(|conjunct| {
                        build_canonical_integer_proposition(
                            context,
                            conjunct,
                            assumptions,
                            semantic_axioms,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
            ),
        }),
        Proposition::Disjunction(disjuncts) => {
            let (index, disjunct) =
                disjuncts.iter().enumerate().find_map(|(index, disjunct)| {
                    build_canonical_integer_proposition(
                        context,
                        disjunct,
                        assumptions,
                        semantic_axioms,
                    )
                    .map(|proof| (index, proof))
                })?;
            Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::DisjunctionIntroduction {
                    disjunct: Box::new(disjunct),
                    index,
                },
            })
        }
        _ => None,
    }
}

fn prove_integer_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };

    if let Some(proof) =
        prove_exact_or_closed_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) = prove_two_fact_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = fact else {
            continue;
        };
        for (old, replacement) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            let (endpoint, relation) = if old == goal_left {
                (
                    0,
                    Proposition::LessOrEqual(replacement.clone(), goal_right.clone()),
                )
            } else if old == goal_right {
                (
                    1,
                    Proposition::LessOrEqual(goal_left.clone(), replacement.clone()),
                )
            } else {
                continue;
            };
            if let Some(relation_proof) = prove_exact_or_closed_transitive_integer_bound(
                &relation,
                assumptions,
                semantic_axioms,
            )
            .or_else(|| {
                prove_two_fact_transitive_integer_bound(&relation, assumptions, semantic_axioms)
            }) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(relation_proof),
                        equality: Box::new(citation.proof(fact)),
                        endpoint,
                    },
                });
            }
            if replacement.integer_value().is_none() {
                continue;
            }
            if let Some(relation) = closed_integer_relation(relation) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(relation),
                        equality: Box::new(citation.proof(fact)),
                        endpoint,
                    },
                });
            }
        }
    }
    prove_bounded_affine_bound(context, goal, assumptions, semantic_axioms)
}

fn prove_bounded_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (citation, root_bound) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
            continue;
        };
        for root in [root_left, root_right]
            .into_iter()
            .filter(|root| matches!(root, psi_core::ScalarTerm::Value { .. }))
        {
            for target in [goal_left, goal_right]
                .into_iter()
                .filter(|target| matches!(target, psi_core::ScalarTerm::Value { .. }))
            {
                for definition_axioms in affine_definition_words(context, semantic_axioms, root) {
                    let proof = ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::IntegerAffineBound {
                            root_bound: Box::new(citation.proof(root_bound)),
                            witness: IntegerAffineWitness {
                                root: root.clone(),
                                target: target.clone(),
                                definition_axioms,
                            },
                        },
                    };
                    if check_certificate(context, goal, assumptions, semantic_axioms, &proof)
                        .is_ok()
                    {
                        return Some(proof);
                    }
                }
            }
        }
    }
    None
}

fn affine_definition_words(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &psi_core::ScalarTerm,
) -> Vec<Vec<usize>> {
    const MAX_DEFINITIONS: usize = 4;

    // This is candidate pruning, not proof authority: only prefixes replayed
    // successfully by the kernel advance, and the completed proof is checked
    // again before it leaves this module.
    let mut words = Vec::new();
    let mut frontier = vec![(Vec::new(), 0)];
    for _ in 0..MAX_DEFINITIONS {
        let mut next = Vec::new();
        for (prefix, start) in frontier {
            for index in start..semantic_axioms.len() {
                let Proposition::Equal(left, right) = &semantic_axioms[index] else {
                    continue;
                };
                let mut word = prefix.clone();
                word.push(index);
                let continues = [left, right]
                    .into_iter()
                    .filter(|target| matches!(target, psi_core::ScalarTerm::Value { .. }))
                    .any(|target| {
                        check_integer_affine_witness(
                            context,
                            semantic_axioms,
                            &IntegerAffineWitness {
                                root: root.clone(),
                                target: target.clone(),
                                definition_axioms: word.clone(),
                            },
                        )
                        .is_ok()
                    });
                if continues {
                    words.push(word.clone());
                    next.push((word, index + 1));
                }
            }
        }
        frontier = next;
    }
    words
}

fn prove_two_fact_transitive_integer_bound(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (left_citation, left_fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, middle) = left_fact else {
            continue;
        };
        if left != goal_left {
            continue;
        }
        for (right_citation, right_fact) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::LessOrEqual(right_middle, right) = right_fact else {
                continue;
            };
            if right_middle == middle && right == goal_right {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(left_citation.proof(left_fact)),
                        middle_less_or_equal_right: Box::new(right_citation.proof(right_fact)),
                    },
                });
            }
        }
    }
    None
}

fn prove_exact_or_closed_transitive_integer_bound(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        if fact == goal {
            return Some(citation.proof(fact));
        }
    }
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(fact_left, fact_right) = fact else {
            continue;
        };
        if fact_left == goal_left {
            let tail = Proposition::LessOrEqual(fact_right.clone(), goal_right.clone());
            if let Some(tail) = closed_integer_relation(tail) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(citation.proof(fact)),
                        middle_less_or_equal_right: Box::new(tail),
                    },
                });
            }
        }
        if fact_right == goal_right {
            let head = Proposition::LessOrEqual(goal_left.clone(), fact_left.clone());
            if let Some(head) = closed_integer_relation(head) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(head),
                        middle_less_or_equal_right: Box::new(citation.proof(fact)),
                    },
                });
            }
        }
    }
    None
}

fn cited_facts<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition)> {
    assumptions
        .iter()
        .enumerate()
        .map(|(index, fact)| (Citation::Assumption(index), fact))
        .chain(
            semantic_axioms
                .iter()
                .enumerate()
                .map(|(index, fact)| (Citation::SemanticAxiom(index), fact)),
        )
}

fn closed_integer_relation(conclusion: Proposition) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = &conclusion else {
        return None;
    };
    let (left_type, left) = left.integer_value()?;
    let (right_type, right) = right.integer_value()?;
    (left_type == right_type
        && left_type
            .compare(left, right)
            .is_some_and(|order| !order.is_gt()))
    .then_some(ProofNode {
        conclusion,
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    })
}

impl Citation {
    fn proof(self, conclusion: &Proposition) -> ProofNode {
        ProofNode {
            conclusion: conclusion.clone(),
            rule: match self {
                Self::Assumption(index) => ProofRule::Assumption { index },
                Self::SemanticAxiom(index) => ProofRule::SemanticAxiom { index },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::{IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId};

    fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(integer_type),
        )
    }

    fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
        ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("integer")
    }

    fn two_value_context(integer_type: IntegerType) -> PropositionContext {
        PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
        ])
        .unwrap()
    }

    #[test]
    fn signed_goal_prefers_negative_arm_and_tightens_requirement() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = value(1, integer_type);
        let negative_one = integer(integer_type, -1);
        let goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), negative_one),
            Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
        ]);
        let requirements = [Proposition::LessOrEqual(divisor, integer(integer_type, -2))];
        let proof = prove_canonical_integer_proposition(
            &PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(integer_type),
            )])
            .unwrap(),
            &goal,
            &requirements,
            &[],
        )
        .expect("negative bound proves nonzero");
        assert!(matches!(
            proof.rule,
            ProofRule::DisjunctionIntroduction { index: 0, .. }
        ));
    }

    #[test]
    fn signed_goal_selects_positive_arm_from_exact_requirement() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = value(1, integer_type);
        let positive = Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone());
        let goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(divisor, integer(integer_type, -1)),
            positive.clone(),
        ]);
        let proof = prove_canonical_integer_proposition(
            &PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(integer_type),
            )])
            .unwrap(),
            &goal,
            &[positive],
            &[],
        )
        .expect("positive requirement proves nonzero");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
            panic!("signed goal uses disjunction introduction")
        };
        assert_eq!(index, 1);
        assert!(matches!(disjunct.rule, ProofRule::Assumption { index: 0 }));
    }

    #[test]
    fn literal_equality_substitution_uses_only_prior_fact() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let divisor = value(1, integer_type);
        let goal = Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).unwrap(),
            divisor.clone(),
        );
        let facts = [Proposition::Equal(
            divisor,
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).unwrap(),
        )];
        let proof = prove_canonical_integer_proposition(
            &PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(integer_type),
            )])
            .unwrap(),
            &goal,
            &[],
            &facts,
        )
        .expect("literal equality proves nonzero");
        assert!(matches!(
            proof.rule,
            ProofRule::IntegerLessOrEqualSubstitution { endpoint: 1, .. }
        ));
        assert!(
            prove_canonical_integer_proposition(
                &PropositionContext::from_value_types([(
                    ValueId::new(1).unwrap(),
                    ScalarType::Integer(integer_type),
                )])
                .unwrap(),
                &goal,
                &[],
                &[],
            )
            .is_none()
        );
    }

    #[test]
    fn exact_division_goal_composes_ordered_three_arm_and_joint_exception_proofs() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let dividend = value(1, integer_type);
        let divisor = value(2, integer_type);
        let negative_safe = Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -2));
        let positive_safe = Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone());
        let negative_one = Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1));
        let dividend_safe = Proposition::LessOrEqual(integer(integer_type, -127), dividend.clone());
        let goal = Proposition::Disjunction(vec![
            negative_safe.clone(),
            positive_safe,
            Proposition::Conjunction(vec![negative_one.clone(), dividend_safe.clone()]),
        ]);

        let context = two_value_context(integer_type);
        let negative = prove_canonical_integer_proposition(&context, &goal, &[negative_safe], &[])
            .expect("first exact-division arm is cited");
        assert!(matches!(
            negative.rule,
            ProofRule::DisjunctionIntroduction { index: 0, .. }
        ));

        let joint = prove_canonical_integer_proposition(
            &context,
            &goal,
            &[negative_one, dividend_safe],
            &[],
        )
        .expect("joint -1/dividend exception is composed");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = joint.rule else {
            panic!("exact division uses disjunction introduction")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("joint exact bounds use conjunction introduction")
        };
        assert_eq!(conjuncts.len(), 2);
        assert!(matches!(
            conjuncts[0].rule,
            ProofRule::Assumption { index: 0 }
        ));
        assert!(matches!(
            conjuncts[1].rule,
            ProofRule::Assumption { index: 1 }
        ));
        assert!(prove_canonical_integer_proposition(&context, &goal, &[], &[]).is_none());
    }

    #[test]
    fn exact_division_goal_composes_runtime_negative_bound_and_landed_dividend() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let dividend = value(1, integer_type);
        let divisor = value(2, integer_type);
        let goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -2)),
            Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1)),
                Proposition::LessOrEqual(integer(integer_type, -127), dividend.clone()),
            ]),
        ]);
        let proof = prove_canonical_integer_proposition(
            &two_value_context(integer_type),
            &goal,
            &[Proposition::LessOrEqual(divisor, integer(integer_type, -1))],
            &[Proposition::Equal(dividend, integer(integer_type, -7))],
        )
        .expect("runtime negative bound and landed dividend prove the joint arm");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
            panic!("mixed joint evidence selects its canonical disjunct")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("mixed joint evidence proves both canonical premises")
        };
        assert!(matches!(
            conjuncts[0].rule,
            ProofRule::Assumption { index: 0 }
        ));
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } = &conjuncts[1].rule
        else {
            panic!("landed dividend proves its canonical floor by substitution")
        };
        assert_eq!(*endpoint, 1);
        assert!(matches!(
            relation.rule,
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
        ));
        assert!(matches!(
            equality.rule,
            ProofRule::SemanticAxiom { index: 0 }
        ));
    }

    #[test]
    fn exact_division_goal_cites_complete_retained_goal_or_arm() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let dividend = value(1, integer_type);
        let divisor = value(2, integer_type);
        let joint_arm = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), integer(integer_type, -1)),
            Proposition::LessOrEqual(integer(integer_type, -127), dividend.clone()),
        ]);
        let goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(divisor, integer(integer_type, -2)),
            Proposition::LessOrEqual(integer(integer_type, 1), value(2, integer_type)),
            joint_arm.clone(),
        ]);
        let whole_goal = prove_canonical_integer_proposition(
            &two_value_context(integer_type),
            &goal,
            std::slice::from_ref(&goal),
            &[],
        )
        .expect("exact retained canonical goal is cited directly");
        assert!(matches!(
            whole_goal.rule,
            ProofRule::Assumption { index: 0 }
        ));

        let retained_arm = prove_canonical_integer_proposition(
            &two_value_context(integer_type),
            &goal,
            &[],
            std::slice::from_ref(&joint_arm),
        )
        .expect("exact retained canonical arm is introduced");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = retained_arm.rule else {
            panic!("retained canonical arm is wrapped by disjunction introduction")
        };
        assert_eq!(index, 2);
        assert!(matches!(
            disjunct.rule,
            ProofRule::SemanticAxiom { index: 0 }
        ));

        let redirected_arm = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(value(3, integer_type), integer(integer_type, -1)),
            Proposition::LessOrEqual(integer(integer_type, -127), dividend),
        ]);
        assert!(
            prove_canonical_integer_proposition(
                &two_value_context(integer_type),
                &goal,
                &[],
                &[redirected_arm],
            )
            .is_none(),
            "redirected canonical arm cannot prove the goal",
        );
    }

    #[test]
    fn exact_division_goal_composes_literal_equality_requirements() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_divisor = value(2, unsigned);
        let unsigned_goal = Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
            unsigned_divisor.clone(),
        );
        let unsigned_proof = prove_canonical_integer_proposition(
            &two_value_context(unsigned),
            &unsigned_goal,
            &[Proposition::Equal(
                unsigned_divisor,
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5)).expect("safe u8 divisor"),
            )],
            &[],
        )
        .expect("literal equality requirement proves unsigned definedness");
        let ProofRule::IntegerLessOrEqualSubstitution {
            equality, endpoint, ..
        } = unsigned_proof.rule
        else {
            panic!("unsigned literal requirement uses equality substitution")
        };
        assert_eq!(endpoint, 1);
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 0 }));
        assert!(
            prove_canonical_integer_proposition(
                &two_value_context(unsigned),
                &unsigned_goal,
                &[Proposition::Equal(
                    value(2, unsigned),
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0))
                        .expect("zero u8 divisor"),
                )],
                &[],
            )
            .is_none(),
            "zero equality requirement cannot prove unsigned definedness",
        );
        assert!(
            prove_canonical_integer_proposition(
                &two_value_context(unsigned),
                &unsigned_goal,
                &[Proposition::Equal(
                    value(1, unsigned),
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5))
                        .expect("redirected u8 divisor"),
                )],
                &[],
            )
            .is_none(),
            "wrong-operand equality requirement cannot prove unsigned definedness",
        );

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_dividend = value(1, signed);
        let signed_divisor = value(2, signed);
        let signed_goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
            Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -1)),
                Proposition::LessOrEqual(integer(signed, -127), signed_dividend.clone()),
            ]),
        ]);
        let signed_proof = prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            &[
                Proposition::LessOrEqual(signed_divisor, integer(signed, -1)),
                Proposition::Equal(signed_dividend, integer(signed, -7)),
            ],
            &[],
        )
        .expect("runtime negative bound and dividend requirement prove the joint arm");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
            panic!("mixed requirement evidence selects the canonical joint arm")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("mixed requirement evidence proves both joint premises")
        };
        assert!(matches!(
            conjuncts[0].rule,
            ProofRule::Assumption { index: 0 }
        ));
        let ProofRule::IntegerLessOrEqualSubstitution { equality, .. } = &conjuncts[1].rule else {
            panic!("dividend requirement proves its floor by substitution")
        };
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
        assert!(
            prove_canonical_integer_proposition(
                &two_value_context(signed),
                &signed_goal,
                &[
                    Proposition::LessOrEqual(value(2, signed), integer(signed, -1)),
                    Proposition::Equal(
                        value(1, signed),
                        ScalarTerm::integer(signed, signed.minimum_value())
                            .expect("minimum i8 dividend"),
                    ),
                ],
                &[],
            )
            .is_none(),
            "minimum dividend equality cannot prove the joint arm",
        );
    }

    #[test]
    fn exact_division_goal_transports_exact_bound_across_endpoint_equality() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(unsigned)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(unsigned)),
        ])
        .expect("three u8 values");
        let unsigned_goal = Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
            value(2, unsigned),
        );
        let unsigned_proof = prove_canonical_integer_proposition(
            &unsigned_context,
            &unsigned_goal,
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                    value(3, unsigned),
                ),
                Proposition::Equal(value(3, unsigned), value(2, unsigned)),
            ],
            &[],
        )
        .expect("exact intermediate bound transports to the unsigned divisor");
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } = unsigned_proof.rule
        else {
            panic!("unsigned endpoint transport uses integer-order substitution")
        };
        assert_eq!(endpoint, 1);
        assert!(matches!(relation.rule, ProofRule::Assumption { index: 0 }));
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
        assert!(
            prove_canonical_integer_proposition(
                &unsigned_context,
                &unsigned_goal,
                &[Proposition::Equal(value(3, unsigned), value(2, unsigned),)],
                &[],
            )
            .is_none(),
            "endpoint equality without its bound cannot prove definedness",
        );

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
        ])
        .expect("three i8 values");
        let signed_divisor = value(2, signed);
        let signed_goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
            Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(signed_divisor, integer(signed, -1)),
                Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
            ]),
        ]);
        let signed_proof = prove_canonical_integer_proposition(
            &signed_context,
            &signed_goal,
            &[
                Proposition::LessOrEqual(value(3, signed), integer(signed, -2)),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
            &[],
        )
        .expect("exact intermediate bound transports to the signed divisor");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
            panic!("signed endpoint transport selects its canonical arm")
        };
        assert_eq!(index, 0);
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } = disjunct.rule
        else {
            panic!("signed endpoint transport uses integer-order substitution")
        };
        assert_eq!(endpoint, 0);
        assert!(matches!(relation.rule, ProofRule::Assumption { index: 0 }));
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));

        let dividend_transport_proof = prove_canonical_integer_proposition(
            &signed_context,
            &signed_goal,
            &[
                Proposition::LessOrEqual(value(2, signed), integer(signed, -1)),
                Proposition::LessOrEqual(integer(signed, -127), value(3, signed)),
                Proposition::Equal(value(3, signed), value(1, signed)),
            ],
            &[],
        )
        .expect("exact intermediate floor transports to the signed dividend");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = dividend_transport_proof.rule
        else {
            panic!("dividend endpoint transport selects the canonical joint arm")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("dividend endpoint transport proves both joint premises")
        };
        assert!(matches!(
            conjuncts[0].rule,
            ProofRule::Assumption { index: 0 }
        ));
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } = &conjuncts[1].rule
        else {
            panic!("dividend floor uses integer-order substitution")
        };
        assert_eq!(*endpoint, 1);
        assert!(matches!(relation.rule, ProofRule::Assumption { index: 1 }));
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 2 }));
        assert!(
            prove_canonical_integer_proposition(
                &signed_context,
                &signed_goal,
                &[
                    Proposition::LessOrEqual(value(2, signed), integer(signed, -1)),
                    Proposition::LessOrEqual(integer(signed, -127), value(3, signed)),
                    Proposition::Equal(value(3, signed), value(2, signed)),
                ],
                &[],
            )
            .is_none(),
            "redirected equality cannot transport the dividend floor",
        );
    }

    #[test]
    fn exact_division_goal_composes_complete_prior_fact_proofs() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_divisor = value(2, unsigned);
        let unsigned_goal = Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 literal"),
            unsigned_divisor.clone(),
        );
        let unsigned_direct = prove_canonical_integer_proposition(
            &two_value_context(unsigned),
            &unsigned_goal,
            std::slice::from_ref(&unsigned_goal),
            &[],
        )
        .expect("exact unsigned divisor floor is cited directly");
        assert!(matches!(
            unsigned_direct.rule,
            ProofRule::Assumption { index: 0 }
        ));
        let unsigned_stronger = prove_canonical_integer_proposition(
            &two_value_context(unsigned),
            &unsigned_goal,
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2))
                    .expect("stronger u8 floor"),
                unsigned_divisor.clone(),
            )],
            &[],
        )
        .expect("stronger unsigned divisor floor composes transitively");
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = unsigned_stronger.rule
        else {
            panic!("stronger unsigned floor uses exact transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index: 0 }
        ));
        let unsigned_literal =
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(5)).expect("u8 literal");
        let unsigned_proof = prove_canonical_integer_proposition(
            &two_value_context(unsigned),
            &unsigned_goal,
            &[],
            &[Proposition::Equal(unsigned_divisor, unsigned_literal)],
        )
        .expect("landed positive literal proves unsigned definedness");
        assert!(matches!(
            unsigned_proof.rule,
            ProofRule::IntegerLessOrEqualSubstitution { endpoint: 1, .. }
        ));

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_dividend = value(1, signed);
        let signed_divisor = value(2, signed);
        let signed_goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
            Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -1)),
                Proposition::LessOrEqual(integer(signed, -127), signed_dividend),
            ]),
        ]);
        let positive_divisor = Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone());
        let positive_proof = prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            std::slice::from_ref(&positive_divisor),
            &[],
        )
        .expect("exact signed positive-divisor arm is cited directly");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = positive_proof.rule else {
            panic!("signed positive divisor selects its canonical arm")
        };
        assert_eq!(index, 1);
        assert!(matches!(disjunct.rule, ProofRule::Assumption { index: 0 }));
        let stronger_positive_proof = prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            &[Proposition::LessOrEqual(
                integer(signed, 3),
                signed_divisor.clone(),
            )],
            &[],
        )
        .expect("stronger signed positive floor composes transitively");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = stronger_positive_proof.rule
        else {
            panic!("stronger positive floor selects its canonical arm")
        };
        assert_eq!(index, 1);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = disjunct.rule
        else {
            panic!("stronger signed positive floor uses exact transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index: 0 }
        ));
        let stronger_negative_proof = prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            &[Proposition::LessOrEqual(
                signed_divisor.clone(),
                integer(signed, -3),
            )],
            &[],
        )
        .expect("stronger signed negative ceiling composes transitively");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = stronger_negative_proof.rule
        else {
            panic!("stronger negative ceiling selects its canonical arm")
        };
        assert_eq!(index, 0);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = disjunct.rule
        else {
            panic!("stronger signed negative ceiling uses exact transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index: 0 }
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
        ));
        let safe_fact = Proposition::Equal(signed_divisor.clone(), integer(signed, -3));
        let signed_proof = prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            &[],
            &[safe_fact],
        )
        .expect("landed negative literal proves signed definedness");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
            panic!("signed exact division proves one canonical disjunct")
        };
        assert_eq!(index, 0);
        assert!(matches!(
            disjunct.rule,
            ProofRule::IntegerLessOrEqualSubstitution { endpoint: 0, .. }
        ));

        for excluded in [0, -1] {
            assert!(
                prove_canonical_integer_proposition(
                    &two_value_context(signed),
                    &signed_goal,
                    &[],
                    &[Proposition::Equal(
                        signed_divisor.clone(),
                        integer(signed, excluded),
                    )],
                )
                .is_none(),
                "signed literal {excluded} is not carrier-total",
            );
        }

        let exceptional_proof = prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            &[],
            &[
                Proposition::Equal(signed_divisor, integer(signed, -1)),
                Proposition::Equal(value(1, signed), integer(signed, -7)),
            ],
        )
        .expect("landed -1 and nonminimum dividend prove the exceptional arm");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = exceptional_proof.rule else {
            panic!("signed -1 exact division proves the joint exceptional arm")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("signed -1 arm proves both canonical bounds")
        };
        assert_eq!(conjuncts.len(), 2);
        assert!(
            conjuncts.iter().all(|proof| matches!(
                proof.rule,
                ProofRule::IntegerLessOrEqualSubstitution { .. }
            ))
        );

        let dividend_bound = Proposition::LessOrEqual(integer(signed, -127), value(1, signed));
        let retained_bound_proof = prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            std::slice::from_ref(&dividend_bound),
            &[Proposition::Equal(value(2, signed), integer(signed, -1))],
        )
        .expect("landed -1 and exact retained dividend bound prove the exceptional arm");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = retained_bound_proof.rule
        else {
            panic!("retained dividend bound selects the joint exceptional arm")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("retained dividend bound proves both canonical bounds")
        };
        assert!(matches!(
            conjuncts[0].rule,
            ProofRule::IntegerLessOrEqualSubstitution { .. }
        ));
        assert!(matches!(
            conjuncts[1].rule,
            ProofRule::Assumption { index: 0 }
        ));

        let stronger_bound_proof = prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            &[Proposition::LessOrEqual(
                integer(signed, -120),
                value(1, signed),
            )],
            &[Proposition::Equal(value(2, signed), integer(signed, -1))],
        )
        .expect("stronger retained dividend floor proves the exceptional arm");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = stronger_bound_proof.rule
        else {
            panic!("stronger retained bound selects the joint exceptional arm")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("stronger retained bound proves both canonical bounds")
        };
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = &conjuncts[1].rule
        else {
            panic!("canonical dividend floor follows by one checked transitivity step")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index: 0 }
        ));

        let retained_axiom_proof = prove_canonical_integer_proposition(
            &two_value_context(signed),
            &signed_goal,
            &[],
            &[
                Proposition::Equal(value(2, signed), integer(signed, -1)),
                dividend_bound,
            ],
        )
        .expect("pre-site exact dividend axiom proves the exceptional arm");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = retained_axiom_proof.rule
        else {
            panic!("pre-site dividend axiom selects the joint exceptional arm")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("pre-site dividend axiom proves both canonical bounds")
        };
        assert!(matches!(
            conjuncts[1].rule,
            ProofRule::SemanticAxiom { index: 1 }
        ));
    }

    #[test]
    fn i1_exact_division_goal_requires_both_joint_bounds() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let dividend = value(1, integer_type);
        let divisor = value(2, integer_type);
        let divisor_negative = Proposition::LessOrEqual(divisor, integer(integer_type, -1));
        let dividend_nonnegative = Proposition::LessOrEqual(integer(integer_type, 0), dividend);
        let goal =
            Proposition::Conjunction(vec![divisor_negative.clone(), dividend_nonnegative.clone()]);
        assert!(
            prove_canonical_integer_proposition(
                &two_value_context(integer_type),
                &goal,
                std::slice::from_ref(&divisor_negative),
                &[],
            )
            .is_none()
        );
        let retained_bound_proof = prove_canonical_integer_proposition(
            &two_value_context(integer_type),
            &goal,
            &[divisor_negative.clone(), dividend_nonnegative.clone()],
            &[],
        )
        .expect("both exact i1 bounds prove the joint goal");
        let ProofRule::ConjunctionIntroduction(conjuncts) = retained_bound_proof.rule else {
            panic!("exact i1 bounds compose through conjunction introduction")
        };
        assert_eq!(conjuncts.len(), 2);
        assert!(matches!(
            conjuncts[0].rule,
            ProofRule::Assumption { index: 0 }
        ));
        assert!(matches!(
            conjuncts[1].rule,
            ProofRule::Assumption { index: 1 }
        ));
        assert!(
            prove_canonical_integer_proposition(
                &two_value_context(integer_type),
                &goal,
                &[
                    divisor_negative,
                    Proposition::LessOrEqual(integer(integer_type, 0), value(3, integer_type),),
                ],
                &[],
            )
            .is_none(),
            "wrong-dividend bound cannot prove the joint goal",
        );

        let landed = [
            Proposition::Equal(value(2, integer_type), integer(integer_type, -1)),
            Proposition::Equal(value(1, integer_type), integer(integer_type, 0)),
        ];
        let proof = prove_canonical_integer_proposition(
            &two_value_context(integer_type),
            &goal,
            &[],
            &landed,
        )
        .expect("landed i1 -1/zero pair proves exact definedness");
        assert!(matches!(
            proof.rule,
            ProofRule::ConjunctionIntroduction(ref conjuncts) if conjuncts.len() == 2
        ));
    }

    #[test]
    fn i1_exact_division_goal_transports_both_joint_endpoints() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(integer_type)),
            (ValueId::new(4).unwrap(), ScalarType::Integer(integer_type)),
        ])
        .expect("four i1 values");
        let goal = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(value(2, integer_type), integer(integer_type, -1)),
            Proposition::LessOrEqual(integer(integer_type, 0), value(1, integer_type)),
        ]);
        let assumptions = [
            Proposition::LessOrEqual(value(3, integer_type), integer(integer_type, -1)),
            Proposition::Equal(value(3, integer_type), value(2, integer_type)),
            Proposition::LessOrEqual(integer(integer_type, 0), value(4, integer_type)),
            Proposition::Equal(value(4, integer_type), value(1, integer_type)),
        ];
        let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
            .expect("both i1 joint endpoints transport independently");
        let ProofRule::ConjunctionIntroduction(conjuncts) = proof.rule else {
            panic!("i1 endpoint transport constructs the canonical conjunction")
        };
        assert_eq!(conjuncts.len(), 2);
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } = &conjuncts[0].rule
        else {
            panic!("i1 divisor bound uses endpoint substitution")
        };
        assert_eq!(*endpoint, 0);
        assert!(matches!(relation.rule, ProofRule::Assumption { index: 0 }));
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } = &conjuncts[1].rule
        else {
            panic!("i1 dividend bound uses endpoint substitution")
        };
        assert_eq!(*endpoint, 1);
        assert!(matches!(relation.rule, ProofRule::Assumption { index: 2 }));
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 3 }));

        let crossed = [
            Proposition::LessOrEqual(value(3, integer_type), integer(integer_type, -1)),
            Proposition::Equal(value(3, integer_type), value(1, integer_type)),
            Proposition::LessOrEqual(integer(integer_type, 0), value(4, integer_type)),
            Proposition::Equal(value(4, integer_type), value(2, integer_type)),
        ];
        assert!(
            prove_canonical_integer_proposition(&context, &goal, &crossed, &[]).is_none(),
            "crossed i1 endpoint equalities cannot prove the joint goal",
        );
        assert!(
            prove_canonical_integer_proposition(&context, &goal, &assumptions[..3], &[]).is_none(),
            "missing dividend equality cannot prove the joint goal",
        );
    }

    #[test]
    fn exact_division_goal_nests_closed_transitivity_under_endpoint_transport() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(unsigned)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(unsigned)),
        ])
        .expect("three u8 values");
        let unsigned_goal = Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
            value(2, unsigned),
        );
        let unsigned_proof = prove_canonical_integer_proposition(
            &unsigned_context,
            &unsigned_goal,
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(2))
                        .expect("stronger u8 floor"),
                    value(3, unsigned),
                ),
                Proposition::Equal(value(3, unsigned), value(2, unsigned)),
            ],
            &[],
        )
        .expect("stronger intermediate bound transports to the unsigned divisor");
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } = unsigned_proof.rule
        else {
            panic!("unsigned stronger endpoint transport uses substitution")
        };
        assert_eq!(endpoint, 1);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = relation.rule
        else {
            panic!("transported unsigned relation uses closed transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index: 0 }
        ));
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 1 }));
        assert!(
            prove_canonical_integer_proposition(
                &unsigned_context,
                &unsigned_goal,
                &[
                    Proposition::LessOrEqual(
                        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(0))
                            .expect("weak u8 floor"),
                        value(3, unsigned),
                    ),
                    Proposition::Equal(value(3, unsigned), value(2, unsigned)),
                ],
                &[],
            )
            .is_none(),
            "weak transported bound cannot prove unsigned definedness",
        );

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
        ])
        .expect("three i8 values");
        let signed_divisor = value(2, signed);
        let signed_goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
            Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(signed_divisor, integer(signed, -1)),
                Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
            ]),
        ]);
        let signed_proof = prove_canonical_integer_proposition(
            &signed_context,
            &signed_goal,
            &[
                Proposition::LessOrEqual(value(3, signed), integer(signed, -3)),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
            &[],
        )
        .expect("stronger intermediate ceiling transports to the signed divisor");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
            panic!("signed stronger transport selects its canonical arm")
        };
        assert_eq!(index, 0);
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation, endpoint, ..
        } = disjunct.rule
        else {
            panic!("signed stronger endpoint transport uses substitution")
        };
        assert_eq!(endpoint, 0);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = relation.rule
        else {
            panic!("transported signed relation uses closed transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index: 0 }
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)
        ));
    }

    #[test]
    fn exact_division_goal_nests_two_citation_transitivity_under_endpoint_transport() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(unsigned)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(unsigned)),
            (ValueId::new(4).unwrap(), ScalarType::Integer(unsigned)),
        ])
        .expect("four u8 values");
        let unsigned_goal = Proposition::LessOrEqual(
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
            value(2, unsigned),
        );
        let unsigned_proof = prove_canonical_integer_proposition(
            &unsigned_context,
            &unsigned_goal,
            &[
                Proposition::LessOrEqual(
                    ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                    value(4, unsigned),
                ),
                Proposition::LessOrEqual(value(4, unsigned), value(3, unsigned)),
                Proposition::Equal(value(3, unsigned), value(2, unsigned)),
            ],
            &[],
        )
        .expect("two cited bounds transport to the unsigned divisor");
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } = unsigned_proof.rule
        else {
            panic!("unsigned two-citation transport uses substitution")
        };
        assert_eq!(endpoint, 1);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = relation.rule
        else {
            panic!("transported unsigned relation uses two-citation transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index: 0 }
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index: 1 }
        ));
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 2 }));
        assert!(
            prove_canonical_integer_proposition(
                &unsigned_context,
                &unsigned_goal,
                &[
                    Proposition::LessOrEqual(
                        ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one"),
                        value(4, unsigned),
                    ),
                    Proposition::Equal(value(3, unsigned), value(2, unsigned)),
                ],
                &[],
            )
            .is_none(),
            "missing middle relation cannot prove transported definedness",
        );

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(4).unwrap(), ScalarType::Integer(signed)),
        ])
        .expect("four i8 values");
        let signed_divisor = value(2, signed);
        let signed_goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
            Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(signed_divisor, integer(signed, -1)),
                Proposition::LessOrEqual(integer(signed, -127), value(1, signed)),
            ]),
        ]);
        let signed_proof = prove_canonical_integer_proposition(
            &signed_context,
            &signed_goal,
            &[
                Proposition::LessOrEqual(value(3, signed), value(4, signed)),
                Proposition::LessOrEqual(value(4, signed), integer(signed, -2)),
                Proposition::Equal(value(3, signed), value(2, signed)),
            ],
            &[],
        )
        .expect("two cited bounds transport to the signed divisor");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = signed_proof.rule else {
            panic!("signed two-citation transport selects its canonical arm")
        };
        assert_eq!(index, 0);
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation, endpoint, ..
        } = disjunct.rule
        else {
            panic!("signed two-citation transport uses substitution")
        };
        assert_eq!(endpoint, 0);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = relation.rule
        else {
            panic!("transported signed relation uses two-citation transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index: 0 }
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index: 1 }
        ));
    }

    #[test]
    fn exact_division_goal_nests_two_citation_dividend_floor_under_endpoint_transport() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(4).unwrap(), ScalarType::Integer(signed)),
        ])
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
        let assumptions = [
            Proposition::LessOrEqual(value(2, signed), integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(4, signed)),
            Proposition::LessOrEqual(value(4, signed), value(3, signed)),
            Proposition::Equal(value(3, signed), value(1, signed)),
        ];
        let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
            .expect("two cited bounds transport to the signed dividend");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
            panic!("two-citation dividend transport selects the joint arm")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("two-citation dividend transport proves both joint premises")
        };
        assert!(matches!(
            conjuncts[0].rule,
            ProofRule::Assumption { index: 0 }
        ));
        let ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } = &conjuncts[1].rule
        else {
            panic!("dividend floor uses endpoint substitution")
        };
        assert_eq!(*endpoint, 1);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = &relation.rule
        else {
            panic!("transported dividend floor uses two-citation transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index: 1 }
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index: 2 }
        ));
        assert!(matches!(equality.rule, ProofRule::Assumption { index: 3 }));

        assert!(
            prove_canonical_integer_proposition(
                &context,
                &goal,
                &[
                    assumptions[0].clone(),
                    assumptions[1].clone(),
                    assumptions[3].clone(),
                ],
                &[],
            )
            .is_none(),
            "missing middle bound cannot prove the transported dividend floor",
        );
    }

    #[test]
    fn i1_exact_division_goal_nests_two_citation_transport_for_both_endpoints() {
        let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(i1)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(i1)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(i1)),
            (ValueId::new(4).unwrap(), ScalarType::Integer(i1)),
            (ValueId::new(5).unwrap(), ScalarType::Integer(i1)),
            (ValueId::new(6).unwrap(), ScalarType::Integer(i1)),
        ])
        .expect("six i1 values");
        let goal = Proposition::Conjunction(vec![
            Proposition::LessOrEqual(value(2, i1), integer(i1, -1)),
            Proposition::LessOrEqual(integer(i1, 0), value(1, i1)),
        ]);
        let assumptions = [
            Proposition::LessOrEqual(value(3, i1), value(4, i1)),
            Proposition::LessOrEqual(value(4, i1), integer(i1, -1)),
            Proposition::Equal(value(3, i1), value(2, i1)),
            Proposition::LessOrEqual(integer(i1, 0), value(6, i1)),
            Proposition::LessOrEqual(value(6, i1), value(5, i1)),
            Proposition::Equal(value(5, i1), value(1, i1)),
        ];
        let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
            .expect("two cited bounds transport to both i1 operands");
        let ProofRule::ConjunctionIntroduction(conjuncts) = proof.rule else {
            panic!("i1 two-citation transport proves both canonical conjuncts")
        };
        for (conjunct, endpoint, first, second, equality) in
            [(&conjuncts[0], 0, 0, 1, 2), (&conjuncts[1], 1, 3, 4, 5)]
        {
            let ProofRule::IntegerLessOrEqualSubstitution {
                relation,
                equality: equality_proof,
                endpoint: actual_endpoint,
            } = &conjunct.rule
            else {
                panic!("i1 bound uses endpoint substitution")
            };
            assert_eq!(*actual_endpoint, endpoint);
            let ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } = &relation.rule
            else {
                panic!("transported i1 bound uses two-citation transitivity")
            };
            assert!(matches!(
                left_less_or_equal_middle.rule,
                ProofRule::Assumption { index } if index == first
            ));
            assert!(matches!(
                middle_less_or_equal_right.rule,
                ProofRule::Assumption { index } if index == second
            ));
            assert!(matches!(
                equality_proof.rule,
                ProofRule::Assumption { index } if index == equality
            ));
        }

        assert!(
            prove_canonical_integer_proposition(
                &context,
                &goal,
                &[
                    assumptions[0].clone(),
                    assumptions[1].clone(),
                    assumptions[2].clone(),
                    assumptions[3].clone(),
                    assumptions[5].clone(),
                ],
                &[],
            )
            .is_none(),
            "missing one middle bound cannot prove the complete i1 conjunction",
        );
    }

    #[test]
    fn exact_division_goal_composes_two_citation_bounds_for_both_signed_joint_conjuncts() {
        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(4).unwrap(), ScalarType::Integer(signed)),
        ])
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
        let assumptions = [
            Proposition::LessOrEqual(value(2, signed), value(3, signed)),
            Proposition::LessOrEqual(value(3, signed), integer(signed, -1)),
            Proposition::LessOrEqual(integer(signed, -127), value(4, signed)),
            Proposition::LessOrEqual(value(4, signed), value(1, signed)),
        ];
        let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
            .expect("two cited bounds prove each signed joint conjunct");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
            panic!("two-citation joint bounds select the canonical joint arm")
        };
        assert_eq!(index, 2);
        let ProofRule::ConjunctionIntroduction(conjuncts) = disjunct.rule else {
            panic!("two-citation joint bounds prove both canonical conjuncts")
        };
        for (conjunct, first, second) in [(&conjuncts[0], 0, 1), (&conjuncts[1], 2, 3)] {
            let ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } = &conjunct.rule
            else {
                panic!("joint conjunct uses two-citation transitivity")
            };
            assert!(matches!(
                left_less_or_equal_middle.rule,
                ProofRule::Assumption { index } if index == first
            ));
            assert!(matches!(
                middle_less_or_equal_right.rule,
                ProofRule::Assumption { index } if index == second
            ));
        }

        assert!(
            prove_canonical_integer_proposition(
                &context,
                &goal,
                &[
                    assumptions[0].clone(),
                    assumptions[1].clone(),
                    assumptions[2].clone(),
                ],
                &[],
            )
            .is_none(),
            "missing one citation cannot prove both joint conjuncts",
        );
    }

    #[test]
    fn exact_division_goal_composes_two_exact_transitive_bound_citations() {
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(unsigned)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(unsigned)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(unsigned)),
        ])
        .expect("three u8 values");
        let unsigned_one =
            ScalarTerm::integer(unsigned, IntegerValue::Unsigned(1)).expect("u8 one");
        let unsigned_goal = Proposition::LessOrEqual(unsigned_one.clone(), value(2, unsigned));
        let unsigned_proof = prove_canonical_integer_proposition(
            &context,
            &unsigned_goal,
            &[
                Proposition::LessOrEqual(unsigned_one, value(3, unsigned)),
                Proposition::LessOrEqual(value(3, unsigned), value(2, unsigned)),
            ],
            &[],
        )
        .expect("two exact unsigned bounds compose transitively");
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = unsigned_proof.rule
        else {
            panic!("two exact unsigned bounds use transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index: 0 }
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index: 1 }
        ));

        let signed = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(signed)),
            (ValueId::new(3).unwrap(), ScalarType::Integer(signed)),
        ])
        .expect("three i8 values");
        let signed_dividend = value(1, signed);
        let signed_divisor = value(2, signed);
        let signed_goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -2)),
            Proposition::LessOrEqual(integer(signed, 1), signed_divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(signed_divisor.clone(), integer(signed, -1)),
                Proposition::LessOrEqual(integer(signed, -127), signed_dividend),
            ]),
        ]);
        let negative_proof = prove_canonical_integer_proposition(
            &context,
            &signed_goal,
            &[
                Proposition::LessOrEqual(signed_divisor, value(3, signed)),
                Proposition::LessOrEqual(value(3, signed), integer(signed, -2)),
            ],
            &[],
        )
        .expect("two exact signed negative bounds compose transitively");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = negative_proof.rule else {
            panic!("signed negative transitivity selects its canonical arm")
        };
        assert_eq!(index, 0);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = disjunct.rule
        else {
            panic!("two exact signed negative bounds use transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index: 0 }
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index: 1 }
        ));
    }

    #[test]
    fn exact_division_goal_proves_single_definition_affine_safe_divisor() {
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
        let root_bound = Proposition::LessOrEqual(integer(signed, 0), value(3, signed));
        let definition = Proposition::Equal(
            value(2, signed),
            ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
                .expect("exact add"),
        );
        let proof = prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&root_bound),
            std::slice::from_ref(&definition),
        )
        .expect("affine root bound proves the canonical positive divisor arm");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
            panic!("affine safe divisor selects one canonical arm")
        };
        assert_eq!(index, 1);
        let ProofRule::IntegerAffineBound {
            root_bound: child,
            witness,
        } = disjunct.rule
        else {
            panic!("affine safe divisor uses the checked affine-bound rule")
        };
        assert!(matches!(child.rule, ProofRule::Assumption { index: 0 }));
        assert_eq!(witness.root, value(3, signed));
        assert_eq!(witness.target, value(2, signed));
        assert_eq!(witness.definition_axioms, vec![0]);

        assert!(
            prove_canonical_integer_proposition(
                &context,
                &goal,
                &[],
                std::slice::from_ref(&definition),
            )
            .is_none(),
            "an affine definition without its root bound is not proof authority",
        );
        let redirected = Proposition::Equal(
            value(4, signed),
            ScalarTerm::exact_integer_add(signed, value(3, signed), integer(signed, 1))
                .expect("redirected exact add"),
        );
        assert!(
            prove_canonical_integer_proposition(&context, &goal, &[root_bound], &[redirected],)
                .is_none(),
            "a definition for another target cannot prove divisor safety",
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
    fn exact_division_goal_proves_three_and_four_definition_affine_safe_divisors() {
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
        let three_step_goal = Proposition::LessOrEqual(integer(signed, 1), value(6, signed));
        let three_step_root_bound = Proposition::LessOrEqual(integer(signed, -2), value(3, signed));
        let four_step_root_bound = Proposition::LessOrEqual(integer(signed, -3), value(3, signed));
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
                value(2, signed),
                ScalarTerm::exact_integer_add(signed, value(6, signed), integer(signed, 1))
                    .expect("fourth exact add"),
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

        let proof = prove_canonical_integer_proposition(
            &context,
            &goal,
            std::slice::from_ref(&four_step_root_bound),
            &definitions,
        )
        .expect("four-definition affine word proves the positive divisor arm");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
            panic!("four-definition affine divisor selects one canonical arm")
        };
        assert_eq!(index, 1);
        let ProofRule::IntegerAffineBound { witness, .. } = disjunct.rule else {
            panic!("four-definition affine divisor uses the affine-bound rule")
        };
        assert_eq!(witness.root, value(3, signed));
        assert_eq!(witness.target, value(2, signed));
        assert_eq!(witness.definition_axioms, vec![0, 1, 2, 3]);

        assert!(
            prove_canonical_integer_proposition(
                &context,
                &goal,
                std::slice::from_ref(&four_step_root_bound),
                &definitions[..3],
            )
            .is_none(),
            "an incomplete four-definition word cannot prove divisor safety",
        );
        assert!(
            prove_canonical_integer_proposition(
                &context,
                &goal,
                &[four_step_root_bound],
                &[
                    definitions[3].clone(),
                    definitions[2].clone(),
                    definitions[1].clone(),
                    definitions[0].clone(),
                ],
            )
            .is_none(),
            "a reversed four-definition word cannot claim canonical custody",
        );
    }
}
