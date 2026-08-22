//! Producer-local ordered substitution bounds for one directly landed literal.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

use super::super::super::root_bounds;

pub(super) fn prove(
    root: &ScalarTerm,
    literal: &ScalarTerm,
    equality: &ProofNode,
) -> [ProofNode; 2] {
    root_bounds::ordered(root, literal).map(|bound| {
        substitution_bound(
            bound.proposition,
            literal,
            equality,
            bound.substitution_endpoint,
        )
    })
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
