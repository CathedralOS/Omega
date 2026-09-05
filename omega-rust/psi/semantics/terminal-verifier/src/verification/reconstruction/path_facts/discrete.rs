//! Exact consequences of a branch predicate over a fixed discrete carrier.

use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm};

pub(super) fn strict_bound(proposition: &Proposition) -> Option<Proposition> {
    let Proposition::LessThan(left, right) = proposition else {
        return None;
    };
    if left.scalar_type() != right.scalar_type() {
        return None;
    }
    if let ScalarTerm::Integer { scalar_type, .. } = left
        && scalar_type.carrier() == IntegerCarrier::Fixed
    {
        return Some(adjacent(left, true).map_or(Proposition::Falsehood, |next| {
            Proposition::LessOrEqual(next, right.clone())
        }));
    }
    if let ScalarTerm::Integer { scalar_type, .. } = right
        && scalar_type.carrier() == IntegerCarrier::Fixed
    {
        return Some(
            adjacent(right, false).map_or(Proposition::Falsehood, |previous| {
                Proposition::LessOrEqual(left.clone(), previous)
            }),
        );
    }
    None
}

pub(super) fn unequal(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    if left == right {
        return Proposition::Falsehood;
    }
    let literal_and_value = match (&left, &right) {
        (ScalarTerm::Integer { scalar_type, .. }, value)
            if scalar_type.carrier() == IntegerCarrier::Fixed =>
        {
            Some((&left, value))
        }
        (value, ScalarTerm::Integer { scalar_type, .. })
            if scalar_type.carrier() == IntegerCarrier::Fixed =>
        {
            Some((&right, value))
        }
        _ => None,
    };
    if let Some((literal, value)) = literal_and_value {
        let mut alternatives = Vec::new();
        if let Some(previous) = adjacent(literal, false) {
            alternatives.push(Proposition::LessOrEqual(value.clone(), previous));
        }
        if let Some(next) = adjacent(literal, true) {
            alternatives.push(Proposition::LessOrEqual(next, value.clone()));
        }
        return disjunction(alternatives);
    }
    disjunction(vec![
        Proposition::LessThan(left.clone(), right.clone()),
        Proposition::LessThan(right, left),
    ])
}

fn disjunction(mut alternatives: Vec<Proposition>) -> Proposition {
    alternatives.sort();
    alternatives.dedup();
    match alternatives.len() {
        0 => Proposition::Falsehood,
        1 => alternatives.pop().expect("one alternative"),
        _ => Proposition::Disjunction(alternatives),
    }
}

fn adjacent(literal: &ScalarTerm, increasing: bool) -> Option<ScalarTerm> {
    let ScalarTerm::Integer { scalar_type, value } = literal else {
        return None;
    };
    let value = match (value, increasing) {
        (IntegerValue::Signed(value), true) => IntegerValue::Signed(value.checked_add(1)?),
        (IntegerValue::Signed(value), false) => IntegerValue::Signed(value.checked_sub(1)?),
        (IntegerValue::Unsigned(value), true) => IntegerValue::Unsigned(value.checked_add(1)?),
        (IntegerValue::Unsigned(value), false) => IntegerValue::Unsigned(value.checked_sub(1)?),
    };
    ScalarTerm::integer(*scalar_type, value).ok()
}
