//! Select case assumptions through the goal's transitive value dependencies.

use std::collections::BTreeSet;

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};

use super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) struct ProjectedFact<'a> {
    citation: Citation,
    root: &'a Proposition,
    projection: Vec<usize>,
    pub(super) proposition: &'a Proposition,
}

impl ProjectedFact<'_> {
    pub(super) fn proof(&self) -> ProofNode {
        let mut proof = self.citation.proof(self.root);
        for &conjunct in &self.projection {
            let Proposition::Conjunction(parts) = &proof.conclusion else {
                unreachable!("projection follows retained conjunction children")
            };
            proof = ProofNode {
                conclusion: parts[conjunct].clone(),
                rule: ProofRule::ConjunctionElimination {
                    conjunction: Box::new(proof),
                    conjunct,
                },
            };
        }
        proof
    }
}

fn projected_facts<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> Vec<ProjectedFact<'a>> {
    let mut facts = Vec::new();
    for (citation, root) in cited_facts(assumptions, semantic_axioms) {
        let mut pending = vec![(root, Vec::new())];
        while let Some((proposition, projection)) = pending.pop() {
            if let Proposition::Conjunction(parts) = proposition {
                for (conjunct, part) in parts.iter().enumerate().rev() {
                    let mut child_projection = projection.clone();
                    child_projection.push(conjunct);
                    pending.push((part, child_projection));
                }
            } else {
                facts.push(ProjectedFact {
                    citation,
                    root,
                    projection,
                    proposition,
                });
            }
        }
    }
    facts
}

struct Dependencies {
    values: BTreeSet<ValueId>,
    complete: bool,
}

impl Dependencies {
    fn of(proposition: &Proposition, literals: &BTreeSet<ValueId>) -> Self {
        let mut values = BTreeSet::new();
        let complete = proposition.visit_value_ids(|value| {
            if !literals.contains(&value) {
                values.insert(value);
            }
        });
        Self { values, complete }
    }
}

pub(super) fn connected_cases<'a>(
    goal: &Proposition,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> Vec<ProjectedFact<'a>> {
    // A conjunction packages independent facts; it must not connect their
    // value dependencies. Only its exact projected leaves become graph rows.
    let facts = projected_facts(assumptions, semantic_axioms);
    let literals = literal_values(&facts);
    let goal = Dependencies::of(goal, &literals);
    let dependencies = facts
        .iter()
        .map(|fact| Dependencies::of(fact.proposition, &literals))
        .collect::<Vec<_>>();
    let mut relevant = goal.values;
    // Structural and opaque identities are outside this value-only graph.
    // Keep their cases and their links to ordinary values conservatively.
    let retain_all = !goal.complete || relevant.is_empty();
    for dependency in &dependencies {
        if !dependency.complete {
            relevant.extend(&dependency.values);
        }
    }
    loop {
        let previous = relevant.len();
        for dependency in &dependencies {
            if !dependency.values.is_disjoint(&relevant) {
                relevant.extend(&dependency.values);
            }
        }
        if relevant.len() == previous {
            break;
        }
    }
    let mut cases: Vec<ProjectedFact<'a>> = Vec::new();
    for (fact, dependency) in facts.into_iter().zip(dependencies) {
        let proposition = fact.proposition;
        if matches!(proposition, Proposition::Disjunction(_))
            && (retain_all
                || !dependency.complete
                || dependency.values.is_empty()
                || !dependency.values.is_disjoint(&relevant))
            && !cases
                .iter()
                .any(|previous| previous.proposition == proposition)
        {
            cases.push(fact);
        }
    }
    cases
}

fn literal_values(facts: &[ProjectedFact<'_>]) -> BTreeSet<ValueId> {
    let mut literals = BTreeSet::new();
    let mut aliases = Vec::new();
    let mut pending = facts
        .iter()
        .map(|fact| fact.proposition)
        .collect::<Vec<_>>();
    while let Some(fact) = pending.pop() {
        match fact {
            Proposition::Conjunction(parts) => pending.extend(parts),
            Proposition::Equal(left, right) => {
                for (value, definition) in [(left, right), (right, left)] {
                    let ScalarTerm::Value { id, scalar_type } = value else {
                        continue;
                    };
                    if *scalar_type != definition.scalar_type() {
                        continue;
                    }
                    match definition {
                        ScalarTerm::Boolean(_) => {
                            literals.insert(*id);
                        }
                        ScalarTerm::Integer { scalar_type, value }
                            if scalar_type.admits(*value) =>
                        {
                            literals.insert(*id);
                        }
                        ScalarTerm::Value { id: other, .. } => aliases.push((*id, *other)),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    // Only unconditional, exact same-type aliases inherit literal identity.
    // A shared zero SSA value must not connect every independent comparison.
    loop {
        let previous = literals.len();
        for (left, right) in &aliases {
            if literals.contains(right) {
                literals.insert(*left);
            }
        }
        if literals.len() == previous {
            break;
        }
    }
    literals
}

#[cfg(test)]
mod tests;
