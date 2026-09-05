//! Selected Boolean polarity reconstructed from prior terminal equations.

use psi_core::{Proposition, ScalarTerm, ScalarType, ValueId};

#[cfg(test)]
mod tests;

pub(in super::super) fn condition_fact(
    condition: ValueId,
    mut positive: bool,
    axioms: &[Proposition],
    value_term: &impl Fn(ValueId) -> ScalarTerm,
) -> Option<Proposition> {
    let mut predicate = value_term(condition);
    let mut expanded = Vec::new();
    loop {
        if predicate.scalar_type() != ScalarType::Boolean {
            return None;
        }
        match predicate {
            ScalarTerm::Value { id, .. } => {
                if expanded.contains(&id) {
                    return None;
                }
                expanded.push(id);
                if let Some(definition) = defining_term(&predicate, axioms) {
                    predicate = definition.clone();
                } else {
                    return Some(Proposition::Equal(predicate, ScalarTerm::Boolean(positive)));
                }
            }
            ScalarTerm::Boolean(value) => {
                return Some(if value == positive {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                });
            }
            ScalarTerm::BooleanNot { operand } => {
                predicate = *operand;
                positive = !positive;
            }
            ScalarTerm::BooleanEqual { left, right } => {
                let left = constant_or_original(*left, axioms);
                let right = constant_or_original(*right, axioms);
                match (left, right) {
                    (ScalarTerm::Boolean(value), other) | (other, ScalarTerm::Boolean(value)) => {
                        predicate = other;
                        positive = positive == value;
                    }
                    (left, right) => {
                        return Some(Proposition::Equal(
                            if positive {
                                left
                            } else {
                                ScalarTerm::boolean_not(left).ok()?
                            },
                            right,
                        ));
                    }
                }
            }
            ScalarTerm::IntegerEqual { left, right, .. } => {
                let left = constant_or_original(*left, axioms);
                let right = constant_or_original(*right, axioms);
                return Some(if positive {
                    Proposition::Equal(left, right)
                } else {
                    super::discrete::unequal(left, right)
                });
            }
            ScalarTerm::IntegerLessThan { left, right, .. } => {
                let left = constant_or_original(*left, axioms);
                let right = constant_or_original(*right, axioms);
                return Some(if positive {
                    Proposition::LessThan(left, right)
                } else {
                    Proposition::LessOrEqual(right, left)
                });
            }
            ScalarTerm::IntegerLessOrEqual { left, right, .. } => {
                let left = constant_or_original(*left, axioms);
                let right = constant_or_original(*right, axioms);
                return Some(if positive {
                    Proposition::LessOrEqual(left, right)
                } else {
                    Proposition::LessThan(right, left)
                });
            }
            // No integer order law is applied to an unrelated Boolean
            // observation. Its selected truth value remains an exact fact.
            other => return Some(Proposition::Equal(other, ScalarTerm::Boolean(positive))),
        }
    }
}

fn defining_term<'a>(term: &ScalarTerm, axioms: &'a [Proposition]) -> Option<&'a ScalarTerm> {
    // Operation and edge equations put their newly defined value on the left.
    // Prefer that definition over a later alias which mentions it on the right;
    // otherwise a forward alias immediately walks back to its predecessor.
    axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term && right != term => Some(right),
            _ => None,
        })
        .or_else(|| {
            axioms.iter().rev().find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if right == term && left != term => Some(left),
                _ => None,
            })
        })
}

fn constant_or_original(term: ScalarTerm, axioms: &[Proposition]) -> ScalarTerm {
    let mut current = &term;
    let mut expanded = Vec::new();
    while let ScalarTerm::Value { id, .. } = current {
        if expanded.contains(id) {
            return term;
        }
        expanded.push(*id);
        let Some(definition) = defining_term(current, axioms) else {
            return term;
        };
        current = definition;
    }
    match current {
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => current.clone(),
        _ => term,
    }
}
