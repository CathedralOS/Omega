use psi_core::{Proposition, PropositionContext};

use crate::{KernelError, PrimitiveJudgment, decide_primitive};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofNode {
    pub conclusion: Proposition,
    pub rule: ProofRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofRule {
    Primitive(PrimitiveJudgment),
    /// Cite one verifier-reconstructed semantic axiom.
    SemanticAxiom {
        index: usize,
    },
    Assumption {
        index: usize,
    },
    ConjunctionIntroduction(Vec<ProofNode>),
    ConjunctionElimination {
        conjunction: Box<ProofNode>,
        conjunct: usize,
    },
    ImplicationIntroduction {
        body: Box<ProofNode>,
    },
    ImplicationElimination {
        implication: Box<ProofNode>,
        premise: Box<ProofNode>,
    },
    EqualityTransitivity {
        left_equals_middle: Box<ProofNode>,
        middle_equals_right: Box<ProofNode>,
    },
}

pub fn check_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
) -> Result<(), ProofError> {
    context
        .validate(goal)
        .map_err(ProofError::MalformedProposition)?;
    for assumption in assumptions {
        context
            .validate(assumption)
            .map_err(ProofError::MalformedProposition)?;
    }
    for axiom in semantic_axioms {
        context
            .validate(axiom)
            .map_err(ProofError::MalformedProposition)?;
    }
    check_node(context, assumptions, semantic_axioms, proof)?;
    if &proof.conclusion != goal {
        return Err(ProofError::CertificateConclusionMismatch);
    }
    Ok(())
}

