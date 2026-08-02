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
}

pub fn check_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
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
    check_node(context, assumptions, proof)?;
    if &proof.conclusion != goal {
        return Err(ProofError::CertificateConclusionMismatch);
    }
    Ok(())
}

fn check_node(
    context: &PropositionContext,
    assumptions: &[Proposition],
    proof: &ProofNode,
) -> Result<(), ProofError> {
    context
        .validate(&proof.conclusion)
        .map_err(ProofError::MalformedProposition)?;
    match &proof.rule {
        ProofRule::Primitive(judgment) => decide_primitive(context, &proof.conclusion, *judgment)
            .map_err(ProofError::PrimitiveJudgment),
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
                check_node(context, assumptions, conjunct)?;
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
            check_node(context, assumptions, conjunction)?;
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
            check_node(context, &nested_assumptions, body)?;
            (&body.conclusion == conclusion.as_ref())
                .then_some(())
                .ok_or(ProofError::ImplicationConclusionMismatch)
        }
        ProofRule::ImplicationElimination {
            implication,
            premise,
        } => {
            check_node(context, assumptions, implication)?;
            check_node(context, assumptions, premise)?;
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
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofError {
    MalformedProposition(psi_core::PropositionError),
    PrimitiveJudgment(KernelError),
    UnknownAssumption(usize),
    AssumptionConclusionMismatch(usize),
    UnknownConjunct(usize),
    ConjunctionArityMismatch,
    ConjunctConclusionMismatch,
    ImplicationPremiseMismatch,
    ImplicationConclusionMismatch,
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
        check_certificate(&PropositionContext::default(), &goal, &[], &proof).expect("P implies P");
    }
}
