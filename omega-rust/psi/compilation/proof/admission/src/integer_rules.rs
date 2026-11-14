/*
    Independently checked integer rules: budgeted closed integer evaluation,
    open-term ring normalization, fixed native integer readings, and cast,
    forbidden-root, shift, and wrapping-equality witness replay.

    We re-decide closed arbitrary-precision integer relations under explicit node
    and limb budgets and replay step-by-step cast, shift, and correlated
    forbidden-root witnesses against cited semantic axioms.
*/

#[cfg(test)]
use semantic_vocabulary::IntegerType;
pub(crate) mod closed_integer {
    use std::cmp::Ordering;

    use numerics::bignum::BigInt;
    use semantic_vocabulary::{IntegerMathLiteral, IntegerMathTerm};

    const MAXIMUM_TERM_NODES: usize = 8_192;
    const MAXIMUM_TERM_DEPTH: usize = 128;
    const MAXIMUM_RESULT_BITS: usize = 65_536;
    const MAXIMUM_ALLOCATION_LIMBS: usize = 262_144;
    const MAXIMUM_LIMB_WORK: usize = 1_048_576;

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

    #[derive(Default)]
    pub struct ClosedIntegerEvaluator {
        term_nodes: usize,
        allocation_limbs: usize,
        limb_work: usize,
    }

    impl ClosedIntegerEvaluator {
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

