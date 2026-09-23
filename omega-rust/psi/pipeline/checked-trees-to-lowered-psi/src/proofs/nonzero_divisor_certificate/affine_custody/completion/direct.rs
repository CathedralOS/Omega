//! Producer-local direct completion of one affine witness.

use std::rc::Rc;

use proof_admission::{
    CheckedIntegerAffineForm, IntegerAffineWitness, ProofNode, ProofRule, check_certificate,
    lower_integer_math_relation,
};
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::DefinitionIndex;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root_bound: &ProofNode,
    witness: &IntegerAffineWitness,
    form: &Rc<CheckedIntegerAffineForm>,
) -> Option<ProofNode> {
    // The kernel's IntegerAffineBound check is this witness check plus the
    // bound conversion; the caller already replayed the witness, so the
    // conversion alone decides whether the certificate can pass. A word whose
    // mapped bound misses the goal can never certify it — skipping the full
    // acceptance for those candidates is exact, while every candidate that
    // could pass still pays the kernel's own check. Both the mapped bound and
    // the truth bound set are memoized: the same word meets the same root
    // bound under many goals.
    let normalized = lower_integer_math_relation(goal).unwrap_or_else(|| goal.clone());
    let converts = if root_bound.conclusion == Proposition::Truth {
        definitions
            .affine_truth(form)
            .is_some_and(|bounds| bounds.contains(&normalized))
    } else {
        definitions
            .affine_mapped(form, &root_bound.conclusion)
            .is_some_and(|mapped| mapped == normalized)
    };
    if !converts {
        return None;
    }
    let direct = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound.clone()),
            witness: witness.clone(),
        },
    };
    check_certificate(context, goal, assumptions, semantic_axioms, &direct)
        .is_ok()
        .then_some(direct)
}
