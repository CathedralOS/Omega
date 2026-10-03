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
    )
}

impl<'input> ValueEqualities<'input> {
    pub(super) fn from_semantic_axioms(
        axioms: impl IntoIterator<Item = &'input Proposition>,
    ) -> Self {
        let mut definitions = Vec::new();
        for axiom in axioms {
            let Proposition::Equal(left, right) = axiom else {
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
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
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
            Proposition::IntegerMathEqual(left, right)
            | Proposition::IntegerMathLessThan(left, right)
            | Proposition::IntegerMathLessOrEqual(left, right) => {
                // A cited exact scalar definition can introduce mathematical
                // constructors even when this comparison originally names only
                // values. Use the same transport for both forms. Opaque policy
                // images remain typed scalar values; field observations below
                // retain their existing scalar comparison vocabulary.
                let expanded_left = self.mathematical(left, budget, depth + 1)?;
                let expanded_right = self.mathematical(right, budget, depth + 1)?;
                let is_operation = |term: &IntegerMathTerm| {
                    matches!(
                        term,
                        IntegerMathTerm::Add(..)
                            | IntegerMathTerm::Subtract(..)
                            | IntegerMathTerm::Multiply(..)
                            | IntegerMathTerm::ShiftLeft { .. }
                    )
                };
                if is_operation(left)
                    || is_operation(right)
                    || is_operation(&expanded_left)
                    || is_operation(&expanded_right)
                {
                    *left = expanded_left;
                    *right = expanded_right;
                    return Ok(transported);
                }
                // Leaf comparisons can also rejoin scalar field observations.
                // MathValue embeds this exact typed scalar value.
                let carrier = [&*left, &*right].into_iter().find_map(|term| match term {
                    IntegerMathTerm::MathValue { source_type, .. } => Some(*source_type),
                    _ => None,
                });
                let scalar = |term: &IntegerMathTerm| match term {
                    IntegerMathTerm::MathValue { source_type, value } => {
                        Some(ScalarTerm::value(*value, ScalarType::Integer(*source_type)))
                    }
                    IntegerMathTerm::IntegerLiteral(literal) => {
                        let carrier = carrier?;
                        ScalarTerm::integer(carrier, literal.as_integer_value(carrier)?).ok()
                    }
                    _ => None,
                };
                let (Some(left), Some(right)) = (scalar(left), scalar(right)) else {
                    return Ok(transported);
                };
                let left = self.scalar(&left, budget, depth + 1, &mut Vec::new())?;
                let right = self.scalar(&right, budget, depth + 1, &mut Vec::new())?;
                let ScalarType::Integer(carrier) = left.scalar_type() else {
                    return Err(PredicateDenotationError::InvalidValueEquality);
                };
                let comparison = match original {
                    Proposition::IntegerMathEqual(..) => {
                        ScalarTerm::integer_equal(carrier, left, right)
                    }
                    Proposition::IntegerMathLessThan(..) => {
                        ScalarTerm::integer_less_than(carrier, left, right)
                    }
                    _ => ScalarTerm::integer_less_or_equal(carrier, left, right),
                }
                .map_err(PredicateDenotationError::Malformed)?;
                transported = Proposition::Equal(comparison, ScalarTerm::boolean(true));
            }
            // Structural places remain independently named observations.
            Proposition::IeeeFloatComparison { .. }
            | Proposition::ByteSequenceEqual { .. }
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
                // Address literals can discharge address-geometry obligations
                // under their retained model too. Keep nonliteral address terms
                // opaque: this does not reinterpret arbitrary address arithmetic.
                if let ScalarTerm::Integer { value, .. } = scalar {
                    IntegerMathTerm::literal(value)
                } else {
                    crate::proof::integer_math_normalization::lift_exact_integer_term(&scalar)
                        .unwrap_or_else(|| original.clone())
                }
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
            | ScalarTerm::IeeeFloatField { .. }
            | ScalarTerm::ViewExtent { .. }
            | ScalarTerm::ByteSequencePredicate { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_) => {}
        }
        Ok(transported)
    }
}
