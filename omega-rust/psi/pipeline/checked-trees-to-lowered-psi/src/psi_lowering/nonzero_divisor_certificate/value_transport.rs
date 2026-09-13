//! Rejoin nested scalar computations through their established equations.
//!
//! Predicate case analysis can expand a compact computation into many Boolean
//! alternatives. Transport instead cites the exact equations reachable from
//! the goal, preserving every constructor and operand identity. The simplified
//! question is proved under the original premises, then a carried inference
//! rejoins it to the original question. No ambient citation is rewritten.

use std::collections::BTreeSet;

use proof_admission::{ProofNode, ProofRule, check_value_equality_denotation};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};

use super::integer_evidence::projected_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameters: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    let facts = projected_facts(assumptions, semantic_axioms);
    let mut pending = Vec::new();
    goal.visit_value_ids(|value| pending.push(value));
    let mut visited = BTreeSet::new();
    let mut equalities = Vec::new();
    let mut remaining = 4096usize;
    while let Some(value) = pending.pop() {
        remaining = remaining.checked_sub(1)?;
        if !visited.insert(value) {
            continue;
        }
        // Prefer a defining equation over an alias in the opposite direction.
        // In particular an incoming formal must not be expanded back into the
        // result whose definition already depends on that formal.
        let fact = facts.iter().find(|fact| {
            matches!(fact.proposition,
                Proposition::Equal(left @ ScalarTerm::Value { id, .. }, right)
                    if *id == value && left != right
            )
        });
        let Some(fact) = fact else { continue };
        fact.proposition.visit_value_ids(|dependency| {
            if dependency != value && !visited.contains(&dependency) {
                pending.push(dependency);
            }
        });
        equalities.push(fact);
    }
    if equalities.is_empty() {
        return None;
    }
    let transported = check_value_equality_denotation(
        context,
        goal,
        equalities.iter().map(|fact| fact.proposition),
    )
    .ok()?;
    if &transported == goal {
        return None;
    }
    let premise = super::integer_selection::build_with_machine_parameters(
        context,
        &transported,
        assumptions,
        semantic_axioms,
        machine_parameters,
    )?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(premise),
            equalities: equalities.iter().map(|fact| fact.proof()).collect(),
        },
    })
}
