//! Operand-level integer arithmetic shared by source and captured-value readers.
//!
//! Normalization retains mathematical denotation, including both operands of an
//! opaque quotient. It does not establish execution formation. Consumers prove
//! each operation before using its consequences; this keeps cancellation from
//! hiding a zero divisor or an overflowing intermediate, including MIN % -1.
use super::{BigInt, BinaryOperator, Engine, Polynomial};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::operator::builtin_integer_result_carrier;
use typed_trees::types::PrimitiveType;

impl Engine<'_> {
    /// Bitwise denotation belongs to the selected fixed carrier, independently
    /// of overflow policy. Substitute both operands before evaluation, but keep
    /// their declaration's operator selection and reject unlanded magnitudes.
    /// Non-singleton ranges remain unknown; endpoint evaluation is unsound.
    pub(super) fn selected_closed_bitwise_value(
        &mut self,
        expression: typed_trees::expression::ExpressionHandle,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<Polynomial> {
        use crate::proof_contracts::arithmetic_domains::{
            admitted_integer, integer_magnitude, integer_type,
        };
        // AND/OR/XOR have no overloadable OperatorSpelling. The declaration's
        // result query establishes compatible integer operands and contextual
        // literals; a substituted number cannot supply that carrier itself.
        let reference = self.source_result_type(expression)?;
        let carrier = integer_type(self.program.primitive_type_reference(reference)?)?;
        let left = self.normalize(binary.left)?;
        let right = self.normalize(binary.right)?;
        let left = admitted_integer(carrier, &self.substituted(&left).constant_value()?)?;
        let right = admitted_integer(carrier, &self.substituted(&right).constant_value()?)?;
        let value = match binary.operator {
            BinaryOperator::BitwiseAnd => carrier.bitwise_and(left, right),
            BinaryOperator::BitwiseOr => carrier.bitwise_or(left, right),
            BinaryOperator::BitwiseXor => carrier.bitwise_xor(left, right),
            _ => None,
        }?;
        Some(Polynomial::constant(integer_magnitude(value)))
    }

    /// Shifts take width and overflow policy from the value, not the count.
    /// The count is an independently completed integer, including when its
    /// own arithmetic policy is Wrapping. Bitwise AND has no overflow policy;
    /// its children still owe their individually selected formation.
    pub(in crate::proof_contracts::contract_entailment) fn integer_operation_meaning(
        operator: BinaryOperator,
        meaning: [(PrimitiveType, ArithmeticDomain); 2],
    ) -> bool {
        let [(left, left_policy), (right, right_policy)] = meaning;
        if builtin_integer_result_carrier(operator, [left, right]).is_none() {
            return false;
        }
        match operator {
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
                left_policy == ArithmeticDomain::Exact
            }
            BinaryOperator::BitwiseAnd => true,
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo => {
                left_policy == ArithmeticDomain::Exact && right_policy == ArithmeticDomain::Exact
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
        let value = match operator {
            BinaryOperator::Add => Some(left.add(&right)),
            BinaryOperator::Subtract => Some(left.sub(&right)),
            BinaryOperator::Multiply => left.checked_mul(&right),
            BinaryOperator::Divide => self.integer_quotient(left.clone(), right.clone()),
            BinaryOperator::Modulo => self.integer_remainder(left.clone(), right.clone(), true),
            _ => None,
        }?;
        if matches!(operator, BinaryOperator::Divide | BinaryOperator::Modulo) {
            self.install_integer_operation_bounds(operator, &left, &right, &value);
        }
        Some(value)
    }

    pub(in crate::proof_contracts::contract_entailment) fn integer_value_lands(
        &self,
        primitive: PrimitiveType,
        value: &Polynomial,
    ) -> bool {
        if primitive == PrimitiveType::Addr {
            let Some(bound) = self.address_model_bound_value() else {
                return false;
            };
            return self.prove_at_least(&self.substituted(value), &BigInt::zero())
                && self.prove_at_least(&self.substituted(&bound.sub(value)), &BigInt::from_i64(1));
        }
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
        let Some(result) = builtin_integer_result_carrier(operator, primitives) else {
            return false;
        };
        if !self.integer_value_lands(primitive, left)
            || !self.integer_value_lands(right_primitive, right)
            || !self.integer_value_lands(result, value)
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
    /// negative values. Division by one proves only non-increase, not descent.
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
                if self.prove_at_least(&dividend, &BigInt::zero())
                    && self.prove_at_least(&divisor, &BigInt::from_i64(1)) =>
            {
                let strict = self.prove_at_least(&dividend, &BigInt::from_i64(1))
                    && self.prove_at_least(&divisor, &BigInt::from_i64(2));
                Some((
                    dividend.sub(&self.substituted(value)),
                    BigInt::from_i64(i64::from(strict)),
                ))
            }
            BinaryOperator::ShiftRight
                if self.prove_at_least(&dividend, &BigInt::from_i64(1))
                    && self.prove_at_least(&divisor, &BigInt::from_i64(1)) =>
            {
                Some((dividend.sub(&self.substituted(value)), BigInt::from_i64(1)))
            }
            BinaryOperator::Modulo
                if self.prove_at_least(&dividend, &BigInt::zero())
                    && self.prove_at_least(&divisor, &BigInt::from_i64(1)) =>
            {
                Some((divisor.sub(&self.substituted(value)), BigInt::from_i64(1)))
            }
            _ => None,
        };
        if let Some(bound) = consequence {
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
