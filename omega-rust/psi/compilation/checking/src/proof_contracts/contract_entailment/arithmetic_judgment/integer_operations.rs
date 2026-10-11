//! Operand-level integer arithmetic shared by source and captured-value readers.
//!
//! Normalization retains mathematical denotation, including both operands of an
//! opaque quotient. It does not establish execution formation. Consumers prove
//! each operation before using its consequences; this keeps cancellation from
//! hiding a zero divisor or an overflowing intermediate, including MIN % -1.
use super::{BigInt, BinaryOperator, Engine, Polynomial};
use typed_trees::types::PrimitiveType;

impl Engine<'_> {
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
        primitive: PrimitiveType,
        left: &Polynomial,
        right: &Polynomial,
        value: &Polynomial,
    ) -> bool {
        if !self.integer_value_lands(primitive, left)
            || !self.integer_value_lands(primitive, right)
            || !self.integer_value_lands(primitive, value)
        {
            return false;
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
    /// their independent ranges overlap. No rule applies to division by one.
    pub(in crate::proof_contracts::contract_entailment) fn install_integer_division_bounds(
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
