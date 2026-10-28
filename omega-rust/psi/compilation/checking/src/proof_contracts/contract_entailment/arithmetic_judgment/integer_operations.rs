//! Operand-level integer arithmetic shared by source and captured-value readers.
//!
//! Normalization retains mathematical denotation, including both operands of an
//! opaque quotient. It does not establish execution formation. Consumers prove
//! each operation before using its consequences; this keeps cancellation from
//! hiding a zero divisor or an overflowing intermediate, including MIN % -1.
use super::{BigInt, BinaryOperator, Engine, OpaqueTerm, Polynomial};
use crate::proof_contracts::arithmetic_domains::BigIntInterval as Interval;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::operator::builtin_integer_result_carrier;
use typed_trees::types::PrimitiveType;

impl Engine<'_> {
    /// Complete a selected finite integer calculation once. A representative
    /// is an actual value, not the unbounded polynomial that produced it.
    /// Same-carrier wrapping children can expand only inside the residue
    /// polynomial: quotient/shift atoms and foreign policies remain values.
    pub(super) fn integer_policy_value(
        &mut self,
        primitive: PrimitiveType,
        domain: ArithmeticDomain,
        value: Polynomial,
    ) -> Option<Polynomial> {
        if !primitive.is_fixed_integer()
            || !matches!(
                domain,
                ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating
            )
        {
            return None;
        }
        let value = self.substituted(&value);
        if self.integer_value_lands(primitive, &value) {
            return Some(value);
        }
        let (minimum, maximum) = super::super::ranking_range::integer_carrier_bounds(primitive)?;
        let value = if domain == ArithmeticDomain::Wrapping {
            self.wrapping_residue(
                primitive,
                value,
                &maximum.sub(&minimum).add(&BigInt::from_i64(1)),
            )?
        } else {
            value
        };
        if let Some(value) = value.constant_value() {
            return Some(Polynomial::constant(complete_integer_policy(
                domain, &value, &minimum, &maximum,
            )?));
        }
        let atom = format!("\0integer-policy:{primitive:?}:{domain:?}:{value:?}");
        let interval = self.integer_policy_interval(primitive, domain, &value);
        self.register_opaque_term(
            atom.clone(),
            OpaqueTerm::IntegerPolicy {
                primitive,
                domain,
                value,
            },
        );
        self.arithmetic_intervals.insert(atom.clone(), interval);
        Some(Polynomial::atom(atom))
    }

    fn wrapping_residue(
        &self,
        primitive: PrimitiveType,
        value: Polynomial,
        modulus: &BigInt,
    ) -> Option<Polynomial> {
        let mut substitutions = super::BTreeMap::new();
        for monomial in value.terms.keys() {
            for atom in monomial.keys() {
                let replacement = self
                    .opaque_terms
                    .iter()
                    .find_map(|(known, term)| {
                        if known != atom {
                            return None;
                        }
                        match term {
                            OpaqueTerm::IntegerPolicy {
                                primitive: source,
                                domain: ArithmeticDomain::Wrapping,
                                value,
                            } if *source == primitive => Some(value.clone()),
                            _ => None,
                        }
                    })
                    .unwrap_or_else(|| Polynomial::atom(atom.clone()));
                substitutions.insert(atom.clone(), replacement);
            }
        }
        let mut value =
            super::super::inductive_judgment::apply_argument_map(&value, &substitutions)?;
        for coefficient in value.terms.values_mut() {
            let (_, remainder) = coefficient.div_rem(modulus)?;
            *coefficient = if remainder.is_negative() {
                remainder.add(modulus)
            } else {
                remainder
            };
        }
        value.terms.retain(|_, coefficient| !coefficient.is_zero());
        Some(value)
    }

    pub(super) fn integer_policy_interval(
        &self,
        primitive: PrimitiveType,
        domain: ArithmeticDomain,
        value: &Polynomial,
    ) -> Interval {
        let Some((minimum, maximum)) =
            super::super::ranking_range::integer_carrier_bounds(primitive)
        else {
            return Interval::unbounded();
        };
        let value = self.substituted(value);
        if let Some(value) = value.constant_value()
            && let Some(value) = complete_integer_policy(domain, &value, &minimum, &maximum)
        {
            return Interval::constant(value);
        }
        if domain == ArithmeticDomain::Saturating {
            let interval = self.polynomial_interval(&value);
            return Interval {
                low: Some(interval.low.map_or_else(
                    || minimum.clone(),
                    |value| value.max(minimum.clone()).min(maximum.clone()),
                )),
                high: Some(interval.high.map_or_else(
                    || maximum.clone(),
                    |value| value.max(minimum).min(maximum.clone()),
                )),
            };
        }
        Interval {
            low: Some(minimum),
            high: Some(maximum),
        }
    }

    /// Bitwise denotation belongs to the selected fixed carrier, independently
    /// of overflow policy. Substitute both operands before evaluation, but keep
    /// their declaration's operator selection and reject unlanded magnitudes.
    /// Mask composition retains justified known bits; all other bits remain
    /// unknown. Evaluating range endpoints is not a bitwise proof.
    pub(super) fn selected_bitwise_value(
        &mut self,
        expression: typed_trees::expression::ExpressionHandle,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<Polynomial> {
        // Bitwise operators have no overloadable spelling. The selected
        // result query still establishes the common carrier; neither a mask
        // nor substituted actuals can invent an operand's width or policy.
        let reference = self.source_result_type(expression)?;
        let primitive = self.program.primitive_type_reference(reference)?;
        let bits = crate::proof_contracts::arithmetic_domains::integer_bit_width(primitive)? as u32;
        let left = self.normalize(binary.left)?;
        let right = self.normalize(binary.right)?;
        self.integer_bitwise(
            binary.operator,
            left,
            right,
            bits,
            primitive.is_signed_integer(),
        )
    }

    /// Normalize a selected Exact shift in a captured scalar expression.
    /// Width belongs to the shifted carrier. Formation still separately owes
    /// its count and result bounds. A shifted value or result with another
    /// arithmetic policy cannot acquire this Exact denotation in a proof goal.
    pub(super) fn selected_exact_shift_value(
        &mut self,
        expression: typed_trees::expression::ExpressionHandle,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<Polynomial> {
        let value_type = self.source_result_type(binary.left)?;
        let result_type = self.source_result_type(expression)?;
        let primitive = self.program.primitive_type_reference(value_type)?;
        if !primitive.is_fixed_integer()
            || self.program.primitive_type_reference(result_type) != Some(primitive)
            || self
                .program
                .arithmetic_domain_for_type_reference(value_type)
                != ArithmeticDomain::Exact
            || self
                .program
                .arithmetic_domain_for_type_reference(result_type)
                != ArithmeticDomain::Exact
        {
            return None;
        }
        let bits = crate::proof_contracts::arithmetic_domains::integer_bit_width(primitive)? as u32;
        let value = self.normalize(binary.left)?;
        let count = self.normalize(binary.right)?;
        match binary.operator {
            BinaryOperator::ShiftLeft => self.integer_shift_left(value, count, bits),
            BinaryOperator::ShiftRight => self.integer_shift_right(value, count, bits),
            _ => None,
        }
    }

    /// Shifts take width and overflow policy from the value, not the count.
    /// The count is an independently completed integer, including when its
    /// own arithmetic policy is Wrapping. Bitwise AND has no overflow policy;
    /// its children still owe their individually selected formation.
    /// Exact and Trapping operations share a candidate integer denotation when
    /// their operands and result fit. Consumers must admit that landing before
    /// installing any consequence; this does not alter raw policy evaluation.
    pub(in crate::proof_contracts::contract_entailment) fn integer_operation_meaning(
        operator: BinaryOperator,
        meaning: [(PrimitiveType, ArithmeticDomain); 2],
    ) -> bool {
        let [(left, left_policy), (right, right_policy)] = meaning;
        if builtin_integer_result_carrier(operator, [left, right]).is_none() {
            return false;
        }
        let preserves_formed_value =
            |policy| matches!(policy, ArithmeticDomain::Exact | ArithmeticDomain::Trapping);
        match operator {
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
                preserves_formed_value(left_policy)
            }
            BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr | BinaryOperator::BitwiseXor => {
                true
            }
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo => {
                preserves_formed_value(left_policy) && preserves_formed_value(right_policy)
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
            BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor => {
                let bits = crate::proof_contracts::arithmetic_domains::integer_bit_width(primitive)?
                    as u32;
                match operator {
                    BinaryOperator::ShiftLeft => self.integer_shift_left(left, right, bits),
                    BinaryOperator::ShiftRight => self.integer_shift_right(left, right, bits),
                    _ => self.integer_bitwise(
                        operator,
                        left,
                        right,
                        bits,
                        primitive.is_signed_integer(),
                    ),
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
        // Candidate denotation is not admission. In particular quotient and
        // remainder laws must not enter the engine while this operation's
        // divisor, child formation or carrier fit is still being proved.
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

    /// Only independently available operand facts can justify creating a
    /// candidate. No result atom or interval exists at this admission cut.
    fn integer_operation_inputs_land(
        &self,
        operator: BinaryOperator,
        primitives: [PrimitiveType; 2],
        policy: ArithmeticDomain,
        left: &Polynomial,
        right: &Polynomial,
    ) -> bool {
        let [primitive, right_primitive] = primitives;
        if builtin_integer_result_carrier(operator, primitives).is_none()
            || !self.integer_value_lands(primitive, left)
            || !self.integer_value_lands(right_primitive, right)
        {
            return false;
        }
        if matches!(
            operator,
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
        ) && policy != ArithmeticDomain::Wrapping
        {
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
                && matches!(policy, ArithmeticDomain::Exact | ArithmeticDomain::Trapping)
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

    /// Build and publish one selected operation transactionally. Failed
    /// operand, definedness or result judgments leave the original engine
    /// unchanged, including opaque intervals and contradiction state.
    pub(in crate::proof_contracts::contract_entailment) fn admit_selected_integer_operation(
        &mut self,
        operator: BinaryOperator,
        meaning: [(PrimitiveType, ArithmeticDomain); 2],
        left: &Polynomial,
        right: &Polynomial,
        binding: Option<&Polynomial>,
    ) -> Option<Polynomial> {
        let primitives = meaning.map(|(primitive, _)| primitive);
        let policy = meaning[0].1;
        if !self.integer_operation_inputs_land(operator, primitives, policy, left, right) {
            return None;
        }
        let result = builtin_integer_result_carrier(operator, primitives)?;
        let mut admitted = self.clone();
        let left_value = admitted.substituted(left);
        let mut right_value = admitted.substituted(right);
        if policy == ArithmeticDomain::Wrapping
            && matches!(
                operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            )
        {
            let width = crate::proof_contracts::arithmetic_domains::integer_bit_width(result)?;
            let count_bits =
                crate::proof_contracts::arithmetic_domains::integer_bit_width(primitives[1])?
                    as u32;
            // Every builtin width is a power of two. The low count bits are
            // Euclidean modulo width for signed and unsigned carrier values,
            // including a negative signed count. This is language semantics,
            // never an ISA-mask substitute for an Exact/Saturating count proof.
            right_value = admitted.integer_bitwise(
                BinaryOperator::BitwiseAnd,
                right_value,
                Polynomial::constant(BigInt::from_i64(width - 1)),
                count_bits,
                primitives[1].is_signed_integer(),
            )?;
        }
        let effective_right = right_value.clone();
        let value = if Self::integer_operation_meaning(operator, meaning) {
            admitted.selected_integer_binary_value(operator, meaning, left_value, right_value)?
        } else {
            // Source formation can carry an explicitly selected Wrapping or
            // Saturating operation. Preserve that policy's actual value; this
            // does not make its unbounded candidate an Exact recipe.
            let policy = meaning[0].1;
            if !matches!(
                policy,
                ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating
            ) || (!matches!(
                operator,
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
            ) && meaning[1].1 != policy)
            {
                return None;
            }
            let candidate = match operator {
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
                    let bits =
                        crate::proof_contracts::arithmetic_domains::integer_bit_width(result)?
                            as u32;
                    if operator == BinaryOperator::ShiftLeft {
                        admitted.integer_shift_left(left_value, right_value, bits)?
                    } else {
                        admitted.integer_shift_right(left_value, right_value, bits)?
                    }
                }
                _ => admitted.integer_binary_value(operator, left_value, right_value)?,
            };
            admitted.integer_policy_value(result, policy, candidate)?
        };
        if !admitted.integer_value_lands(result, &value) {
            return None;
        }
        // Shift consequences refer to the reduced count, not a discarded
        // positive multiple of width whose actual shift is zero.
        admitted.install_integer_operation_bounds(operator, left, &effective_right, &value);
        if let Some(binding) = binding
            && !admitted.install_hypotheses(vec![(
                BinaryOperator::Equal,
                binding.clone(),
                value.clone(),
            )])
        {
            return None;
        }
        *self = admitted;
        Some(value)
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
        if let Some(bound) = consequence
            && !self.bounds.contains(&bound)
        {
            self.bounds.push(bound);
            self.refresh_difference_bounds();
            return true;
        }
        false
    }
}

fn complete_integer_policy(
    domain: ArithmeticDomain,
    value: &BigInt,
    minimum: &BigInt,
    maximum: &BigInt,
) -> Option<BigInt> {
    match domain {
        ArithmeticDomain::Wrapping => {
            let modulus = maximum.sub(minimum).add(&BigInt::from_i64(1));
            let (_, remainder) = value.sub(minimum).div_rem(&modulus)?;
            let residue = if remainder.is_negative() {
                remainder.add(&modulus)
            } else {
                remainder
            };
            Some(minimum.add(&residue))
        }
        ArithmeticDomain::Saturating => {
            Some(value.clone().max(minimum.clone()).min(maximum.clone()))
        }
        _ => None,
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;

    fn constant(value: i64) -> Polynomial {
        Polynomial::constant(BigInt::from_i64(value))
    }

    #[test]
    fn signed_quotient_overflow_follows_selected_policy() {
        let program = typed_trees::TypedTrees::default();
        for (policy, quotient) in [
            (ArithmeticDomain::Exact, None),
            (ArithmeticDomain::Trapping, None),
            (ArithmeticDomain::Wrapping, Some(-128)),
            (ArithmeticDomain::Saturating, Some(127)),
        ] {
            for (operator, expected) in [
                (BinaryOperator::Divide, quotient),
                (BinaryOperator::Modulo, quotient.map(|_| 0)),
            ] {
                let mut engine = Engine::for_explicit_bindings(&program);
                let result = engine.admit_selected_integer_operation(
                    operator,
                    [(PrimitiveType::I8, policy); 2],
                    &constant(-128),
                    &constant(-1),
                    None,
                );
                assert_eq!(
                    result.and_then(|value| value.constant_value()),
                    expected.map(BigInt::from_i64)
                );
            }
            let mut engine = Engine::for_explicit_bindings(&program);
            let before = engine.integer_projection_registrations();
            assert!(
                engine
                    .admit_selected_integer_operation(
                        BinaryOperator::Modulo,
                        [(PrimitiveType::I8, policy); 2],
                        &constant(7),
                        &constant(0),
                        None,
                    )
                    .is_none()
            );
            assert_eq!(before, engine.integer_projection_registrations());
        }
    }

    #[test]
    fn wrapping_shifts_reduce_signed_counts_without_false_descent() {
        let program = typed_trees::TypedTrees::default();
        for (operator, value, count, expected) in [
            (BinaryOperator::ShiftLeft, 1, 8, 1),
            (BinaryOperator::ShiftLeft, 1, -1, 128),
            (BinaryOperator::ShiftRight, 7, 8, 7),
        ] {
            let mut engine = Engine::for_explicit_bindings(&program);
            let result = engine
                .admit_selected_integer_operation(
                    operator,
                    [
                        (PrimitiveType::U8, ArithmeticDomain::Wrapping),
                        (PrimitiveType::I8, ArithmeticDomain::Exact),
                    ],
                    &constant(value),
                    &constant(count),
                    None,
                )
                .expect("wrapping count has a defined Euclidean reduction");
            assert_eq!(result.constant_value(), Some(BigInt::from_i64(expected)));
            if operator == BinaryOperator::ShiftRight {
                assert!(
                    !engine.prove_at_least(&constant(value).sub(&result), &BigInt::from_i64(1))
                );
            }
        }
        for policy in [
            ArithmeticDomain::Exact,
            ArithmeticDomain::Trapping,
            ArithmeticDomain::Saturating,
        ] {
            let mut engine = Engine::for_explicit_bindings(&program);
            assert!(
                engine
                    .admit_selected_integer_operation(
                        BinaryOperator::ShiftLeft,
                        [
                            (PrimitiveType::U8, policy),
                            (PrimitiveType::I8, ArithmeticDomain::Wrapping)
                        ],
                        &constant(1),
                        &constant(8),
                        None,
                    )
                    .is_none()
            );
        }
    }
}
