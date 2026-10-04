//! Integer contradictions over the checker's current value observations.
//!
//! This adapter preserves the ordered-value owner's resolved identities and
//! mutation lifetime. It supplies mathematical comparisons to the shared
//! integer judgment; it neither replays source guards nor establishes the
//! formation of operand arithmetic. Callers must check those operands first.

use super::ordered_values::{Operand, RelationConstraint};
use super::{
    ArithmeticDomain, BinaryOperator, ExpressionHandle, Machine, State, TypedTrees,
    ValueEnvironment,
};
use crate::proof_contracts::contract_entailment::{
    ScopedArithmeticComparison, ScopedArithmeticValue, scoped_arithmetic_comparisons_unsatisfiable,
};
use numerics::bignum::BigInt;

/// Whether the current facts exclude this simultaneous assignment of values.
/// Each expression denotes its current observation, not an earlier source
/// occurrence which merely has the same spelling.
pub(super) fn excludes_values(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    environment: &ValueEnvironment,
    assignments: &[(ExpressionHandle, i64)],
) -> bool {
    let Some(state) = state else {
        return false;
    };
    let mut observations = Observations::default();
    let mut comparisons = Vec::new();
    for &(expression, value) in assignments {
        let Some(operand) = super::ordered_values::operand(program, machine, state, expression)
        else {
            return false;
        };
        let Some(left) = observations.value(&operand) else {
            return false;
        };
        comparisons.push(ScopedArithmeticComparison {
            operator: BinaryOperator::Equal,
            left,
            right: integer(value),
        });
    }
    for relation in &environment.ordered_values {
        let (Some(left), Some(right)) = (
            observations.value(&relation.left),
            observations.value(&relation.right),
        ) else {
            continue;
        };
        let (operator, right) = match relation.constraint {
            RelationConstraint::Unequal => (BinaryOperator::NotEqual, right),
            RelationConstraint::MinimumDifference(floor) => (
                BinaryOperator::GreaterOrEqual,
                ScopedArithmeticValue::IntegerOperation {
                    operator: BinaryOperator::Add,
                    left: Box::new(right),
                    right: Box::new(integer(floor)),
                },
            ),
        };
        comparisons.push(ScopedArithmeticComparison {
            operator,
            left,
            right,
        });
    }
    // Only paths belonging to resolved current observations can contribute
    // interval facts. The environment already owns their invalidation and
    // all-predecessor joins; unrelated same-spelled source expressions are not
    // invented to turn an interval into a symbol binding.
    for (position, operand) in observations.atoms.iter().enumerate() {
        let Operand::Place { path, .. } = operand else {
            continue;
        };
        let Some(interval) = environment.intervals.get(path) else {
            continue;
        };
        for (operator, bound) in [
            (BinaryOperator::GreaterOrEqual, interval.low()),
            (BinaryOperator::LessOrEqual, interval.high()),
        ] {
            if let Some(bound) = bound {
                comparisons.push(ScopedArithmeticComparison {
                    operator,
                    left: atom(position),
                    right: integer(bound),
                });
            }
        }
    }
    scoped_arithmetic_comparisons_unsatisfiable(program, machine, &comparisons)
}

#[derive(Default)]
struct Observations {
    atoms: Vec<Operand>,
}

impl Observations {
    fn value(&mut self, operand: &Operand) -> Option<ScopedArithmeticValue> {
        match operand {
            Operand::Integer(literal, _) => {
                literal.value_bignum().map(ScopedArithmeticValue::Integer)
            }
            Operand::Binary {
                operator,
                domain: ArithmeticDomain::Exact,
                operands,
                ..
            } if matches!(
                operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            ) =>
            {
                let [left, right] = operands.as_slice() else {
                    return None;
                };
                Some(ScopedArithmeticValue::IntegerOperation {
                    operator: *operator,
                    left: Box::new(self.value(left)?),
                    right: Box::new(self.value(right)?),
                })
            }
            // An opaque admitted integer observation can share identity with
            // another read, but its implementation is not mathematical Exact
            // arithmetic (notably Wrapping and Saturating operations).
            _ => {
                let position = self
                    .atoms
                    .iter()
                    .position(|candidate| candidate == operand)
                    .unwrap_or_else(|| {
                        self.atoms.push(operand.clone());
                        self.atoms.len() - 1
                    });
                Some(atom(position))
            }
        }
    }
}

fn atom(position: usize) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Atom {
        identity: format!("observation:{position}"),
        unsigned: false,
    }
}

fn integer(value: i64) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Integer(BigInt::from_i64(value))
}