        pub fn evaluate_closed(
            &mut self,
            term: &IntegerMathTerm,
        ) -> Result<Option<BigInt>, ClosedIntegerEvaluationError> {
            self.check_term_size(term)?;
            self.evaluate(term)
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
                    IntegerMathTerm::Embed(scalar) => {
                        let mut pending = vec![(scalar.as_ref(), depth + 1)];
                        while let Some((scalar, depth)) = pending.pop() {
                            charge_limit(&mut self.term_nodes, 1, MAXIMUM_TERM_NODES)?;
                            if depth > MAXIMUM_TERM_DEPTH {
                                return Err(ClosedIntegerEvaluationError::ResourceLimitExceeded);
                            }
                            pending.extend(
                                scalar
                                    .operands()
                                    .into_iter()
                                    .map(|operand| (operand, depth + 1)),
                            );
                        }
                    }
                    term => {
                        for operand in term.operands().into_iter().rev() {
                            pending.push((operand, depth + 1));
                        }
                    }
                }
            }
            Ok(())
        }

        fn evaluate(
            &mut self,
            term: &IntegerMathTerm,
        ) -> Result<Option<BigInt>, ClosedIntegerEvaluationError> {
            let (left, right) = match term {
                IntegerMathTerm::MathValue { .. } => return Ok(None),
                IntegerMathTerm::Embed(scalar) => {
                    self.charge(8, 8)?;
                    return Ok(scalar.integer_value().map(|(_, value)| {
                        big_integer_literal(IntegerMathLiteral::from_integer_value(value))
                    }));
                }
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
            let allocation_limbs = 4 * (left_limbs + right_limbs + result_bits.div_ceil(64) + 1);
            self.charge(allocation_limbs, work)?;
            let result = match term {
                IntegerMathTerm::Add(_, _) => left.add(&right),
                IntegerMathTerm::Subtract(_, _) => left.sub(&right),
                IntegerMathTerm::Multiply(_, _) => left.mul(&right),
                IntegerMathTerm::ShiftLeft { .. } => left.shl_bits(shift_count),
                IntegerMathTerm::MathValue { .. }
                | IntegerMathTerm::IntegerLiteral(_)
                | IntegerMathTerm::Embed(_) => {
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

    pub fn compare_integer_math_terms(
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<Option<Ordering>, ClosedIntegerEvaluationError> {
        let mut evaluator = ClosedIntegerEvaluator::default();
        if let Some(ordering) = evaluator.compare(left, right)? {
            return Ok(Some(ordering));
        }
        super::open_terms::constant_difference_ordering(left, right)
    }

    pub(crate) fn check_integer_math_term_size(
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<(), ClosedIntegerEvaluationError> {
        let mut evaluator = ClosedIntegerEvaluator::default();
        evaluator.check_term_size(left)?;
        evaluator.check_term_size(right)
    }

    pub fn big_integer_literal(literal: IntegerMathLiteral) -> BigInt {
        let magnitude = BigInt::from_u128(literal.magnitude());
        if literal.negative() {
            magnitude.negate()
        } else {
            magnitude
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            ClosedIntegerEvaluationError, ClosedIntegerEvaluator, IntegerMathTerm,
            MAXIMUM_ALLOCATION_LIMBS, MAXIMUM_LIMB_WORK, MAXIMUM_RESULT_BITS, MAXIMUM_TERM_DEPTH,
            MAXIMUM_TERM_NODES, Ordering, compare_integer_math_terms,
        };
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
            let negative =
                IntegerMathTerm::Subtract(Box::new(integer(0)), Box::new(beyond.clone()));
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
        fn embedding_observes_wrapped_result_not_unbounded_sum() {
            use semantic_vocabulary::ScalarTerm;
            let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let scalar = ScalarTerm::WrappingIntegerAdd {
                scalar_type: byte,
                left: Box::new(ScalarTerm::integer(byte, IntegerValue::Unsigned(255)).unwrap()),
                right: Box::new(ScalarTerm::integer(byte, IntegerValue::Unsigned(1)).unwrap()),
            };
            let embedded = IntegerMathTerm::embed(scalar).unwrap();
            assert_eq!(
                compare_integer_math_terms(&embedded, &integer(0)),
                Ok(Some(Ordering::Equal))
            );
            assert_eq!(
                compare_integer_math_terms(&embedded, &integer(256)),
                Ok(Some(Ordering::Less))
            );
            let mathematical = IntegerMathTerm::Add(Box::new(integer(255)), Box::new(integer(1)));
            assert_eq!(
                compare_integer_math_terms(&mathematical, &integer(256)),
                Ok(Some(Ordering::Equal))
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
}
pub(crate) mod fixed_integer {
    use semantic_vocabulary::{IntegerType, IntegerValue, ScalarTerm};

    pub(crate) fn fixed_native(integer_type: IntegerType) -> bool {
        !integer_type.is_address() && matches!(integer_type.bits(), Some(8 | 16 | 32 | 64))
    }

    pub(crate) fn fixed_native_interval(integer_type: IntegerType) -> Option<(i128, i128)> {
        if !fixed_native(integer_type) {
            return None;
        }
        Some((
            integer_value_as_i128(integer_type.minimum_value()?)?,
            integer_value_as_i128(integer_type.maximum_value()?)?,
        ))
    }

    pub(crate) fn integer_value_as_i128(value: IntegerValue) -> Option<i128> {
        match value {
            IntegerValue::Signed(value) => Some(value),
            IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
        }
    }

    pub(crate) fn fixed_integer_as_i128(
        term: &ScalarTerm,
        integer_type: IntegerType,
    ) -> Option<i128> {
        let (actual_type, value) = term.integer_value()?;
        (actual_type == integer_type)
            .then(|| integer_value_as_i128(value))
            .flatten()
    }

    pub(crate) fn fixed_integer_term(integer_type: IntegerType, value: i128) -> Option<ScalarTerm> {
        let value = IntegerValue::for_sign(integer_type.sign(), value).ok()?;
        ScalarTerm::integer(integer_type, value).ok()
    }
}
pub(crate) use crate::integer_affine;
pub(crate) mod integer_cast {
    pub use terminal_psi::IntegerCastChainWitness;

    use crate::integer_rules::fixed_integer::{
        fixed_integer_as_i128, fixed_integer_term, fixed_native_interval,
    };
    use semantic_vocabulary::{
        IntegerMathTerm, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedIntegerCastChain {
        root: ScalarTerm,
        target: ScalarTerm,
        carriers: Vec<IntegerType>,
        definition_axioms: Vec<usize>,
        surviving_root_interval: Option<(i128, i128)>,
    }

    impl CheckedIntegerCastChain {
        pub const fn root(&self) -> &ScalarTerm {
            &self.root
        }

        pub const fn target(&self) -> &ScalarTerm {
            &self.target
        }

        pub fn carriers(&self) -> &[IntegerType] {
            &self.carriers
        }

        pub fn definition_axioms(&self) -> &[usize] {
            &self.definition_axioms
        }

        pub const fn surviving_root_interval(&self) -> Option<(i128, i128)> {
            self.surviving_root_interval
        }
    }

    pub fn check_integer_cast_chain_witness(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        witness: &IntegerCastChainWitness,
    ) -> Result<CheckedIntegerCastChain, IntegerCastChainWitnessError> {
        let Some(root_type) = witness.root.scalar_type().as_integer() else {
            return Err(IntegerCastChainWitnessError::RootNotInteger);
        };
        let mut interval = fixed_native_interval(root_type)
            .ok_or(IntegerCastChainWitnessError::UnsupportedCarrier(root_type))?;
        if witness.definition_axioms.is_empty() {
            return Err(IntegerCastChainWitnessError::EmptyCastChain);
        }
        if witness
            .definition_axioms
            .windows(2)
            .any(|indices| indices[0] >= indices[1])
        {
            return Err(IntegerCastChainWitnessError::NonCanonicalDefinitionOrder);
        }

        let mut current = witness.root.clone();
        let mut current_type = root_type;
        let mut carriers = vec![root_type];
        let mut values = vec![current.clone()];
        for &index in &witness.definition_axioms {
            let proposition = checked_axiom(context, semantic_axioms, index)?;
            let Proposition::Equal(next, definition) = proposition else {
                return Err(IntegerCastChainWitnessError::DefinitionNotEquality(index));
            };
            let ScalarTerm::Value { id: next_id, .. } = next else {
                return Err(IntegerCastChainWitnessError::DefinitionTargetNotValue(
                    index,
                ));
            };
            let (source_type, target_type, operand, exact_cast) = match definition {
                ScalarTerm::IntegerExactCast {
                    source_type,
                    target_type,
                    operand,
                } => (source_type, target_type, operand, true),
                ScalarTerm::IntegerWiden {
                    source_type,
                    target_type,
                    operand,
                } => (source_type, target_type, operand, false),
                _ => {
                    return Err(IntegerCastChainWitnessError::DefinitionNotExactCast(index));
                }
            };
            if operand.as_ref() != &current
                || *source_type != current_type
                || next.scalar_type() != ScalarType::Integer(*target_type)
            {
                return Err(IntegerCastChainWitnessError::CastChainMismatch(index));
            }
            if exact_cast && !partial_fixed_native_cast(*source_type, *target_type) {
                return Err(IntegerCastChainWitnessError::NonPartialCastEdge {
                    index,
                    source: *source_type,
                    target: *target_type,
                });
            }
            if !exact_cast && !strict_fixed_native_widen(*source_type, *target_type) {
                return Err(IntegerCastChainWitnessError::InvalidWidenEdge {
                    index,
                    source: *source_type,
                    target: *target_type,
                });
            }
            if values.contains(next) || witness.root.any_value_id(|root_id| root_id == *next_id) {
                return Err(IntegerCastChainWitnessError::CyclicValue(index));
            }
            let target_interval = fixed_native_interval(*target_type).ok_or(
                IntegerCastChainWitnessError::UnsupportedCarrier(*target_type),
            )?;
            interval.0 = interval.0.max(target_interval.0);
            interval.1 = interval.1.min(target_interval.1);
            current = next.clone();
            current_type = *target_type;
            carriers.push(*target_type);
            values.push(current.clone());
        }
        if current != witness.target {
            return Err(IntegerCastChainWitnessError::TargetMismatch);
        }

        Ok(CheckedIntegerCastChain {
            root: witness.root.clone(),
            target: witness.target.clone(),
            carriers,
            definition_axioms: witness.definition_axioms.clone(),
            surviving_root_interval: (interval.0 <= interval.1).then_some(interval),
        })
    }

    fn partial_fixed_native_cast(source: IntegerType, target: IntegerType) -> bool {
        fixed_native_interval(source).is_some()
            && fixed_native_interval(target).is_some()
            && source != target
            && source.can_exact_cast_to(target)
            && !source.can_widen_to(target)
    }

    fn strict_fixed_native_widen(source: IntegerType, target: IntegerType) -> bool {
        fixed_native_interval(source).is_some()
            && fixed_native_interval(target).is_some()
            && source != target
            && source.can_widen_to(target)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum IntegerCastChainWitnessError {
        RootNotInteger,
        UnsupportedCarrier(IntegerType),
        EmptyCastChain,
        NonCanonicalDefinitionOrder,
        UnknownSemanticAxiom(usize),
        MalformedProposition(semantic_vocabulary::PropositionError),
        DefinitionNotEquality(usize),
        DefinitionTargetNotValue(usize),
        DefinitionNotExactCast(usize),
        CastChainMismatch(usize),
        NonPartialCastEdge {
            index: usize,
            source: IntegerType,
            target: IntegerType,
        },
        InvalidWidenEdge {
            index: usize,
            source: IntegerType,
            target: IntegerType,
        },
        CyclicValue(usize),
        TargetMismatch,
    }

    crate::debug_display!(IntegerCastChainWitnessError);
    crate::checked_axiom!(IntegerCastChainWitnessError);

    impl std::error::Error for IntegerCastChainWitnessError {}

    pub fn check_integer_cast_bound_conversion(
        chain: &CheckedIntegerCastChain,
        root_bound: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), IntegerCastBoundConversionError> {
        fn embedded_value(term: &ScalarTerm) -> Option<IntegerMathTerm> {
            let ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(source_type),
            } = term
            else {
                return None;
            };
            Some(IntegerMathTerm::MathValue {
                source_type: *source_type,
                value: *id,
            })
        }
        let mathematical_root =
            crate::lift_fixed_integer_relation(root_bound).unwrap_or_else(|| root_bound.clone());
        let mathematical_conclusion =
            crate::lift_fixed_integer_relation(conclusion).unwrap_or_else(|| conclusion.clone());
        if let (
            Some((root_strict, root_left, root_right)),
            Some((target_strict, target_left, target_right)),
        ) = (
            crate::proof::mathematical_order(&mathematical_root),
            crate::proof::mathematical_order(&mathematical_conclusion),
        ) && root_strict == target_strict
            && ((Some(root_left) == embedded_value(chain.root()).as_ref()
                && Some(target_left) == embedded_value(chain.target()).as_ref()
                && root_right == target_right)
                || (Some(root_right) == embedded_value(chain.root()).as_ref()
                    && Some(target_right) == embedded_value(chain.target()).as_ref()
                    && root_left == target_left))
        {
            return Ok(());
        }
        let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
            return Err(IntegerCastBoundConversionError::RootBoundNotLessOrEqual);
        };
        let (root_literal, root_is_left) = if root_left == chain.root() {
            (root_right, true)
        } else if root_right == chain.root() {
            (root_left, false)
        } else {
            return Err(IntegerCastBoundConversionError::RootBoundMismatch);
        };
        let source_type = chain.carriers()[0];
        let Some(root_value) = fixed_integer_as_i128(root_literal, source_type) else {
            return Err(IntegerCastBoundConversionError::RootBoundNotTypedLiteral);
        };

        let Proposition::LessOrEqual(target_left, target_right) = conclusion else {
            return Err(IntegerCastBoundConversionError::ConclusionNotLessOrEqual);
        };
        let target_literal = if root_is_left {
            if target_left != chain.target() {
                return Err(IntegerCastBoundConversionError::ConclusionTargetMismatch);
            }
            target_right
        } else {
            if target_right != chain.target() {
                return Err(IntegerCastBoundConversionError::ConclusionTargetMismatch);
            }
            target_left
        };
        let target_type = *chain
            .carriers()
            .last()
            .expect("a checked cast chain retains its target carrier");
        let Some(target_value) = fixed_integer_as_i128(target_literal, target_type) else {
            return Err(IntegerCastBoundConversionError::ConclusionNotTypedLiteral);
        };
        if target_value != root_value {
            return Err(IntegerCastBoundConversionError::ConclusionLiteralMismatch);
        }
        Ok(())
    }

    pub fn integer_cast_truth_bounds(
        chain: &CheckedIntegerCastChain,
    ) -> Result<Vec<Proposition>, IntegerCastBoundConversionError> {
        let (minimum, maximum) = chain
            .surviving_root_interval()
            .ok_or(IntegerCastBoundConversionError::EmptySurvivingInterval)?;
        let target_type = *chain
            .carriers()
            .last()
            .expect("a checked cast chain retains its target carrier");
        let literal = |value| {
            fixed_integer_term(target_type, value)
                .ok_or(IntegerCastBoundConversionError::ConclusionNotTypedLiteral)
        };
        Ok(vec![
            Proposition::LessOrEqual(literal(minimum)?, chain.target().clone()),
            Proposition::LessOrEqual(chain.target().clone(), literal(maximum)?),
        ])
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum IntegerCastBoundConversionError {
        RootBoundNotLessOrEqual,
        RootBoundMismatch,
        RootBoundNotTypedLiteral,
        ConclusionNotLessOrEqual,
        ConclusionTargetMismatch,
        ConclusionNotTypedLiteral,
        ConclusionLiteralMismatch,
        EmptySurvivingInterval,
    }

    crate::debug_display!(IntegerCastBoundConversionError);

    impl std::error::Error for IntegerCastBoundConversionError {}

    #[cfg(test)]
    mod tests {
        use super::super::value;
        use super::{
            CheckedIntegerCastChain, IntegerCastBoundConversionError, IntegerCastChainWitness,
            IntegerCastChainWitnessError, IntegerType, Proposition, PropositionContext, ScalarTerm,
            ScalarType, check_integer_cast_bound_conversion, check_integer_cast_chain_witness,
        };
        use semantic_vocabulary::{IntegerSign, IntegerValue, ValueId};

        fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
            IntegerType::new(sign, bits).expect("fixed integer type")
        }

        fn cast_definition(
            target: ScalarTerm,
            source_type: IntegerType,
            target_type: IntegerType,
            operand: ScalarTerm,
        ) -> Proposition {
            Proposition::Equal(
                target,
                ScalarTerm::integer_exact_cast(source_type, target_type, operand)
                    .expect("fixed exact cast term"),
            )
        }

        #[test]
        fn normalizes_single_and_multi_cast_cores_with_exact_carrier_intersection() {
            let i64_type = integer_type(IntegerSign::Signed, 64);
            let u64_type = integer_type(IntegerSign::Unsigned, 64);
            let i32_type = integer_type(IntegerSign::Signed, 32);
            let u8_type = integer_type(IntegerSign::Unsigned, 8);
            let root = value(1, i64_type);
            let first = value(2, u64_type);
            let second = value(3, i32_type);
            let target = value(4, u8_type);
            let axioms = vec![
                cast_definition(first.clone(), i64_type, u64_type, root.clone()),
                cast_definition(second.clone(), u64_type, i32_type, first.clone()),
                cast_definition(target.clone(), i32_type, u8_type, second),
            ];
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(i64_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(u64_type)),
                (ValueId::new(3).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(4).unwrap(), ScalarType::Integer(u8_type)),
            ])
            .unwrap();

            let single = check_integer_cast_chain_witness(
                &context,
                &axioms,
                &IntegerCastChainWitness {
                    root: root.clone(),
                    target: first,
                    definition_axioms: vec![0],
                },
            )
            .expect("one partial cast is the shared sandwich core");
            assert_eq!(single.surviving_root_interval(), Some((0, i64::MAX.into())));

            let checked = check_integer_cast_chain_witness(
                &context,
                &axioms,
                &IntegerCastChainWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms: vec![0, 1, 2],
                },
            )
            .expect("ordered partial cast chain");
            assert_eq!(checked.root(), &root);
            assert_eq!(checked.target(), &target);
            assert_eq!(checked.carriers(), &[i64_type, u64_type, i32_type, u8_type],);
            assert_eq!(checked.definition_axioms(), &[0, 1, 2]);
            assert_eq!(checked.surviving_root_interval(), Some((0, u8::MAX.into())));
        }

        #[test]
        fn maps_contiguous_cast_bounds_and_rejects_shape_or_endpoint_drift() {
            let i16_type = integer_type(IntegerSign::Signed, 16);
            let i8_type = integer_type(IntegerSign::Signed, 8);
            let root = value(1, i16_type);
            let target = value(2, i8_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(i16_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
            ])
            .unwrap();
            let chain = check_integer_cast_chain_witness(
                &context,
                &[cast_definition(
                    target.clone(),
                    i16_type,
                    i8_type,
                    root.clone(),
                )],
                &IntegerCastChainWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms: vec![0],
                },
            )
            .expect("single partial cast");
            let i16_one = ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).unwrap();
            let i8_one = ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).unwrap();
            let root_bound = Proposition::LessOrEqual(i16_one, root);
            let conclusion = Proposition::LessOrEqual(i8_one.clone(), target.clone());
            assert_eq!(
                check_integer_cast_bound_conversion(&chain, &root_bound, &conclusion),
                Ok(()),
            );
            assert_eq!(
                check_integer_cast_bound_conversion(
                    &chain,
                    &root_bound,
                    &Proposition::LessOrEqual(target, i8_one),
                ),
                Err(IntegerCastBoundConversionError::ConclusionTargetMismatch),
            );

            let i32_type = integer_type(IntegerSign::Signed, 32);
            let wide_root = value(3, i32_type);
            let middle = value(1, i16_type);
            let target = value(2, i8_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(3).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(1).unwrap(), ScalarType::Integer(i16_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
            ])
            .unwrap();
            let chain = check_integer_cast_chain_witness(
                &context,
                &[
                    cast_definition(middle.clone(), i32_type, i16_type, wide_root.clone()),
                    cast_definition(target.clone(), i16_type, i8_type, middle),
                ],
                &IntegerCastChainWitness {
                    root: wide_root.clone(),
                    target,
                    definition_axioms: vec![0, 1],
                },
            )
            .expect("two partial casts");
            let wide_bound = Proposition::LessOrEqual(
                ScalarTerm::integer(i32_type, IntegerValue::Signed(1)).unwrap(),
                wide_root,
            );
            assert_eq!(
                check_integer_cast_bound_conversion(&chain, &wide_bound, &conclusion),
                Ok(()),
            );
            assert_eq!(
                check_integer_cast_bound_conversion(
                    &chain,
                    &wide_bound,
                    &Proposition::LessOrEqual(
                        ScalarTerm::integer(i8_type, IntegerValue::Signed(2)).unwrap(),
                        value(2, i8_type),
                    ),
                ),
                Err(IntegerCastBoundConversionError::ConclusionLiteralMismatch),
            );
        }

        #[test]
        fn rejects_widening_stale_reordered_reversed_and_discontinuous_claims() {
            let i16_type = integer_type(IntegerSign::Signed, 16);
            let u16_type = integer_type(IntegerSign::Unsigned, 16);
            let u8_type = integer_type(IntegerSign::Unsigned, 8);
            let i8_type = integer_type(IntegerSign::Signed, 8);
            let root = value(1, i16_type);
            let first = value(2, u16_type);
            let target = value(3, i8_type);
            let axioms = vec![
                cast_definition(first.clone(), i16_type, u16_type, root.clone()),
                cast_definition(target.clone(), u16_type, i8_type, first.clone()),
            ];
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(i16_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(u16_type)),
                (ValueId::new(3).unwrap(), ScalarType::Integer(i8_type)),
                (ValueId::new(4).unwrap(), ScalarType::Integer(u8_type)),
            ])
            .unwrap();
            let witness = |definition_axioms| IntegerCastChainWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms,
            };
            assert_eq!(
                check_integer_cast_chain_witness(&context, &axioms, &witness(vec![2])),
                Err(IntegerCastChainWitnessError::UnknownSemanticAxiom(2)),
            );
            assert_eq!(
                check_integer_cast_chain_witness(&context, &axioms, &witness(vec![1, 0])),
                Err(IntegerCastChainWitnessError::NonCanonicalDefinitionOrder),
            );
            assert_eq!(
                check_integer_cast_chain_witness(
                    &context,
                    &[Proposition::Equal(
                        ScalarTerm::integer_exact_cast(i16_type, u16_type, root.clone(),).unwrap(),
                        first.clone(),
                    )],
                    &IntegerCastChainWitness {
                        root: root.clone(),
                        target: first.clone(),
                        definition_axioms: vec![0],
                    },
                ),
                Err(IntegerCastChainWitnessError::DefinitionTargetNotValue(0)),
            );
            assert_eq!(
                check_integer_cast_chain_witness(
                    &context,
                    &[cast_definition(
                        value(4, u8_type),
                        i16_type,
                        u8_type,
                        root.clone(),
                    )],
                    &IntegerCastChainWitness {
                        root: root.clone(),
                        target: value(4, u8_type),
                        definition_axioms: vec![0],
                    },
                ),
                Ok(CheckedIntegerCastChain {
                    root: root.clone(),
                    target: value(4, u8_type),
                    carriers: vec![i16_type, u8_type],
                    definition_axioms: vec![0],
                    surviving_root_interval: Some((0, u8::MAX.into())),
                }),
                "a narrowing edge is the intended partial core",
            );

            let u8_root = value(4, u8_type);
            let widened = value(1, i16_type);
            assert_eq!(
                check_integer_cast_chain_witness(
                    &context,
                    &[cast_definition(
                        widened.clone(),
                        u8_type,
                        i16_type,
                        u8_root.clone(),
                    )],
                    &IntegerCastChainWitness {
                        root: u8_root,
                        target: widened,
                        definition_axioms: vec![0],
                    },
                ),
                Err(IntegerCastChainWitnessError::NonPartialCastEdge {
                    index: 0,
                    source: u8_type,
                    target: i16_type,
                }),
            );
            assert_eq!(
                check_integer_cast_chain_witness(
                    &context,
                    &axioms,
                    &IntegerCastChainWitness {
                        root,
                        target,
                        definition_axioms: vec![1],
                    },
                ),
                Err(IntegerCastChainWitnessError::CastChainMismatch(1)),
            );
            assert_eq!(
                check_integer_cast_chain_witness(
                    &context,
                    &axioms,
                    &IntegerCastChainWitness {
                        root: value(1, i16_type),
                        target: value(3, i8_type),
                        definition_axioms: vec![0],
                    },
                ),
                Err(IntegerCastChainWitnessError::TargetMismatch),
            );
        }

        #[test]
        fn rejects_non_native_and_cyclic_cast_words() {
            let address_type = IntegerType::address(64).expect("address");
            let address_root = value(1, address_type);
            let address_context = PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(address_type),
            )])
            .unwrap();
            assert_eq!(
                check_integer_cast_chain_witness(
                    &address_context,
                    &[],
                    &IntegerCastChainWitness {
                        root: address_root.clone(),
                        target: address_root,
                        definition_axioms: vec![0],
                    },
                ),
                Err(IntegerCastChainWitnessError::UnsupportedCarrier(
                    address_type,
                )),
            );

            let i128_type = integer_type(IntegerSign::Signed, 128);
            let u128_type = integer_type(IntegerSign::Unsigned, 128);
            let unsupported_root = value(1, i128_type);
            let unsupported_target = value(2, u128_type);
            let unsupported_context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(i128_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(u128_type)),
            ])
            .unwrap();
            assert_eq!(
                check_integer_cast_chain_witness(
                    &unsupported_context,
                    &[cast_definition(
                        unsupported_target.clone(),
                        i128_type,
                        u128_type,
                        unsupported_root.clone(),
                    )],
                    &IntegerCastChainWitness {
                        root: unsupported_root,
                        target: unsupported_target,
                        definition_axioms: vec![0],
                    },
                ),
                Err(IntegerCastChainWitnessError::UnsupportedCarrier(i128_type)),
            );

            let i16_type = integer_type(IntegerSign::Signed, 16);
            let u16_type = integer_type(IntegerSign::Unsigned, 16);
            let root = value(1, i16_type);
            let middle = value(2, u16_type);
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(i16_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(u16_type)),
            ])
            .unwrap();
            assert_eq!(
                check_integer_cast_chain_witness(
                    &context,
                    &[
                        cast_definition(middle.clone(), i16_type, u16_type, root.clone()),
                        cast_definition(root.clone(), u16_type, i16_type, middle),
                    ],
                    &IntegerCastChainWitness {
                        root: root.clone(),
                        target: root,
                        definition_axioms: vec![0, 1],
                    },
                ),
                Err(IntegerCastChainWitnessError::CyclicValue(1)),
            );
        }
    }

    #[cfg(test)]
    mod expression_roots {
        use super::{
            IntegerCastChainWitness, IntegerCastChainWitnessError, check_integer_cast_chain_witness,
        };
        use crate::proof::assumption_proof;
        use crate::{MathematicalCoreDecision, ProofNode, ProofRule, accept_certificate};
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
            PropositionError, ScalarTerm, ScalarType, ValueId,
        };

        macro_rules! check {
            ($context:expr => $axioms:expr => $witness:expr) => {
                check_integer_cast_chain_witness($context, $axioms, $witness)
            };
        }

        fn unsigned_type(bits: u16) -> IntegerType {
            IntegerType::new(IntegerSign::Unsigned, bits).unwrap()
        }

        fn value(identity: u64, integer_type: IntegerType) -> ScalarTerm {
            ScalarTerm::value(
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer_type),
            )
        }

        fn literal(integer_type: IntegerType, number: u128) -> ScalarTerm {
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap()
        }

        struct RemainderCast {
            context: PropositionContext,
            source_type: IntegerType,
            target_type: IntegerType,
            root: ScalarTerm,
            target: ScalarTerm,
        }

        impl RemainderCast {
            fn new() -> Self {
                let source_type = unsigned_type(32);
                let target_type = unsigned_type(8);
                Self {
                    context: PropositionContext::from_value_types([
                        (ValueId::new(1).unwrap(), ScalarType::Integer(source_type)),
                        (ValueId::new(2).unwrap(), ScalarType::Integer(target_type)),
                    ])
                    .unwrap(),
                    source_type,
                    target_type,
                    root: ScalarTerm::exact_integer_remainder(
                        source_type,
                        value(1, source_type),
                        literal(source_type, 256),
                    )
                    .unwrap(),
                    target: value(2, target_type),
                }
            }

            fn definition(&self) -> Proposition {
                Proposition::Equal(
                    self.target.clone(),
                    ScalarTerm::IntegerExactCast {
                        source_type: self.source_type,
                        target_type: self.target_type,
                        operand: Box::new(self.root.clone()),
                    },
                )
            }

            fn witness(&self) -> IntegerCastChainWitness {
                IntegerCastChainWitness {
                    root: self.root.clone(),
                    target: self.target.clone(),
                    definition_axioms: vec![0],
                }
            }
        }

        #[test]
        fn expression_root_transports_only_independently_proved_endpoints() {
            let fixture = RemainderCast::new();
            let definition = fixture.definition();
            for root_is_left in [false, true] {
                let endpoint = if root_is_left { 42 } else { 1 };
                let relation = |root, endpoint| {
                    if root_is_left {
                        Proposition::LessOrEqual(root, endpoint)
                    } else {
                        Proposition::LessOrEqual(endpoint, root)
                    }
                };
                let root_bound =
                    relation(fixture.root.clone(), literal(fixture.source_type, endpoint));
                let conclusion = relation(
                    fixture.target.clone(),
                    literal(fixture.target_type, endpoint),
                );
                let proof = ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::IntegerCastBound {
                        root_bound: Box::new(assumption_proof(root_bound.clone(), 0)),
                        witness: fixture.witness(),
                    },
                };
                let acceptance = accept_certificate(
                    &fixture.context,
                    &conclusion,
                    std::slice::from_ref(&root_bound),
                    std::slice::from_ref(&definition),
                    &proof,
                )
                .expect("the admitted expression's exact bound transports through the cast");
                assert!(matches!(
                    acceptance.mathematical_core,
                    MathematicalCoreDecision::Judged(_)
                ));
                assert_eq!(acceptance.assumptions.len(), 1);
                assert_eq!(acceptance.assumptions[0].proposition, root_bound);
                assert_eq!(acceptance.semantic_axioms.len(), 1);
                assert_eq!(acceptance.semantic_axioms[0].proposition, definition);
                assert!(
                    accept_certificate(
                        &fixture.context,
                        &conclusion,
                        &[],
                        std::slice::from_ref(&definition),
                        &proof,
                    )
                    .is_err(),
                    "the cast equality cannot supply its own missing root-bound premise"
                );

                let stronger_endpoint = if root_is_left {
                    endpoint - 1
                } else {
                    endpoint + 1
                };
                let stronger = relation(
                    fixture.target.clone(),
                    literal(fixture.target_type, stronger_endpoint),
                );
                let stronger_proof = ProofNode {
                    conclusion: stronger.clone(),
                    rule: proof.rule,
                };
                assert!(
                    accept_certificate(
                        &fixture.context,
                        &stronger,
                        &[root_bound],
                        std::slice::from_ref(&definition),
                        &stronger_proof,
                    )
                    .is_err(),
                    "transport cannot strengthen either endpoint"
                );
            }
        }

        #[test]
        fn expression_root_rejects_unknown_and_mistyped_nested_values() {
            let mut fixture = RemainderCast::new();
            fixture.root = ScalarTerm::exact_integer_remainder(
                fixture.source_type,
                value(9, fixture.source_type),
                literal(fixture.source_type, 256),
            )
            .unwrap();
            assert_eq!(
                check!(&fixture.context => &[fixture.definition()] => &fixture.witness()),
                Err(IntegerCastChainWitnessError::MalformedProposition(
                    PropositionError::UnknownValue(ValueId::new(9).unwrap())
                ))
            );

            fixture.root = ScalarTerm::ExactIntegerRemainder {
                scalar_type: fixture.source_type,
                left: Box::new(value(2, fixture.target_type)),
                right: Box::new(literal(fixture.source_type, 256)),
            };
            assert!(matches!(
                check!(&fixture.context => &[fixture.definition()] => &fixture.witness()),
                Err(IntegerCastChainWitnessError::MalformedProposition(_))
            ));

            fixture.root = ScalarTerm::exact_integer_remainder(
                fixture.source_type,
                value(2, fixture.source_type),
                literal(fixture.source_type, 256),
            )
            .unwrap();
            assert!(matches!(
                check!(&fixture.context => &[fixture.definition()] => &fixture.witness()),
                Err(IntegerCastChainWitnessError::MalformedProposition(
                    PropositionError::ValueTypeMismatch { .. }
                ))
            ));
        }

        #[test]
        fn expression_root_preserves_cast_identity_policy_and_canonical_order() {
            let fixture = RemainderCast::new();
            let definition = fixture.definition();
            let mut witness = fixture.witness();
            witness.root = ScalarTerm::wrapping_integer_remainder(
                fixture.source_type,
                value(1, fixture.source_type),
                literal(fixture.source_type, 256),
            )
            .unwrap();
            assert_eq!(
                check!(&fixture.context => std::slice::from_ref(&definition) => &witness),
                Err(IntegerCastChainWitnessError::CastChainMismatch(0))
            );

            witness = fixture.witness();
            witness.definition_axioms = vec![0, 0];
            assert_eq!(
                check!(&fixture.context => std::slice::from_ref(&definition) => &witness),
                Err(IntegerCastChainWitnessError::NonCanonicalDefinitionOrder)
            );

            witness = fixture.witness();
            witness.root = ScalarTerm::Boolean(false);
            assert_eq!(
                check_integer_cast_chain_witness(&fixture.context, &[definition], &witness),
                Err(IntegerCastChainWitnessError::RootNotInteger)
            );
        }

        #[test]
        fn expression_root_rejects_cast_targets_nested_in_the_root() {
            let mut fixture = RemainderCast::new();
            let return_to_source = Proposition::Equal(
                value(1, fixture.source_type),
                ScalarTerm::integer_widen(
                    fixture.target_type,
                    fixture.source_type,
                    fixture.target.clone(),
                )
                .unwrap(),
            );
            let mut witness = fixture.witness();
            witness.definition_axioms.push(1);
            witness.target = value(1, fixture.source_type);
            assert_eq!(
                check!(&fixture.context => &[fixture.definition(), return_to_source] => &witness),
                Err(IntegerCastChainWitnessError::CyclicValue(1)),
                "a later cast cannot define a value already used inside the root"
            );

            fixture.root = ScalarTerm::exact_integer_remainder(
                fixture.source_type,
                ScalarTerm::integer_widen(
                    fixture.target_type,
                    fixture.source_type,
                    fixture.target.clone(),
                )
                .unwrap(),
                literal(fixture.source_type, 256),
            )
            .unwrap();
            assert_eq!(
                check!(&fixture.context => &[fixture.definition()] => &fixture.witness()),
                Err(IntegerCastChainWitnessError::CyclicValue(0)),
                "the first cast cannot define a value nested inside the root either"
            );
        }
    }
}
pub(crate) mod integer_forbidden_root {
    pub use terminal_psi::{
        CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness,
        IntegerCorrelatedForbiddenRootWitness,
    };

