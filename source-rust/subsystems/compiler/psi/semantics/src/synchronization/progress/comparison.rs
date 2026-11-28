//! Facts about the order of two actual scalar values. The three admissible
//! orders suffice even for wide integers. Only comparisons of this exact
//! typed pair decide a leaf; the ordinary proposition evaluator handles the
//! conjunction, disjunction or implication around it.
use super::{ScalarFact, condition::Assumption};
use semantic_vocabulary::{Proposition, ScalarTerm};
use std::borrow::Cow;
use std::cmp::Ordering;

/// Only entry formals can occur in the published entry contract. Branches
/// may also constrain a particular runtime observation, including a guard
/// attached to one possible value of a joined cell.
#[derive(Default)]
pub(in super::super) struct Evidence<'a> {
    pub(super) obligations: Cow<'a, [Proposition]>,
    common: &'a [Assumption],
    witness: &'a [Assumption],
}

impl<'a> Evidence<'a> {
    pub(in super::super) fn new(requirements: &'a [Proposition], common: &'a [Assumption]) -> Self {
        Self {
            obligations: extend(requirements, common),
            common,
            witness: &[],
        }
    }

    pub(in super::super) fn with_witness<'b>(&'b self, witness: &'b [Assumption]) -> Evidence<'b> {
        Evidence {
            obligations: extend(&self.obligations, witness),
            common: self.common,
            witness,
        }
    }
}

fn extend<'a>(prior: &'a [Proposition], guards: &[Assumption]) -> Cow<'a, [Proposition]> {
    let mut requirements = Cow::Borrowed(prior);
    for guard in guards {
        if let Some(proposition) = guard.proposition() {
            requirements.to_mut().push(proposition);
        }
    }
    requirements
}

pub(in super::super) fn decided(
    left: ScalarFact,
    right: ScalarFact,
    order: Ordering,
    positive: bool,
    evidence: &Evidence<'_>,
) -> Option<bool> {
    if left.ty() != right.ty() {
        return None;
    }
    let accepts = |actual: Ordering| (actual == order) == positive;
    if left == right {
        return Some(accepts(Ordering::Equal));
    }
    if let (Some((_, left)), Some((_, right))) = (left.known_integer(), right.known_integer()) {
        return Some(accepts(left.cmp(&right)));
    }
    let terms = left.term().zip(right.term());
    let mut answer = None;
    for candidate in [Ordering::Less, Ordering::Equal, Ordering::Greater] {
        if !evidence
            .common
            .iter()
            .chain(evidence.witness)
            .all(|guard| guard.permits(left, right, candidate))
        {
            continue;
        }
        let excluded = terms.as_ref().is_some_and(|(lhs, rhs)| {
            evidence.obligations.iter().any(|predicate| {
                let result = closed_predicates::closed_guard_with(predicate, |leaf| {
                    Ok::<_, std::convert::Infallible>(at_order(lhs, rhs, candidate, leaf))
                });
                matches!(result, Ok(Some(false)))
            })
        });
        if !excluded {
            let current = accepts(candidate);
            if answer.is_some_and(|prior| prior != current) {
                return None;
            }
            answer = Some(current);
        }
    }
    answer
}

fn at_order(
    left: &ScalarTerm,
    right: &ScalarTerm,
    candidate: Ordering,
    clause: &Proposition,
) -> Option<bool> {
    let (a, b, order, positive) = match clause {
        Proposition::Equal(a, b) => (a, b, Ordering::Equal, true),
        Proposition::LessThan(a, b) => (a, b, Ordering::Less, true),
        Proposition::LessOrEqual(a, b) => (a, b, Ordering::Greater, false),
        _ => return None,
    };
    let actual = if a == left && b == right {
        candidate
    } else if a == right && b == left {
        candidate.reverse()
    } else {
        return None;
    };
    Some((actual == order) == positive)
}
