//! Fail-closed entry from untrusted integer proof selection to the kernel.

use std::collections::BTreeSet;

use proof_admission::{ProofNode, accept_certificate_with_machine_parameters};
use semantic_vocabulary::{Proposition, PropositionContext, ValueId};

#[cfg(test)]
use proof_admission::check_certificate;

use super::integer_selection;

#[cfg(test)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let proof = select(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &BTreeSet::new(),
    )?;
    check_certificate(context, goal, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}

pub(super) fn prove_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    let proof = select(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    )?;
    accept_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        &proof,
    )
    .is_ok()
    .then_some(proof)
}

/// Integer selection splits connectives only over its own producers, so a
/// remainder's definedness disjunction whose `1 <= divisor` disjunct only
/// value transport reaches would be refused. After every route has answered
/// the whole goal, split a connective and give each atomic part the two
/// routes that selection's split never consulted.
fn select(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    let transported = |goal: &Proposition| {
        super::value_transport::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
        )
        .or_else(|| {
            super::predicate_conversion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                machine_parameter_values,
            )
        })
    };
    integer_selection::build_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    )
    .or_else(|| transported(goal))
    .or_else(|| split(goal, &transported))
}

/// Whole-connective searches repeat for every part they contain; only the
/// atomic leaves are asked again.
fn split(
    goal: &Proposition,
    atomic: &impl Fn(&Proposition) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let part = |part: &Proposition| match part {
        Proposition::Conjunction(_) | Proposition::Disjunction(_) => split(part, atomic),
        _ => atomic(part),
    };
    match goal {
        Proposition::Conjunction(parts) => integer_selection::prove_conjunction(goal, parts, part),
        Proposition::Disjunction(parts) => integer_selection::prove_disjunction(goal, parts, part),
        _ => None,
    }
}