    use std::collections::BTreeSet;

    use crate::integer_rules::fixed_integer::{
        fixed_integer_as_i128, fixed_native, fixed_native_interval,
    };
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
        ScalarType, ValueId,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedIntegerCorrelatedForbiddenRoots {
        integer_type: IntegerType,
        root: ScalarTerm,
        dividend: ScalarTerm,
        divisor: ScalarTerm,
        dividend_coefficient: i128,
        dividend_offset: i128,
        divisor_coefficient: i128,
        divisor_offset: i128,
        lower_bound_axiom: usize,
        upper_bound_axiom: usize,
        interval: (i128, i128),
        forbidden_roots: BTreeSet<i128>,
        conclusion: Proposition,
    }

    impl CheckedIntegerCorrelatedForbiddenRoots {
        pub const fn integer_type(&self) -> IntegerType {
            self.integer_type
        }

        pub const fn root(&self) -> &ScalarTerm {
            &self.root
        }

        pub const fn dividend(&self) -> &ScalarTerm {
            &self.dividend
        }

        pub const fn divisor(&self) -> &ScalarTerm {
            &self.divisor
        }

        pub const fn dividend_form(&self) -> (i128, i128) {
            (self.dividend_coefficient, self.dividend_offset)
        }

        pub const fn divisor_form(&self) -> (i128, i128) {
            (self.divisor_coefficient, self.divisor_offset)
        }

        pub const fn bound_axioms(&self) -> (usize, usize) {
            (self.lower_bound_axiom, self.upper_bound_axiom)
        }

        pub const fn interval(&self) -> (i128, i128) {
            self.interval
        }

        pub const fn forbidden_roots(&self) -> &BTreeSet<i128> {
            &self.forbidden_roots
        }

