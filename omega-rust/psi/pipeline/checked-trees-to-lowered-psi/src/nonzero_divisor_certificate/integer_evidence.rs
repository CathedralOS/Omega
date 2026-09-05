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

pub(super) fn closed_integer_relation(conclusion: Proposition) -> Option<ProofNode> {
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
