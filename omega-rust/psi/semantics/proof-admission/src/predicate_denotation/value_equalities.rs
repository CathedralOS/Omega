//! Checked contextual congruence over exact scalar constructors.
//!
//! Under the original unconditional equations, each transported proposition is
//! equivalent to its original. Thus a certificate using the transported premise
//! slots proves the original goal under the original premises. The equations
//! are dependencies of this checked view, never newly introduced assumptions.

use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};

use super::{PredicateDenotationError, budget::Budget};

pub(super) struct ValueEqualities<'input> {
    definitions: Vec<(ValueId, &'input ScalarTerm)>,
}

impl<'input> ValueEqualities<'input> {
    pub(super) fn from_semantic_axioms(axioms: &'input [Proposition]) -> Self {
        let mut definitions = Vec::new();
        for axiom in axioms {
            let Proposition::Equal(left @ ScalarTerm::Value { id, .. }, right) = axiom else {
                continue;
            };
            // Either unconditional equality for a repeated value is sound.
            // Prefer its first equation; retain every original premise in its
            // original slot, including subsequent selected-polarity equations.
            if left != right && !definitions.iter().any(|(value, _)| value == id) {
                definitions.push((*id, right));
            }
        }
        Self { definitions }
    }

    pub(super) fn proposition(
        &self,
        original: &Proposition,
        budget: &mut Budget,
        depth: usize,
    ) -> Result<Proposition, PredicateDenotationError> {
        // Charge the entire source subtree before cloning it. Replacements
        // are separately charged before allocation, including repeated DAG
        // children; a small input cannot cause unbounded expanded output.
        budget.proposition(original, depth)?;
        let mut transported = original.clone();
        match &mut transported {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                *left = self.scalar(left, budget, depth + 1, &mut Vec::new())?;
                *right = self.scalar(right, budget, depth + 1, &mut Vec::new())?;
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    *child = self.proposition(child, budget, depth + 1)?;
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                **premise = self.proposition(premise, budget, depth + 1)?;
                **conclusion = self.proposition(conclusion, budget, depth + 1)?;
            }
            // These observations have no ScalarTerm children. In particular,
            // mathematical values and structural places are distinct identities.
            Proposition::IntegerMathEqual(..)
            | Proposition::IntegerMathLessThan(..)
            | Proposition::IntegerMathLessOrEqual(..)
            | Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::StructuralCaseMembership { .. }
            | Proposition::ContentConservation(_)
            | Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_) => {}
        }
        Ok(transported)
    }

    fn scalar(
        &self,
        original: &ScalarTerm,
        budget: &mut Budget,
        depth: usize,
        active: &mut Vec<ValueId>,
    ) -> Result<ScalarTerm, PredicateDenotationError> {
        budget.scalar(original, depth)?;
        if let ScalarTerm::Value { id, .. } = original
            && let Some((_, definition)) = self.definitions.iter().find(|(value, _)| value == id)
        {
            if active.contains(id) {
                return Err(PredicateDenotationError::CyclicValueEquality);
            }
            active.push(*id);
            let result = self.scalar(definition, budget, depth + 1, active)?;
            active.pop();
            return Ok(result);
        }
        // Clone the already charged constructor, then replace only children.
        // Its exact tag, carrier, source/target widths and authored order stay
        // intact; this judgment assumes no arithmetic equation beyond premises.
        let mut transported = original.clone();
        match &mut transported {
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                **operand = self.scalar(operand, budget, depth + 1, active)?;
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
                **left = self.scalar(left, budget, depth + 1, active)?;
                **right = self.scalar(right, budget, depth + 1, active)?;
            }
            ScalarTerm::Value { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IntegerField { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. } => {}
        }
        Ok(transported)
    }
}