fn check_node(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
) -> Result<(), ProofError> {
    context
        .validate(&proof.conclusion)
        .map_err(ProofError::MalformedProposition)?;
    match &proof.rule {
        ProofRule::Primitive(judgment) => decide_primitive(context, &proof.conclusion, *judgment)
            .map_err(ProofError::PrimitiveJudgment),
        ProofRule::SemanticAxiom { index } => {
            let axiom = semantic_axioms
                .get(*index)
                .ok_or(ProofError::UnknownSemanticAxiom(*index))?;
            (axiom == &proof.conclusion)
                .then_some(())
                .ok_or(ProofError::SemanticAxiomConclusionMismatch(*index))
        }
        ProofRule::Assumption { index } => {
            let assumption = assumptions
                .get(*index)
                .ok_or(ProofError::UnknownAssumption(*index))?;
            (assumption == &proof.conclusion)
                .then_some(())
                .ok_or(ProofError::AssumptionConclusionMismatch(*index))
        }
        ProofRule::ConjunctionIntroduction(conjuncts) => {
            let Proposition::Conjunction(expected) = &proof.conclusion else {
                return Err(ProofError::RuleConclusionMismatch(
                    "conjunction introduction",
                ));
            };
            if expected.len() != conjuncts.len() {
                return Err(ProofError::ConjunctionArityMismatch);
            }
            for (expected, conjunct) in expected.iter().zip(conjuncts) {
                check_node(context, assumptions, semantic_axioms, conjunct)?;
                if &conjunct.conclusion != expected {
                    return Err(ProofError::ConjunctConclusionMismatch);
                }
            }
            Ok(())
        }
        ProofRule::ConjunctionElimination {
            conjunction,
            conjunct,
        } => {
            check_node(context, assumptions, semantic_axioms, conjunction)?;
            let Proposition::Conjunction(conjuncts) = &conjunction.conclusion else {
                return Err(ProofError::RulePremiseMismatch("conjunction elimination"));
            };
            let selected = conjuncts
                .get(*conjunct)
                .ok_or(ProofError::UnknownConjunct(*conjunct))?;
            (selected == &proof.conclusion)
                .then_some(())
                .ok_or(ProofError::ConjunctConclusionMismatch)
        }
        ProofRule::ImplicationIntroduction { body } => {
            let Proposition::Implication {
                premise,
                conclusion,
            } = &proof.conclusion
            else {
                return Err(ProofError::RuleConclusionMismatch(
                    "implication introduction",
                ));
            };
            let mut nested_assumptions = assumptions.to_vec();
            nested_assumptions.push((**premise).clone());
            check_node(context, &nested_assumptions, semantic_axioms, body)?;
            (&body.conclusion == conclusion.as_ref())
                .then_some(())
                .ok_or(ProofError::ImplicationConclusionMismatch)
        }
        ProofRule::ImplicationElimination {
            implication,
            premise,
        } => {
            check_node(context, assumptions, semantic_axioms, implication)?;
            check_node(context, assumptions, semantic_axioms, premise)?;
            let Proposition::Implication {
                premise: required,
                conclusion,
            } = &implication.conclusion
            else {
                return Err(ProofError::RulePremiseMismatch("implication elimination"));
            };
            if premise.conclusion != **required {
                return Err(ProofError::ImplicationPremiseMismatch);
            }
            (&proof.conclusion == conclusion.as_ref())
                .then_some(())
                .ok_or(ProofError::ImplicationConclusionMismatch)
        }
        ProofRule::EqualityTransitivity {
            left_equals_middle,
            middle_equals_right,
        } => {
            check_node(context, assumptions, semantic_axioms, left_equals_middle)?;
            check_node(context, assumptions, semantic_axioms, middle_equals_right)?;
            match (
                &left_equals_middle.conclusion,
                &middle_equals_right.conclusion,
                &proof.conclusion,
            ) {
                (
                    Proposition::Equal(left, first_middle),
                    Proposition::Equal(second_middle, right),
                    Proposition::Equal(expected_left, expected_right),
                ) => {
                    if first_middle != second_middle {
                        return Err(ProofError::EqualityMiddleMismatch);
                    }
                    if left != expected_left || right != expected_right {
                        return Err(ProofError::EqualityConclusionMismatch);
                    }
                    Ok(())
                }
                (
                    Proposition::ContentConservation(left_equation),
                    Proposition::ContentConservation(right_equation),
                    Proposition::ContentConservation(expected),
                ) => {
                    if left_equation.algebra() != right_equation.algebra()
                        || left_equation.algebra() != expected.algebra()
                    {
                        return Err(ProofError::EqualityAlgebraMismatch);
                    }
                    let left_terms = [left_equation.left(), left_equation.right()];
                    let right_terms = [right_equation.left(), right_equation.right()];
                    let mut shared_middle = false;
                    for (left_index, left_term) in left_terms.iter().enumerate() {
                        for (right_index, right_term) in right_terms.iter().enumerate() {
                            if left_term != right_term {
                                continue;
                            }
                            shared_middle = true;
                            let composed = psi_core::ContentConservation::new(
                                left_equation.algebra().clone(),
                                left_terms[1 - left_index].clone(),
                                right_terms[1 - right_index].clone(),
                            );
                            if &composed == expected {
                                return Ok(());
                            }
                        }
                    }
                    Err(if shared_middle {
                        ProofError::EqualityConclusionMismatch
                    } else {
                        ProofError::EqualityMiddleMismatch
                    })
                }
                _ => Err(ProofError::RulePremiseMismatch("equality transitivity")),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofError {
    MalformedProposition(psi_core::PropositionError),
    PrimitiveJudgment(KernelError),
    UnknownSemanticAxiom(usize),
    SemanticAxiomConclusionMismatch(usize),
    UnknownAssumption(usize),
    AssumptionConclusionMismatch(usize),
    UnknownConjunct(usize),
    ConjunctionArityMismatch,
    ConjunctConclusionMismatch,
    ImplicationPremiseMismatch,
    ImplicationConclusionMismatch,
    EqualityMiddleMismatch,
    EqualityAlgebraMismatch,
    EqualityConclusionMismatch,
    CertificateConclusionMismatch,
    RuleConclusionMismatch(&'static str),
    RulePremiseMismatch(&'static str),
}

impl std::fmt::Display for ProofError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProofError {}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::PropositionId;

    #[test]
    fn implication_certificate_is_checked_structurally() {
        let proposition = Proposition::Atom(PropositionId::new(1).expect("atom identity"));
        let goal = Proposition::Implication {
            premise: Box::new(proposition.clone()),
            conclusion: Box::new(proposition.clone()),
        };
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ImplicationIntroduction {
                body: Box::new(ProofNode {
                    conclusion: proposition,
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
            .expect("P implies P");
    }

    #[test]
    fn semantic_equalities_compose_only_through_the_same_middle_term() {
        use psi_core::{IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId};

        let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let a = ScalarTerm::value(ValueId::new(1).expect("a"), ScalarType::Integer(integer));
        let b = ScalarTerm::value(ValueId::new(2).expect("b"), ScalarType::Integer(integer));
        let seven = ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("seven");
        let axioms = vec![
            Proposition::Equal(a.clone(), b.clone()),
            Proposition::Equal(b.clone(), seven.clone()),
        ];
        let goal = Proposition::Equal(a.clone(), seven.clone());
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(ProofNode {
                    conclusion: axioms[0].clone(),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: axioms[1].clone(),
                    rule: ProofRule::SemanticAxiom { index: 1 },
                }),
            },
        };
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).expect("a"), ScalarType::Integer(integer)),
            (ValueId::new(2).expect("b"), ScalarType::Integer(integer)),
        ])
        .expect("context");
        check_certificate(&context, &goal, &[], &axioms, &proof).expect("transitive equality");
    }

    #[test]
    fn canonical_content_equalities_compose_through_either_orientation() {
        use psi_core::{
            ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
            ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity,
            ContentStructuralPlace, ContentTerm, PlaceId, StructuralPlaceKind,
        };

        let root = PlaceId::new(1).expect("place");
        let projection = ContentProjectionIdentity {
            domain: ContentDomainId::new(2).expect("domain"),
            projection_fingerprint: 3,
        };
        let term = |field: &str| ContentTerm::Projection {
            projection,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root,
                segments: vec![ContentPlaceSegment::Field(field.to_owned())],
            },
        };
        let algebra = ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "Byte".to_owned(),
        };
        let a = term("a");
        let b = term("b");
        let c = term("c");
        let axioms = vec![
            Proposition::ContentConservation(ContentConservation::new(
                algebra.clone(),
                a.clone(),
                c.clone(),
            )),
            Proposition::ContentConservation(ContentConservation::new(
                algebra.clone(),
                b.clone(),
                c,
            )),
        ];
        let goal = Proposition::ContentConservation(ContentConservation::new(algebra, a, b));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(ProofNode {
                    conclusion: axioms[0].clone(),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: axioms[1].clone(),
                    rule: ProofRule::SemanticAxiom { index: 1 },
                }),
            },
        };
        let context = PropositionContext::from_value_types_and_places(
            [],
            [(
                root,
                StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            )],
        )
        .expect("context");

        check_certificate(&context, &goal, &[], &axioms, &proof)
            .expect("canonical equality orientation must not erase transitivity");
    }
}
