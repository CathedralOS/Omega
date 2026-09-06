//! Prior-fact custody and closed-order primitives for integer certificates.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

#[derive(Clone, Copy)]
pub(super) enum Citation {
    Assumption(usize),
    SemanticAxiom(usize),
}

pub(super) fn cited_facts<'a>(
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

pub(super) struct ProjectedFact<'a> {
    citation: Citation,
    root: &'a Proposition,
    pub(super) projection: Vec<usize>,
    pub(super) proposition: &'a Proposition,
}

impl ProjectedFact<'_> {
    pub(super) fn proof(&self) -> ProofNode {
        let mut proof = self.citation.proof(self.root);
        for &conjunct in &self.projection {
            let Proposition::Conjunction(parts) = &proof.conclusion else {
                unreachable!("projection follows retained conjunction children")
            };
            proof = ProofNode {
                conclusion: parts[conjunct].clone(),
                rule: ProofRule::ConjunctionElimination {
                    conjunction: Box::new(proof),
                    conjunct,
                },
            };
        }
        proof
    }
}

/// Project unconditional conjunction leaves without copying their proof trees.
/// Disjunction branches and implication premises never become ambient facts.
pub(super) fn projected_facts<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> Vec<ProjectedFact<'a>> {
    let mut facts = Vec::new();
    for (citation, root) in cited_facts(assumptions, semantic_axioms) {
        let mut pending = vec![(root, Vec::new())];
        while let Some((proposition, projection)) = pending.pop() {
            if let Proposition::Conjunction(parts) = proposition {
                for (conjunct, part) in parts.iter().enumerate().rev() {
                    let mut child_projection = projection.clone();
                    child_projection.push(conjunct);
                    pending.push((part, child_projection));
                }
            } else {
                facts.push(ProjectedFact {
                    citation,
                    root,
                    projection,
                    proposition,
                });
            }
        }
    }
    facts
}

pub(super) fn closed_integer_relation(conclusion: Proposition) -> Option<ProofNode> {
    let (strict, left, right) = match &conclusion {
        Proposition::LessThan(left, right) => (true, left, right),
        Proposition::LessOrEqual(left, right) => (false, left, right),
        _ => return None,
    };
    let (left_type, left) = left.integer_value()?;
    let (right_type, right) = right.integer_value()?;
    (left_type == right_type
        && left_type.compare(left, right).is_some_and(|order| {
            if strict {
                order.is_lt()
            } else {
                !order.is_gt()
            }
        }))
    .then_some(ProofNode {
        conclusion,
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    })
}

impl Citation {
    pub(super) fn proof(self, conclusion: &Proposition) -> ProofNode {
        ProofNode {
            conclusion: conclusion.clone(),
            rule: match self {
                Self::Assumption(index) => ProofRule::Assumption { index },
                Self::SemanticAxiom(index) => ProofRule::SemanticAxiom { index },
            },
        }
    }
}
