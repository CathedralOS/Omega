//! Affine-witness completion for canonical order certificates.
//!
//! Evidence selection remains in the root selection. This module owns the
//! bounded witness frontier, the exact mapped bound, and the optional closed
//! relaxation that complete one already-constructed affine root bound.
//!
//! A witness pairs one definition word from the root to a goal endpoint with
//! the literal rows its landed siblings cite. Words come from the frontier in
//! source order and each goal endpoint is tried left first, so the first
//! completed witness decides the certificate.

use std::rc::Rc;

use proof_admission::{
    CheckedIntegerAffineForm, IntegerAffineWitness, ProofNode, ProofRule, check_certificate,
    lower_integer_math_relation,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::integer_evidence::closed_integer_relation;

mod definition_index;
mod frontier;

pub(super) use definition_index::{CheckedWord, DefinitionIndex};
pub(super) use frontier::{
    definition_words_to_target, definition_words_to_targets, literal_axioms,
};

/// Complete `root_bound` to the goal through any checked affine word from
/// `root` to a value goal endpoint.
pub(super) fn prove_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    goal_targets(goal).find_map(|target| {
        find_witness(
            context,
            semantic_axioms,
            definitions,
            root,
            target,
            |_| true,
            |definitions, witness| {
                complete(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    definitions,
                    &root_bound,
                    witness,
                )
            },
        )
    })
}

/// Complete `root_bound` to the goal only through words whose every
/// definition and literal row lies strictly after `minimum_axiom`.
#[allow(clippy::too_many_arguments)]
pub(super) fn prove_from_root_after(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    minimum_axiom: usize,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    goal_targets(goal).find_map(|target| {
        find_witness(
            context,
            semantic_axioms,
            definitions,
            root,
            target,
            |word| starts_after(word, minimum_axiom),
            |definitions, witness| {
                cites_only(&witness, |index| index > minimum_axiom)
                    .then(|| {
                        complete(
                            context,
                            goal,
                            assumptions,
                            semantic_axioms,
                            definitions,
                            &root_bound,
                            witness,
                        )
                    })
                    .flatten()
            },
        )
    })
}

/// Map `root_bound` through one word from `root` to `target` whose every
/// definition and literal row lies strictly before `maximum_axiom`, and
/// return the checked mapped bound itself rather than a goal.
#[allow(clippy::too_many_arguments)]
pub(super) fn prove_mapped_to_target_before(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
    root_bound: &ProofNode,
) -> Option<ProofNode> {
    find_witness(
        context,
        semantic_axioms,
        definitions,
        root,
        target,
        |word| ends_before(word, maximum_axiom),
        |definitions, witness| {
            if !cites_only(&witness, |index| index < maximum_axiom) {
                return None;
            }
            let form = definitions.affine_form(context, semantic_axioms, &witness)?;
            let conclusion = definitions.affine_mapped(&form, &root_bound.conclusion)?;
            let proof = ProofNode {
                conclusion: conclusion.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(root_bound.clone()),
                    witness,
                },
            };
            check_certificate(context, &conclusion, assumptions, semantic_axioms, &proof)
                .is_ok()
                .then_some(proof)
        },
    )
}

/// Whether some word from `root` reaches a value goal endpoint starting
/// strictly after `minimum_axiom`.
pub(super) fn has_target_after(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    minimum_axiom: usize,
) -> bool {
    goal_targets(goal).any(|target| {
        definition_words_to_target(context, semantic_axioms, definitions, root, target)
            .iter()
            .any(|word| starts_after(word, minimum_axiom))
    })
}

/// Whether some word from `root` reaches `target` ending strictly before
/// `maximum_axiom`.
pub(super) fn has_target_before(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
) -> bool {
    definition_words_to_target(context, semantic_axioms, definitions, root, target)
        .iter()
        .any(|word| ends_before(word, maximum_axiom))
}

