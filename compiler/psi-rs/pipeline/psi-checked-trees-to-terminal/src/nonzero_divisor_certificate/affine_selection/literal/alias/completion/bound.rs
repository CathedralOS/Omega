//! Producer-local ordered substitution bounds for one landed literal alias.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

use super::super::super::root_bounds;

pub(super) fn prove(
    root: &ScalarTerm,
    alias: &ScalarTerm,
    literal: &ScalarTerm,
    outer_equality: &ProofNode,
    inner_equality: &ProofNode,
) -> [ProofNode; 2] {
    let alias_bounds = root_bounds::ordered(alias, literal);
    let root_bounds = root_bounds::ordered(root, literal);
    [
        substitution_bound(
            alias_bounds[0].proposition.clone(),
            root_bounds[0].proposition.clone(),
            literal,
            outer_equality,
            inner_equality,
            root_bounds[0].substitution_endpoint,
        ),
        substitution_bound(
            alias_bounds[1].proposition.clone(),
            root_bounds[1].proposition.clone(),
            literal,
            outer_equality,
            inner_equality,
            root_bounds[1].substitution_endpoint,
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