        pub const fn conclusion(&self) -> &Proposition {
            &self.conclusion
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CheckedBranch {
        integer_type: IntegerType,
        coefficient: i128,
        offset: i128,
        definition_axioms: Vec<usize>,
    }

    pub fn check_integer_correlated_forbidden_root_witness(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        machine_parameter_values: &BTreeSet<ValueId>,
        witness: &IntegerCorrelatedForbiddenRootWitness,
    ) -> Result<CheckedIntegerCorrelatedForbiddenRoots, IntegerCorrelatedForbiddenRootWitnessError>
    {
        if witness.definition_axiom_count > semantic_axioms.len() {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::InvalidDefinitionBoundary);
        }
        let dividend = check_branch(
            context,
            semantic_axioms,
            machine_parameter_values,
            witness.definition_axiom_count,
            &witness.dividend,
            CorrelatedAffineBranch::Dividend,
        )?;
        let divisor = check_branch(
            context,
            semantic_axioms,
            machine_parameter_values,
            witness.definition_axiom_count,
            &witness.divisor,
            CorrelatedAffineBranch::Divisor,
        )?;
        if dividend.integer_type != divisor.integer_type {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::BranchTypeMismatch);
        }
        if witness.dividend.root != witness.divisor.root {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::RootMismatch);
        }
        if dividend.coefficient == 0 || divisor.coefficient == 0 {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::ConstantBranch);
        }
        let dividend_indices = &dividend.definition_axioms;
        let divisor_indices = &divisor.definition_axioms;
        if !dividend_indices
            .iter()
            .all(|index| !divisor_indices.contains(index))
            || dividend_indices.last().expect("nonempty branch")
                >= divisor_indices.first().expect("nonempty branch")
        {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::BranchOrderMismatch);
        }

        let integer_type = dividend.integer_type;
        let bounds = selected_signature_bounds(
            context,
            semantic_axioms,
            witness.definition_axiom_count,
            &witness.dividend.root,
            integer_type,
        )?;
        if witness.lower_bound_axiom != bounds.lower_axiom
            || witness.upper_bound_axiom != bounds.upper_axiom
        {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::BoundIdentityMismatch);
        }

        let mut forbidden_roots = BTreeSet::new();
        if let Some(root) = affine_equation_root(divisor.coefficient, divisor.offset, 0)?
            && in_interval(root, bounds.interval)
        {
            forbidden_roots.insert(root);
        }
        let (carrier_minimum, _) = fixed_native_interval(integer_type)
            .ok_or(IntegerCorrelatedForbiddenRootWitnessError::UnsupportedCarrier(integer_type))?;
        if let Some(root) = affine_equation_root(divisor.coefficient, divisor.offset, -1)?
            && in_interval(root, bounds.interval)
            && affine_value(dividend.coefficient, dividend.offset, root)? == carrier_minimum
        {
            forbidden_roots.insert(root);
        }
        let interval_size = bounds
            .interval
            .1
            .checked_sub(bounds.interval.0)
            .and_then(|span| span.checked_add(1))
            .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?;
        let expected = if forbidden_roots.is_empty() {
            Proposition::Conjunction(vec![bounds.lower.clone(), bounds.upper.clone()])
        } else if interval_size
            == i128::try_from(forbidden_roots.len())
                .map_err(|_| IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?
        {
            Proposition::Falsehood
        } else {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::PartiallyUnsafeInterval);
        };
        if witness.conclusion != expected {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::ConclusionMismatch);
        }
        context
            .validate(&witness.conclusion)
            .map_err(IntegerCorrelatedForbiddenRootWitnessError::MalformedProposition)?;

        Ok(CheckedIntegerCorrelatedForbiddenRoots {
            integer_type,
            root: witness.dividend.root.clone(),
            dividend: witness.dividend.target.clone(),
            divisor: witness.divisor.target.clone(),
            dividend_coefficient: dividend.coefficient,
            dividend_offset: dividend.offset,
            divisor_coefficient: divisor.coefficient,
            divisor_offset: divisor.offset,
            lower_bound_axiom: bounds.lower_axiom,
            upper_bound_axiom: bounds.upper_axiom,
            interval: bounds.interval,
            forbidden_roots,
            conclusion: expected,
        })
    }

    pub fn check_integer_correlated_forbidden_root_conversion(
        checked: &CheckedIntegerCorrelatedForbiddenRoots,
        conclusion: &Proposition,
    ) -> Result<(), IntegerCorrelatedForbiddenRootConversionError> {
        if !checked.forbidden_roots.is_empty() {
            return Err(IntegerCorrelatedForbiddenRootConversionError::ForbiddenRoots);
        }
        let integer_type = checked.integer_type;
        let minimum_plus_one = match integer_type.minimum_value() {
            Some(IntegerValue::Signed(minimum)) => IntegerValue::Signed(
                minimum
                    .checked_add(1)
                    .expect("fixed signed minimum has a successor"),
            ),
            Some(IntegerValue::Unsigned(_)) | None => {
                unreachable!("checked correlated roots are signed and of a native width")
            }
        };
        let integer = |value| {
            ScalarTerm::integer(integer_type, value)
                .expect("checked signed fixed literal belongs to its carrier")
        };
        let expected = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(checked.divisor.clone(), integer(IntegerValue::Signed(-2))),
            Proposition::LessOrEqual(integer(IntegerValue::Signed(1)), checked.divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    checked.divisor.clone(),
                    integer(IntegerValue::Signed(-1)),
                ),
                Proposition::LessOrEqual(integer(minimum_plus_one), checked.dividend.clone()),
            ]),
        ]);
        (conclusion == &expected)
            .then_some(())
            .ok_or(IntegerCorrelatedForbiddenRootConversionError::ConclusionMismatch)
    }

    fn check_branch(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        machine_parameter_values: &BTreeSet<ValueId>,
        definition_axiom_count: usize,
        witness: &CorrelatedAffineBranchWitness,
        branch: CorrelatedAffineBranch,
    ) -> Result<CheckedBranch, IntegerCorrelatedForbiddenRootWitnessError> {
        let ScalarTerm::Value {
            id,
            scalar_type: ScalarType::Integer(integer_type),
        } = witness.root
        else {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(branch));
        };
        if !machine_parameter_values.contains(&id)
            || !fixed_native(integer_type)
            || integer_type.sign() != IntegerSign::Signed
            || witness.target.scalar_type() != ScalarType::Integer(integer_type)
        {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(branch));
        }
        if witness.steps.is_empty() {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::EmptyBranch(
                branch,
            ));
        }
        if witness
            .steps
            .windows(2)
            .any(|steps| steps[0].definition_axiom >= steps[1].definition_axiom)
        {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::NonCanonicalBranchOrder(branch),
            );
        }

        let mut current = witness.target.clone();
        let mut prior = definition_axiom_count;
        let mut coefficient = 1_i128;
        let mut offset = 0_i128;
        for step in witness.steps.iter().rev() {
            let index = step.definition_axiom;
            if index >= prior {
                return Err(
                    IntegerCorrelatedForbiddenRootWitnessError::NonCanonicalBranchOrder(branch),
                );
            }
            let latest = semantic_axioms[..prior].iter().enumerate().rev().find_map(
                |(candidate, proposition)| match proposition {
                    Proposition::Equal(left, _) if left == &current => Some(candidate),
                    _ => None,
                },
            );
            if latest != Some(index) {
                return Err(
                    IntegerCorrelatedForbiddenRootWitnessError::StaleDefinition { branch, index },
                );
            }
            let proposition = checked_axiom(context, semantic_axioms, index)?;
            let Proposition::Equal(target, expression) = proposition else {
                return Err(
                    IntegerCorrelatedForbiddenRootWitnessError::DefinitionShape { branch, index },
                );
            };
            if target != &current {
                return Err(
                    IntegerCorrelatedForbiddenRootWitnessError::DefinitionShape { branch, index },
                );
            }
            let (next, sibling, nested_coefficient, nested_offset) = match expression {
                ScalarTerm::ExactIntegerAdd {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type => (left, right, 1_i128, false),
                ScalarTerm::ExactIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type => (left, right, 1_i128, true),
                ScalarTerm::ExactIntegerMultiply {
                    scalar_type,
                    left,
                    right,
                } if *scalar_type == integer_type => (left, right, 0_i128, false),
                _ => {
                    return Err(
                        IntegerCorrelatedForbiddenRootWitnessError::DefinitionShape {
                            branch,
                            index,
                        },
                    );
                }
            };
            if known_landed_literal(semantic_axioms, index, integer_type, next).is_some() {
                return Err(
                    IntegerCorrelatedForbiddenRootWitnessError::ConstantBranchPath {
                        branch,
                        index,
                    },
                );
            }
            let literal = landed_literal(
                context,
                semantic_axioms,
                index,
                step.literal_axiom,
                integer_type,
                sibling,
            )?;
            let (step_coefficient, step_offset) =
                if matches!(expression, ScalarTerm::ExactIntegerMultiply { .. }) {
                    (literal, 0)
                } else if nested_offset {
                    (
                        nested_coefficient,
                        literal.checked_neg().ok_or(
                            IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow,
                        )?,
                    )
                } else {
                    (nested_coefficient, literal)
                };
            offset = step_offset
                .checked_mul(coefficient)
                .and_then(|nested| nested.checked_add(offset))
                .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?;
            coefficient = coefficient
                .checked_mul(step_coefficient)
                .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?;
            current = (**next).clone();
            prior = index;
        }
        if current != witness.root {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::BranchRootDrift(
                branch,
            ));
        }
        Ok(CheckedBranch {
            integer_type,
            coefficient,
            offset,
            definition_axioms: witness
                .steps
                .iter()
                .map(|step| step.definition_axiom)
                .collect(),
        })
    }

    fn known_landed_literal(
        semantic_axioms: &[Proposition],
        prior_axiom_count: usize,
        integer_type: IntegerType,
        term: &ScalarTerm,
    ) -> Option<i128> {
        signed_literal(term, integer_type).or_else(|| {
            semantic_axioms[..prior_axiom_count]
                .iter()
                .rev()
                .find_map(|proposition| match proposition {
                    Proposition::Equal(left, right) if left == term => {
                        signed_literal(right, integer_type)
                    }
                    _ => None,
                })
        })
    }

    fn landed_literal(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        definition_axiom: usize,
        literal_axiom: Option<usize>,
        integer_type: IntegerType,
        term: &ScalarTerm,
    ) -> Result<i128, IntegerCorrelatedForbiddenRootWitnessError> {
        if let Some(value) = signed_literal(term, integer_type) {
            if literal_axiom.is_some() {
                return Err(
                    IntegerCorrelatedForbiddenRootWitnessError::UnexpectedLiteralAxiom(
                        definition_axiom,
                    ),
                );
            }
            return Ok(value);
        }
        let selected = literal_axiom.ok_or(
            IntegerCorrelatedForbiddenRootWitnessError::MissingLiteralAxiom(definition_axiom),
        )?;
        if selected >= definition_axiom {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::LiteralAxiomNotPrior {
                    definition_axiom,
                    literal_axiom: selected,
                },
            );
        }
        let latest = semantic_axioms[..definition_axiom]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, proposition)| match proposition {
                Proposition::Equal(left, right) if left == term => {
                    signed_literal(right, integer_type).map(|value| (index, value))
                }
                _ => None,
            });
        let Some((index, value)) = latest else {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::MissingLiteralAxiom(definition_axiom),
            );
        };
        if index != selected {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::LiteralIdentityMismatch(
                    definition_axiom,
                ),
            );
        }
        checked_axiom(context, semantic_axioms, selected)?;
        Ok(value)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SelectedBounds {
        lower_axiom: usize,
        upper_axiom: usize,
        lower: Proposition,
        upper: Proposition,
        interval: (i128, i128),
    }

    fn selected_signature_bounds(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        definition_axiom_count: usize,
        root: &ScalarTerm,
        integer_type: IntegerType,
    ) -> Result<SelectedBounds, IntegerCorrelatedForbiddenRootWitnessError> {
        let (mut minimum, mut maximum) = carrier_interval(integer_type)?;
        let mut lower = None;
        let mut upper = None;
        for (index, proposition) in semantic_axioms
            .iter()
            .enumerate()
            .skip(definition_axiom_count)
        {
            let Proposition::LessOrEqual(left, right) = proposition else {
                continue;
            };
            if right == root
                && let Some(candidate) = signed_literal(left, integer_type)
                && candidate > minimum
            {
                context
                    .validate(proposition)
                    .map_err(IntegerCorrelatedForbiddenRootWitnessError::MalformedProposition)?;
                minimum = candidate;
                lower = Some((index, proposition.clone()));
            }
            if left == root
                && let Some(candidate) = signed_literal(right, integer_type)
                && candidate < maximum
            {
                context
                    .validate(proposition)
                    .map_err(IntegerCorrelatedForbiddenRootWitnessError::MalformedProposition)?;
                maximum = candidate;
                upper = Some((index, proposition.clone()));
            }
        }
        if minimum > maximum {
            return Err(IntegerCorrelatedForbiddenRootWitnessError::InconsistentBounds);
        }
        let (lower_axiom, lower) =
            lower.ok_or(IntegerCorrelatedForbiddenRootWitnessError::MissingTightBounds)?;
        let (upper_axiom, upper) =
            upper.ok_or(IntegerCorrelatedForbiddenRootWitnessError::MissingTightBounds)?;
        Ok(SelectedBounds {
            lower_axiom,
            upper_axiom,
            lower,
            upper,
            interval: (minimum, maximum),
        })
    }

    crate::checked_axiom!(IntegerCorrelatedForbiddenRootWitnessError);

    fn signed_literal(term: &ScalarTerm, integer_type: IntegerType) -> Option<i128> {
        (integer_type.sign() == IntegerSign::Signed)
            .then(|| fixed_integer_as_i128(term, integer_type))
            .flatten()
    }

    fn carrier_interval(
        integer_type: IntegerType,
    ) -> Result<(i128, i128), IntegerCorrelatedForbiddenRootWitnessError> {
        if integer_type.sign() != IntegerSign::Signed {
            return Err(
                IntegerCorrelatedForbiddenRootWitnessError::UnsupportedCarrier(integer_type),
            );
        }
        fixed_native_interval(integer_type)
            .ok_or(IntegerCorrelatedForbiddenRootWitnessError::UnsupportedCarrier(integer_type))
    }

    fn affine_equation_root(
        coefficient: i128,
        offset: i128,
        target: i128,
    ) -> Result<Option<i128>, IntegerCorrelatedForbiddenRootWitnessError> {
        let numerator = target
            .checked_sub(offset)
            .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?;
        if numerator
            .checked_rem(coefficient)
            .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)?
            != 0
        {
            return Ok(None);
        }
        numerator
            .checked_div(coefficient)
            .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)
            .map(Some)
    }

    fn affine_value(
        coefficient: i128,
        offset: i128,
        root: i128,
    ) -> Result<i128, IntegerCorrelatedForbiddenRootWitnessError> {
        coefficient
            .checked_mul(root)
            .and_then(|value| value.checked_add(offset))
            .ok_or(IntegerCorrelatedForbiddenRootWitnessError::ArithmeticOverflow)
    }

    fn in_interval(value: i128, interval: (i128, i128)) -> bool {
        value >= interval.0 && value <= interval.1
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CorrelatedAffineBranch {
        Dividend,
        Divisor,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum IntegerCorrelatedForbiddenRootWitnessError {
        InvalidDefinitionBoundary,
        RootNotSignedNative(CorrelatedAffineBranch),
        EmptyBranch(CorrelatedAffineBranch),
        NonCanonicalBranchOrder(CorrelatedAffineBranch),
        UnknownSemanticAxiom(usize),
        MalformedProposition(semantic_vocabulary::PropositionError),
        StaleDefinition {
            branch: CorrelatedAffineBranch,
            index: usize,
        },
        DefinitionShape {
            branch: CorrelatedAffineBranch,
            index: usize,
        },
        ConstantBranchPath {
            branch: CorrelatedAffineBranch,
            index: usize,
        },
        MissingLiteralAxiom(usize),
        UnexpectedLiteralAxiom(usize),
        LiteralAxiomNotPrior {
            definition_axiom: usize,
            literal_axiom: usize,
        },
        LiteralIdentityMismatch(usize),
        BranchRootDrift(CorrelatedAffineBranch),
        BranchTypeMismatch,
        RootMismatch,
        ConstantBranch,
        BranchOrderMismatch,
        UnsupportedCarrier(IntegerType),
        MissingTightBounds,
        InconsistentBounds,
        BoundIdentityMismatch,
        ArithmeticOverflow,
        PartiallyUnsafeInterval,
        ConclusionMismatch,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IntegerCorrelatedForbiddenRootConversionError {
        ForbiddenRoots,
        ConclusionMismatch,
    }

    crate::debug_display!(IntegerCorrelatedForbiddenRootConversionError);

    impl std::error::Error for IntegerCorrelatedForbiddenRootConversionError {}

    crate::debug_display!(IntegerCorrelatedForbiddenRootWitnessError);

    impl std::error::Error for IntegerCorrelatedForbiddenRootWitnessError {}

    #[cfg(test)]
    mod tests {
        use super::{
            BTreeSet, CorrelatedAffineBranch, CorrelatedAffineBranchWitness,
            IntegerCorrelatedForbiddenRootWitness, IntegerCorrelatedForbiddenRootWitnessError,
            IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
            ScalarType, ValueId, check_integer_correlated_forbidden_root_witness,
        };
        use crate::correlated_step;

        fn i8_type() -> IntegerType {
            IntegerType::new(IntegerSign::Signed, 8).expect("i8")
        }

        fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
            ScalarTerm::value(
                ValueId::new(id).expect("value id"),
                ScalarType::Integer(integer_type),
            )
        }

        fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
            ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("integer")
        }

        fn add(
            target: ScalarTerm,
            integer_type: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Proposition {
            Proposition::Equal(
                target,
                ScalarTerm::exact_integer_add(integer_type, left, right).expect("add"),
            )
        }

        fn subtract(
            target: ScalarTerm,
            integer_type: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Proposition {
            Proposition::Equal(
                target,
                ScalarTerm::exact_integer_subtract(integer_type, left, right).expect("subtract"),
            )
        }

        fn multiply(
            target: ScalarTerm,
            integer_type: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Proposition {
            Proposition::Equal(
                target,
                ScalarTerm::exact_integer_multiply(integer_type, left, right).expect("multiply"),
            )
        }

        fn safe_fixture() -> (
            PropositionContext,
            Vec<Proposition>,
            BTreeSet<ValueId>,
            IntegerCorrelatedForbiddenRootWitness,
        ) {
            let integer_type = i8_type();
            let root = value(1, integer_type);
            let sixty_four = value(2, integer_type);
            let left_offset = value(3, integer_type);
            let negative_two = value(4, integer_type);
            let dividend = value(5, integer_type);
            let two = value(6, integer_type);
            let right_product = value(7, integer_type);
            let divisor = value(8, integer_type);
            let lower = Proposition::LessOrEqual(integer(integer_type, -1), root.clone());
            let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, 0));
            let axioms = vec![
                Proposition::Equal(sixty_four.clone(), integer(integer_type, 64)),
                add(left_offset.clone(), integer_type, root.clone(), sixty_four),
                Proposition::Equal(negative_two.clone(), integer(integer_type, -2)),
                multiply(dividend.clone(), integer_type, left_offset, negative_two),
                Proposition::Equal(two.clone(), integer(integer_type, 2)),
                multiply(right_product.clone(), integer_type, root.clone(), two),
                add(
                    divisor.clone(),
                    integer_type,
                    right_product,
                    integer(integer_type, 1),
                ),
                lower.clone(),
                upper.clone(),
            ];
            let context = PropositionContext::from_value_types((1..=8).map(|id| {
                (
                    ValueId::new(id).expect("value id"),
                    ScalarType::Integer(integer_type),
                )
            }))
            .expect("context");
            let witness = IntegerCorrelatedForbiddenRootWitness {
                dividend: CorrelatedAffineBranchWitness {
                    root: root.clone(),
                    target: dividend,
                    steps: vec![correlated_step(1, Some(0)), correlated_step(3, Some(2))],
                },
                divisor: CorrelatedAffineBranchWitness {
                    root: root.clone(),
                    target: divisor,
                    steps: vec![correlated_step(5, Some(4)), correlated_step(6, None)],
                },
                definition_axiom_count: 7,
                lower_bound_axiom: 7,
                upper_bound_axiom: 8,
                conclusion: Proposition::Conjunction(vec![lower, upper]),
            };
            (
                context,
                axioms,
                BTreeSet::from([ValueId::new(1).expect("root")]),
                witness,
            )
        }

        #[test]
        fn checks_complete_landed_affine_forbidden_root_family() {
            let (context, axioms, parameters, witness) = safe_fixture();
            let checked = check_integer_correlated_forbidden_root_witness(
                &context,
                &axioms,
                &parameters,
                &witness,
            )
            .expect("safe correlated witness");
            assert_eq!(checked.dividend_form(), (-2, -128));
            assert_eq!(checked.divisor_form(), (2, 1));
            assert_eq!(checked.interval(), (-1, 0));
            assert!(checked.forbidden_roots().is_empty());
            assert_eq!(checked.bound_axioms(), (7, 8));
            assert_eq!(checked.conclusion(), &witness.conclusion);
        }

        #[test]
        fn checks_when_forbidden_roots_cover_the_complete_interval() {
            let integer_type = i8_type();
            let root = value(20, integer_type);
            let left_offset = value(21, integer_type);
            let dividend = value(22, integer_type);
            let divisor = value(23, integer_type);
            let lower = Proposition::LessOrEqual(integer(integer_type, -1), root.clone());
            let upper = Proposition::LessOrEqual(root.clone(), integer(integer_type, 0));
            let axioms = vec![
                subtract(
                    left_offset.clone(),
                    integer_type,
                    root.clone(),
                    integer(integer_type, 63),
                ),
                multiply(
                    dividend.clone(),
                    integer_type,
                    left_offset,
                    integer(integer_type, 2),
                ),
                multiply(
                    divisor.clone(),
                    integer_type,
                    root.clone(),
                    integer(integer_type, 1),
                ),
                lower,
                upper,
            ];
            let context = PropositionContext::from_value_types((20..=23).map(|id| {
                (
                    ValueId::new(id).expect("value id"),
                    ScalarType::Integer(integer_type),
                )
            }))
            .expect("context");
            let witness = IntegerCorrelatedForbiddenRootWitness {
                dividend: CorrelatedAffineBranchWitness {
                    root: root.clone(),
                    target: dividend,
                    steps: vec![correlated_step(0, None), correlated_step(1, None)],
                },
                divisor: CorrelatedAffineBranchWitness {
                    root,
                    target: divisor,
                    steps: vec![correlated_step(2, None)],
                },
                definition_axiom_count: 3,
                lower_bound_axiom: 3,
                upper_bound_axiom: 4,
                conclusion: Proposition::Falsehood,
            };
            let checked = check_integer_correlated_forbidden_root_witness(
                &context,
                &axioms,
                &BTreeSet::from([ValueId::new(20).expect("root")]),
                &witness,
            )
            .expect("whole interval forbidden");
            assert_eq!(checked.forbidden_roots(), &BTreeSet::from([-1, 0]));
            assert_eq!(checked.conclusion(), &Proposition::Falsehood);
        }

        #[test]
        fn rejects_partial_safety_and_identity_order_root_or_type_drift() {
            let (context, axioms, parameters, witness) = safe_fixture();
            macro_rules! w {
                ($axioms:expr => $witness:expr) => {
                    check_integer_correlated_forbidden_root_witness(
                        &context,
                        $axioms,
                        &parameters,
                        $witness,
                    )
                };
            }

            let mut bound_drift = witness.clone();
            bound_drift.lower_bound_axiom = 8;
            assert_eq!(
                w!(&axioms => &bound_drift),
                Err(IntegerCorrelatedForbiddenRootWitnessError::BoundIdentityMismatch),
            );

            let mut conclusion_drift = witness.clone();
            conclusion_drift.conclusion =
                Proposition::Conjunction(vec![axioms[8].clone(), axioms[7].clone()]);
            assert_eq!(
                w!(&axioms => &conclusion_drift),
                Err(IntegerCorrelatedForbiddenRootWitnessError::ConclusionMismatch),
            );

            let mut order_drift = witness.clone();
            std::mem::swap(&mut order_drift.dividend, &mut order_drift.divisor);
            assert_eq!(
                w!(&axioms => &order_drift),
                Err(IntegerCorrelatedForbiddenRootWitnessError::BranchOrderMismatch),
            );

            let mut root_drift = witness.clone();
            root_drift.divisor.root = value(9, i8_type());
            assert!(matches!(
                w!(&axioms => &root_drift),
                Err(
                    IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(
                        CorrelatedAffineBranch::Divisor
                    )
                ),
            ));

            let unsigned = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
            let unsigned_root = value(1, unsigned);
            let mut type_drift = witness.clone();
            type_drift.dividend.root = unsigned_root;
            assert!(matches!(
                w!(&axioms => &type_drift),
                Err(
                    IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(
                        CorrelatedAffineBranch::Dividend
                    )
                ),
            ));

            let mut partial_axioms = axioms.clone();
            partial_axioms[6] = multiply(
                witness.divisor.target.clone(),
                i8_type(),
                value(7, i8_type()),
                integer(i8_type(), 1),
            );
            assert_eq!(
                w!(&partial_axioms => &witness),
                Err(IntegerCorrelatedForbiddenRootWitnessError::PartiallyUnsafeInterval),
            );
        }
    }
}
pub(crate) mod integer_shift {
    use crate::integer_rules::fixed_integer::fixed_native;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
        ScalarType,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IntegerShiftDirection {
        Left,
        Right,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct IntegerShiftStepWitness {
        pub definition_axiom: usize,
        pub count_axiom: Option<usize>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct IntegerShiftChainWitness {
        pub root: ScalarTerm,
        pub target: ScalarTerm,
        pub steps: Vec<IntegerShiftStepWitness>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedIntegerShiftStep {
        direction: IntegerShiftDirection,
        count_type: IntegerType,
        count: u128,
        definition_axiom: usize,
        count_axiom: Option<usize>,
    }

    impl CheckedIntegerShiftStep {
        pub const fn direction(&self) -> IntegerShiftDirection {
            self.direction
        }

        pub const fn count_type(&self) -> IntegerType {
            self.count_type
        }

        pub const fn count(&self) -> u128 {
            self.count
        }

        pub const fn definition_axiom(&self) -> usize {
            self.definition_axiom
        }

        pub const fn count_axiom(&self) -> Option<usize> {
            self.count_axiom
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct CheckedIntegerShiftChain {
        root: ScalarTerm,
        target: ScalarTerm,
        value_type: IntegerType,
        steps: Vec<CheckedIntegerShiftStep>,
    }

    impl CheckedIntegerShiftChain {
        pub const fn root(&self) -> &ScalarTerm {
            &self.root
        }

        pub const fn target(&self) -> &ScalarTerm {
            &self.target
        }

        pub const fn value_type(&self) -> IntegerType {
            self.value_type
        }

        pub fn steps(&self) -> &[CheckedIntegerShiftStep] {
            &self.steps
        }
    }

    pub fn check_integer_shift_chain_witness(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        witness: &IntegerShiftChainWitness,
    ) -> Result<CheckedIntegerShiftChain, IntegerShiftChainWitnessError> {
        if !matches!(witness.root, ScalarTerm::Value { .. }) {
            return Err(IntegerShiftChainWitnessError::RootNotValue);
        }
        let Some(value_type) = witness.root.scalar_type().as_integer() else {
            return Err(IntegerShiftChainWitnessError::RootNotInteger);
        };
        if !fixed_native(value_type) {
            return Err(IntegerShiftChainWitnessError::UnsupportedValueCarrier(
                value_type,
            ));
        }
        if witness.steps.is_empty() {
            return Err(IntegerShiftChainWitnessError::EmptyShiftChain);
        }
        if witness
            .steps
            .windows(2)
            .any(|steps| steps[0].definition_axiom >= steps[1].definition_axiom)
        {
            return Err(IntegerShiftChainWitnessError::NonCanonicalDefinitionOrder);
        }

        let mut current = witness.root.clone();
        let mut values = vec![current.clone()];
        let mut checked_steps = Vec::with_capacity(witness.steps.len());
        for step in &witness.steps {
            let index = step.definition_axiom;
            let proposition = checked_axiom(context, semantic_axioms, index)?;
            let Proposition::Equal(next, definition) = proposition else {
                return Err(IntegerShiftChainWitnessError::DefinitionNotEquality(index));
            };
            if !matches!(next, ScalarTerm::Value { .. }) {
                return Err(IntegerShiftChainWitnessError::DefinitionTargetNotValue(
                    index,
                ));
            }
            let (direction, nested_value_type, count_type, operand, count_term) = match definition {
                ScalarTerm::ExactIntegerShiftLeft {
                    value_type,
                    count_type,
                    value,
                    count,
                } => (
                    IntegerShiftDirection::Left,
                    *value_type,
                    *count_type,
                    value,
                    count,
                ),
                ScalarTerm::ExactIntegerShiftRight {
                    value_type,
                    count_type,
                    value,
                    count,
                } => (
                    IntegerShiftDirection::Right,
                    *value_type,
                    *count_type,
                    value,
                    count,
                ),
                _ => {
                    return Err(IntegerShiftChainWitnessError::DefinitionNotExactShift(
                        index,
                    ));
                }
            };
            if nested_value_type != value_type
                || operand.as_ref() != &current
                || next.scalar_type() != ScalarType::Integer(value_type)
            {
                return Err(IntegerShiftChainWitnessError::ShiftChainMismatch(index));
            }
            if !fixed_native(count_type) {
                return Err(IntegerShiftChainWitnessError::UnsupportedCountCarrier {
                    index,
                    count_type,
                });
            }
            let count = checked_count(
                context,
                semantic_axioms,
                index,
                step.count_axiom,
                count_type,
                count_term,
            )?;
            let Some(width) = value_type.bits() else {
                return Err(IntegerShiftChainWitnessError::UnsupportedValueCarrier(
                    value_type,
                ));
            };
            if count >= u128::from(width) {
                return Err(IntegerShiftChainWitnessError::CountOutsideValueWidth {
                    index,
                    count,
                    width,
                });
            }
            if values.contains(next) {
                return Err(IntegerShiftChainWitnessError::CyclicValue(index));
            }
            current = next.clone();
            values.push(current.clone());
            checked_steps.push(CheckedIntegerShiftStep {
                direction,
                count_type,
                count,
                definition_axiom: index,
                count_axiom: step.count_axiom,
            });
        }
        if current != witness.target {
            return Err(IntegerShiftChainWitnessError::TargetMismatch);
        }

        Ok(CheckedIntegerShiftChain {
            root: witness.root.clone(),
            target: witness.target.clone(),
            value_type,
            steps: checked_steps,
        })
    }

    fn checked_count(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        definition_axiom: usize,
        count_axiom: Option<usize>,
        count_type: IntegerType,
        count_term: &ScalarTerm,
    ) -> Result<u128, IntegerShiftChainWitnessError> {
        let value = if let Some((actual_type, value)) = count_term.integer_value() {
            if count_axiom.is_some() {
                return Err(IntegerShiftChainWitnessError::UnexpectedCountAxiom(
                    definition_axiom,
                ));
            }
            if actual_type != count_type {
                return Err(IntegerShiftChainWitnessError::CountTypeMismatch(
                    definition_axiom,
                ));
            }
            value
        } else {
            let count_axiom = count_axiom.ok_or(
                IntegerShiftChainWitnessError::MissingCountAxiom(definition_axiom),
            )?;
            if count_axiom >= definition_axiom {
                return Err(IntegerShiftChainWitnessError::CountAxiomNotPrior {
                    definition_axiom,
                    count_axiom,
                });
            }
            let proposition = checked_axiom(context, semantic_axioms, count_axiom)?;
            let Proposition::Equal(left, right) = proposition else {
                return Err(IntegerShiftChainWitnessError::CountAxiomNotEquality(
                    count_axiom,
                ));
            };
            if left != count_term {
                return Err(IntegerShiftChainWitnessError::CountAxiomMismatch(
                    count_axiom,
                ));
            }
            let Some((actual_type, value)) = right.integer_value() else {
                return Err(IntegerShiftChainWitnessError::CountAxiomMismatch(
                    count_axiom,
                ));
            };
            if actual_type != count_type {
                return Err(IntegerShiftChainWitnessError::CountTypeMismatch(
                    definition_axiom,
                ));
            }
            value
        };
        match (count_type.sign(), value) {
            (IntegerSign::Signed, IntegerValue::Signed(value)) => u128::try_from(value)
                .map_err(|_| IntegerShiftChainWitnessError::NegativeCount(definition_axiom)),
            (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => Ok(value),
            _ => Err(IntegerShiftChainWitnessError::CountTypeMismatch(
                definition_axiom,
            )),
        }
    }

    crate::checked_axiom!(IntegerShiftChainWitnessError);

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum IntegerShiftChainWitnessError {
        RootNotValue,
        RootNotInteger,
        UnsupportedValueCarrier(IntegerType),
        EmptyShiftChain,
        NonCanonicalDefinitionOrder,
        UnknownSemanticAxiom(usize),
        MalformedProposition(semantic_vocabulary::PropositionError),
        DefinitionNotEquality(usize),
        DefinitionTargetNotValue(usize),
        DefinitionNotExactShift(usize),
        ShiftChainMismatch(usize),
        UnsupportedCountCarrier {
            index: usize,
            count_type: IntegerType,
        },
        MissingCountAxiom(usize),
        UnexpectedCountAxiom(usize),
        CountAxiomNotPrior {
            definition_axiom: usize,
            count_axiom: usize,
        },
        CountAxiomNotEquality(usize),
        CountAxiomMismatch(usize),
        CountTypeMismatch(usize),
        NegativeCount(usize),
        CountOutsideValueWidth {
            index: usize,
            count: u128,
            width: u16,
        },
        CyclicValue(usize),
        TargetMismatch,
    }

    crate::debug_display!(IntegerShiftChainWitnessError);

    impl std::error::Error for IntegerShiftChainWitnessError {}

    #[cfg(test)]
    mod tests {
        use super::super::value;
        use super::{
            IntegerShiftChainWitness, IntegerShiftChainWitnessError, IntegerShiftDirection,
            IntegerShiftStepWitness, IntegerSign, IntegerType, IntegerValue, Proposition,
            PropositionContext, ScalarTerm, ScalarType, check_integer_shift_chain_witness,
        };
        use semantic_vocabulary::ValueId;
        use std::slice::from_ref;

        fn integer_type(sign: IntegerSign, bits: u16) -> IntegerType {
            IntegerType::new(sign, bits).expect("fixed integer type")
        }

        fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
            let value = IntegerValue::for_sign(integer_type.sign(), value).expect("unsigned");
            ScalarTerm::integer(integer_type, value).expect("integer term")
        }

        fn shift_definition(
            target: ScalarTerm,
            direction: IntegerShiftDirection,
            value_type: IntegerType,
            count_type: IntegerType,
            operand: ScalarTerm,
            count: ScalarTerm,
        ) -> Proposition {
            let definition = match direction {
                IntegerShiftDirection::Left => {
                    ScalarTerm::exact_integer_shift_left(value_type, count_type, operand, count)
                }
                IntegerShiftDirection::Right => {
                    ScalarTerm::exact_integer_shift_right(value_type, count_type, operand, count)
                }
            }
            .expect("exact shift");
            Proposition::Equal(target, definition)
        }

        #[test]
        fn normalizes_mixed_directions_and_heterogeneous_independently_landed_counts() {
            let i32_type = integer_type(IntegerSign::Signed, 32);
            let u8_type = integer_type(IntegerSign::Unsigned, 8);
            let u16_type = integer_type(IntegerSign::Unsigned, 16);
            let i8_type = integer_type(IntegerSign::Signed, 8);
            let root = value(1, i32_type);
            let left = value(2, i32_type);
            let right_count = value(3, u16_type);
            let right = value(4, i32_type);
            let final_count = value(5, i8_type);
            let target = value(6, i32_type);
            let axioms = vec![
                shift_definition(
                    left.clone(),
                    IntegerShiftDirection::Left,
                    i32_type,
                    u8_type,
                    root.clone(),
                    integer(u8_type, 2),
                ),
                Proposition::Equal(right_count.clone(), integer(u16_type, 1)),
                shift_definition(
                    right.clone(),
                    IntegerShiftDirection::Right,
                    i32_type,
                    u16_type,
                    left,
                    right_count,
                ),
                Proposition::Equal(final_count.clone(), integer(i8_type, 3)),
                shift_definition(
                    target.clone(),
                    IntegerShiftDirection::Left,
                    i32_type,
                    i8_type,
                    right,
                    final_count,
                ),
            ];
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(3).unwrap(), ScalarType::Integer(u16_type)),
                (ValueId::new(4).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(5).unwrap(), ScalarType::Integer(i8_type)),
                (ValueId::new(6).unwrap(), ScalarType::Integer(i32_type)),
            ])
            .unwrap();
            let checked = check_integer_shift_chain_witness(
                &context,
                &axioms,
                &IntegerShiftChainWitness {
                    root: root.clone(),
                    target: target.clone(),
                    steps: vec![
                        IntegerShiftStepWitness {
                            definition_axiom: 0,
                            count_axiom: None,
                        },
                        IntegerShiftStepWitness {
                            definition_axiom: 2,
                            count_axiom: Some(1),
                        },
                        IntegerShiftStepWitness {
                            definition_axiom: 4,
                            count_axiom: Some(3),
                        },
                    ],
                },
            )
            .expect("complete mixed shift word");
            assert_eq!(checked.root(), &root);
            assert_eq!(checked.target(), &target);
            assert_eq!(checked.value_type(), i32_type);
            assert_eq!(
                checked
                    .steps()
                    .iter()
                    .map(|step| (step.direction(), step.count_type(), step.count()))
                    .collect::<Vec<_>>(),
                vec![
                    (IntegerShiftDirection::Left, u8_type, 2),
                    (IntegerShiftDirection::Right, u16_type, 1),
                    (IntegerShiftDirection::Left, i8_type, 3),
                ],
            );
            assert_eq!(checked.steps()[1].definition_axiom(), 2);
            assert_eq!(checked.steps()[1].count_axiom(), Some(1));
        }

        #[test]
        fn rejects_missing_late_reversed_negative_and_out_of_range_count_facts() {
            let i32_type = integer_type(IntegerSign::Signed, 32);
            let i8_type = integer_type(IntegerSign::Signed, 8);
            let root = value(1, i32_type);
            let count = value(2, i8_type);
            let target = value(3, i32_type);
            let definition = shift_definition(
                target.clone(),
                IntegerShiftDirection::Left,
                i32_type,
                i8_type,
                root.clone(),
                count.clone(),
            );
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(i8_type)),
                (ValueId::new(3).unwrap(), ScalarType::Integer(i32_type)),
            ])
            .unwrap();
            let witness = |definition_axiom, count_axiom| IntegerShiftChainWitness {
                root: root.clone(),
                target: target.clone(),
                steps: vec![IntegerShiftStepWitness {
                    definition_axiom,
                    count_axiom,
                }],
            };
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    from_ref(&definition),
                    &witness(0, None)
                ),
                Err(IntegerShiftChainWitnessError::MissingCountAxiom(0)),
            );
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    &[
                        definition.clone(),
                        Proposition::Equal(count.clone(), integer(i8_type, 1))
                    ],
                    &witness(0, Some(1)),
                ),
                Err(IntegerShiftChainWitnessError::CountAxiomNotPrior {
                    definition_axiom: 0,
                    count_axiom: 1,
                }),
            );
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    &[
                        Proposition::Equal(integer(i8_type, 1), count.clone()),
                        definition.clone(),
                    ],
                    &witness(1, Some(0)),
                ),
                Err(IntegerShiftChainWitnessError::CountAxiomMismatch(0)),
            );
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    &[
                        Proposition::Equal(count.clone(), integer(i8_type, -1)),
                        definition.clone(),
                    ],
                    &witness(1, Some(0)),
                ),
                Err(IntegerShiftChainWitnessError::NegativeCount(1)),
            );
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    &[Proposition::Equal(count, integer(i8_type, 32)), definition,],
                    &witness(1, Some(0)),
                ),
                Err(IntegerShiftChainWitnessError::CountOutsideValueWidth {
                    index: 1,
                    count: 32,
                    width: 32,
                }),
            );
        }

        #[test]
        fn rejects_reusing_one_count_axiom_for_a_distinct_shift_count() {
            let i32_type = integer_type(IntegerSign::Signed, 32);
            let u8_type = integer_type(IntegerSign::Unsigned, 8);
            let root = value(1, i32_type);
            let first_count = value(2, u8_type);
            let middle = value(3, i32_type);
            let second_count = value(4, u8_type);
            let target = value(5, i32_type);
            let axioms = vec![
                Proposition::Equal(first_count.clone(), integer(u8_type, 1)),
                shift_definition(
                    middle.clone(),
                    IntegerShiftDirection::Left,
                    i32_type,
                    u8_type,
                    root.clone(),
                    first_count,
                ),
                Proposition::Equal(second_count.clone(), integer(u8_type, 2)),
                shift_definition(
                    target.clone(),
                    IntegerShiftDirection::Right,
                    i32_type,
                    u8_type,
                    middle,
                    second_count,
                ),
            ];
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(u8_type)),
                (ValueId::new(3).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(4).unwrap(), ScalarType::Integer(u8_type)),
                (ValueId::new(5).unwrap(), ScalarType::Integer(i32_type)),
            ])
            .unwrap();
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    &axioms,
                    &IntegerShiftChainWitness {
                        root,
                        target,
                        steps: vec![
                            IntegerShiftStepWitness {
                                definition_axiom: 1,
                                count_axiom: Some(0),
                            },
                            IntegerShiftStepWitness {
                                definition_axiom: 3,
                                count_axiom: Some(0),
                            },
                        ],
                    },
                ),
                Err(IntegerShiftChainWitnessError::CountAxiomMismatch(0)),
            );
        }

        #[test]
        fn rejects_reordered_discontinuous_nonexact_and_target_drifted_words() {
            let i32_type = integer_type(IntegerSign::Signed, 32);
            let u8_type = integer_type(IntegerSign::Unsigned, 8);
            let root = value(1, i32_type);
            let middle = value(2, i32_type);
            let target = value(3, i32_type);
            let axioms = vec![
                shift_definition(
                    middle.clone(),
                    IntegerShiftDirection::Left,
                    i32_type,
                    u8_type,
                    root.clone(),
                    integer(u8_type, 1),
                ),
                shift_definition(
                    target.clone(),
                    IntegerShiftDirection::Right,
                    i32_type,
                    u8_type,
                    middle,
                    integer(u8_type, 1),
                ),
            ];
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(i32_type)),
                (ValueId::new(3).unwrap(), ScalarType::Integer(i32_type)),
            ])
            .unwrap();
            let witness = |target, steps| IntegerShiftChainWitness {
                root: root.clone(),
                target,
                steps,
            };
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    &axioms,
                    &witness(
                        target.clone(),
                        vec![
                            IntegerShiftStepWitness {
                                definition_axiom: 1,
                                count_axiom: None,
                            },
                            IntegerShiftStepWitness {
                                definition_axiom: 0,
                                count_axiom: None,
                            },
                        ],
                    ),
                ),
                Err(IntegerShiftChainWitnessError::NonCanonicalDefinitionOrder),
            );
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    &axioms,
                    &witness(
                        target.clone(),
                        vec![IntegerShiftStepWitness {
                            definition_axiom: 1,
                            count_axiom: None,
                        }],
                    ),
                ),
                Err(IntegerShiftChainWitnessError::ShiftChainMismatch(1)),
            );
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    &[Proposition::Equal(target.clone(), root.clone())],
                    &witness(
                        target.clone(),
                        vec![IntegerShiftStepWitness {
                            definition_axiom: 0,
                            count_axiom: None,
                        }],
                    ),
                ),
                Err(IntegerShiftChainWitnessError::DefinitionNotExactShift(0)),
            );
            assert_eq!(
                check_integer_shift_chain_witness(
                    &context,
                    &axioms,
                    &witness(
                        target,
                        vec![IntegerShiftStepWitness {
                            definition_axiom: 0,
                            count_axiom: None,
                        }],
                    ),
                ),
                Err(IntegerShiftChainWitnessError::TargetMismatch),
            );
        }
    }
}
pub(crate) mod open_terms {
    use std::cmp::Ordering;
    use std::collections::BTreeMap;

