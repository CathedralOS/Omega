//! Shared, stack-safe traversal of proposition value identities.

use super::{IntegerMathTerm, Proposition, ScalarTerm};
use crate::ValueId;

enum Pending<'a> {
    Proposition(&'a Proposition),
    Propositions(std::slice::Iter<'a, Proposition>),
    Scalar(&'a ScalarTerm),
    IntegerMath(&'a IntegerMathTerm),
}

impl Proposition {
    /// Visit each value occurrence in order. Returns false if structural or
    /// opaque identities also occur, so dependency analysis can remain conservative.
    pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId)) -> bool {
        walk(
            self,
            |value| {
                visit(value);
                false
            },
            || true,
        )
        .1
    }

    /// Visit values under a caller-owned node budget. Every proposition,
    /// scalar and mathematical term costs one step, including literals and
    /// repeated occurrences. `None` means traversal stopped at the budget;
    /// any visited prefix is incomplete. `Some(false)` retains the ordinary
    /// visitor's indication that structural or opaque identities occur.
    pub fn visit_value_ids_bounded(
        &self,
        remaining: &mut usize,
        mut visit: impl FnMut(ValueId),
    ) -> Option<bool> {
        let mut exhausted = false;
        let result = walk(
            self,
            |value| {
                visit(value);
                false
            },
            || {
                if let Some(rest) = remaining.checked_sub(1) {
                    *remaining = rest;
                    true
                } else {
                    exhausted = true;
                    false
                }
            },
        );
        (!exhausted).then_some(result.1)
    }

    /// Whether any scalar or mathematical value identity matches the predicate.
    /// Stops at the first match; structural identities are not value identities.
    pub fn any_value_id(&self, predicate: impl FnMut(ValueId) -> bool) -> bool {
        walk(self, predicate, || true).0
    }
}

fn walk(
    proposition: &Proposition,
    mut visit: impl FnMut(ValueId) -> bool,
    mut step: impl FnMut() -> bool,
) -> (bool, bool) {
    let mut complete = true;
    let mut pending = vec![Pending::Proposition(proposition)];
    while let Some(node) = pending.pop() {
        // A list frame schedules one child at a time. Its children still pay
        // their ordinary node cost, without work proportional to unvisited width.
        if !matches!(node, Pending::Propositions(_)) && !step() {
            return (false, false);
        }
        match node {
            Pending::Propositions(mut parts) => {
                if let Some(part) = parts.next() {
                    pending.push(Pending::Propositions(parts));
                    pending.push(Pending::Proposition(part));
                }
            }
            Pending::Proposition(proposition) => match proposition {
                Proposition::Truth | Proposition::Falsehood => {}
                Proposition::Atom(_)
                | Proposition::IeeeFloatComparison { .. }
                | Proposition::ByteSequenceEqual { .. }
                | Proposition::StructuralCaseMembership { .. }
                | Proposition::ContentConservation(_) => complete = false,
                Proposition::Equal(left, right)
                | Proposition::LessThan(left, right)
                | Proposition::LessOrEqual(left, right)
                | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                    pending.push(Pending::Scalar(right));
                    pending.push(Pending::Scalar(left));
                }
                Proposition::IntegerMathEqual(left, right)
                | Proposition::IntegerMathLessThan(left, right)
                | Proposition::IntegerMathLessOrEqual(left, right) => {
                    pending.push(Pending::IntegerMath(right));
                    pending.push(Pending::IntegerMath(left));
                }
                Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
                    pending.push(Pending::Propositions(parts.iter()));
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    pending.push(Pending::Proposition(conclusion));
                    pending.push(Pending::Proposition(premise));
                }
            },
            Pending::IntegerMath(term) => match term {
                IntegerMathTerm::IntegerLiteral(_) => {}
                IntegerMathTerm::MathValue { value, .. } => {
                    if visit(*value) {
                        return (true, complete);
                    }
                }
                IntegerMathTerm::Add(left, right)
                | IntegerMathTerm::Subtract(left, right)
                | IntegerMathTerm::Multiply(left, right)
                | IntegerMathTerm::ShiftLeft {
                    value: left,
                    count: right,
                } => {
                    pending.push(Pending::IntegerMath(right));
                    pending.push(Pending::IntegerMath(left));
                }
            },
            Pending::Scalar(term) => match term {
                ScalarTerm::Value { id, .. } => {
                    if visit(*id) {
                        return (true, complete);
                    }
                }
                ScalarTerm::BooleanField { .. }
                | ScalarTerm::IntegerField { .. }
                | ScalarTerm::IeeeFloatField { .. }
                | ScalarTerm::ViewExtent { .. }
                | ScalarTerm::ByteSequencePredicate { .. } => complete = false,
                ScalarTerm::Boolean(_)
                | ScalarTerm::Integer { .. }
                | ScalarTerm::IeeeFloatLiteral(_) => {}
                ScalarTerm::BooleanNot { operand }
                | ScalarTerm::IntegerBitwiseNot { operand, .. }
                | ScalarTerm::IntegerWiden { operand, .. }
                | ScalarTerm::IntegerExactCast { operand, .. } => {
                    pending.push(Pending::Scalar(operand));
                }
                ScalarTerm::BooleanEqual { left, right }
                | ScalarTerm::IntegerEqual { left, right, .. }
                | ScalarTerm::IntegerLessThan { left, right, .. }
                | ScalarTerm::IntegerLessOrEqual { left, right, .. }
                | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
                | ScalarTerm::IntegerBitwiseOr { left, right, .. }
                | ScalarTerm::IntegerBitwiseXor { left, right, .. }
                | ScalarTerm::WrappingIntegerShiftLeft {
                    value: left,
                    count: right,
                    ..
                }
                | ScalarTerm::WrappingIntegerShiftRight {
                    value: left,
                    count: right,
                    ..
                }
                | ScalarTerm::ExactIntegerShiftLeft {
                    value: left,
                    count: right,
                    ..
                }
                | ScalarTerm::ExactIntegerShiftRight {
                    value: left,
                    count: right,
                    ..
                }
                | ScalarTerm::ExactIntegerAdd { left, right, .. }
                | ScalarTerm::ExactIntegerSubtract { left, right, .. }
                | ScalarTerm::ExactIntegerMultiply { left, right, .. }
                | ScalarTerm::ExactIntegerDivide { left, right, .. }
                | ScalarTerm::ExactIntegerRemainder { left, right, .. }
                | ScalarTerm::WrappingIntegerDivide { left, right, .. }
                | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
                | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
                | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
                | ScalarTerm::WrappingIntegerAdd { left, right, .. }
                | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
                | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
                | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
                | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
                | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                    pending.push(Pending::Scalar(right));
                    pending.push(Pending::Scalar(left));
                }
            },
        }
    }
    (false, complete)
}

#[cfg(test)]
mod tests;
