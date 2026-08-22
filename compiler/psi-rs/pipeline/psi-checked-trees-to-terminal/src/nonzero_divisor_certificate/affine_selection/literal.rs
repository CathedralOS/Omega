//! Direct and one-alias landed-literal affine evidence construction.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

use super::super::affine_custody;
use super::super::integer_evidence::cited_facts;

mod alias;

pub(super) fn prove_landed_literal_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(left, right) = equality else {
            continue;
        };
        for (root, literal) in [(left, right), (right, left)] {
            if !matches!(root, psi_core::ScalarTerm::Value { .. }) {
                continue;
            }
            let Some((integer_type, _)) = literal.integer_value() else {
                continue;
            };
            if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                continue;
            }
            let reflexive = Proposition::LessOrEqual(literal.clone(), literal.clone());
            for (root_bound, endpoint) in [
                (Proposition::LessOrEqual(literal.clone(), root.clone()), 1),
                (Proposition::LessOrEqual(root.clone(), literal.clone()), 0),
            ] {
                let root_bound = ProofNode {
                    conclusion: root_bound,
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(ProofNode {
                            conclusion: reflexive.clone(),
                            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                        }),
                        equality: Box::new(citation.proof(equality)),
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

    alias::prove(context, goal, assumptions, semantic_axioms)
}