    use numerics::bignum::BigInt;
    use semantic_vocabulary::IntegerMathTerm;

    use super::closed_integer::{
        ClosedIntegerEvaluationError, ClosedIntegerEvaluator, big_integer_literal,
        check_integer_math_term_size,
    };

    const MAXIMUM_MONOMIALS: usize = 4_096;

    type Monomial = BTreeMap<IntegerMathTerm, u32>;

    type NormalForm = BTreeMap<Monomial, BigInt>;

    fn constant(value: BigInt) -> NormalForm {
        if value.is_zero() {
            return NormalForm::new();
        }
        let mut form = NormalForm::new();
        form.insert(Monomial::new(), value);
        form
    }

    fn atom(term: &IntegerMathTerm) -> NormalForm {
        let mut monomial = Monomial::new();
        monomial.insert(term.clone(), 1);
        let mut form = NormalForm::new();
        form.insert(monomial, BigInt::from_u128(1));
        form
    }

    #[derive(Default)]
    struct Normalizer {
        monomials: usize,
        modulus: Option<BigInt>,
    }

    impl Normalizer {
        fn coefficient(&self, value: BigInt) -> BigInt {
            let Some(modulus) = &self.modulus else {
                return value;
            };
            let (_, residue) = value.div_rem(modulus).expect("positive wrapping modulus");
            if residue.is_negative() {
                residue.add(modulus)
            } else {
                residue
            }
        }

