//! A concrete counterexample can stop an unsuccessful proof search. It
//! supplies no evidence for a contract or for an unreachable-arm warning.

#[path = "counterexample/ranges.rs"]
mod ranges;

use super::*;
use ranges::{Interval, Seed};
use terminal_semantics::inhabitation::{known_predicate_with, known_scalar_value_with};

impl Prover<'_> {
    pub(super) fn has_scalar_counterexample(&self, goal: &Proposition) -> bool {
        [false, true].into_iter().any(|upper| {
            self.observe_scalar_assignment(upper, &[], None, |assigned| {
                (known_predicate_with(goal, assigned) == Some(false)).then_some(())
            })
            .is_some()
        })
    }

    /// A feasible value disproves every proposed bound that excludes it.
    /// Replay the original premises for each assignment before exposing the value.
    pub(super) fn validated_endpoint_values(
        &self,
        id: ValueId,
        subject: &ScalarTerm,
        integer: IntegerType,
        minimum: IntegerValue,
        maximum: IntegerValue,
    ) -> [Option<IntegerValue>; 2] {
        let range = self.projected_endpoints(subject, Interval::new(integer, minimum, maximum));
        let seeds = self.integer_seeds();
        // A low value for the subject can require high values for its peers.
        [false, true].map(|upper| {
            let endpoint = range.endpoint(upper);
            let observe = |assigned: &dyn Fn(ValueId) -> Option<(ScalarType, IntegerValue)>| {
                assigned(id).map(|(_, n)| n)
            };
            let ordinary = self.observe_scalar_assignment(upper, &seeds, None, observe);
            if ordinary == Some(endpoint) {
                return ordinary;
            }
            let crossed = self.observe_scalar_assignment(
                !upper,
                &seeds,
                Some((id, ScalarType::Integer(integer), endpoint)),
                observe,
            );
            let farther = if upper {
                Ordering::Greater
            } else {
                Ordering::Less
            };
            match (ordinary, crossed) {
                (Some(first), Some(second)) if integer.compare(second, first) == Some(farther) => {
                    Some(second)
                }
                _ => ordinary.or(crossed),
            }
        })
    }

    fn observe_scalar_assignment<T>(
        &self,
        upper: bool,
        seeds: &[Seed],
        subject_endpoint: Option<(ValueId, ScalarType, IntegerValue)>,
        observe: impl FnOnce(&dyn Fn(ValueId) -> Option<(ScalarType, IntegerValue)>) -> Option<T>,
    ) -> Option<T> {
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
            let Some((left, right)) = fact.proposition().as_equal() else {
                continue;
            };
            for (subject, literal) in [(left, right), (right, left)] {
                let ScalarTerm::Value { id, .. } = subject else {
                    continue;
                };
                let proposed = match literal {
                    ScalarTerm::Integer { scalar_type, value } => {
                        (ScalarType::Integer(*scalar_type), *value)
                    }
                    ScalarTerm::Boolean(holds) => (
                        ScalarType::Boolean,
                        IntegerValue::Unsigned(u128::from(*holds)),
                    ),
                    _ => continue,
                };
                if let Some(slot) = values.get_mut(*id) {
                    *slot = Some(Some(proposed));
                }
            }
        }
        for seed in seeds {
            if let Some(endpoint) = seed.endpoints[usize::from(upper)]
                && let Some(slot) = values.get_mut(seed.value)
                && slot.is_none()
            {
                *slot = Some(Some((ScalarType::Integer(seed.integer), endpoint)));
            }
        }
        if let Some((subject, ty, endpoint)) = subject_endpoint
            && let Some(slot) = values.get_mut(subject)
        {
            *slot = Some(Some((ty, endpoint)));
        }
        // Exactly one of a sum's case names holds at each boundary
        // (`case_name_premises`). No extremal assignment satisfies that, so
        // without a choice every machine with a sum in scope paid the whole
        // bounded case split. Choose one case: the one a taken fact already
        // names, else the first case on the lower pass and the last on the
        // upper. The other names are false, unless a fact already set them.
        for fact in &self.facts {
            let Proposition::Disjunction(parts) = fact.proposition() else {
                continue;
            };
            let Some(names) = distinct_positive_boolean_choice(parts) else {
                continue;
            };
            // A defined name is computed from its definition below.
            if names
                .iter()
                .any(|id| self.definitions.get(*id).is_some_and(Option::is_some))
            {
                continue;
            }
            let set = |id: &ValueId| values.get(*id).copied().flatten().flatten();
            let truth = |holds: bool| {
                (
                    ScalarType::Boolean,
                    IntegerValue::Unsigned(u128::from(holds)),
                )
            };
            let chosen = names
                .iter()
                .position(|id| set(id) == Some(truth(true)))
                .or_else(|| {
                    let open = |id: &ValueId| set(id) != Some(truth(false));
                    if upper {
                        names.iter().rposition(open)
                    } else {
                        names.iter().position(open)
                    }
                });
            let Some(chosen) = chosen else {
                continue;
            };
            for (position, id) in names.iter().enumerate() {
                if let Some(slot) = values.get_mut(*id)
                    && slot.is_none()
                {
                    *slot = Some(Some(truth(position == chosen)));
                }
            }
        }
        for &index in &self.equations {
            let Some((ScalarTerm::Value { id, .. }, term)) =
                self.facts[index].proposition().as_equal()
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
        // Expose the observation only if this one assignment satisfies every
        // original premise and axiom. An unknown result leaves the proof search open.
        let holds = |p: &Proposition| known_predicate_with(p, |id| assigned(&values, id));
        let observation = observe(&|id| assigned(&values, id))?;
        self.assumptions
            .iter()
            .chain(self.axioms)
            .all(|p| holds(p) == Some(true))
            .then_some(observation)
    }
}
