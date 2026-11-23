//! A concrete counterexample can stop an unsuccessful proof search. It
//! supplies no evidence for a contract or for an unreachable-arm warning.

use super::*;
use terminal_semantics::inhabitation::{known_predicate_with, known_scalar_value_with};

impl Prover<'_> {
    pub(super) fn has_scalar_counterexample(&self, goal: &Proposition) -> bool {
        [false, true].into_iter().any(|upper| {
            // Extremal free scalars often survive a chain of exclusions. A
            // checked SSA definition, including a known literal, takes priority.
            let extremal = |id: ValueId| {
                if self.definitions.get(id).is_some_and(Option::is_some) {
                    return None;
                }
                let ty = self.values.scalar_type(id)?;
                let value = match ty {
                    ScalarType::Boolean => IntegerValue::Unsigned(u128::from(upper)),
                    ScalarType::Integer(integer) if integer.bits().is_some() => {
                        if upper {
                            integer.maximum_value()?
                        } else {
                            integer.minimum_value()?
                        }
                    }
                    _ => return None,
                };
                Some((ty, value))
            };
            // A row holds `Some` once an equality below assigns its value. A
            // value nothing assigned reads as its extreme, so the table needs
            // no row per value of the namespace.
            let mut values = self.definitions.like(None);
            let assigned = |values: &ValueRows<Option<Option<(ScalarType, IntegerValue)>>>,
                            id: ValueId| {
                match values.get(id)? {
                    Some(assigned) => *assigned,
                    None => extremal(id),
                }
            };
            // A taken comparison can establish an input's exact value even
            // when that input has no defining SSA operation. Use the prepared
            // equality to propose that value, then replay the original path.
            for fact in &self.facts {
                let Some((left, right)) = fact.proposition.as_equal() else {
                    continue;
                };
                for (subject, literal) in [(left, right), (right, left)] {
                    if let ScalarTerm::Value { id, .. } = subject
                        && let ScalarTerm::Integer { scalar_type, value } = literal
                        && let Some(slot) = values.get_mut(*id)
                    {
                        *slot = Some(Some((ScalarType::Integer(*scalar_type), *value)));
                    }
                }
            }
            for &index in &self.equations {
                let Some((ScalarTerm::Value { id, .. }, term)) =
                    self.facts[index].proposition.as_equal()
                else {
                    continue;
                };
                let calculated = known_scalar_value_with(term, |id| assigned(&values, id));
                if let Some(calculated) = calculated
                    && let Some(slot) = values.get_mut(*id)
                {
                    *slot = Some(Some(calculated));
                }
            }
            // A guessed SSA value is no authority. The same assignment must
            // actually falsify the goal and satisfy every original premise and
            // semantic axiom. Unknown truth falls back to ordinary proof search.
            let holds = |p: &Proposition| known_predicate_with(p, |id| assigned(&values, id));
            holds(goal) == Some(false)
                && self
                    .assumptions
                    .iter()
                    .chain(self.axioms)
                    .all(|p| holds(p) == Some(true))
        })
    }
}