        fn charge(&mut self, monomials: usize) -> Result<(), ClosedIntegerEvaluationError> {
            self.monomials = self
                .monomials
                .checked_add(monomials)
                .filter(|total| *total <= MAXIMUM_MONOMIALS)
                .ok_or(ClosedIntegerEvaluationError::ResourceLimitExceeded)?;
            Ok(())
        }

        fn merge(
            &mut self,
            difference: &mut NormalForm,
            added: &NormalForm,
            negate: bool,
        ) -> Result<(), ClosedIntegerEvaluationError> {
            self.charge(added.len())?;
            for (monomial, coefficient) in added {
                let coefficient = if negate {
                    coefficient.negate()
                } else {
                    coefficient.clone()
                };
                let merged = self.coefficient(
                    difference
                        .entry(monomial.clone())
                        .or_insert_with(BigInt::zero)
                        .add(&coefficient),
                );
                if merged.is_zero() {
                    difference.remove(monomial);
                } else {
                    difference.insert(monomial.clone(), merged);
                }
            }
            Ok(())
        }

        fn product(
            &mut self,
            left: &NormalForm,
            right: &NormalForm,
        ) -> Result<NormalForm, ClosedIntegerEvaluationError> {
            self.charge(
                left.len()
                    .checked_mul(right.len())
                    .ok_or(ClosedIntegerEvaluationError::ResourceLimitExceeded)?,
            )?;
            let mut result = NormalForm::new();
            for (left_monomial, left_coefficient) in left {
                for (right_monomial, right_coefficient) in right {
                    let mut monomial = left_monomial.clone();
                    for (atom, exponent) in right_monomial {
                        let exponent = monomial
                            .get(atom)
                            .copied()
                            .unwrap_or(0)
                            .checked_add(*exponent)
                            .ok_or(ClosedIntegerEvaluationError::ResourceLimitExceeded)?;
                        monomial.insert(atom.clone(), exponent);
                    }
                    let coefficient = self.coefficient(left_coefficient.mul(right_coefficient));
                    if coefficient.is_zero() {
                        continue;
                    }
                    let merged = self.coefficient(
                        result
                            .entry(monomial.clone())
                            .or_insert_with(BigInt::zero)
                            .add(&coefficient),
                    );
                    if merged.is_zero() {
                        result.remove(&monomial);
                    } else {
                        result.insert(monomial, merged);
                    }
                }
            }
            Ok(result)
        }

