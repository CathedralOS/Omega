//! Producer-local ordered substitution bounds for one landed literal alias.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

pub(super) fn prove(
    root: &ScalarTerm,
    alias: &ScalarTerm,
    literal: &ScalarTerm,
    outer_equality: &ProofNode,
    inner_equality: &ProofNode,
) -> [ProofNode; 2] {
    [
        substitution_bound(
            Proposition::LessOrEqual(literal.clone(), alias.clone()),
            Proposition::LessOrEqual(literal.clone(), root.clone()),
            literal,
            outer_equality,
            inner_equality,
            1,
        ),
        substitution_bound(
            Proposition::LessOrEqual(alias.clone(), literal.clone()),
            Proposition::LessOrEqual(root.clone(), literal.clone()),
            literal,
            outer_equality,
            inner_equality,
            0,
        ),
    ]
}

fn substitution_bound(
    alias_bound: Proposition,
    root_bound: Proposition,
    literal: &ScalarTerm,
    outer_equality: &ProofNode,
    inner_equality: &ProofNode,
    endpoint: usize,
) -> ProofNode {
    let alias_bound = ProofNode {
        conclusion: alias_bound,
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(ProofNode {
                conclusion: Proposition::LessOrEqual(literal.clone(), literal.clone()),
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
            equality: Box::new(inner_equality.clone()),
            endpoint,
        },
    };
    ProofNode {
        conclusion: root_bound,
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(alias_bound),
            equality: Box::new(outer_equality.clone()),
            endpoint,
        },
    }
}
