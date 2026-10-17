//! Checked contextual congruence over exact scalar constructors.
//!
//! Under the original unconditional equations, each transported proposition is
//! equivalent to its original. Thus a certificate using the transported premise
//! slots proves the original goal under the original premises. The equations
//! are dependencies of this checked view, never newly introduced assumptions.

use semantic_vocabulary::{IntegerMathTerm, Proposition, ScalarTerm, ScalarType};

use super::{PredicateDenotationError, budget::Budget};

pub(super) struct ValueEqualities<'input> {
    definitions: Vec<(&'input ScalarTerm, &'input ScalarTerm)>,
}

pub(super) fn is_definition_subject(term: &ScalarTerm) -> bool {
    matches!(
        term,
        ScalarTerm::Value { .. }
            | ScalarTerm::IntegerField { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IeeeFloatField { .. }
            | ScalarTerm::ViewExtent { .. }
            | ScalarTerm::StructuralRead { .. }
            | ScalarTerm::ByteSequencePredicate { .. }
    )
}

impl<'input> ValueEqualities<'input> {
    pub(super) fn from_semantic_axioms(
        axioms: impl IntoIterator<Item = &'input Proposition>,
    ) -> Self {
        let mut definitions = Vec::new();
        for axiom in axioms {
            let Some((left, right)) = axiom.as_equal() else {
                continue;
            };
            if !is_definition_subject(left) {
                continue;
            }
            // Either unconditional equality for a repeated value is sound.
            // Prefer its first equation; retain every original premise in its
            // original slot, including subsequent selected-polarity equations.
            // Keep authored orientation: a read gives value -> field, a store
            // field -> stored value. Reversing both would create cycles. Field
            // equations are current observations supplied by reconstruction;
            // expired observations cannot be recovered here from operation history.
            if left != right && !definitions.iter().any(|(subject, _)| *subject == left) {
                definitions.push((left, right));
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
            semantic_vocabulary::ordered_comparison_operands!(left, right) => {
                *left = self.scalar(left, budget, depth + 1, &mut Vec::new())?;
                *right = self.scalar(right, budget, depth + 1, &mut Vec::new())?;
            }
            Proposition::ScalarIeeeFloatComparison {
                kind, left, right, ..
            } => {
                *left = self.scalar(left, budget, depth + 1, &mut Vec::new())?;
                *right = self.scalar(right, budget, depth + 1, &mut Vec::new())?;
                if kind.is_symmetric() && left > right {
                    std::mem::swap(left, right);
                }
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
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                // One transport preserves scalar observations below arbitrary
                // mathematical context. Field leaves and policy-bearing finite
                // computations no longer need a separate comparison fallback.
                *left = self.mathematical(left, budget, depth + 1)?;
                *right = self.mathematical(right, budget, depth + 1)?;
            }
            // Structural places remain independently named observations.
            Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
            | Proposition::StructuralIdentity { .. }
            | Proposition::StructuralCaseMembership { .. }
            | Proposition::ContentConservation(_)
            | Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Atom(_) => {}
        }
        Ok(transported)
    }

    fn mathematical(
        &self,
        original: &IntegerMathTerm,
        budget: &mut Budget,
        depth: usize,
    ) -> Result<IntegerMathTerm, PredicateDenotationError> {
        budget.step(depth)?;
        Ok(match original {
            IntegerMathTerm::Embed(operand) => {
                let scalar = self.scalar(operand, budget, depth + 1, &mut Vec::new())?;
                if scalar.scalar_type() != operand.scalar_type() {
                    return Err(PredicateDenotationError::InvalidValueEquality);
                }
                crate::proof::integer_math_normalization::lift_exact_integer_term(&scalar)
                    .or_else(|| IntegerMathTerm::embed(scalar))
                    .ok_or(PredicateDenotationError::InvalidValueEquality)?
            }
            IntegerMathTerm::MathValue { source_type, value } => {
                let scalar = self.scalar(
                    &ScalarTerm::value(*value, ScalarType::Integer(*source_type)),
                    budget,
                    depth + 1,
                    &mut Vec::new(),
                )?;
                // Reuse the kernel's exact fixed-carrier denotation. This
                // includes nested exact sums, but never reinterprets wrapping,
                // saturating, address, or other opaque scalar constructors.
                if scalar.scalar_type() != ScalarType::Integer(*source_type) {
                    return Err(PredicateDenotationError::InvalidValueEquality);
                }
                crate::proof::integer_math_normalization::lift_exact_integer_term(&scalar)
                    .or_else(|| IntegerMathTerm::embed(scalar))
                    .ok_or(PredicateDenotationError::InvalidValueEquality)?
            }
            IntegerMathTerm::IntegerLiteral(_) => original.clone(),
            IntegerMathTerm::Add(left, right) => IntegerMathTerm::Add(
                Box::new(self.mathematical(left, budget, depth + 1)?),
                Box::new(self.mathematical(right, budget, depth + 1)?),
            ),
            IntegerMathTerm::Subtract(left, right) => IntegerMathTerm::Subtract(
                Box::new(self.mathematical(left, budget, depth + 1)?),
                Box::new(self.mathematical(right, budget, depth + 1)?),
            ),
            IntegerMathTerm::Multiply(left, right) => IntegerMathTerm::Multiply(
                Box::new(self.mathematical(left, budget, depth + 1)?),
                Box::new(self.mathematical(right, budget, depth + 1)?),
            ),
            IntegerMathTerm::ShiftLeft { value, count } => IntegerMathTerm::ShiftLeft {
                value: Box::new(self.mathematical(value, budget, depth + 1)?),
                count: Box::new(self.mathematical(count, budget, depth + 1)?),
            },
        })
    }

    fn scalar(
        &self,
        original: &ScalarTerm,
        budget: &mut Budget,
        depth: usize,
        active: &mut Vec<usize>,
    ) -> Result<ScalarTerm, PredicateDenotationError> {
        budget.scalar(original, depth)?;
        if let Some((position, (_, definition))) = self
            .definitions
            .iter()
            .enumerate()
            .find(|(_, (subject, _))| *subject == original)
        {
            if active.contains(&position) {
                return Err(PredicateDenotationError::CyclicValueEquality);
            }
            active.push(position);
            let result = self.scalar(definition, budget, depth + 1, active)?;
            active.pop();
            return Ok(result);
        }
        // Clone the already charged constructor, then replace only children.
        // Its exact tag, carrier, source/target widths and authored order stay
        // intact; this judgment assumes no arithmetic equation beyond premises.
        let mut transported = original.clone();
        match &mut transported {
            ScalarTerm::StructuralRead { .. } => {
                for operand in transported.operands_mut() {
                    *operand = self.scalar(operand, budget, depth + 1, active)?;
                }
            }
            super::scalar_term_unary_operand!(operand) => {
                **operand = self.scalar(operand, budget, depth + 1, active)?;
            }
            super::scalar_term_binary_operands!(left, right) => {
                **left = self.scalar(left, budget, depth + 1, active)?;
                **right = self.scalar(right, budget, depth + 1, active)?;
            }
            ScalarTerm::Value { .. }
            | semantic_vocabulary::scalar_term_predicate_quint_pattern!(..)
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_) => {}
        }
        Ok(transported)
    }
}