        fn normalize(
            &mut self,
            term: &IntegerMathTerm,
        ) -> Result<NormalForm, ClosedIntegerEvaluationError> {
            match term {
                IntegerMathTerm::IntegerLiteral(literal) => {
                    Ok(constant(self.coefficient(big_integer_literal(*literal))))
                }
                IntegerMathTerm::Add(left, right) => {
                    let left = self.normalize(left)?;
                    let right = self.normalize(right)?;
                    let mut sum = left;
                    self.merge(&mut sum, &right, false)?;
                    Ok(sum)
                }
                IntegerMathTerm::Subtract(left, right) => {
                    let left = self.normalize(left)?;
                    let right = self.normalize(right)?;
                    let mut difference = left;
                    self.merge(&mut difference, &right, true)?;
                    Ok(difference)
                }
                IntegerMathTerm::Multiply(left, right) => {
                    let left = self.normalize(left)?;
                    let right = self.normalize(right)?;
                    self.product(&left, &right)
                }
                _ => {
                    if let IntegerMathTerm::Embed(scalar) = term
                        && let Some(expanded) =
                            crate::proof::integer_math_normalization::lift_exact_integer_term(
                                scalar,
                            )
                    {
                        return self.normalize(&expanded);
                    }
                    match ClosedIntegerEvaluator::default().evaluate_closed(term)? {
                        Some(value) => Ok(constant(self.coefficient(value))),
                        None => Ok(atom(term)),
                    }
                }
            }
        }
    }

    pub(crate) fn constant_difference(
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<Option<BigInt>, ClosedIntegerEvaluationError> {
        check_integer_math_term_size(left, right)?;
        let mut normalizer = Normalizer::default();
        let left = normalizer.normalize(left)?;
        let right = normalizer.normalize(right)?;
        let mut difference = left;
        normalizer.merge(&mut difference, &right, true)?;
        Ok(match difference.len() {
            0 => Some(BigInt::zero()),
            1 => difference
                .first_key_value()
                .and_then(|(monomial, coefficient)| {
                    monomial.is_empty().then(|| coefficient.clone())
                }),
            _ => None,
        })
    }

    pub(crate) fn equal_modulo(
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
        bits: u16,
    ) -> Result<bool, ClosedIntegerEvaluationError> {
        check_integer_math_term_size(left, right)?;
        let mut normalizer = Normalizer {
            modulus: Some(BigInt::from_u64(1).shl_bits(usize::from(bits))),
            ..Normalizer::default()
        };
        let mut difference = normalizer.normalize(left)?;
        let right = normalizer.normalize(right)?;
        normalizer.merge(&mut difference, &right, true)?;
        Ok(difference.is_empty())
    }

    pub(crate) fn constant_difference_ordering(
        left: &IntegerMathTerm,
        right: &IntegerMathTerm,
    ) -> Result<Option<Ordering>, ClosedIntegerEvaluationError> {
        Ok(constant_difference(left, right)?.map(|difference| difference.cmp(&BigInt::zero())))
    }

    #[cfg(test)]
    mod tests {
        use super::{constant_difference, constant_difference_ordering};
        use numerics::bignum::BigInt;
        use semantic_vocabulary::{
            IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ValueId,
        };
        use std::cmp::Ordering;

        fn math_type() -> IntegerType {
            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64")
        }

        fn atom(id: u64) -> IntegerMathTerm {
            IntegerMathTerm::MathValue {
                source_type: math_type(),
                value: ValueId::new(id).expect("value identity"),
            }
        }

        fn integer(value: i128) -> IntegerMathTerm {
            IntegerMathTerm::literal(IntegerValue::Signed(value))
        }

        fn add(left: IntegerMathTerm, right: IntegerMathTerm) -> IntegerMathTerm {
            IntegerMathTerm::Add(Box::new(left), Box::new(right))
        }

        fn subtract(left: IntegerMathTerm, right: IntegerMathTerm) -> IntegerMathTerm {
            IntegerMathTerm::Subtract(Box::new(left), Box::new(right))
        }

        fn multiply(left: IntegerMathTerm, right: IntegerMathTerm) -> IntegerMathTerm {
            IntegerMathTerm::Multiply(Box::new(left), Box::new(right))
        }

        #[test]
        fn accumulator_recurrence_normalizes_to_zero_difference() {
            let acc = atom(1);
            let remaining = atom(2);
            let updated = add(
                add(acc.clone(), integer(1)),
                subtract(remaining.clone(), integer(1)),
            );
            let conserved = add(acc, remaining);
            assert_eq!(
                constant_difference_ordering(&updated, &conserved),
                Ok(Some(Ordering::Equal))
            );
        }

        #[test]
        fn commutativity_and_distribution_are_identities_not_assumptions() {
            let x = atom(1);
            let y = atom(2);
            assert_eq!(
                constant_difference(
                    &multiply(x.clone(), y.clone()),
                    &multiply(y.clone(), x.clone())
                ),
                Ok(Some(BigInt::zero())),
                "x·y and y·x normalize identically"
            );
            let distributed = multiply(x.clone(), add(y.clone(), integer(3)));
            let expanded = add(multiply(x.clone(), y), multiply(x, integer(3)));
            assert_eq!(
                constant_difference_ordering(&distributed, &expanded),
                Ok(Some(Ordering::Equal))
            );
        }

        #[test]
        fn constant_offsets_carry_their_exact_ordering() {
            let x = atom(1);
            assert_eq!(
                constant_difference_ordering(&x.clone(), &add(x.clone(), integer(1))),
                Ok(Some(Ordering::Less))
            );
            assert_eq!(
                constant_difference_ordering(
                    &add(x.clone(), integer(1)),
                    &subtract(x, integer(-2))
                ),
                Ok(Some(Ordering::Less))
            );
        }

        #[test]
        fn nonconstant_differences_and_unmatched_atoms_establish_nothing() {
            let x = atom(1);
            let y = atom(2);
            assert_eq!(
                constant_difference(&add(x.clone(), integer(1)), &y),
                Ok(None)
            );
            let acc = atom(3);
            let remaining = atom(4);
            let forwarded = add(acc.clone(), subtract(remaining.clone(), integer(1)));
            let conserved = add(acc, remaining);
            assert_eq!(
                constant_difference_ordering(&forwarded, &conserved),
                Ok(Some(Ordering::Less)),
                "the wrong-step difference is decided as <, never as ="
            );
            assert_eq!(constant_difference(&x, &y), Ok(None));
        }

        #[test]
        fn open_operations_outside_the_ring_stay_opaque() {
            let shift = IntegerMathTerm::ShiftLeft {
                value: Box::new(atom(1)),
                count: Box::new(integer(3)),
            };
            let x = atom(1);
            assert_eq!(
                constant_difference(&shift, &multiply(x, integer(8))),
                Ok(None),
                "shift(x,3) is not secretly rewritten to 8x"
            );
            assert_eq!(
                constant_difference_ordering(&shift, &shift),
                Ok(Some(Ordering::Equal))
            );
            let closed = IntegerMathTerm::ShiftLeft {
                value: Box::new(integer(1)),
                count: Box::new(integer(5)),
            };
            assert_eq!(
                constant_difference(&closed, &integer(32)),
                Ok(Some(BigInt::zero()))
            );
        }

        #[test]
        fn embedded_exact_arithmetic_expands_but_wrapping_keeps_its_denotation() {
            use semantic_vocabulary::{ScalarTerm, ScalarType};
            let carrier = math_type();
            let scalar = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(carrier));
            let one = ScalarTerm::integer(carrier, IntegerValue::Unsigned(1)).unwrap();
            let exact = IntegerMathTerm::embed(ScalarTerm::ExactIntegerAdd {
                scalar_type: carrier,
                left: Box::new(scalar.clone()),
                right: Box::new(one.clone()),
            })
            .unwrap();
            assert_eq!(
                constant_difference(&exact, &add(atom(1), integer(1))),
                Ok(Some(BigInt::zero()))
            );
            let wrapping = IntegerMathTerm::embed(ScalarTerm::WrappingIntegerAdd {
                scalar_type: carrier,
                left: Box::new(scalar),
                right: Box::new(one.clone()),
            })
            .unwrap();
            assert_eq!(
                constant_difference(&wrapping, &add(atom(1), integer(1))),
                Ok(None)
            );
            let closed_wrapping = IntegerMathTerm::embed(ScalarTerm::WrappingIntegerAdd {
                scalar_type: carrier,
                left: Box::new(
                    ScalarTerm::integer(carrier, IntegerValue::Unsigned(u64::MAX.into())).unwrap(),
                ),
                right: Box::new(one),
            })
            .unwrap();
            assert_eq!(
                constant_difference(&add(atom(1), closed_wrapping), &atom(1)),
                Ok(Some(BigInt::zero()))
            );
        }
    }
}
pub(crate) mod wrapping_equality {
    use semantic_vocabulary::{IntegerMathTerm, IntegerType, ScalarTerm, ScalarType};

