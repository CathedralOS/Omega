//! Recognize exhaustive integer comparisons before inferring caller obligations.

use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

const LESS: u8 = 1;
const EQUAL: u8 = 2;
const GREATER: u8 = 4;
const ALL: u8 = LESS | EQUAL | GREATER;

type Operands<'a> = (&'a ScalarTerm, &'a ScalarTerm);

/// Every arm must be false for the disjunction to fail. Rule out the orders
/// that would make an arm true. If a conjunction has only one unknown part,
/// that part must be false too. Each new fact removes a possible order, so
/// there is no search over combinations of arms.
pub(super) fn exhaustive(arms: &[Proposition]) -> bool {
    let mut orders = Orders::default();
    loop {
        orders.changed = false;
        if arms.iter().any(|arm| orders.require_false(arm)) {
            return true;
        }
        if !orders.changed {
            return false;
        }
    }
}

#[derive(Default)]
struct Orders<'a> {
    remaining: Vec<(Operands<'a>, u8)>,
    changed: bool,
}

impl<'a> Orders<'a> {
    /// True means the remaining orders cannot make this proposition false.
    fn require_false(&mut self, proposition: &'a Proposition) -> bool {
        match proposition {
            Proposition::Truth => true,
            Proposition::Disjunction(parts) => parts.iter().any(|part| self.require_false(part)),
            Proposition::Conjunction(parts) => {
                let mut unknown = None;
                for part in parts {
                    match self.truth(part) {
                        Some(false) => return false,
                        Some(true) => {}
                        None if unknown.is_some() => return false,
                        None => unknown = Some(part),
                    }
                }
                unknown.is_none_or(|part| self.require_false(part))
            }
            other => {
                let Some((operands, orders)) = comparison(other) else {
                    return false;
                };
                let position = self
                    .remaining
                    .binary_search_by_key(&operands, |item| item.0);
                let (index, previous) = match position {
                    Ok(index) => (index, self.remaining[index].1),
                    Err(index) => {
                        self.remaining.insert(index, (operands, ALL));
                        (index, ALL)
                    }
                };
                let remaining = previous & !orders;
                self.changed |= remaining != previous;
                self.remaining[index].1 = remaining;
                remaining == 0
            }
        }
    }

    fn truth(&self, proposition: &Proposition) -> Option<bool> {
        match proposition {
            Proposition::Truth => Some(true),
            Proposition::Falsehood => Some(false),
            Proposition::Disjunction(parts) | Proposition::Conjunction(parts) => {
                let decisive = matches!(proposition, Proposition::Disjunction(_));
                let mut complete = true;
                for part in parts {
                    match self.truth(part) {
                        Some(value) if value == decisive => return Some(value),
                        Some(_) => {}
                        None => complete = false,
                    }
                }
                complete.then_some(!decisive)
            }
            other => {
                let (operands, orders) = comparison(other)?;
                let position = self
                    .remaining
                    .binary_search_by_key(&operands, |item| item.0);
                let possible = position.map_or(ALL, |position| self.remaining[position].1);
                if possible & orders == 0 {
                    Some(false)
                } else if possible & !orders == 0 {
                    Some(true)
                } else {
                    None
                }
            }
        }
    }
}

fn comparison(proposition: &Proposition) -> Option<(Operands<'_>, u8)> {
    let (left, right, forward, reversed) = match proposition {
        Proposition::Equal(left, right) => (left, right, EQUAL, EQUAL),
        Proposition::LessThan(left, right) => (left, right, LESS, GREATER),
        Proposition::LessOrEqual(left, right) => (left, right, LESS | EQUAL, GREATER | EQUAL),
        _ => return None,
    };
    let ScalarType::Integer(integer) = left.scalar_type() else {
        return None;
    };
    if integer.is_address() || right.scalar_type() != ScalarType::Integer(integer) {
        return None;
    }
    // A comparison of computed terms could owe independent formation.
    let total = |term: &ScalarTerm| {
        matches!(
            term,
            ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } | ScalarTerm::IntegerField { .. }
        )
    };
    if !total(left) || !total(right) {
        return None;
    }
    Some(if left <= right {
        ((left, right), forward)
    } else {
        ((right, left), reversed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ValueId};

    #[test]
    fn all_comparisons_in_a_short_circuit_partition_are_necessary() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar = ScalarType::Integer(integer);
        let first = ScalarTerm::value(ValueId::new(1).unwrap(), scalar);
        let second = ScalarTerm::value(ValueId::new(2).unwrap(), scalar);
        let zero = ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap();
        let passed_first = Proposition::Equal(first.clone(), zero.clone());
        let mut arms = vec![
            Proposition::Conjunction(vec![
                passed_first.clone(),
                Proposition::Disjunction(vec![
                    Proposition::LessThan(second.clone(), zero.clone()),
                    Proposition::LessThan(zero.clone(), second.clone()),
                ]),
            ]),
            Proposition::LessThan(first.clone(), zero.clone()),
            Proposition::LessThan(zero.clone(), first),
        ];
        assert!(!exhaustive(&arms), "both bytes may be equal to zero");
        arms.push(Proposition::Conjunction(vec![
            passed_first,
            Proposition::Equal(zero, second),
        ]));
        assert!(exhaustive(&arms));
        arms.reverse();
        assert!(exhaustive(&arms), "arm order does not change coverage");
    }
}
