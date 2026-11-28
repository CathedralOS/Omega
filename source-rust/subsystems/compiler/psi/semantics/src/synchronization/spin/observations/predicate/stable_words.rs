//! Repeated observations of the same private word see the same bits. Between
//! successive comparison thresholds every word takes the same branches.

use super::{Observation, SpinCondition};
use crate::entry_values::TerminalScalarValue as Scalar;
use crate::synchronization::{progress::ScalarFact, spin::Operand};
use semantic_vocabulary::{IntegerValue, ScalarType};
use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;

type Test = (usize, SpinCondition, IntegerValue);

pub(super) struct StableWords<K> {
    by_home: HashMap<(K, ScalarType, Option<IntegerValue>), usize>,
    // Keep evaluation in the order the observations occur in the body.
    tests: Vec<(ScalarType, Vec<Test>)>,
}

impl<K> Default for StableWords<K> {
    fn default() -> Self {
        Self {
            by_home: HashMap::new(),
            tests: Vec::new(),
        }
    }
}

impl<K: Eq + Hash> StableWords<K> {
    pub(super) fn record(&mut self, home: K, node: usize, condition: SpinCondition) {
        let Some((ty, pivot)) = condition
            .comparison
            .pivot
            .literal()
            .and_then(|p| ScalarFact::Literal(p).known_integer())
        else {
            return;
        };
        // The same raw word can give different answers under different masks.
        // Partition the resulting integer for each distinct projection.
        let mask = match condition.comparison.mask {
            Some(Operand::Literal(Scalar::Integer { value, .. })) => Some(value),
            Some(_) => return,
            None => None,
        };
        let index = self.by_home.entry((home, ty, mask)).or_insert_with(|| {
            self.tests.push((ty, Vec::new()));
            self.tests.len() - 1
        });
        self.tests[*index].1.push((node, condition, pivot));
    }

    /// Vary one storage location at a time, keeping all other unknowns
    /// unknown. The total number of partitions across all words is linear
    /// in the number of tests. Independent cells never form a cross product.
    pub(super) fn decide(
        self,
        answers: &mut [Observation],
        mut reduce: impl FnMut(&mut [Observation]) -> Observation,
    ) -> Option<Observation> {
        for (ty, tests) in self.tests {
            if tests.len() < 2 {
                continue;
            }
            let Some((minimum, maximum)) = bounds(ty) else {
                continue;
            };
            let mut points = BTreeSet::from([minimum]);
            for &(_, _, pivot) in &tests {
                points.insert(pivot);
                if pivot < maximum
                    && let Some(next) = successor(pivot)
                {
                    points.insert(next);
                }
            }
            let mut uniform = None;
            for value in points {
                for &(node, test, pivot) in &tests {
                    answers[node] = Observation::Known(test.accepts(value.cmp(&pivot)));
                }
                let result = reduce(answers);
                if !matches!(result, Observation::Known(_))
                    || uniform.is_some_and(|prior| prior != result)
                {
                    uniform = None;
                    break;
                }
                uniform = Some(result);
            }
            if uniform.is_some() {
                return uniform;
            }
            for (node, _, _) in tests {
                answers[node] = Observation::Unproved;
            }
        }
        None
    }
}

fn bounds(ty: ScalarType) -> Option<(IntegerValue, IntegerValue)> {
    match ty {
        ScalarType::Boolean => Some((IntegerValue::Unsigned(0), IntegerValue::Unsigned(1))),
        ScalarType::Integer(integer) => Some((integer.minimum_value()?, integer.maximum_value()?)),
        _ => None,
    }
}

fn successor(value: IntegerValue) -> Option<IntegerValue> {
    match value {
        IntegerValue::Unsigned(n) => n.checked_add(1).map(IntegerValue::Unsigned),
        IntegerValue::Signed(n) => n.checked_add(1).map(IntegerValue::Signed),
    }
}
