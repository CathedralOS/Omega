/*
Which goals the integer rules can decide at all.

A bound goal is decided from the intervals of the values it names, and an
interval comes from a value's defining operation and its operands' intervals
(`Prover::definition`). For some operations the rules give the result no
interval that says what the operation computes:

- an exclusive or, a bitwise not and every wrapping or saturating operation
  have no rule here at all;
- a bitwise and or a bitwise or gets the hull of a mask, not its value:
  `value & 63` for `value` in `112..=127` lies in `48..=63`, and the rules
  say `0..=63`. It "needs the operation evaluated", and closed evaluation
  covers add, subtract, multiply and left shift only;
- a quotient, a remainder or a right shift whose divisor or count is a value
  that no earlier result equation fixes to a literal: the endpoint chain
  replays those steps only by a literal or landed operand, so the operand's
  own interval, even `3..=3`, is not used.

A goal that depends on such a value has no derivation in the rules whether
or not the program is right, so failing to prove it says nothing about the
program. `wiki/spec/build/compiler_request.md`, "Outcomes": an unsupported
valid case is `Incomplete`, with "no claim that the unproved conclusion is
false". The owner's ruling on `unproved-goal-form-outcome` draws the same
line by the goal's form; this reads the form with each value replaced by the
operation that defines it, which is what the goal says once its equations
are substituted.

The reading is by form alone, never by the outcome of a search: an operation
the rules do bound exactly (add, subtract, multiply, a cast, a left shift, a
divide or right shift by a literal) is reached, and a goal that fails on
those alone stays a rejection.

Two kinds of value have no defining operation in the machine that names them
and can still stand for one the rules do not reach: a formal whose input
condition was inferred from a caller's unreached actual, and the result of a
call to a private helper whose body returns such a value. Those are passed
in as `unreached` (`crate::Unreached`).
*/

use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};

use super::Prover;

impl Prover<'_> {
    /// Whether deciding `goal` needs the interval of a value the rules do
    /// not reach: one of `unreached`, or one a result equation defines by an
    /// operation the rules give no exact interval, followed through every
    /// equation that defines a value the goal names.
    pub(crate) fn beyond_rules(&self, goal: &Proposition, unreached: &[ValueId]) -> bool {
        let mut pending = Vec::new();
        goal.visit_value_ids(|value| pending.push(value));
        self.any_beyond_rules(pending, unreached)
    }

    /// `beyond_rules` for one value: whether its interval is one the rules
    /// do not reach.
    pub(crate) fn value_beyond_rules(&self, value: ValueId, unreached: &[ValueId]) -> bool {
        self.any_beyond_rules(vec![value], unreached)
    }

    fn any_beyond_rules(&self, mut pending: Vec<ValueId>, unreached: &[ValueId]) -> bool {
        let mut seen = self.values.map(|_, _| false);
        while let Some(value) = pending.pop() {
            let Some(seen) = seen.get_mut(value) else {
                continue;
            };
            if std::mem::replace(seen, true) {
                continue;
            }
            if unreached.contains(&value) {
                return true;
            }
            for fact in &self.facts {
                let Proposition::Equal(left, right) = &fact.proposition else {
                    continue;
                };
                for (subject, definition) in [(left, right), (right, left)] {
                    if !matches!(subject, ScalarTerm::Value { id, .. } if *id == value) {
                        continue;
                    }
                    if let Some(axiom) = fact.axiom
                        && !self.reaches(axiom, definition)
                    {
                        return true;
                    }
                    definition.visit_value_ids(|operand| {
                        pending.push(operand);
                        true
                    });
                }
            }
        }
        false
    }

    /// Whether the rules give the result of `definition`, the operation
    /// result equation `axiom` states, the interval the operation maps its
    /// operands' intervals to.
    fn reaches(&self, axiom: usize, definition: &ScalarTerm) -> bool {
        let fixed = |operand: &ScalarTerm| {
            matches!(operand, ScalarTerm::Integer { .. }) || self.landing(operand, axiom).is_some()
        };
        match definition {
            ScalarTerm::IntegerBitwiseAnd { .. }
            | ScalarTerm::IntegerBitwiseOr { .. }
            | ScalarTerm::IntegerBitwiseXor { .. }
            | ScalarTerm::IntegerBitwiseNot { .. }
            | ScalarTerm::WrappingIntegerAdd { .. }
            | ScalarTerm::WrappingIntegerSubtract { .. }
            | ScalarTerm::WrappingIntegerMultiply { .. }
            | ScalarTerm::WrappingIntegerDivide { .. }
            | ScalarTerm::WrappingIntegerRemainder { .. }
            | ScalarTerm::WrappingIntegerShiftLeft { .. }
            | ScalarTerm::WrappingIntegerShiftRight { .. }
            | ScalarTerm::SaturatingIntegerAdd { .. }
            | ScalarTerm::SaturatingIntegerSubtract { .. }
            | ScalarTerm::SaturatingIntegerMultiply { .. }
            | ScalarTerm::SaturatingIntegerDivide { .. }
            | ScalarTerm::SaturatingIntegerRemainder { .. } => false,
            ScalarTerm::ExactIntegerDivide { right, .. }
            | ScalarTerm::ExactIntegerRemainder { right, .. } => fixed(right),
            ScalarTerm::ExactIntegerShiftRight { count, .. } => fixed(count),
            // Add, subtract, multiply, a cast and a left shift map their
            // operands' intervals exactly; a copy, a read, a literal or a
            // Boolean is read through its own equation. A failure on these
            // is a failure of the program's facts.
            _ => true,
        }
    }
}
