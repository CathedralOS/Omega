//! Select case assumptions through the goal's transitive value dependencies.

use std::collections::BTreeSet;

use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};

use super::super::super::integer_evidence::{Citation, cited_facts};

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
) -> Vec<(Citation, &'a Proposition)> {
    let facts = cited_facts(assumptions, semantic_axioms).collect::<Vec<_>>();
    let literals = literal_values(&facts);
    let goal = Dependencies::of(goal, &literals);
    let dependencies = facts
        .iter()
        .map(|(_, fact)| Dependencies::of(fact, &literals))
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
    let mut cases = Vec::new();
    for ((citation, proposition), dependency) in facts.into_iter().zip(dependencies) {
        if matches!(proposition, Proposition::Disjunction(_))
            && (retain_all
                || !dependency.complete
                || dependency.values.is_empty()
                || !dependency.values.is_disjoint(&relevant))
            && !cases.iter().any(|(_, previous)| *previous == proposition)
        {
            cases.push((citation, proposition));
        }
    }
    cases
}

fn literal_values(facts: &[(Citation, &Proposition)]) -> BTreeSet<ValueId> {
    let mut literals = BTreeSet::new();
    let mut aliases = Vec::new();
    let mut pending = facts.iter().map(|(_, fact)| *fact).collect::<Vec<_>>();
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
