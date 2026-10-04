//! Compose scoped mathematical equalities with checked ring identities.
//!
//! SSA transport preserves mathematical constructors, so a retained equality
//! may differ from a goal by exact arithmetic on either endpoint. Cite that
//! equality unchanged and prove both connecting identities with the existing
//! primitive. No normalization result becomes an ambient axiom. This bounded
//! producer searches one retained equality at a time in the canonical forward
//! orientation supported by the existing equality-transitivity checker.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, decide_primitive};
use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext};

use super::super::integer_evidence::projected_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::IntegerMathEqual(..) = goal else {
        return None;
    };
    for fact in projected_facts(assumptions, semantic_axioms) {
        super::super::search_context::spend(1)?;
        let Proposition::IntegerMathEqual(..) = fact.proposition else {
            continue;
        };
        for citation in super::super::value_transport::citation_forms(
            context,
            fact.proof(),
            assumptions,
            semantic_axioms,
        ) {
            if let Some(proof) = compose_citation(context, goal, citation) {
                return Some(proof);
            }
        }
    }
    None
}

fn compose_citation(
    context: &PropositionContext,
    goal: &Proposition,
    citation: ProofNode,
) -> Option<ProofNode> {
    let (Proposition::IntegerMathEqual(left, right), Proposition::IntegerMathEqual(first, second)) =
        (goal, &citation.conclusion)
    else {
        return None;
    };
    let prefix = (left != first).then(|| identity(context, left, first));
    let suffix = (second != right).then(|| identity(context, second, right));
    let mut proof = citation;
    if let Some(prefix) = prefix {
        proof = compose(prefix?, proof)?;
    }
    if let Some(suffix) = suffix {
        proof = compose(proof, suffix?)?;
    }
    (proof.conclusion == *goal).then_some(proof)
}

fn identity(
    context: &PropositionContext,
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
) -> Option<ProofNode> {
    // Mathematical equality uses canonical operands. Do not manufacture a
    // reversed intermediate proposition to satisfy the directed rule.
    if left > right {
        return None;
    }
    super::super::search_context::spend(1)?;
    let conclusion = Proposition::IntegerMathEqual(left.clone(), right.clone());
    decide_primitive(
        context,
        &conclusion,
        PrimitiveJudgment::ClosedIntegerRelation,
    )
    .ok()?;
    Some(ProofNode {
        conclusion,
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    })
}

fn compose(first: ProofNode, second: ProofNode) -> Option<ProofNode> {
    let (
        Proposition::IntegerMathEqual(left, first_middle),
        Proposition::IntegerMathEqual(second_middle, right),
    ) = (&first.conclusion, &second.conclusion)
    else {
        return None;
    };
    if first_middle != second_middle || left > right {
        return None;
    }
    Some(ProofNode {
        conclusion: Proposition::IntegerMathEqual(left.clone(), right.clone()),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(first),
            middle_equals_right: Box::new(second),
        },
    })
}
