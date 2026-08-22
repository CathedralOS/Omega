//! One-intermediate-alias literal landing for affine certificate production.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

use super::super::super::affine_custody;
use super::super::super::integer_evidence::cited_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (outer_citation, outer_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(outer_left, outer_right) = outer_equality else {
            continue;
        };
        for (root, alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
            if root == alias
                || !matches!(root, psi_core::ScalarTerm::Value { .. })
                || !matches!(alias, psi_core::ScalarTerm::Value { .. })
                || root.scalar_type() != alias.scalar_type()
            {
                continue;
            }
            for (inner_citation, inner_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(outer_equality, inner_equality) {
                    continue;
                }
                let Proposition::Equal(inner_left, inner_right) = inner_equality else {
                    continue;
                };
                let literal = if inner_left == alias {
                    inner_right
                } else if inner_right == alias {
                    inner_left
                } else {
                    continue;
                };
                let Some((integer_type, _)) = literal.integer_value() else {
                    continue;
                };
                if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                    continue;
                }
                let reflexive = Proposition::LessOrEqual(literal.clone(), literal.clone());
                for (alias_bound, root_bound, endpoint) in [
                    (
                        Proposition::LessOrEqual(literal.clone(), alias.clone()),
                        Proposition::LessOrEqual(literal.clone(), root.clone()),
                        1,
                    ),
                    (
                        Proposition::LessOrEqual(alias.clone(), literal.clone()),
                        Proposition::LessOrEqual(root.clone(), literal.clone()),
                        0,
                    ),
                ] {
                    let alias_bound = ProofNode {
                        conclusion: alias_bound,
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(ProofNode {
                                conclusion: reflexive.clone(),
                                rule: ProofRule::Primitive(
                                    PrimitiveJudgment::ClosedIntegerRelation,
                                ),
                            }),
                            equality: Box::new(inner_citation.proof(inner_equality)),
                            endpoint,
                        },
                    };
                    let root_bound = ProofNode {
                        conclusion: root_bound,
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(alias_bound),
                            equality: Box::new(outer_citation.proof(outer_equality)),
                            endpoint,
                        },
                    };
                    if let Some(proof) = affine_custody::prove_from_root(
                        context,
                        goal,
                        assumptions,
                        semantic_axioms,
                        root,
                        root_bound,
                    ) {
                        return Some(proof);
                    }
                }
            }
        }
    }
    None
}
