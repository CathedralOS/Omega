//! Integer ranges propose concrete assignments for optional bound searches.
//! Only an assignment satisfying the original premises can exclude a bound.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Interval {
    integer: IntegerType,
    lower: IntegerValue,
    upper: IntegerValue,
}

impl Interval {
    pub(super) fn new(integer: IntegerType, lower: IntegerValue, upper: IntegerValue) -> Self {
        Self {
            integer,
            lower,
            upper,
        }
    }

    fn carrier(integer: IntegerType) -> Option<Self> {
        Some(Self::new(
            integer,
            integer.minimum_value()?,
            integer.maximum_value()?,
        ))
    }

    fn select(self, left: IntegerValue, right: IntegerValue, order: Ordering) -> IntegerValue {
        if self.integer.compare(right, left) == Some(order) {
            right
        } else {
            left
        }
    }

    fn intersect(self, other: Self) -> Self {
        Self::new(
            self.integer,
            self.select(self.lower, other.lower, Ordering::Greater),
            self.select(self.upper, other.upper, Ordering::Less),
        )
    }

    fn enclose(self, other: Self) -> Self {
        Self::new(
            self.integer,
            self.select(self.lower, other.lower, Ordering::Less),
            self.select(self.upper, other.upper, Ordering::Greater),
        )
    }

    fn nonempty(self) -> Option<Self> {
        (self.integer.compare(self.lower, self.upper)? != Ordering::Greater).then_some(self)
    }

    pub(super) fn endpoint(self, upper: bool) -> IntegerValue {
        if upper { self.upper } else { self.lower }
    }

    /// Unrelated atoms leave the full range possible. Intersecting enclosing
    /// intervals can lose relationships between arms, so the result is a guess.
    fn project(self, proposition: &Proposition, subject: &ScalarTerm) -> Option<Self> {
        match proposition {
            Proposition::Falsehood => None,
            Proposition::Conjunction(parts) => parts.iter().try_fold(self, |range, part| {
                range.intersect(self.project(part, subject)?).nonempty()
            }),
            Proposition::Disjunction(parts) => parts
                .iter()
                .filter_map(|part| self.project(part, subject))
                .reduce(Self::enclose),
            Proposition::Equal(left, right)
            | Proposition::LessOrEqual(left, right)
            | Proposition::LessThan(left, right) => {
                let (literal, upper) = if left == subject {
                    (right, true)
                } else if right == subject {
                    (left, false)
                } else {
                    return Some(self);
                };
                let Some((integer, value)) = literal.integer_value() else {
                    return Some(self);
                };
                if integer != self.integer {
                    return Some(self);
                }
                let limit = if matches!(proposition, Proposition::LessThan(_, _)) {
                    step(value, !upper)?
                } else {
                    value
                };
                let range = match proposition {
                    Proposition::Equal(_, _) => Self::new(integer, limit, limit),
                    _ if upper => Self::new(integer, self.lower, limit),
                    _ => Self::new(integer, limit, self.upper),
                };
                self.intersect(range).nonempty()
            }
            _ => Some(self),
        }
    }
}

pub(super) struct Seed {
    pub(super) value: ValueId,
    pub(super) integer: IntegerType,
    pub(super) endpoints: [Option<IntegerValue>; 2],
}

impl Prover<'_> {
    pub(super) fn projected_endpoints(&self, subject: &ScalarTerm, carrier: Interval) -> Interval {
        self.facts
            .iter()
            .filter_map(|fact| match fact.proposition() {
                p @ Proposition::Disjunction(_) => carrier.project(p, subject),
                _ => None,
            })
            .fold(carrier, Interval::intersect)
    }

    /// Collect scalar restrictions once. Both assignments use the same facts,
    /// including literals excluded by a fact such as x < 3 or 3 < x.
    pub(super) fn integer_seeds(&self) -> Vec<Seed> {
        let mut intervals = self.definitions.like(None::<Interval>);
        let mut touched = Vec::new();
        let mut excluded = Vec::new();
        for fact in &self.facts {
            let proposition = fact.proposition();
            let (left, right, exclusion) = match proposition {
                Proposition::Disjunction(arms) => {
                    let [
                        Proposition::LessThan(left, right),
                        Proposition::LessThan(other_left, other_right),
                    ] = arms.as_slice()
                    else {
                        continue;
                    };
                    if left != other_right || right != other_left {
                        continue;
                    }
                    (left, right, true)
                }
                Proposition::LessOrEqual(left, right) | Proposition::LessThan(left, right) => {
                    (left, right, false)
                }
                _ => continue,
            };
            for (subject, literal) in [(left, right), (right, left)] {
                let ScalarTerm::Value { id, .. } = subject else {
                    continue;
                };
                let Some((integer, value)) = literal.integer_value() else {
                    continue;
                };
                let Some(carrier) = Interval::carrier(integer) else {
                    continue;
                };
                let Some(slot) = intervals.get_mut(*id) else {
                    continue;
                };
                if slot.is_none() {
                    touched.push(*id)
                }
                let range = slot.get_or_insert(carrier);
                if integer != range.integer {
                    continue;
                }
                if exclusion {
                    excluded.push((*id, integer, value));
                } else if let Some(restriction) = carrier.project(proposition, subject) {
                    *range = range.intersect(restriction);
                }
            }
        }
        let mut seeds = Vec::with_capacity(touched.len());
        for value in touched {
            if self.definitions.get(value).is_some_and(Option::is_some) {
                continue;
            }
            let Some(range) = intervals
                .get(value)
                .copied()
                .flatten()
                .and_then(Interval::nonempty)
            else {
                continue;
            };
            let endpoints = [false, true].map(|upper| {
                let mut endpoint = range.endpoint(upper);
                while excluded.contains(&(value, range.integer, endpoint)) {
                    endpoint = step(endpoint, !upper)?;
                }
                let inside = range.integer.compare(endpoint, range.lower)? != Ordering::Less
                    && range.integer.compare(endpoint, range.upper)? != Ordering::Greater;
                inside.then_some(endpoint)
            });
            seeds.push(Seed {
                value,
                integer: range.integer,
                endpoints,
            });
        }
        seeds
    }
}
