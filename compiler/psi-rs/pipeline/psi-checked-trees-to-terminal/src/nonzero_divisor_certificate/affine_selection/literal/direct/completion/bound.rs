//! Producer-local ordered substitution bounds for one directly landed literal.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

pub(super) fn prove(
    root: &ScalarTerm,
    literal: &ScalarTerm,
    equality: &ProofNode,
) -> [ProofNode; 2] {
    [
        substitution_bound(
            Proposition::LessOrEqual(literal.clone(), root.clone()),
            literal,
            equality,
            1,
        ),
        substitution_bound(
            Proposition::LessOrEqual(root.clone(), literal.clone()),
            literal,
            equality,
            0,
        ),
    ]
}

fn substitution_bound(
    conclusion: Proposition,
    literal: &ScalarTerm,
    equality: &ProofNode,
    endpoint: usize,
) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(ProofNode {
                conclusion: Proposition::LessOrEqual(literal.clone(), literal.clone()),
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
            equality: Box::new(equality.clone()),
            endpoint,
        },
    }
}
