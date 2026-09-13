//! One exact mathematical denotation for primitive proofs and closed runtime
//! guard checks. Integer widths constrain source values, not proof intermediates.
//! Reusing this owner avoids a second evaluator with subtly different overflow
//! or sign rules. Its invocation budget refuses excessive work before calling
//! the allocating BigInt operations; refusal establishes no mathematical fact.

use std::cmp::Ordering;

use numerics::bignum::BigInt;
use semantic_vocabulary::{IntegerMathLiteral, IntegerMathTerm};

// Private service ceilings, not integer widths or proof rules. BigInt uses u64
// limbs, schoolbook multiplication, and growing vectors for left shifts.
const MAXIMUM_TERM_NODES: usize = 8_192;
const MAXIMUM_TERM_DEPTH: usize = 128;
const MAXIMUM_RESULT_BITS: usize = 65_536;
const MAXIMUM_ALLOCATION_LIMBS: usize = 262_144;
const MAXIMUM_LIMB_WORK: usize = 1_048_576;

/// Incomplete evaluation is distinct from an open term or a false relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedIntegerEvaluationError {
    ResourceLimitExceeded,
}

impl std::fmt::Display for ClosedIntegerEvaluationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("closed mathematical integer evaluation resource limit exceeded")
    }
}

impl std::error::Error for ClosedIntegerEvaluationError {}

/// Invocation-owned budget for exact closed integer comparisons.
///
/// Reuse one evaluator when checking several relations in one request. Traversal,
/// allocation and arithmetic-work charges are cumulative, including both operands.
#[derive(Default)]
pub struct ClosedIntegerEvaluator {
    term_nodes: usize,
    allocation_limbs: usize,
    limb_work: usize,
}