    use super::{closed_integer::ClosedIntegerEvaluationError, open_terms};

    const MAXIMUM_NODES: usize = 8_192;
    const MAXIMUM_DEPTH: usize = 128;

    pub(crate) fn equal(
        left: &ScalarTerm,
        right: &ScalarTerm,
    ) -> Result<bool, ClosedIntegerEvaluationError> {
        let Some(carrier) = left.scalar_type().as_integer() else {
            return Ok(false);
        };
        if carrier.is_address() || right.scalar_type() != ScalarType::Integer(carrier) {
            return Ok(false);
        }
        let mut remaining = MAXIMUM_NODES;
        let Some(left) = project(left, carrier, &mut remaining, 0)? else {
            return Ok(false);
        };
        let Some(right) = project(right, carrier, &mut remaining, 0)? else {
            return Ok(false);
        };
        let Some(bits) = carrier.bits() else {
            return Ok(false);
        };
        open_terms::equal_modulo(&left, &right, bits)
    }

    fn project(
        term: &ScalarTerm,
        carrier: IntegerType,
        remaining: &mut usize,
        depth: usize,
    ) -> Result<Option<IntegerMathTerm>, ClosedIntegerEvaluationError> {
        if depth >= MAXIMUM_DEPTH || *remaining == 0 {
            return Err(ClosedIntegerEvaluationError::ResourceLimitExceeded);
        }
        *remaining -= 1;
        if term.scalar_type() != ScalarType::Integer(carrier) {
            return Ok(None);
        }
        let (left, right) = match term {
            ScalarTerm::Value { id, .. } => {
                return Ok(Some(IntegerMathTerm::MathValue {
                    source_type: carrier,
                    value: *id,
                }));
            }
            ScalarTerm::Integer { value, .. } if carrier.admits(*value) => {
                return Ok(Some(IntegerMathTerm::literal(*value)));
            }
            ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. } => (left, right),
            _ => return Ok(None),
        };
        let Some(left) = project(left, carrier, remaining, depth + 1)? else {
            return Ok(None);
        };
        let Some(right) = project(right, carrier, remaining, depth + 1)? else {
            return Ok(None);
        };
        let (left, right) = (Box::new(left), Box::new(right));
        Ok(Some(match term {
            ScalarTerm::WrappingIntegerAdd { .. } => IntegerMathTerm::Add(left, right),
            ScalarTerm::WrappingIntegerSubtract { .. } => IntegerMathTerm::Subtract(left, right),
            ScalarTerm::WrappingIntegerMultiply { .. } => IntegerMathTerm::Multiply(left, right),
            _ => unreachable!("projected wrapping ring operation"),
        }))
    }

    #[cfg(test)]
    mod tests {
        use crate::mathematical_core::{assert_refused, verify_and_assert_forged};
        use crate::{PrimitiveJudgment, ProofNode, ProofRule, check_certificate};
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
            ScalarType, ValueId,
        };

        fn carrier(sign: IntegerSign, bits: u16) -> IntegerType {
            IntegerType::new(sign, bits).unwrap()
        }
        fn value(carrier: IntegerType, identity: u64) -> ScalarTerm {
            ScalarTerm::value(
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(carrier),
            )
        }
        fn literal(carrier: IntegerType, number: u64) -> ScalarTerm {
            let number = IntegerValue::for_sign(carrier.sign(), i128::from(number)).unwrap();
            ScalarTerm::integer(carrier, number).unwrap()
        }
        fn add(carrier: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
            ScalarTerm::WrappingIntegerAdd {
                scalar_type: carrier,
                left: Box::new(left),
                right: Box::new(right),
            }
        }
        fn subtract(carrier: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
            ScalarTerm::WrappingIntegerSubtract {
                scalar_type: carrier,
                left: Box::new(left),
                right: Box::new(right),
            }
        }
        fn multiply(carrier: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
            ScalarTerm::WrappingIntegerMultiply {
                scalar_type: carrier,
                left: Box::new(left),
                right: Box::new(right),
            }
        }
        fn context(carrier: IntegerType, count: u64) -> PropositionContext {
            PropositionContext::from_value_types((1..=count).map(|identity| {
                (
                    ValueId::new(identity).unwrap(),
                    ScalarType::Integer(carrier),
                )
            }))
            .unwrap()
        }
        fn certificate(conclusion: Proposition) -> ProofNode {
            ProofNode {
                conclusion,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }
        }
        fn gauss(carrier: IntegerType, wrong_step: bool) -> Proposition {
            let count = value(carrier, 1);
            let accumulated = value(carrier, 2);
            let next_accumulated = if wrong_step {
                accumulated.clone()
            } else {
                add(carrier, accumulated.clone(), count.clone())
            };
            let next_count = subtract(carrier, count.clone(), literal(carrier, 1));
            Proposition::Equal(
                add(
                    carrier,
                    multiply(carrier, next_accumulated, literal(carrier, 2)),
                    multiply(
                        carrier,
                        next_count.clone(),
                        add(carrier, next_count, literal(carrier, 1)),
                    ),
                ),
                add(
                    carrier,
                    multiply(carrier, accumulated, literal(carrier, 2)),
                    multiply(
                        carrier,
                        count.clone(),
                        add(carrier, count, literal(carrier, 1)),
                    ),
                ),
            )
        }

        #[test]
        fn wrapping_polynomial_gauss_preservation_is_independently_certified() {
            for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
                for bits in [8, 64, 128] {
                    let carrier = carrier(sign, bits);
                    let proof = certificate(gauss(carrier, false));
                    let context = context(carrier, 2);
                    check_certificate(&context, &proof.conclusion, &[], &[], &proof).unwrap();
                    let wrong = certificate(gauss(carrier, true));
                    assert!(
                        check_certificate(&context, &wrong.conclusion, &[], &[], &wrong).is_err()
                    );
                }
            }
        }

        #[test]
        fn wrapping_polynomial_reduces_coefficients_without_cancelling_nonunits() {
            for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
                let carrier = carrier(sign, 8);
                let subject = value(carrier, 1);
                let zero_multiple = multiply(
                    carrier,
                    multiply(carrier, subject.clone(), literal(carrier, 16)),
                    literal(carrier, 16),
                );
                let proof = certificate(Proposition::Equal(
                    add(carrier, subject.clone(), zero_multiple),
                    subject.clone(),
                ));
                check_certificate(&context(carrier, 2), &proof.conclusion, &[], &[], &proof)
                    .unwrap();
                let other = value(carrier, 2);
                let doubled = Proposition::Equal(
                    multiply(carrier, subject.clone(), literal(carrier, 2)),
                    multiply(carrier, other.clone(), literal(carrier, 2)),
                );
                let cancelled = certificate(Proposition::Equal(subject, other));
                assert!(
                    check_certificate(
                        &context(carrier, 2),
                        &cancelled.conclusion,
                        &[doubled],
                        &[],
                        &cancelled
                    )
                    .is_err()
                );
            }
        }

        #[test]
        fn wrapping_polynomial_does_not_supply_order_or_disequality() {
            let carrier = carrier(IntegerSign::Unsigned, 8);
            let subject = value(carrier, 1);
            let overflowed = add(carrier, subject.clone(), literal(carrier, 1));
            let restored = add(
                carrier,
                add(carrier, subject.clone(), literal(carrier, 128)),
                literal(carrier, 128),
            );
            for goal in [
                Proposition::LessThan(subject.clone(), overflowed.clone()),
                Proposition::LessOrEqual(subject.clone(), overflowed),
                Proposition::LessThan(subject.clone(), restored.clone()),
                Proposition::Implication {
                    premise: Box::new(Proposition::Equal(subject, restored)),
                    conclusion: Box::new(Proposition::Falsehood),
                },
            ] {
                let proof = certificate(goal);
                assert!(
                    check_certificate(&context(carrier, 1), &proof.conclusion, &[], &[], &proof)
                        .is_err()
                );
            }
        }

        #[test]
        fn wrapping_polynomial_retains_policy_carrier_and_subject_identity() {
            let carrier = carrier(IntegerSign::Unsigned, 64);
            let subject = value(carrier, 1);
            let wrapped = add(carrier, subject.clone(), literal(carrier, 1));
            for changed in [
                ScalarTerm::SaturatingIntegerAdd {
                    scalar_type: carrier,
                    left: Box::new(subject.clone()),
                    right: Box::new(literal(carrier, 1)),
                },
                ScalarTerm::ExactIntegerAdd {
                    scalar_type: carrier,
                    left: Box::new(subject.clone()),
                    right: Box::new(literal(carrier, 1)),
                },
                add(carrier, value(carrier, 99), literal(carrier, 1)),
                add(
                    carrier,
                    value(IntegerType::new(IntegerSign::Unsigned, 32).unwrap(), 1),
                    literal(carrier, 1),
                ),
                add(
                    carrier,
                    value(IntegerType::new(IntegerSign::Signed, 64).unwrap(), 1),
                    literal(carrier, 1),
                ),
            ] {
                let proof = certificate(Proposition::Equal(wrapped.clone(), changed));
                assert!(
                    check_certificate(&context(carrier, 1), &proof.conclusion, &[], &[], &proof)
                        .is_err()
                );
            }
            let address = IntegerType::address(64).unwrap();
            let proof = certificate(Proposition::Equal(
                add(address, value(address, 1), literal(address, 0)),
                value(address, 1),
            ));
            assert!(
                check_certificate(&context(address, 1), &proof.conclusion, &[], &[], &proof)
                    .is_err()
            );
        }

        #[test]
        fn wrapping_polynomial_core_rechecks_the_primitive_and_rejects_forged_evidence() {
            let carrier = carrier(IntegerSign::Unsigned, 64);
            let context = context(carrier, 2);
            let proof = certificate(gauss(carrier, false));
            verify_and_assert_forged(&context, &proof.conclusion, &[], &[], &proof);
            let wrong = certificate(gauss(carrier, true));
            assert_refused(&context, &wrong.conclusion, &[], &[], &wrong);
        }

        #[test]
        fn wrapping_polynomial_expansion_and_depth_exhaustion_establish_nothing() {
            let carrier = carrier(IntegerSign::Unsigned, 64);
            let mut expanded = literal(carrier, 1);
            for identity in 1..=13 {
                expanded = multiply(
                    carrier,
                    expanded,
                    add(carrier, value(carrier, identity), literal(carrier, 1)),
                );
            }
            assert_eq!(
                super::equal(&expanded, &expanded),
                Err(super::ClosedIntegerEvaluationError::ResourceLimitExceeded)
            );
            let mut deep = value(carrier, 1);
            for _ in 0..128 {
                deep = add(carrier, deep, literal(carrier, 1));
            }
            assert_eq!(
                super::equal(&deep, &value(carrier, 1)),
                Err(super::ClosedIntegerEvaluationError::ResourceLimitExceeded)
            );
        }
    }
}

#[cfg(test)]
pub(crate) fn value(id: u64, integer_type: IntegerType) -> semantic_vocabulary::ScalarTerm {
    semantic_vocabulary::ScalarTerm::value(
        semantic_vocabulary::ValueId::new(id).expect("value"),
        semantic_vocabulary::ScalarType::Integer(integer_type),
    )
}
