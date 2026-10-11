//! Operand-level integer arithmetic shared by source and captured-value readers.
//!
//! Normalization retains mathematical denotation, including both operands of an
//! opaque quotient. It does not establish execution formation. Consumers prove
//! each operation before using its consequences; this keeps cancellation from
//! hiding a zero divisor or an overflowing intermediate, including MIN % -1.
use super::{BigInt, BinaryOperator, Engine, Polynomial};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::types::PrimitiveType;

impl Engine<'_> {
    /// Shifts take width and overflow policy from the value, not the count.
    /// The count is an independently completed integer, including when its
    /// own arithmetic policy is Wrapping. Bitwise AND has no overflow policy;
    /// its children still owe their individually selected formation.
    pub(in crate::proof_contracts::contract_entailment) fn integer_operation_meaning(
        operator: BinaryOperator,
        meaning: [(PrimitiveType, ArithmeticDomain); 2],
    ) -> bool {
        let [(left, left_policy), (right, right_policy)] = meaning;
        if !left.accepts_integer_literal() || !right.accepts_integer_literal() {
            return false;
        }
        match operator {
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
                left_policy == ArithmeticDomain::Exact
            }
            BinaryOperator::BitwiseAnd => left == right,
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo => {
                left == right
                    && left_policy == ArithmeticDomain::Exact
                    && right_policy == ArithmeticDomain::Exact
            }
            _ => false,
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn selected_integer_binary_value(
        &mut self,
        operator: BinaryOperator,
        meaning: [(PrimitiveType, ArithmeticDomain); 2],
        left: Polynomial,
        right: Polynomial,
    ) -> Option<Polynomial> {
        if !Self::integer_operation_meaning(operator, meaning) {
            return None;
        }
        let primitive = meaning[0].0;
        match operator {
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight | BinaryOperator::BitwiseAnd => {
                let bits = crate::proof_contracts::arithmetic_domains::integer_bit_width(primitive)?
                    as u32;
                match operator {
                    BinaryOperator::ShiftLeft => self.integer_shift_left(left, right, bits),
                    BinaryOperator::ShiftRight => self.integer_shift_right(left, right, bits),
                    _ => self.integer_bitwise_and(left, right, bits, primitive.is_signed_integer()),
                }
            }
            _ => self.integer_binary_value(operator, left, right),
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn integer_binary_value(
        &mut self,
        operator: BinaryOperator,
        left: Polynomial,
        right: Polynomial,
    ) -> Option<Polynomial> {
        match operator {
            BinaryOperator::Add => Some(left.add(&right)),
            BinaryOperator::Subtract => Some(left.sub(&right)),
            BinaryOperator::Multiply => left.checked_mul(&right),
            BinaryOperator::Divide => self.integer_quotient(left, right),
            BinaryOperator::Modulo => self.integer_remainder(left, right, true),
            _ => None,
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn integer_value_lands(
        &self,
        primitive: PrimitiveType,
        value: &Polynomial,
    ) -> bool {
        let Some((minimum, maximum)) = primitive.integer_bounds() else {
            return false;
        };
        self.prove_at_least(
            &self.substituted(&value.sub(&Polynomial::constant(BigInt::from_i128(minimum)))),
            &BigInt::zero(),
        ) && self.prove_at_least(
            &self.substituted(&Polynomial::constant(BigInt::from_i128(maximum)).sub(value)),
            &BigInt::zero(),
        )
    }

    /// Selected builtin Exact meaning and already formed children belong to the
    /// caller. These obligations apply before any consequence of this operation
    /// is installed; result bounds alone do not establish remainder formation.
    pub(in crate::proof_contracts::contract_entailment) fn integer_operation_lands(
        &self,
        operator: BinaryOperator,
        primitives: [PrimitiveType; 2],
        left: &Polynomial,
        right: &Polynomial,
        value: &Polynomial,
    ) -> bool {
        let [primitive, right_primitive] = primitives;
        if !self.integer_value_lands(primitive, left)
            || !self.integer_value_lands(right_primitive, right)
            || !self.integer_value_lands(primitive, value)
        {
            return false;
        }
        if matches!(
            operator,
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
        ) {
            let Some(bits) =
                crate::proof_contracts::arithmetic_domains::integer_bit_width(primitive)
            else {
                return false;
            };
            // Opaque intervals clamp counts to defined executions; they cannot
            // discharge formation. Prove the original count is in range.
            if !self.prove_at_least(&self.substituted(right), &BigInt::zero())
                || !self.prove_at_least(
                    &self.substituted(&Polynomial::constant(BigInt::from_i64(bits)).sub(right)),
                    &BigInt::from_i64(1),
                )
            {
                return false;
            }
        }
        if matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo) {
            let divisor = self.substituted(right);
            let positive = self.prove_at_least(&divisor, &BigInt::from_i64(1));
            let negative = Polynomial::default().sub(&divisor);
            if !positive && !self.prove_at_least(&negative, &BigInt::from_i64(1)) {
                return false;
            }
            let Some((minimum, _)) = primitive.integer_bounds() else {
                return false;
            };
            if minimum < 0
                && !positive
                && !self.prove_at_least(&negative, &BigInt::from_i64(2))
                && !self.prove_at_least(
                    &self.substituted(&left.sub(&Polynomial::constant(BigInt::from_i128(minimum)))),
                    &BigInt::from_i64(1),
                )
            {
                return false;
            }
        }
        true
    }

    /// Correlated consequences of an independently admitted integer operation.
    /// Intervals forget that a quotient and its dividend share the same value:
    /// for positive n and d >= 2, trunc(n / d) is strictly below n even when
    /// their independent ranges overlap. A positive right shift has the same
    /// strict consequence for positive values, but keeps floor semantics for
    /// negative values. Division by one and shifting by zero prove no descent.
    pub(in crate::proof_contracts::contract_entailment) fn install_integer_operation_bounds(
        &mut self,
        operator: BinaryOperator,
        left: &Polynomial,
        right: &Polynomial,
        value: &Polynomial,
    ) -> bool {
        let dividend = self.substituted(left);
        let divisor = self.substituted(right);
        let consequence = match operator {
            BinaryOperator::Divide
                if self.prove_at_least(&dividend, &BigInt::from_i64(1))
                    && self.prove_at_least(&divisor, &BigInt::from_i64(2)) =>
            {
                Some(dividend.sub(&self.substituted(value)))
            }
            BinaryOperator::ShiftRight
                if self.prove_at_least(&dividend, &BigInt::from_i64(1))
                    && self.prove_at_least(&divisor, &BigInt::from_i64(1)) =>
            {
                Some(dividend.sub(&self.substituted(value)))
            }
            BinaryOperator::Modulo
                if self.prove_at_least(&dividend, &BigInt::zero())
                    && self.prove_at_least(&divisor, &BigInt::from_i64(1)) =>
            {
                Some(divisor.sub(&self.substituted(value)))
            }
            _ => None,
        };
        if let Some(consequence) = consequence {
            let bound = (consequence, BigInt::from_i64(1));
            if !self.bounds.contains(&bound) {
                self.bounds.push(bound);
                self.seed_matrix();
                self.close_matrix();
                return true;
            }
        }
        false
    }
}