impl ClosedIntegerEvaluator {
    /// Compares exact mathematical denotations without fixed-width truncation.
    ///
    /// `None` means an open value or an undefined negative shift count. A positive
    /// count too large to evaluate is a resource error, not an undefined integer.
    /// This does not validate value identities, proposition formation, or evidence.
    pub fn compare(
        &mut self,
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<Option<Ordering>, ClosedIntegerEvaluationError> {
        self.check_term_size(left)?;
        self.check_term_size(right)?;
        let Some(left) = self.evaluate(left)? else {
            return Ok(None);
        };
        let Some(right) = self.evaluate(right)? else {
            return Ok(None);
        };
        self.charge(0, left.bit_length().max(right.bit_length()).div_ceil(64))?;
        Ok(Some(left.cmp(&right)))
    }

    fn check_term_size(
        &mut self,
        term: &IntegerMathTerm,
    ) -> Result<(), ClosedIntegerEvaluationError> {
        let mut pending = vec![(term, 1)];
        while let Some((term, depth)) = pending.pop() {
            charge_limit(&mut self.term_nodes, 1, MAXIMUM_TERM_NODES)?;
            if depth > MAXIMUM_TERM_DEPTH {
                return Err(ClosedIntegerEvaluationError::ResourceLimitExceeded);
            }
            match term {
                IntegerMathTerm::MathValue { .. } | IntegerMathTerm::IntegerLiteral(_) => {}
                IntegerMathTerm::Add(left, right)
                | IntegerMathTerm::Subtract(left, right)
                | IntegerMathTerm::Multiply(left, right)
                | IntegerMathTerm::ShiftLeft {
                    value: left,
                    count: right,
                } => {
                    pending.push((right, depth + 1));
                    pending.push((left, depth + 1));
                }
            }
        }
        Ok(())
    }

    // The iterative preflight bounds recursion before entering this function.
    fn evaluate(
        &mut self,
        term: &IntegerMathTerm,
    ) -> Result<Option<BigInt>, ClosedIntegerEvaluationError> {
        let (left, right) = match term {
            IntegerMathTerm::MathValue { .. } => return Ok(None),
            IntegerMathTerm::IntegerLiteral(literal) => {
                self.charge(8, 8)?;
                return Ok(Some(big_integer_literal(*literal)));
            }
            IntegerMathTerm::Add(left, right)
            | IntegerMathTerm::Subtract(left, right)
            | IntegerMathTerm::Multiply(left, right)
            | IntegerMathTerm::ShiftLeft {
                value: left,
                count: right,
            } => (left, right),
        };
        let Some(left) = self.evaluate(left)? else {
            return Ok(None);
        };
        let Some(right) = self.evaluate(right)? else {
            return Ok(None);
        };
        let left_bits = left.bit_length();
        let right_bits = right.bit_length();
        let left_limbs = left_bits.div_ceil(64);
        let right_limbs = right_bits.div_ceil(64);
        let (result_bits, work, shift_count) = match term {
            IntegerMathTerm::Multiply(_, _) => (
                left_bits + right_bits,
                2 * left_limbs * right_limbs + 2 * (left_limbs + right_limbs + 1),
                0,
            ),
            IntegerMathTerm::ShiftLeft { .. } => {
                if right.is_negative() {
                    return Ok(None);
                }
                let count = right
                    .to_u64()
                    .and_then(|count| usize::try_from(count).ok())
                    .ok_or(ClosedIntegerEvaluationError::ResourceLimitExceeded)?;
                let result_bits = left_bits
                    .checked_add(count)
                    .ok_or(ClosedIntegerEvaluationError::ResourceLimitExceeded)?;
                (
                    result_bits,
                    4 * (result_bits.div_ceil(64) + left_limbs + right_limbs + 1),
                    count,
                )
            }
            _ => (
                left_bits.max(right_bits) + 1,
                4 * (left_limbs + right_limbs + 1),
                0,
            ),
        };
        if result_bits > MAXIMUM_RESULT_BITS {
            return Err(ClosedIntegerEvaluationError::ResourceLimitExceeded);
        }
        // Charge before BigInt allocates. Four times the combined input/output
        // limb bound covers sign-negation scratch, carry limbs, and shift-vector
        // growth/slack. Cumulative allocation also bounds all retained results;
        // dropping intermediates does not refund the invocation's budget.
        let allocation_limbs = 4 * (left_limbs + right_limbs + result_bits.div_ceil(64) + 1);
        self.charge(allocation_limbs, work)?;
        let result = match term {
            IntegerMathTerm::Add(_, _) => left.add(&right),
            IntegerMathTerm::Subtract(_, _) => left.sub(&right),
            IntegerMathTerm::Multiply(_, _) => left.mul(&right),
            IntegerMathTerm::ShiftLeft { .. } => left.shl_bits(shift_count),
            IntegerMathTerm::MathValue { .. } | IntegerMathTerm::IntegerLiteral(_) => {
                return Ok(None);
            }
        };
        Ok(Some(result))
    }

    fn charge(
        &mut self,
        allocation_limbs: usize,
        limb_work: usize,
    ) -> Result<(), ClosedIntegerEvaluationError> {
        charge_limit(
            &mut self.allocation_limbs,
            allocation_limbs,
            MAXIMUM_ALLOCATION_LIMBS,
        )?;
        charge_limit(&mut self.limb_work, limb_work, MAXIMUM_LIMB_WORK)
    }
}

fn charge_limit(
    used: &mut usize,
    amount: usize,
    limit: usize,
) -> Result<(), ClosedIntegerEvaluationError> {
    *used = used
        .checked_add(amount)
        .filter(|total| *total <= limit)
        .ok_or(ClosedIntegerEvaluationError::ResourceLimitExceeded)?;
    Ok(())
}

/// Evaluates one comparison with a fresh shared budget for both operands.
pub fn compare_integer_math_terms(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
) -> Result<Option<Ordering>, ClosedIntegerEvaluationError> {
    ClosedIntegerEvaluator::default().compare(left, right)
}

pub(crate) fn check_integer_math_term_size(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
) -> Result<(), ClosedIntegerEvaluationError> {
    let mut evaluator = ClosedIntegerEvaluator::default();
    evaluator.check_term_size(left)?;
    evaluator.check_term_size(right)
}

fn big_integer_literal(literal: IntegerMathLiteral) -> BigInt {
    let magnitude = BigInt::from_u128(literal.magnitude());
    if literal.negative() {
        magnitude.negate()
    } else {
        magnitude
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ValueId};

    fn integer(value: i128) -> IntegerMathTerm {
        IntegerMathTerm::literal(IntegerValue::Signed(value))
    }

    fn shift(value: IntegerMathTerm, count: IntegerMathTerm) -> IntegerMathTerm {
        IntegerMathTerm::ShiftLeft {
            value: Box::new(value),
            count: Box::new(count),
        }
    }

    #[test]
    fn every_closed_constructor_preserves_signed_unbounded_denotation() {
        let maximum = IntegerMathTerm::literal(IntegerValue::Unsigned(u128::MAX));
        let beyond = IntegerMathTerm::Add(Box::new(maximum.clone()), Box::new(integer(1)));
        assert_eq!(
            compare_integer_math_terms(&beyond, &maximum),
            Ok(Some(Ordering::Greater))
        );
        assert_eq!(
            compare_integer_math_terms(&beyond, &shift(integer(1), integer(128))),
            Ok(Some(Ordering::Equal))
        );
        let negative = IntegerMathTerm::Subtract(Box::new(integer(0)), Box::new(beyond.clone()));
        assert_eq!(
            compare_integer_math_terms(&negative, &integer(i128::MIN)),
            Ok(Some(Ordering::Less))
        );
        let product = IntegerMathTerm::Multiply(Box::new(negative), Box::new(integer(-2)));
        assert_eq!(
            compare_integer_math_terms(&product, &shift(integer(1), integer(129))),
            Ok(Some(Ordering::Equal))
        );
        assert_eq!(
            compare_integer_math_terms(&shift(integer(-3), integer(4)), &integer(-48)),
            Ok(Some(Ordering::Equal))
        );
        assert_eq!(
            compare_integer_math_terms(&shift(integer(-3), integer(0)), &integer(-3)),
            Ok(Some(Ordering::Equal))
        );
        assert_eq!(
            compare_integer_math_terms(&shift(integer(0), integer(100)), &integer(0)),
            Ok(Some(Ordering::Equal))
        );
    }

    #[test]
    fn open_values_and_negative_shift_counts_have_no_closed_relation() {
        let open = IntegerMathTerm::MathValue {
            source_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
            value: ValueId::new(1).expect("value identity"),
        };
        assert_eq!(compare_integer_math_terms(&open, &integer(0)), Ok(None));
        assert_eq!(compare_integer_math_terms(&integer(0), &open), Ok(None));
        assert_eq!(
            compare_integer_math_terms(&shift(integer(1), integer(-1)), &integer(0)),
            Ok(None)
        );
    }

    #[test]
    fn giant_and_unrepresentable_positive_shifts_refuse_before_allocation() {
        for count in [MAXIMUM_RESULT_BITS as u128, u64::MAX as u128, u128::MAX] {
            let term = shift(
                integer(1),
                IntegerMathTerm::literal(IntegerValue::Unsigned(count)),
            );
            assert_eq!(
                compare_integer_math_terms(&term, &integer(0)),
                Err(ClosedIntegerEvaluationError::ResourceLimitExceeded)
            );
        }
        let limit = shift(integer(1), integer((MAXIMUM_RESULT_BITS - 1) as i128));
        assert_eq!(
            compare_integer_math_terms(&limit, &integer(0)),
            Ok(Some(Ordering::Greater))
        );
    }

    #[test]
    fn depth_is_checked_iteratively_before_evaluation() {
        let mut term = integer(1);
        for _ in 0..MAXIMUM_TERM_DEPTH {
            term = IntegerMathTerm::Add(Box::new(term), Box::new(integer(0)));
        }
        assert_eq!(
            compare_integer_math_terms(&term, &integer(0)),
            Err(ClosedIntegerEvaluationError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn node_budget_is_shared_between_operands() {
        fn tree(depth: usize) -> IntegerMathTerm {
            if depth == 0 {
                return integer(1);
            }
            IntegerMathTerm::Add(Box::new(tree(depth - 1)), Box::new(tree(depth - 1)))
        }
        let term = tree(12);
        assert_eq!(
            compare_integer_math_terms(&term, &integer(4096)),
            Ok(Some(Ordering::Equal))
        );
        assert_eq!(
            compare_integer_math_terms(&term, &term),
            Err(ClosedIntegerEvaluationError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn multiplication_work_is_shared_between_operands() {
        let operand = shift(integer(1), integer(32_767));
        let product = IntegerMathTerm::Multiply(Box::new(operand.clone()), Box::new(operand));
        assert_eq!(
            compare_integer_math_terms(&product, &integer(0)),
            Ok(Some(Ordering::Greater))
        );
        assert_eq!(
            compare_integer_math_terms(&product, &product),
            Err(ClosedIntegerEvaluationError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn retained_allocation_and_work_are_charged_before_arithmetic() {
        let mut evaluator = ClosedIntegerEvaluator {
            allocation_limbs: MAXIMUM_ALLOCATION_LIMBS - 7,
            ..ClosedIntegerEvaluator::default()
        };
        assert_eq!(
            evaluator.compare(&integer(1), &integer(1)),
            Err(ClosedIntegerEvaluationError::ResourceLimitExceeded)
        );
        let mut evaluator = ClosedIntegerEvaluator {
            limb_work: MAXIMUM_LIMB_WORK - 7,
            ..ClosedIntegerEvaluator::default()
        };
        assert_eq!(
            evaluator.compare(&integer(1), &integer(1)),
            Err(ClosedIntegerEvaluationError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn repeated_relations_do_not_reset_the_invocation_budget() {
        let mut evaluator = ClosedIntegerEvaluator::default();
        for _ in 0..MAXIMUM_TERM_NODES / 2 {
            assert_eq!(
                evaluator.compare(&integer(1), &integer(1)),
                Ok(Some(Ordering::Equal))
            );
        }
        assert_eq!(
            evaluator.compare(&integer(1), &integer(1)),
            Err(ClosedIntegerEvaluationError::ResourceLimitExceeded)
        );
    }
}
