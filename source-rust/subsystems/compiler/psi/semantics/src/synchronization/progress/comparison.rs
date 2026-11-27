//! Facts about the order of two actual scalar values. The three admissible
//! orders suffice even for wide integers. Only comparisons of this exact
//! typed pair decide a leaf; the ordinary proposition evaluator handles the
//! conjunction, disjunction or implication around it.
use super::ScalarFact;
use semantic_vocabulary::{Proposition, ScalarTerm};
use std::cmp::Ordering;

pub(in super::super) fn decided(
    left: ScalarFact,
    right: ScalarFact,
    order: Ordering,
    positive: bool,
    requirements: &[Proposition],
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
    if requirements.is_empty() {
        return None;
    }
    let lhs = left.term()?;
    let rhs = right.term()?;
    let mut answer = None;
    for candidate in [Ordering::Less, Ordering::Equal, Ordering::Greater] {
        let excluded = requirements.iter().any(|predicate| {
            let result = closed_predicates::closed_guard_with(predicate, |leaf| {
                Ok::<_, std::convert::Infallible>(at_order(&lhs, &rhs, candidate, leaf))
            });
            matches!(result, Ok(Some(false)))
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
