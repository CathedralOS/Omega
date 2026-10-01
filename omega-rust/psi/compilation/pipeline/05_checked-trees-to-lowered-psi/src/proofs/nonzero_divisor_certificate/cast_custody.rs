//! Exact integer-cast custody: carry a proved root bound along one exact cast
//! spine to a goal endpoint.

use proof_admission::{
    IntegerCastChainWitness, ProofNode, ProofRule, check_certificate,
    check_integer_cast_bound_conversion, check_integer_cast_chain_witness,
    integer_cast_truth_bounds, lower_integer_math_relation,
};
use semantic_vocabulary::{IntegerType, Proposition, PropositionContext, ScalarTerm};

use super::integer_evidence::relax;

/// Recover the unique non-cast source of `target` and the first cast's ledger
/// position.
///
/// The walk is a single exact SSA spine, not a graph search. It stops at the
/// first value without an exact-cast definition and rejects ambiguous,
/// cyclic, or non-source-ordered cast words.
pub(super) fn source_root(
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<(ScalarTerm, usize)> {
    let (source, word) = cast_spine(target, None, semantic_axioms)?;
    Some((source, word[0]))
}

/// Complete `root_bound` to `goal` through the exact cast chain from `root` to
/// a value goal endpoint, left first.
pub(super) fn prove_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    let (goal_left, goal_right) = match goal {
        Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => (left, right),
        _ => return None,
    };
    [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
        .find_map(|target| {
            prove_to_target(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                &root_bound,
                target,
            )
        })
}

/// Every root [`prove_from_root`] can complete for `goal`: those with an exact
/// cast chain to a value goal endpoint, which its chain replay checks before
/// reading the bound. A chain to a target can only start at a value the
/// target's open cast walk passes, so each such value is confirmed with that
/// same replay.
pub(super) fn reaching_roots(
    goal: &Proposition,
    semantic_axioms: &[Proposition],
) -> Vec<ScalarTerm> {
    let (goal_left, goal_right) = match goal {
        Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => (left, right),
        _ => return Vec::new(),
    };
    let mut roots = Vec::new();
    for target in [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
    {
        for root in open_walk(target, semantic_axioms) {
            if !roots.contains(&root) && cast_spine(target, Some(&root), semantic_axioms).is_some()
            {
                roots.push(root);
            }
        }
    }
    roots
}

/// The values the open walk of [`cast_spine`] from `target` reaches, in walk
/// order, stopping where that walk stops.
fn open_walk(target: &ScalarTerm, semantic_axioms: &[Proposition]) -> Vec<ScalarTerm> {
    let mut current = target.clone();
    let mut reversed = Vec::new();
    let mut reached = Vec::new();
    while reversed.len() < semantic_axioms.len() {
        let mut definitions = semantic_axioms
            .iter()
            .enumerate()
            .filter_map(|(index, axiom)| {
                let Proposition::Equal(
                    output,
                    ScalarTerm::IntegerExactCast { operand, .. }
                    | ScalarTerm::IntegerWiden { operand, .. },
                ) = axiom
                else {
                    return None;
                };
                (output == &current).then(|| (index, operand.as_ref().clone()))
            });
        let Some((index, operand)) = definitions.next() else {
            break;
        };
        if definitions.next().is_some() || reversed.contains(&index) {
            break;
        }
        reversed.push(index);
        current = operand;
        reached.push(current.clone());
    }
    reached
}

/// Restate an integer literal exactly in another integer carrier.
pub(super) fn remap_integer_literal(
    literal: &ScalarTerm,
    target_type: IntegerType,
) -> Option<ScalarTerm> {
    let (source_type, value) = literal.integer_value()?;
    let value = source_type.exact_cast_value_to(target_type, value)?;
    ScalarTerm::integer(target_type, value).ok()
}

fn prove_to_target(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: &ProofNode,
    target: &ScalarTerm,
) -> Option<ProofNode> {
    // The kernel's IntegerCastBound rule is this chain replay plus the bound
    // conversion, so a chain that cannot carry the bound can never certify
    // the candidate. Checking the chain and the conversion before the full
    // certificate acceptance is exact: every candidate that could pass still
    // pays the kernel's own check.
    let (_, definition_axioms) = cast_spine(target, Some(root), semantic_axioms)?;
    let witness = IntegerCastChainWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms,
    };
    let chain = check_integer_cast_chain_witness(context, semantic_axioms, &witness).ok()?;
    let normalized = lower_integer_math_relation(goal).unwrap_or_else(|| goal.clone());
    let proof = || ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerCastBound {
            root_bound: Box::new(root_bound.clone()),
            witness: witness.clone(),
        },
    };
    if root_bound.conclusion == Proposition::Truth {
        let bounds = integer_cast_truth_bounds(&chain).ok()?;
        if bounds.contains(&normalized)
            && check_certificate(context, goal, assumptions, semantic_axioms, &proof()).is_ok()
        {
            return Some(proof());
        }
        for mapped in bounds.iter() {
            let mapped_proof = ProofNode {
                conclusion: mapped.clone(),
                rule: ProofRule::IntegerCastBound {
                    root_bound: Box::new(root_bound.clone()),
                    witness: witness.clone(),
                },
            };
            if let Some(relaxed) = relax(goal, mapped_proof)
                && check_certificate(context, goal, assumptions, semantic_axioms, &relaxed).is_ok()
            {
                return Some(relaxed);
            }
        }
        return None;
    }
    if check_integer_cast_bound_conversion(&chain, &root_bound.conclusion, &normalized).is_ok()
        && check_certificate(context, goal, assumptions, semantic_axioms, &proof()).is_ok()
    {
        return Some(proof());
    }
    None
}

/// Follow the exact SSA cast spine backward from `target`: to `root` when one
/// is named, otherwise to the first value without an exact-cast definition.
/// Returns the value reached and the source-ordered definition word.
///
/// This is intentionally not graph search: every reached value must have one
/// exact-cast definition, no row may repeat, and the word must be non-empty
/// and already canonical (strictly increasing) in the semantic ledger.
fn cast_spine(
    target: &ScalarTerm,
    root: Option<&ScalarTerm>,
    semantic_axioms: &[Proposition],
) -> Option<(ScalarTerm, Vec<usize>)> {
    let mut current = target.clone();
    let mut reversed = Vec::new();
    while root != Some(&current) {
        if reversed.len() >= semantic_axioms.len() {
            return None;
        }
        let mut definitions = semantic_axioms
            .iter()
            .enumerate()
            .filter_map(|(index, axiom)| {
                let Proposition::Equal(
                    output,
                    ScalarTerm::IntegerExactCast { operand, .. }
                    | ScalarTerm::IntegerWiden { operand, .. },
                ) = axiom
                else {
                    return None;
                };
                (output == &current).then(|| (index, operand.as_ref().clone()))
            });
        let Some((index, operand)) = definitions.next() else {
            // A named root must be reached; an open walk ends at the source.
            if root.is_some() {
                return None;
            }
            break;
        };
        if definitions.next().is_some() || reversed.contains(&index) {
            return None;
        }
        reversed.push(index);
        current = operand;
    }
    reversed.reverse();
    (!reversed.is_empty() && reversed.windows(2).all(|pair| pair[0] < pair[1]))
        .then_some((current, reversed))
}
