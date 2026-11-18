//! Prepare semantic equalities once per fixed fact roster. A usable equality
//! fixes one source to a finite nonzero literal; consistent equalities about
//! that source all agree. Search never rescans every fact for each cast or
//! treats IEEE equality with zero as stored-value identity.

use super::{Fact, ProofNode, ProofRule, Proposition, Prover, ScalarTerm, node};
use proof_admission::{float_integer_conversion_substitution, float_integer_equality_subject};
use std::collections::BTreeMap;

pub(super) struct Equalities {
    subjects: BTreeMap<ScalarTerm, usize>,
    preconditions: BTreeMap<Proposition, usize>,
}
impl Equalities {
    pub(super) fn new(facts: &[Fact]) -> Self {
        let mut subjects = BTreeMap::new();
        let mut preconditions = BTreeMap::new();
        for (index, fact) in facts.iter().enumerate() {
            if let Proposition::IeeeFloatIntegerExact { source, target } = &fact.proposition
                && let Ok(carrier) = semantic_vocabulary::fixed_integer_type(*target)
                && let Ok(rows) =
                    semantic_vocabulary::float_integer_preconditions(source.clone(), carrier)
            {
                for row in rows {
                    preconditions.entry(row).or_insert(index);
                }
            }
            if let Some(subject) = float_integer_equality_subject(&fact.proposition) {
                subjects.entry(subject.clone()).or_insert(index);
            }
        }
        Self {
            subjects,
            preconditions,
        }
    }
    fn get(&self, subject: &ScalarTerm) -> Option<usize> {
        self.subjects.get(subject).copied()
    }
}

impl Prover<'_> {
    pub(super) fn float_integer_conversion(&mut self, goal: &Proposition) -> Option<ProofNode> {
        if let Some(&index) = self.float_integer_equalities.preconditions.get(goal) {
            return Some(node(
                goal.clone(),
                ProofRule::FloatIntegerConversion {
                    premise: Box::new(self.facts[index].proof(&self.facts)),
                    computed: None,
                },
            ));
        }
        let candidates = match goal {
            Proposition::IeeeFloatIntegerExact { source, .. } => {
                [self.float_integer_equalities.get(source), None]
            }
            Proposition::ScalarIeeeFloatComparison { left, right, .. } => [
                self.float_integer_equalities.get(left),
                self.float_integer_equalities.get(right),
            ],
            _ => return None,
        };
        for index in candidates.into_iter().flatten() {
            let equality = self.facts[index].proposition.clone();
            let Ok(computed) = float_integer_conversion_substitution(self.context, goal, &equality)
            else {
                continue;
            };
            // Each step removes a source endpoint. An IEEE comparison has at
            // most two; exactness has one. No application expands the graph.
            let Ok(proof) = self.prove(&computed) else {
                continue;
            };
            return Some(node(
                goal.clone(),
                ProofRule::FloatIntegerConversion {
                    premise: Box::new(self.facts[index].proof(&self.facts)),
                    computed: Some(Box::new(proof)),
                },
            ));
        }
        None
    }
}