/// The value endpoints of a `LessOrEqual` goal, left first.
fn goal_targets(goal: &Proposition) -> impl Iterator<Item = &ScalarTerm> {
    let endpoints = match goal {
        Proposition::LessOrEqual(left, right) => [Some(left), Some(right)],
        _ => [None, None],
    };
    endpoints
        .into_iter()
        .flatten()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
}

fn starts_after(word: &[usize], minimum_axiom: usize) -> bool {
    word.first().is_some_and(|&index| index > minimum_axiom)
}

fn ends_before(word: &[usize], maximum_axiom: usize) -> bool {
    word.last().is_some_and(|&index| index < maximum_axiom)
}

fn cites_only(witness: &IntegerAffineWitness, admit: impl Fn(usize) -> bool) -> bool {
    witness.definition_axioms.iter().all(|&index| admit(index))
        && witness
            .literal_axioms
            .iter()
            .flatten()
            .all(|&index| admit(index))
}

/// Offer each admitted word from `root` to `target`, in frontier order and
/// with its landed literal rows resolved, as one witness to `complete`.
fn find_witness<T>(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    admit: impl Fn(&[usize]) -> bool,
    mut complete: impl FnMut(&DefinitionIndex, IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
    let definition_words =
        definition_words_to_target(context, semantic_axioms, definitions, root, target);
    definition_words.iter().find_map(|definition_axioms| {
        if !admit(definition_axioms) {
            return None;
        }
        let literal_axioms = literal_axioms(
            context,
            semantic_axioms,
            definitions,
            root,
            definition_axioms,
            target,
        )?;
        complete(
            definitions,
            IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                literal_axioms,
                definition_axioms: definition_axioms.clone(),
            },
        )
    })
}

/// Complete one enumerated witness: the exact mapped bound when it converts
/// to the goal, otherwise one closed endpoint relaxation of it.
fn complete(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    // The kernel's IntegerAffineBound rule replays this same witness check
    // before anything else, so a witness that fails here cannot produce an
    // acceptable certificate below. The shared table replays it once per
    // distinct word rather than once per candidate, and the checked form
    // feeds both the direct conversion and the relaxed completion.
    let form = definitions.affine_form(context, semantic_axioms, &witness)?;
    if let Some(direct) = complete_directly(
        context,
        goal,
        assumptions,
        semantic_axioms,
        definitions,
        root_bound,
        &witness,
        &form,
    ) {
        return Some(direct);
    }
    let mapped_bound = definitions.affine_mapped(&form, &root_bound.conclusion)?;
    let affine = ProofNode {
        conclusion: mapped_bound,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound.clone()),
            witness,
        },
    };
    let relaxed = relax_mapped_bound(goal, affine)?;
    check_certificate(context, goal, assumptions, semantic_axioms, &relaxed)
        .is_ok()
        .then_some(relaxed)
}

#[allow(clippy::too_many_arguments)]
fn complete_directly(
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

/// Close the one side a mapped affine bound does not share with the goal by
/// a closed literal relation: a shared right side first, then a shared left.
fn relax_mapped_bound(goal: &Proposition, affine: ProofNode) -> Option<ProofNode> {
    let Proposition::LessOrEqual(affine_left, affine_right) = &affine.conclusion else {
        return None;
    };
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let (left_less_or_equal_middle, middle_less_or_equal_right) = if goal_right == affine_right {
        let bridge = closed_integer_relation(Proposition::LessOrEqual(
            goal_left.clone(),
            affine_left.clone(),
        ))?;
        (bridge, affine)
    } else if goal_left == affine_left {
        let bridge = closed_integer_relation(Proposition::LessOrEqual(
            affine_right.clone(),
            goal_right.clone(),
        ))?;
        (affine, bridge)
    } else {
        return None;
    };
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left_less_or_equal_middle),
            middle_less_or_equal_right: Box::new(middle_less_or_equal_right),
        },
    })
}
