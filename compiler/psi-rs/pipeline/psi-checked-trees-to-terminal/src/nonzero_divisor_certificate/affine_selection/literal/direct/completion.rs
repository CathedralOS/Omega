//! Direct landed-literal completion for affine production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

use super::super::super::super::affine_custody::{self, DefinitionIndex};

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    literal: &ScalarTerm,
    equality: ProofNode,
) -> Option<ProofNode> {
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
                equality: Box::new(equality.clone()),
                endpoint,
            },
        };
        if let Some(proof) = affine_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
            root,
            root_bound,
        ) {
            return Some(proof);
        }
    }
    None
}
